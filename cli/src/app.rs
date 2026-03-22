use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::config::*;
use crate::fuzzy::fuzzy_match_score;
use crate::terminal::EmbeddedTerminal;

/// Background clone result.
pub struct CloneResult {
    pub comb: Result<Comb, String>,
    pub comb_name: String,
    pub hive_dir_name: String,
    /// If true, this was a copy operation (auto-switch to new comb).
    /// If false, this was a clone operation (graceful, don't switch focus).
    pub is_copy: bool,
}

/// A pending clone/copy operation with its own slot and description.
pub struct PendingClone {
    pub slot: Arc<Mutex<Option<CloneResult>>>,
    pub activity: String,
}

pub struct DeleteResult {
    pub deleted_comb_names: Vec<String>,
    pub deleted_hive_names: Vec<String>,
    pub errors: Vec<String>,
}

/// A pending delete operation with its own slot and description.
pub struct PendingDelete {
    pub slot: Arc<Mutex<Option<DeleteResult>>>,
    pub activity: String,
}

#[derive(Clone)]
pub enum DeleteTarget {
    Comb {
        hive_dir_name: String,
        comb_id: String,
        comb_name: String,
    },
    Hive {
        dir_name: String,
        repo_name: String,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Sidebar,
    Terminal,
}

#[derive(Clone)]
pub enum NavItem {
    Hive {
        info: HiveInfo,
        expanded: bool,
        comb_count: usize,
    },
    Comb {
        hive_dir_name: String,
        comb: Comb,
    },
}

#[derive(Clone)]
enum NavItemKey {
    Hive { dir_name: String },
    Comb { id: String },
}

#[derive(Clone)]
pub struct CombFinderTarget {
    pub hive_dir_name: String,
    pub hive_repo_name: String,
    pub comb_id: String,
    pub comb_name: String,
    pub branch: String,
}

pub fn filter_comb_finder_targets<'a>(
    targets: &'a [CombFinderTarget],
    filter: &str,
) -> Vec<&'a CombFinderTarget> {
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() {
        return targets.iter().collect();
    }

    let mut scored: Vec<(i64, &CombFinderTarget)> = targets
        .iter()
        .filter_map(|target| {
            let best_score = [
                fuzzy_match_score(&filter, &target.comb_name).map(|score| score + 300),
                fuzzy_match_score(&filter, &target.branch).map(|score| score + 200),
                fuzzy_match_score(&filter, &target.hive_repo_name).map(|score| score + 100),
            ]
            .into_iter()
            .flatten()
            .max();

            best_score.map(|score| (score, target))
        })
        .collect();

    scored.sort_by(|(score_a, target_a), (score_b, target_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| target_a.comb_name.len().cmp(&target_b.comb_name.len()))
            .then_with(|| {
                target_a
                    .comb_name
                    .to_lowercase()
                    .cmp(&target_b.comb_name.to_lowercase())
            })
            .then_with(|| {
                target_a
                    .branch
                    .to_lowercase()
                    .cmp(&target_b.branch.to_lowercase())
            })
    });

    scored.into_iter().map(|(_, target)| target).collect()
}

pub enum AppMode {
    Normal,
    Input {
        prompt: String,
        value: String,
        action: InputAction,
    },
    Confirm {
        message: String,
        action: ConfirmAction,
    },
    BranchPicker {
        hive_dir_name: String,
        comb_name: String,
        branches: Vec<String>,
        default_branch: String,
        filter: String,
        selected: usize,
    },
    /// Reordering a comb within its hive. Up/Down to move, m/Enter to confirm, Esc to cancel.
    MovingComb {
        hive_dir_name: String,
        moving_comb_id: String,
        /// Snapshot of items before moving started, for Esc cancel.
        original_items: Vec<NavItem>,
        original_selected: usize,
    },
    CombFinder {
        targets: Vec<CombFinderTarget>,
        filter: String,
        selected: usize,
    },
    DeleteCombSelection {
        hive_dir_name: String,
        selected_comb_ids: HashSet<String>,
    },
    Help,
    Settings {
        preflight: PreflightResult,
    },
}

pub enum InputAction {
    NewCombName {
        hive_dir_name: String,
    },
    AddHiveUrl,
    CopyCombName {
        hive_dir_name: String,
        #[allow(dead_code)]
        source_comb_name: String,
        source_comb_path: String,
    },
}

