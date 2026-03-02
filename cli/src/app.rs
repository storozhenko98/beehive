use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::config::*;
use crate::terminal::EmbeddedTerminal;

/// Background clone result.
pub struct CloneResult {
    pub comb: Result<Comb, String>,
    pub comb_name: String,
    #[allow(dead_code)]
    pub hive_dir_name: String,
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
    Help,
    Settings {
        preflight: PreflightResult,
    },
}

pub enum InputAction {
    NewCombName { hive_dir_name: String },
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
    pub pending_clone: Option<Arc<Mutex<Option<CloneResult>>>>,
}

impl App {
    pub fn new(beehive_dir: String) -> Result<Self, String> {
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
            pending_clone: None,
        };
        app.load_all(true)?;
        Ok(app)
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
            let live_combs: Vec<Comb> = combs.into_iter().filter(|c| !c.cloning).collect();
            let comb_count = live_combs.len();

            items.push(NavItem::Hive {
                info: info.clone(),
                expanded: was_expanded,
                comb_count,
            });

            if was_expanded {
                for comb in live_combs {
                    items.push(NavItem::Comb {
                        hive_dir_name: dir_name.clone(),
                        comb,
                    });
                }
            }
        }

        if self.selected >= items.len() && !items.is_empty() {
            self.selected = items.len() - 1;
        }
        if items.is_empty() {
            self.selected = 0;
        }
        self.items = items;
        Ok(())
    }

    pub fn refresh(&mut self) {
        let _ = self.load_all(false);
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
                            if !comb.cloning {
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
            self.mode = AppMode::Input {
                prompt: "Comb name".to_string(),
                value: String::new(),
                action: InputAction::NewCombName {
                    hive_dir_name: dir_name,
                },
            };
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
            self.mode = AppMode::Input {
                prompt: format!("Copy '{}' as", comb.name),
                value: String::new(),
                action: InputAction::CopyCombName {
                    hive_dir_name: hive_dir_name.clone(),
                    source_comb_name: comb.name.clone(),
                    source_comb_path: comb.path.clone(),
                },
            };
        } else {
            self.status_message = Some("Select a comb to copy".to_string());
        }
    }

    pub fn start_add_hive(&mut self) {
        self.mode = AppMode::Input {
            prompt: "Repository (owner/repo or URL)".to_string(),
            value: String::new(),
            action: InputAction::AddHiveUrl,
        };
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
                self.mode = AppMode::Confirm {
                    message: format!("Delete comb '{}'?", comb.name),
                    action: ConfirmAction::DeleteComb {
                        hive_dir_name: hive_dir_name.clone(),
                        comb_id: comb.id.clone(),
                        comb_name: comb.name.clone(),
                    },
                };
            }
            NavItem::Hive { info, .. } => {
                self.mode = AppMode::Confirm {
                    message: format!("Delete hive '{}'?", info.repo_name),
                    action: ConfirmAction::DeleteHive {
                        dir_name: info.dir_name.clone(),
                        repo_name: info.repo_name.clone(),
                    },
                };
            }
        }
    }

    pub fn start_quit(&mut self) {
        self.mode = AppMode::Confirm {
            message: "Quit Beehive?".to_string(),
            action: ConfirmAction::Quit,
        };
    }

    pub fn open_settings(&mut self) {
        let pf = preflight();
        self.mode = AppMode::Settings { preflight: pf };
    }

    pub fn open_help(&mut self) {
        self.mode = AppMode::Help;
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