pub enum ConfirmAction {
    DeleteComb {
        hive_dir_name: String,
        comb_id: String,
        comb_name: String,
    },
    DeleteHive {
        dir_name: String,
        repo_name: String,
    },
    ResetConfig,
    Quit,
}

/// Raw data from a background refresh (list_hives + get_combs for each).
pub struct RefreshResult {
    pub hive_data: Vec<(HiveInfo, Vec<Comb>)>,
}

pub struct App {
    pub beehive_dir: String,
    pub items: Vec<NavItem>,
    pub selected: usize,
    pub mode: AppMode,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub active_comb_id: Option<String>,
    pub focus: Focus,
    pub terminals: HashMap<String, EmbeddedTerminal>,
    pub last_term_size: (u16, u16),
    /// Multiple concurrent clone/copy operations
    pub pending_clones: Vec<PendingClone>,
    /// Multiple concurrent delete operations
    pub pending_deletes: Vec<PendingDelete>,
    pub pending_refresh: Option<Arc<Mutex<Option<RefreshResult>>>>,
    pub update_available: Option<String>,
    pub sidebar_width: u16,
    pub deleting_comb_ids: HashSet<String>,
    pub deleting_hive_dir_names: HashSet<String>,
    /// Whether the outer terminal supports the kitty keyboard enhancement protocol.
    /// When true, crossterm reports SUPER/META modifiers and key event kinds (press/repeat/release).
    pub keyboard_enhanced: bool,
    /// Set when a completed operation wanted to refresh but was deferred (e.g. during move mode).
    pub needs_refresh: bool,
}

impl App {
    pub fn enter_sidebar_mode(&mut self, mode: AppMode) {
        self.focus = Focus::Sidebar;
        self.mode = mode;
    }

    fn selected_item_key(&self) -> Option<NavItemKey> {
        self.items.get(self.selected).map(|item| match item {
            NavItem::Hive { info, .. } => NavItemKey::Hive {
                dir_name: info.dir_name.clone(),
            },
            NavItem::Comb { comb, .. } => NavItemKey::Comb {
                id: comb.id.clone(),
            },
        })
    }

    fn find_item_index(items: &[NavItem], key: &NavItemKey) -> Option<usize> {
        items.iter().position(|item| match (item, key) {
            (NavItem::Hive { info, .. }, NavItemKey::Hive { dir_name }) => {
                info.dir_name == *dir_name
            }
            (NavItem::Comb { comb, .. }, NavItemKey::Comb { id }) => comb.id == *id,
            _ => false,
        })
    }

    fn sync_selection_to_items(&mut self, items: &[NavItem]) {
        if items.is_empty() {
            self.selected = 0;
            return;
        }

        if let Some(key) = self.selected_item_key() {
            if let Some(index) = Self::find_item_index(items, &key) {
                self.selected = index;
                return;
            }
        }

        if self.selected >= items.len() {
            self.selected = items.len() - 1;
        }
    }

    pub fn has_pending_work(&self) -> bool {
        !self.pending_clones.is_empty() || !self.pending_deletes.is_empty()
    }

    /// Derive the activity summary from all pending operations.
    pub fn activity_summary(&self) -> Option<String> {
        let total = self.pending_clones.len() + self.pending_deletes.len();
        if total == 0 {
            return None;
        }
        if total == 1 {
            // Show the single activity message
            if let Some(pc) = self.pending_clones.first() {
                return Some(pc.activity.clone());
            }
            if let Some(pd) = self.pending_deletes.first() {
                return Some(pd.activity.clone());
            }
        }
        // Multiple operations: show count
        Some(format!("{} operations running", total))
    }

    pub fn should_pause_refresh(&self) -> bool {
        !self.pending_deletes.is_empty()
            || matches!(
                self.mode,
                AppMode::MovingComb { .. } | AppMode::DeleteCombSelection { .. }
            )
    }

    pub fn delete_mode_hive_dir_name(&self) -> Option<&str> {
        match &self.mode {
            AppMode::DeleteCombSelection { hive_dir_name, .. } => Some(hive_dir_name.as_str()),
            _ => None,
        }
    }

    pub fn delete_selection_count(&self) -> usize {
        match &self.mode {
            AppMode::DeleteCombSelection {
                selected_comb_ids, ..
            } => selected_comb_ids.len(),
            _ => 0,
        }
    }

    pub fn is_marked_for_delete(&self, comb_id: &str) -> bool {
        match &self.mode {
            AppMode::DeleteCombSelection {
                selected_comb_ids, ..
            } => selected_comb_ids.contains(comb_id),
            _ => false,
        }
    }

    fn find_hive_index(&self, hive_dir_name: &str) -> Option<usize> {
        self.items.iter().position(|item| match item {
            NavItem::Hive { info, .. } => info.dir_name == hive_dir_name,
            _ => false,
        })
    }

    fn expand_hive_at(&mut self, hive_index: usize) -> Result<(), String> {
        let (info, expanded, comb_count) = match &self.items[hive_index] {
            NavItem::Hive {
                info,
                expanded,
                comb_count,
            } => (info.clone(), *expanded, *comb_count),
            NavItem::Comb { .. } => return Ok(()),
        };

        if expanded {
            return Ok(());
        }

        self.items[hive_index] = NavItem::Hive {
            info: info.clone(),
            expanded: true,
            comb_count,
        };

        let combs = get_combs(&self.beehive_dir, &info.dir_name)?;
        for (offset, comb) in combs.into_iter().enumerate() {
            self.items.insert(
                hive_index + 1 + offset,
                NavItem::Comb {
                    hive_dir_name: info.dir_name.clone(),
                    comb,
                },
            );
        }

        Ok(())
    }

    pub fn reveal_comb(&mut self, hive_dir_name: &str, comb_id: &str) -> Result<bool, String> {
        if self.select_comb_by_id(comb_id) {
            return Ok(true);
        }

        let Some(hive_index) = self.find_hive_index(hive_dir_name) else {
            return Ok(false);
        };

        self.expand_hive_at(hive_index)?;
        Ok(self.select_comb_by_id(comb_id))
    }

    fn first_deletable_comb_index_in_hive(&self, hive_dir_name: &str) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .find_map(|(index, item)| match item {
                NavItem::Comb {
                    hive_dir_name: h,
                    comb,
                } if h == hive_dir_name
                    && !comb.cloning
                    && !self.deleting_comb_ids.contains(&comb.id) =>
                {
                    Some(index)
                }
                _ => None,
            })
    }

    fn adjacent_deletable_comb_index_in_hive(
        &self,
        hive_dir_name: &str,
        from: usize,
        forward: bool,
    ) -> Option<usize> {
        let len = self.items.len();
        if len == 0 {
            return None;
        }

        for step in 1..=len {
            let index = if forward {
                (from + step) % len
            } else {
                (from + len - (step % len)) % len
            };

            if matches!(
                self.items.get(index),
                Some(NavItem::Comb {
                    hive_dir_name: h,
                    comb,
                }) if h == hive_dir_name
                    && !comb.cloning
                    && !self.deleting_comb_ids.contains(&comb.id)
            ) {
                return Some(index);
            }
        }

        None
    }

    pub fn start_comb_finder(&mut self) {
        let mut targets = Vec::new();
        let hives = match list_hives(&self.beehive_dir) {
            Ok(hives) => hives,
            Err(e) => {
                self.status_message = Some(format!("Failed to load combs: {}", e));
                return;
            }
        };

        for info in hives {
            let combs = match get_combs(&self.beehive_dir, &info.dir_name) {
                Ok(combs) => combs,
                Err(e) => {
                    self.status_message = Some(format!("Failed to load combs: {}", e));
                    return;
                }
            };

            for comb in combs {
                if comb.cloning {
                    continue;
                }
                targets.push(CombFinderTarget {
                    hive_dir_name: info.dir_name.clone(),
                    hive_repo_name: info.repo_name.clone(),
                    comb_id: comb.id,
                    comb_name: comb.name,
                    branch: comb.branch,
                });
            }
        }

        if targets.is_empty() {
            self.status_message = Some("No combs to jump to".to_string());
            return;
        }

        self.enter_sidebar_mode(AppMode::CombFinder {
            targets,
            filter: String::new(),
            selected: 0,
        });
    }

    pub fn start_delete_mode(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let (hive_dir_name, selected_comb_id) = match &self.items[self.selected] {
            NavItem::Hive { info, .. } => (info.dir_name.clone(), None),
            NavItem::Comb {
                hive_dir_name,
                comb,
            } => {
                if comb.cloning {
                    self.status_message =
                        Some("Cannot delete a comb that is still in progress".to_string());
                    return;
                }
                if self.deleting_comb_ids.contains(&comb.id) {
                    self.status_message = Some("That comb is already being deleted".to_string());
                    return;
                }
                (hive_dir_name.clone(), Some(comb.id.clone()))
            }
        };

        let Some(hive_index) = self.find_hive_index(&hive_dir_name) else {
            self.status_message = Some("Hive not found".to_string());
            return;
        };

        if let Err(e) = self.expand_hive_at(hive_index) {
            self.status_message = Some(format!("Failed to open hive: {}", e));
            return;
        }

        let initial_index = selected_comb_id
            .as_ref()
            .and_then(|comb_id| {
                self.items.iter().position(|item| {
                    matches!(
                        item,
                        NavItem::Comb {
                            hive_dir_name: h,
                            comb,
                        } if h == &hive_dir_name && comb.id == *comb_id && !comb.cloning
                    )
                })
            })
            .or_else(|| self.first_deletable_comb_index_in_hive(&hive_dir_name));

        let Some(initial_index) = initial_index else {
            self.status_message = Some("No combs available to delete in this hive".to_string());
            return;
        };

        self.selected = initial_index;
        self.enter_sidebar_mode(AppMode::DeleteCombSelection {
            hive_dir_name,
            selected_comb_ids: HashSet::new(),
        });
    }

    pub fn move_delete_selection_up(&mut self) {
        if let Some(hive_dir_name) = self.delete_mode_hive_dir_name().map(str::to_string) {
            if let Some(index) =
                self.adjacent_deletable_comb_index_in_hive(&hive_dir_name, self.selected, false)
            {
                self.selected = index;
            }
        }
    }

    pub fn move_delete_selection_down(&mut self) {
        if let Some(hive_dir_name) = self.delete_mode_hive_dir_name().map(str::to_string) {
            if let Some(index) =
                self.adjacent_deletable_comb_index_in_hive(&hive_dir_name, self.selected, true)
            {
                self.selected = index;
            }
        }
    }

    pub fn toggle_delete_selection(&mut self) {
        let Some(hive_dir_name) = self.delete_mode_hive_dir_name().map(str::to_string) else {
            return;
        };

        let selected_item = self.items.get(self.selected).cloned();
        let Some(NavItem::Comb {
            hive_dir_name: item_hive,
            comb,
        }) = selected_item
        else {
            self.status_message = Some("Select a comb to mark it for delete".to_string());
            return;
        };

        if item_hive != hive_dir_name {
            self.status_message =
                Some("Delete mode only works within one hive at a time".to_string());
            return;
        }
        if comb.cloning {
            self.status_message =
                Some("Cannot delete a comb that is still in progress".to_string());
            return;
        }

        if let AppMode::DeleteCombSelection {
            selected_comb_ids, ..
        } = &mut self.mode
        {
            if !selected_comb_ids.remove(&comb.id) {
                selected_comb_ids.insert(comb.id);
            }
        }
    }

    pub fn selected_delete_targets(&self) -> Vec<DeleteTarget> {
        let AppMode::DeleteCombSelection {
            hive_dir_name,
            selected_comb_ids,
        } = &self.mode
        else {
            return vec![];
        };

        self.items
            .iter()
            .filter_map(|item| match item {
                NavItem::Comb {
                    hive_dir_name: h,
                    comb,
                } if h == hive_dir_name && selected_comb_ids.contains(&comb.id) => {
                    Some(DeleteTarget::Comb {
                        hive_dir_name: h.clone(),
                        comb_id: comb.id.clone(),
                        comb_name: comb.name.clone(),
                    })
                }
                _ => None,
            })
            .collect()
    }

    pub fn new(beehive_dir: String, keyboard_enhanced: bool) -> Result<Self, String> {
        let config = load_app_config()?;
        let mut app = App {
            beehive_dir,
            items: vec![],
            selected: 0,
            mode: AppMode::Normal,
            should_quit: false,
            status_message: None,
            active_comb_id: None,
            focus: Focus::Sidebar,
            terminals: HashMap::new(),
            last_term_size: (0, 0),
            pending_clones: Vec::new(),
            pending_deletes: Vec::new(),
            pending_refresh: None,
            update_available: None,
            sidebar_width: config.sidebar_width,
            deleting_comb_ids: HashSet::new(),
            deleting_hive_dir_names: HashSet::new(),
            keyboard_enhanced,
            needs_refresh: false,
        };
        app.load_all(true)?;
        Ok(app)
    }

    pub fn save_sidebar_width(&self) {
        if let Ok(mut config) = load_app_config() {
            config.sidebar_width = self.sidebar_width;
            let _ = save_app_config(&config);
        }
    }

    pub fn load_all(&mut self, expand_all: bool) -> Result<(), String> {
        let hives = list_hives(&self.beehive_dir)?;
        let mut items = vec![];

        for info in hives {
            let dir_name = info.dir_name.clone();
            let was_expanded = if expand_all {
                true
            } else {
                self.items.iter().any(|item| {
                    matches!(item, NavItem::Hive { info: h, expanded: true, .. } if h.dir_name == dir_name)
                })
            };

            let combs = get_combs(&self.beehive_dir, &dir_name).unwrap_or_default();
            let comb_count = combs.iter().filter(|c| !c.cloning).count();

            items.push(NavItem::Hive {
                info: info.clone(),
                expanded: was_expanded,
                comb_count,
            });

            if was_expanded {
                for comb in combs {
                    items.push(NavItem::Comb {
                        hive_dir_name: dir_name.clone(),
                        comb,
                    });
                }
            }
        }

        self.sync_selection_to_items(&items);
        self.items = items;
        Ok(())
    }

    pub fn refresh(&mut self) {
        let _ = self.load_all(false);
    }

    /// Apply the result of a background refresh to update sidebar items
    /// without blocking the main thread.
    pub fn apply_refresh(&mut self, result: RefreshResult) {
        let mut items = vec![];

        for (info, combs) in result.hive_data {
            let dir_name = info.dir_name.clone();
            let was_expanded = self.items.iter().any(|item| {
                matches!(item, NavItem::Hive { info: h, expanded: true, .. } if h.dir_name == dir_name)
            });

            let comb_count = combs.iter().filter(|c| !c.cloning).count();

            items.push(NavItem::Hive {
                info: info.clone(),
                expanded: was_expanded,
                comb_count,
            });

            if was_expanded {
                for comb in combs {
                    items.push(NavItem::Comb {
                        hive_dir_name: dir_name.clone(),
                        comb,
                    });
                }
            }
        }

        self.sync_selection_to_items(&items);
        self.items = items;
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    /// Returns Some((comb_id, path)) if a comb should be opened.
    pub fn enter_selected(&mut self) -> Option<(String, String)> {
        if self.items.is_empty() {
            return None;
        }

        let item = self.items[self.selected].clone();
        match item {
            NavItem::Hive {
                info,
                expanded,
                comb_count,
            } => {
                let new_expanded = !expanded;
                self.items[self.selected] = NavItem::Hive {
                    info: info.clone(),
                    expanded: new_expanded,
                    comb_count,
                };

                if new_expanded {
                    if let Ok(combs) = get_combs(&self.beehive_dir, &info.dir_name) {
                        let insert_pos = self.selected + 1;
                        let mut offset = 0;
                        for comb in combs {
                            self.items.insert(
                                insert_pos + offset,
                                NavItem::Comb {
                                    hive_dir_name: info.dir_name.clone(),
                                    comb,
                                },
                            );
                            offset += 1;
                        }
                    }
                } else {
                    let dir_name = &info.dir_name;
                    while self.selected + 1 < self.items.len() {
                        if matches!(&self.items[self.selected + 1], NavItem::Comb { hive_dir_name, .. } if hive_dir_name == dir_name)
                        {
                            self.items.remove(self.selected + 1);
                        } else {
                            break;
                        }
                    }
                }
                None
            }
            NavItem::Comb { comb, .. } => {
                if comb.cloning {
                    self.status_message = Some("Still in progress...".to_string());
                    return None;
                }
                let id = comb.id.clone();
                let path = comb.path.clone();
                self.active_comb_id = Some(id.clone());
                Some((id, path))
            }
        }
    }

    /// Switch to or create a terminal for the given comb.
    pub fn open_terminal(&mut self, comb_id: &str, comb_path: &str) {
        if self.terminals.contains_key(comb_id) {
            self.focus = Focus::Terminal;
            self.last_term_size = (0, 0);
            return;
        }

        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let term_cols = (cols * 70 / 100).max(20);
        let term_rows = rows.saturating_sub(3).max(5);

        match EmbeddedTerminal::new(comb_path, term_rows, term_cols) {
            Ok(term) => {
                self.terminals.insert(comb_id.to_string(), term);
                self.focus = Focus::Terminal;
                self.last_term_size = (0, 0);
            }
            Err(e) => {
                self.status_message = Some(format!("Terminal: {}", e));
            }
        }
    }

    pub fn active_terminal(&self) -> Option<&EmbeddedTerminal> {
        self.active_comb_id
            .as_ref()
            .and_then(|id| self.terminals.get(id))
    }

    pub fn active_terminal_mut(&mut self) -> Option<&mut EmbeddedTerminal> {
        self.active_comb_id
            .as_ref()
            .and_then(|id| self.terminals.get_mut(id))
    }

    pub fn active_comb_name(&self) -> Option<String> {
        let active_id = self.active_comb_id.as_ref()?;
        for item in &self.items {
            if let NavItem::Comb { comb, .. } = item {
                if &comb.id == active_id {
                    return Some(comb.name.clone());
                }
            }
        }
        None
    }

    pub fn remove_terminal(&mut self, comb_id: &str) {
        self.terminals.remove(comb_id);
        if self.active_comb_id.as_deref() == Some(comb_id) {
            self.active_comb_id = None;
            self.focus = Focus::Sidebar;
        }
    }

    pub fn remove_hive_terminals(&mut self, hive_dir_name: &str) {
        let ids_to_remove: Vec<String> = self
            .items
            .iter()
            .filter_map(|item| match item {
                NavItem::Comb {
                    hive_dir_name: h,
                    comb,
                } if h == hive_dir_name => Some(comb.id.clone()),
                _ => None,
            })
            .collect();
        for id in &ids_to_remove {
            self.terminals.remove(id);
        }
        if let Some(active) = &self.active_comb_id {
            if ids_to_remove.contains(active) {
                self.active_comb_id = None;
                self.focus = Focus::Sidebar;
            }
        }
    }

    pub fn start_new_comb(&mut self) {
        if let Some(dir_name) = self.selected_hive_dir() {
            self.enter_sidebar_mode(AppMode::Input {
                prompt: "Comb name".to_string(),
                value: String::new(),
                action: InputAction::NewCombName {
                    hive_dir_name: dir_name,
                },
            });
        } else {
            self.status_message = Some("Add a hive first with 'a'".to_string());
        }
    }

    pub fn start_copy_comb(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if let NavItem::Comb {
            hive_dir_name,
            comb,
        } = &self.items[self.selected]
        {
            if comb.cloning {
                self.status_message =
                    Some("Cannot copy a comb that is still in progress".to_string());
                return;
            }
            self.enter_sidebar_mode(AppMode::Input {
                prompt: format!("Copy '{}' as", comb.name),
                value: String::new(),
                action: InputAction::CopyCombName {
                    hive_dir_name: hive_dir_name.clone(),
                    source_comb_name: comb.name.clone(),
                    source_comb_path: comb.path.clone(),
                },
            });
        } else {
            self.status_message = Some("Select a comb to copy".to_string());
        }
    }

    pub fn start_move_comb(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if let NavItem::Comb {
            hive_dir_name,
            comb,
        } = &self.items[self.selected]
        {
            if comb.cloning {
                self.status_message = Some("Cannot move a comb in progress".to_string());
                return;
            }
            let hive_dir_name = hive_dir_name.clone();
            let moving_comb_id = comb.id.clone();
            let original_items = self.items.clone();
            self.enter_sidebar_mode(AppMode::MovingComb {
                hive_dir_name,
                moving_comb_id,
                original_items,
                original_selected: self.selected,
            });
        } else {
            self.status_message = Some("Select a comb to move".to_string());
        }
    }

    /// Move the currently selected comb up within its hive. Returns true if moved.
    pub fn move_comb_up(&mut self) -> bool {
        if self.selected == 0 {
            return false;
        }
        let prev = self.selected - 1;
        // Only swap with another Comb from the same hive (not past a Hive header)
        if let (
            NavItem::Comb {
                hive_dir_name: h1, ..
            },
            NavItem::Comb {
                hive_dir_name: h2, ..
            },
        ) = (&self.items[self.selected], &self.items[prev])
        {
            if h1 == h2 {
                self.items.swap(self.selected, prev);
                self.selected = prev;
                return true;
            }
        }
        false
    }

    /// Move the currently selected comb down within its hive. Returns true if moved.
    pub fn move_comb_down(&mut self) -> bool {
        let next = self.selected + 1;
        if next >= self.items.len() {
            return false;
        }
        // Only swap with another Comb from the same hive (not past a Hive header)
        if let (
            NavItem::Comb {
                hive_dir_name: h1, ..
            },
            NavItem::Comb {
                hive_dir_name: h2, ..
            },
        ) = (&self.items[self.selected], &self.items[next])
        {
            if h1 == h2 {
                self.items.swap(self.selected, next);
                self.selected = next;
                return true;
            }
        }
        false
    }

    pub fn select_comb_by_id(&mut self, comb_id: &str) -> bool {
        if let Some(index) = self.items.iter().position(|item| match item {
            NavItem::Comb { comb, .. } => comb.id == comb_id,
            _ => false,
        }) {
            self.selected = index;
            true
        } else {
            false
        }
    }

    /// Extract the current comb ID order for a given hive from the flat items list.
    pub fn comb_order_for_hive(&self, hive_dir_name: &str) -> Vec<String> {
        self.items
            .iter()
            .filter_map(|item| match item {
                NavItem::Comb {
                    hive_dir_name: h,
                    comb,
                } if h == hive_dir_name => Some(comb.id.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn start_add_hive(&mut self) {
        self.enter_sidebar_mode(AppMode::Input {
            prompt: "Repository (owner/repo or URL)".to_string(),
            value: String::new(),
            action: InputAction::AddHiveUrl,
        });
    }

    pub fn start_delete(&mut self) {
        if self.items.is_empty() {
            return;
        }
        match &self.items[self.selected] {
            NavItem::Comb {
                hive_dir_name,
                comb,
            } => {
                if comb.cloning {
                    self.status_message = Some("Cannot delete while in progress".to_string());
                    return;
                }
                self.enter_sidebar_mode(AppMode::Confirm {
                    message: format!("Delete comb '{}'?", comb.name),
                    action: ConfirmAction::DeleteComb {
                        hive_dir_name: hive_dir_name.clone(),
                        comb_id: comb.id.clone(),
                        comb_name: comb.name.clone(),
                    },
                });
            }
            NavItem::Hive { info, .. } => {
                self.enter_sidebar_mode(AppMode::Confirm {
                    message: format!("Delete hive '{}'?", info.repo_name),
                    action: ConfirmAction::DeleteHive {
                        dir_name: info.dir_name.clone(),
                        repo_name: info.repo_name.clone(),
                    },
                });
            }
        }
    }

    pub fn start_quit(&mut self) {
        self.enter_sidebar_mode(AppMode::Confirm {
            message: "Quit Beehive?".to_string(),
            action: ConfirmAction::Quit,
        });
    }

    pub fn open_settings(&mut self) {
        let pf = preflight();
        self.enter_sidebar_mode(AppMode::Settings { preflight: pf });
    }

    pub fn open_help(&mut self) {
        self.enter_sidebar_mode(AppMode::Help);
    }

    pub fn selected_hive_dir(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        match &self.items[self.selected] {
            NavItem::Hive { info, .. } => Some(info.dir_name.clone()),
            NavItem::Comb { hive_dir_name, .. } => Some(hive_dir_name.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hive(dir_name: &str, repo_name: &str) -> HiveInfo {
        HiveInfo {
            dir_name: dir_name.to_string(),
            repo_url: format!("https://github.com/acme/{}.git", repo_name),
            repo_name: repo_name.to_string(),
            owner: "acme".to_string(),
            description: None,
            default_branch: Some("main".to_string()),
            custom_buttons: vec![],
        }
    }

    fn comb(id: &str, name: &str, branch: &str) -> Comb {
        Comb {
            id: id.to_string(),
            name: name.to_string(),
            branch: branch.to_string(),
            path: format!("/tmp/{}", name),
            created_at: "0".to_string(),
            panes: vec![],
            cloning: false,
        }
    }

    fn make_app(items: Vec<NavItem>, selected: usize) -> App {
        App {
            beehive_dir: "/tmp/beehive".to_string(),
            items,
            selected,
            mode: AppMode::Normal,
            should_quit: false,
            status_message: None,
            active_comb_id: None,
            focus: Focus::Sidebar,
            terminals: HashMap::new(),
            last_term_size: (0, 0),
            pending_clones: Vec::new(),
            pending_deletes: Vec::new(),
            pending_refresh: None,
            update_available: None,
            sidebar_width: 28,
            deleting_comb_ids: HashSet::new(),
            deleting_hive_dir_names: HashSet::new(),
            keyboard_enhanced: false,
            needs_refresh: false,
        }
    }

    #[test]
    fn filter_comb_finder_targets_matches_name_branch_and_hive_case_insensitively() {
        let targets = vec![
            CombFinderTarget {
                hive_dir_name: "repo_api".to_string(),
                hive_repo_name: "ApiServer".to_string(),
                comb_id: "1".to_string(),
                comb_name: "feature-login".to_string(),
                branch: "fix/auth".to_string(),
            },
            CombFinderTarget {
                hive_dir_name: "repo_web".to_string(),
                hive_repo_name: "Frontend".to_string(),
                comb_id: "2".to_string(),
                comb_name: "homepage".to_string(),
                branch: "main".to_string(),
            },
        ];

        assert_eq!(filter_comb_finder_targets(&targets, "LOGIN").len(), 1);
        assert_eq!(filter_comb_finder_targets(&targets, "auth").len(), 1);
        assert_eq!(filter_comb_finder_targets(&targets, "front").len(), 1);
        assert_eq!(filter_comb_finder_targets(&targets, "").len(), 2);
    }

    #[test]
    fn filter_comb_finder_targets_ranks_name_matches_above_branch_matches() {
        let targets = vec![
            CombFinderTarget {
                hive_dir_name: "repo_api".to_string(),
                hive_repo_name: "ApiServer".to_string(),
                comb_id: "1".to_string(),
                comb_name: "feature-branch".to_string(),
                branch: "main".to_string(),
            },
            CombFinderTarget {
                hive_dir_name: "repo_web".to_string(),
                hive_repo_name: "Frontend".to_string(),
                comb_id: "2".to_string(),
                comb_name: "alpha".to_string(),
                branch: "foo/bar".to_string(),
            },
        ];

        let filtered = filter_comb_finder_targets(&targets, "fb");

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].comb_id, "1");
        assert_eq!(filtered[1].comb_id, "2");
    }

    #[test]
    fn apply_refresh_preserves_selected_comb_by_id() {
        let selected_comb = comb("b", "beta", "main");
        let mut app = make_app(
            vec![
                NavItem::Hive {
                    info: hive("repo_api", "api"),
                    expanded: true,
                    comb_count: 2,
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("a", "alpha", "main"),
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: selected_comb.clone(),
                },
            ],
            2,
        );

        app.apply_refresh(RefreshResult {
            hive_data: vec![(
                hive("repo_api", "api"),
                vec![
                    comb("x", "aardvark", "main"),
                    comb("a", "alpha", "main"),
                    selected_comb,
                ],
            )],
        });

        assert_eq!(app.selected, 3);
        match &app.items[app.selected] {
            NavItem::Comb { comb, .. } => assert_eq!(comb.id, "b"),
            _ => panic!("expected selected comb"),
        }
    }

    #[test]
    fn start_delete_mode_from_hive_selects_first_comb_in_that_hive() {
        let mut app = make_app(
            vec![
                NavItem::Hive {
                    info: hive("repo_api", "api"),
                    expanded: true,
                    comb_count: 2,
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("a", "alpha", "main"),
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("b", "beta", "main"),
                },
            ],
            0,
        );

        app.start_delete_mode();

        assert!(matches!(app.mode, AppMode::DeleteCombSelection { .. }));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn selected_delete_targets_follow_marked_comb_ids() {
        let mut app = make_app(
            vec![
                NavItem::Hive {
                    info: hive("repo_api", "api"),
                    expanded: true,
                    comb_count: 3,
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("a", "alpha", "main"),
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("b", "beta", "main"),
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("c", "gamma", "main"),
                },
            ],
            1,
        );
        app.mode = AppMode::DeleteCombSelection {
            hive_dir_name: "repo_api".to_string(),
            selected_comb_ids: HashSet::from(["a".to_string(), "c".to_string()]),
        };

        let targets = app.selected_delete_targets();

        assert_eq!(targets.len(), 2);
        assert!(matches!(&targets[0], DeleteTarget::Comb { comb_id, .. } if comb_id == "a"));
        assert!(matches!(&targets[1], DeleteTarget::Comb { comb_id, .. } if comb_id == "c"));
    }

    #[test]
    fn start_add_hive_forces_sidebar_focus() {
        let mut app = make_app(vec![], 0);
        app.focus = Focus::Terminal;

        app.start_add_hive();

        assert!(matches!(app.mode, AppMode::Input { .. }));
        assert!(matches!(app.focus, Focus::Sidebar));
    }

    #[test]
    fn start_delete_mode_forces_sidebar_focus() {
        let mut app = make_app(
            vec![
                NavItem::Hive {
                    info: hive("repo_api", "api"),
                    expanded: true,
                    comb_count: 1,
                },
                NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("a", "alpha", "main"),
                },
            ],
            1,
        );
        app.focus = Focus::Terminal;

        app.start_delete_mode();

        assert!(matches!(app.mode, AppMode::DeleteCombSelection { .. }));
        assert!(matches!(app.focus, Focus::Sidebar));
    }
}
