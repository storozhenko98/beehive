mod app;
mod config;
mod fuzzy;
mod keyboard;
mod terminal;
mod ui;
mod update;

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{
    App, AppMode, CloneResult, ConfirmAction, DeleteResult, DeleteTarget, Focus, InputAction,
    RefreshResult,
};
use config::*;

type Term = Terminal<CrosstermBackend<io::Stdout>>;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Preflight checks before entering TUI
    let pf = preflight();
    if pf.git.is_none() {
        eprintln!("Error: git is not installed. Beehive requires git.");
        std::process::exit(1);
    }
    let mut warnings: Vec<String> = vec![];
    if pf.gh.is_none() {
        warnings.push("gh CLI not found — 'add hive' will not work".to_string());
    } else if !pf.gh_auth {
        warnings.push("gh not authenticated — run 'gh auth login'".to_string());
    }

    let beehive_dir = ensure_config()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();

    // Query keyboard enhancement support (kitty protocol).
    // Must be after enable_raw_mode(). Returns quickly for both supporting
    // and non-supporting terminals (< 100ms typical, 2s timeout worst case).
    let keyboard_enhanced = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);

    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste,
        EnableFocusChange,
    )?;

    // Enable kitty keyboard protocol if the terminal supports it.
    // This gives us: SUPER/META modifiers, press/repeat/release event kinds,
    // and unambiguous encoding for Enter, Tab, Backspace, Esc.
    if keyboard_enhanced {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(beehive_dir, keyboard_enhanced)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    // Clean up any stale cloning:true entries left by a previous crash
    cleanup_stale_cloning(&app.beehive_dir);
    app.refresh();

    if let Some(warn) = warnings.first() {
        app.status_message = Some(warn.clone());
    }

    // Background update check
    let update_slot: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
    {
        let slot = Arc::clone(&update_slot);
        std::thread::spawn(move || {
            let result = update::check_for_update();
            *slot.lock().unwrap() = Some(result);
        });
    }

    let result = run_app(&mut terminal, &mut app, update_slot);

    // Pop keyboard enhancement before leaving alternate screen
    if keyboard_enhanced {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        DisableBracketedPaste,
        DisableFocusChange,
        LeaveAlternateScreen,
    )?;
    terminal.show_cursor()?;

    result
}

fn ensure_config() -> Result<String, Box<dyn std::error::Error>> {
    let app_config = load_app_config().map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    if let Some(dir) = app_config.beehive_dir {
        if Path::new(&dir).exists() {
            return Ok(dir);
        }
    }

    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|| "/tmp".to_string());
    let default_dir = format!("{}/beehive", home);

    println!("Beehive directory? [{}]", default_dir);
    print!("> ");
    io::Write::flush(&mut io::stdout())?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let dir = input.trim();
    let dir = if dir.is_empty() {
        default_dir
    } else if dir.starts_with('~') {
        dir.replacen('~', &home, 1)
    } else {
        dir.to_string()
    };

    init_beehive(&dir).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    save_app_config(&AppConfig {
        beehive_dir: Some(dir.clone()),
        mux_preference: None,
        cli_command: None,
        sidebar_width: 28,
    })
    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    Ok(dir)
}

fn run_app(
    terminal: &mut Term,
    app: &mut App,
    update_slot: Arc<Mutex<Option<Option<String>>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let key_trace = std::env::var("BEEHIVE_KEY_TRACE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let mut tick_count: u32 = 0;
    let mut dirty = true;
    let mut term_area = ratatui::layout::Rect::default();

    loop {
        // --- Phase 1: Process pending PTY output (no lock contention) ---
        for term in app.terminals.values_mut() {
            if term.process_pending_output() {
                dirty = true;
            }
        }

        // --- Phase 2: Background checks ---
        let clone_done = check_pending_clones(app);
        let delete_done = check_pending_deletes(app);

        // Force redraw when operations complete or are still in progress (spinner animation)
        if clone_done || delete_done || app.has_pending_work() {
            dirty = true;
        }

        if app.update_available.is_none() {
            if let Ok(mut guard) = update_slot.try_lock() {
                if let Some(result) = guard.take() {
                    app.update_available = result;
                    dirty = true;
                }
            }
        }

        // Check for completed background refresh
        if let Some(slot) = app.pending_refresh.clone() {
            if let Ok(mut guard) = slot.try_lock() {
                if !app.should_pause_refresh() {
                    if let Some(result) = guard.take() {
                        drop(guard);
                        app.pending_refresh = None;
                        app.apply_refresh(result);
                        dirty = true;
                    }
                }
            }
        }

        // Launch async refresh every ~5 seconds (only if not already running)
        tick_count = tick_count.wrapping_add(1);
        if tick_count % 312 == 0 && app.pending_refresh.is_none() && !app.should_pause_refresh() {
            let slot: Arc<Mutex<Option<RefreshResult>>> = Arc::new(Mutex::new(None));
            let slot_clone = Arc::clone(&slot);
            let beehive_dir = app.beehive_dir.clone();
            std::thread::spawn(move || {
                if let Ok(hives) = list_hives(&beehive_dir) {
                    let mut hive_data = Vec::new();
                    for info in hives {
                        let combs = get_combs(&beehive_dir, &info.dir_name).unwrap_or_default();
                        hive_data.push((info, combs));
                    }
                    if let Ok(mut guard) = slot_clone.lock() {
                        *guard = Some(RefreshResult { hive_data });
                    }
                }
            });
            app.pending_refresh = Some(slot);
        }

        // --- Phase 3: Render (only if something changed) ---
        if dirty {
            terminal.draw(|frame| {
                term_area = ui::render(frame, app);
            })?;

            // Resize PTY if terminal pane dimensions changed
            let new_size = (term_area.width, term_area.height);
            if new_size != app.last_term_size && new_size.0 > 0 && new_size.1 > 0 {
                if let Some(t) = app.active_terminal_mut() {
                    t.resize(new_size.1, new_size.0);
                }
                app.last_term_size = new_size;
            }

            dirty = false;
        }

        // --- Phase 4: Drain ALL pending input events, then wait up to 16ms ---
        // Process any already-queued events without blocking first
        while event::poll(Duration::from_millis(0))? {
            let evt = event::read()?;
            dirty = true;
            process_event(app, &evt, term_area, terminal, key_trace)?;
            if app.should_quit {
                break;
            }
        }

        if app.should_quit {
            break;
        }

        // Wait for next event or timeout (yields CPU when idle)
        if event::poll(Duration::from_millis(16))? {
            let evt = event::read()?;
            dirty = true;
            process_event(app, &evt, term_area, terminal, key_trace)?;
        }
        // PTY output may have arrived during the wait — always re-check at top of loop

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

/// Process a single crossterm event, routing to terminal PTY or sidebar handler.
fn process_event(
    app: &mut App,
    evt: &Event,
    term_area: ratatui::layout::Rect,
    terminal: &mut Term,
    key_trace: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Filter out key release events when keyboard enhancement is active.
    // We only process Press and Repeat — Release events would cause double input.
    if let Event::Key(key) = evt {
        if key.kind == KeyEventKind::Release {
            if key_trace {
                eprintln!(
                    "[key-trace] SKIP release: {:?} mods={:?}",
                    key.code, key.modifiers
                );
            }
            return Ok(());
        }
        if key_trace {
            eprintln!(
                "[key-trace] {:?} kind={:?} mods={:?} enhanced={}",
                key.code, key.kind, key.modifiers, app.keyboard_enhanced,
            );
        }
    }

    if app.focus == Focus::Terminal && matches!(app.mode, AppMode::Normal) {
        match evt {
            Event::Key(key) => {
                // Ctrl+Space: toggle focus back to sidebar
                if is_focus_toggle(key) {
                    app.focus = Focus::Sidebar;
                } else if let Some(t) = app.active_terminal() {
                    // All key events go through the unified encoding pipeline.
                    // No special-casing for Ctrl+C etc. — key_to_bytes handles it.
                    let bytes = terminal::event_to_bytes(evt, t, term_area);
                    if !bytes.is_empty() {
                        if key_trace {
                            let inner_enh = t.keyboard_enhanced();
                            eprintln!(
                                "[key-trace] → PTY {} bytes: {:02x?} inner_enhanced={}",
                                bytes.len(),
                                bytes,
                                inner_enh,
                            );
                        }
                        t.write_input(&bytes);
                    }
                }
            }
            Event::Mouse(mouse) => {
                if let Some(t) = app.active_terminal() {
                    let should_open_url =
                        matches!(mouse.kind, event::MouseEventKind::Up(event::MouseButton::Left))
                            && t.mouse_protocol_mode() == vt100::MouseProtocolMode::None;

                    if should_open_url {
                        let col = mouse.column as i32 - term_area.x as i32;
                        let row = mouse.row as i32 - term_area.y as i32;
                        if col >= 0
                            && row >= 0
                            && col < term_area.width as i32
                            && row < term_area.height as i32
                        {
                            let found = t.with_screen(|screen| {
                                terminal::url_at_position(screen, row as u16, col as u16)
                            });
                            if let Some(url) = found {
                                terminal::open_url(&url);
                                // Don't forward this click to the PTY
                                return Ok(());
                            }
                        }
                    }

                    let input = terminal::event_to_bytes(evt, t, term_area);
                    if !input.is_empty() {
                        t.write_input(&input);
                    }
                }
            }
            Event::Paste(_) | Event::FocusGained | Event::FocusLost => {
                if let Some(t) = app.active_terminal() {
                    let bytes = terminal::event_to_bytes(evt, t, term_area);
                    if !bytes.is_empty() {
                        t.write_input(&bytes);
                    }
                }
            }
            Event::Resize(_, _) => {}
        }
    } else {
        match evt {
            Event::Key(key) => {
                handle_key(app, *key, terminal)?;
            }
            Event::Paste(text) => {
                if let AppMode::Input { value, cursor, .. } = &mut app.mode {
                    let byte_pos = value
                        .char_indices()
                        .nth(*cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(value.len());
                    value.insert_str(byte_pos, text);
                    *cursor += text.chars().count();
                } else if let AppMode::BranchPicker {
                    filter,
                    filter_cursor,
                    selected,
                    ..
                } = &mut app.mode
                {
                    let byte_pos = filter
                        .char_indices()
                        .nth(*filter_cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(filter.len());
                    filter.insert_str(byte_pos, text);
                    *filter_cursor += text.chars().count();
                    *selected = 0;
                } else if let AppMode::CombFinder {
                    filter,
                    filter_cursor,
                    selected,
                    ..
                } = &mut app.mode
                {
                    let byte_pos = filter
                        .char_indices()
                        .nth(*filter_cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(filter.len());
                    filter.insert_str(byte_pos, text);
                    *filter_cursor += text.chars().count();
                    *selected = 0;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Returns true if any clone/copy operation completed this tick (needs redraw).
fn check_pending_clones(app: &mut App) -> bool {
    let mut any_completed = false;
    let mut i = 0;
    while i < app.pending_clones.len() {
        let done = {
            let guard = app.pending_clones[i].slot.lock().unwrap();
            guard.is_some()
        };
        if done {
            any_completed = true;
            let pending = app.pending_clones.remove(i);
            let result = pending.slot.lock().unwrap().take().unwrap();
            let paused = app.should_pause_refresh();
            match result.comb {
                Ok(comb) => {
                    if result.is_copy {
                        app.status_message = Some(format!("Copied '{}'", result.comb_name));
                        if paused {
                            // Update the comb in-place so the spinner stops
                            update_comb_in_items(&mut app.items, &comb.id, &comb);
                            app.needs_refresh = true;
                        } else {
                            let id = comb.id.clone();
                            let path = comb.path.clone();
                            app.active_comb_id = Some(id.clone());
                            app.refresh();
                            app.open_terminal(&id, &path);
                        }
                    } else {
                        app.status_message = Some(format!("Created '{}'", result.comb_name));
                        if paused {
                            // Update the comb in-place so the spinner stops
                            update_comb_in_items(&mut app.items, &comb.id, &comb);
                            app.needs_refresh = true;
                        } else {
                            app.refresh();
                        }
                    }
                }
                Err(e) => {
                    let action = if result.is_copy { "Copy" } else { "Clone" };
                    app.status_message = Some(format!("{} failed: {}", action, e));
                    if paused {
                        // Remove the failed placeholder in-place
                        remove_comb_from_items(&mut app.items, &result.comb_name);
                        app.needs_refresh = true;
                    } else {
                        app.refresh();
                    }
                }
            }
            // Don't increment i — the Vec shifted
        } else {
            i += 1;
        }
    }
    any_completed
}

/// Update a comb entry in-place within the items list (e.g. flip cloning to false).
fn update_comb_in_items(items: &mut [app::NavItem], comb_id: &str, fresh: &Comb) {
    for item in items.iter_mut() {
        if let app::NavItem::Comb { comb, .. } = item {
            if comb.id == comb_id {
                comb.cloning = fresh.cloning;
                comb.branch = fresh.branch.clone();
                break;
            }
        }
    }
}

/// Remove a comb entry from the items list by name (for failed clones/copies).
fn remove_comb_from_items(items: &mut Vec<app::NavItem>, comb_name: &str) {
    items.retain(|item| {
        !matches!(item, app::NavItem::Comb { comb, .. } if comb.name == comb_name && comb.cloning)
    });
}

/// Returns true if any delete operation completed this tick (needs redraw).
fn check_pending_deletes(app: &mut App) -> bool {
    let mut any_completed = false;
    let mut i = 0;
    while i < app.pending_deletes.len() {
        let done = {
            let guard = app.pending_deletes[i].slot.lock().unwrap();
            guard.is_some()
        };
        if done {
            any_completed = true;
            let pending = app.pending_deletes.remove(i);
            let result = pending.slot.lock().unwrap().take().unwrap();

            // Clear deleting markers for completed items
            for name in &result.deleted_comb_names {
                // Find the comb ID by name and remove from deleting set
                let comb_id = app.items.iter().find_map(|item| match item {
                    app::NavItem::Comb { comb, .. } if comb.name == *name => Some(comb.id.clone()),
                    _ => None,
                });
                if let Some(id) = comb_id {
                    app.deleting_comb_ids.remove(&id);
                }
            }
            for name in &result.deleted_hive_names {
                let dir = app.items.iter().find_map(|item| match item {
                    app::NavItem::Hive { info, .. } if info.repo_name == *name => {
                        Some(info.dir_name.clone())
                    }
                    _ => None,
                });
                if let Some(d) = dir {
                    app.deleting_hive_dir_names.remove(&d);
                }
            }

            let mut parts = Vec::new();
            if result.deleted_comb_names.len() == 1 {
                parts.push(format!("Deleted '{}'", result.deleted_comb_names[0]));
            } else if !result.deleted_comb_names.is_empty() {
                parts.push(format!("Deleted {} combs", result.deleted_comb_names.len()));
            }
            if result.deleted_hive_names.len() == 1 {
                parts.push(format!("Deleted hive '{}'", result.deleted_hive_names[0]));
            } else if !result.deleted_hive_names.is_empty() {
                parts.push(format!("Deleted {} hives", result.deleted_hive_names.len()));
            }

            app.status_message = if result.errors.is_empty() {
                Some(parts.join("; "))
            } else if parts.is_empty() {
                Some(format!("Delete failed: {}", result.errors.join("; ")))
            } else {
                Some(format!(
                    "{}; {}",
                    parts.join("; "),
                    result.errors.join("; ")
                ))
            };
            app.refresh();
            // Don't increment i — the Vec shifted
        } else {
            i += 1;
        }
    }
    any_completed
}

fn is_focus_toggle(key: &crossterm::event::KeyEvent) -> bool {
    key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn is_delete_mode_toggle(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('D'))
        || (key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::SHIFT))
}

fn handle_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    terminal: &mut Term,
) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(app.mode, AppMode::Normal) && is_focus_toggle(&key) {
        app.focus = Focus::Sidebar;
        return Ok(());
    }

    // Help and Settings: Esc or q to close
    match &app.mode {
        AppMode::Help => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                    app.mode = AppMode::Normal;
                }
                _ => {}
            }
            return Ok(());
        }
        AppMode::Settings { .. } => {
            match key.code {
                KeyCode::Esc | KeyCode::Char('s') => {
                    app.mode = AppMode::Normal;
                }
                KeyCode::Char('R') => {
                    app.enter_sidebar_mode(AppMode::Confirm {
                        message: "Reset config? (repos stay on disk)".to_string(),
                        action: ConfirmAction::ResetConfig,
                    });
                }
                _ => {}
            }
            return Ok(());
        }
        AppMode::BranchPicker { .. } => {
            handle_branch_picker(app, key, terminal)?;
            return Ok(());
        }
        AppMode::CombFinder { .. } => {
            handle_comb_finder(app, key)?;
            return Ok(());
        }
        AppMode::DeleteCombSelection { .. } => {
            handle_delete_mode(app, key)?;
            return Ok(());
        }
        AppMode::MovingComb { .. } => {
            handle_moving_comb(app, key)?;
            return Ok(());
        }
        AppMode::Input { .. } => {
            handle_input_key(app, key, terminal)?;
            return Ok(());
        }
        AppMode::Confirm { .. } => {
            handle_confirm_key(app, key)?;
            return Ok(());
        }
        AppMode::Normal => {}
    }

    // Focus toggle: Ctrl+Space to switch to terminal (sidebar → terminal)
    if is_focus_toggle(&key) {
        if app.focus == Focus::Sidebar && app.active_terminal().is_some() {
            app.focus = Focus::Terminal;
        }
        return Ok(());
    }

    match app.focus {
        Focus::Terminal => {
            // Terminal focus with non-Normal mode (overlay showing) — ignore keys
            // (the overlay-specific handlers above already returned for Help/Settings/BranchPicker)
        }
        Focus::Sidebar => {
            app.status_message = None;
            if is_delete_mode_toggle(&key) {
                app.start_delete_mode();
                return Ok(());
            }
            match key.code {
                KeyCode::Char('q') => app.start_quit(),
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::Enter | KeyCode::Char('l') => {
                    if let Some((id, path)) = app.enter_selected() {
                        if Path::new(&path).exists() {
                            app.open_terminal(&id, &path);
                        } else {
                            app.status_message = Some("Dir not found".to_string());
                        }
                    }
                }
                KeyCode::Char('n') => app.start_new_comb(),
                KeyCode::Char('r') => app.start_rename_comb(),
                KeyCode::Char('f') => app.start_comb_finder(),
                KeyCode::Char('m') => app.start_move_comb(),
                KeyCode::Char('c') => app.start_copy_comb(),
                KeyCode::Char('a') => app.start_add_hive(),
                KeyCode::Char('d') => app.start_delete(),
                KeyCode::Char('R') => {
                    app.refresh();
                    app.status_message = Some("Refreshed".to_string());
                }
                KeyCode::Char('s') => app.open_settings(),
                KeyCode::Char('?') => app.open_help(),
                KeyCode::Char('<') | KeyCode::Char('H') => {
                    if app.sidebar_width > 20 {
                        app.sidebar_width -= 2;
                        app.save_sidebar_width();
                    }
                }
                KeyCode::Char('>') | KeyCode::Char('L') => {
                    app.sidebar_width += 2;
                    app.save_sidebar_width();
                }
                KeyCode::Char('u') => {
                    if let Some(ver) = app.update_available.clone() {
                        app.status_message = Some(format!("Updating to v{}...", ver));
                        terminal.draw(|frame| {
                            ui::render(frame, app);
                        })?;
                        match update::self_update(&ver) {
                            Ok(()) => {
                                app.status_message = Some(format!(
                                    "Updated to v{}! Restart to use new version.",
                                    ver
                                ));
                                app.update_available = None;
                            }
                            Err(e) => {
                                app.status_message = Some(e);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn handle_input_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    terminal: &mut Term,
) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            handle_input_submit(app, terminal)?;
        }
        KeyCode::Left => {
            if let AppMode::Input { cursor, .. } = &mut app.mode {
                if *cursor > 0 {
                    *cursor -= 1;
                }
            }
        }
        KeyCode::Right => {
            if let AppMode::Input { value, cursor, .. } = &mut app.mode {
                if *cursor < value.chars().count() {
                    *cursor += 1;
                }
            }
        }
        KeyCode::Home => {
            if let AppMode::Input { cursor, .. } = &mut app.mode {
                *cursor = 0;
            }
        }
        KeyCode::End => {
            if let AppMode::Input { value, cursor, .. } = &mut app.mode {
                *cursor = value.chars().count();
            }
        }
        KeyCode::Char(c) => {
            if let AppMode::Input { value, cursor, .. } = &mut app.mode {
                let byte_pos = value
                    .char_indices()
                    .nth(*cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(value.len());
                value.insert(byte_pos, c);
                *cursor += 1;
            }
        }
        KeyCode::Backspace => {
            if let AppMode::Input { value, cursor, .. } = &mut app.mode {
                if *cursor > 0 {
                    *cursor -= 1;
                    let byte_pos = value
                        .char_indices()
                        .nth(*cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(value.len());
                    value.remove(byte_pos);
                }
            }
        }
        KeyCode::Delete => {
            if let AppMode::Input { value, cursor, .. } = &mut app.mode {
                if *cursor < value.chars().count() {
                    let byte_pos = value
                        .char_indices()
                        .nth(*cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(value.len());
                    value.remove(byte_pos);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_confirm_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => {
            handle_confirm(app)?;
        }
        _ => {
            app.mode = AppMode::Normal;
        }
    }
    Ok(())
}

fn handle_comb_finder(
    app: &mut App,
    key: crossterm::event::KeyEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let AppMode::CombFinder {
                targets,
                filter,
                selected,
                ..
            } = &mut app.mode
            {
                let filtered_count = app::filter_comb_finder_targets(targets, filter).len();
                if *selected > 0 {
                    *selected -= 1;
                } else if filtered_count > 0 {
                    *selected = filtered_count - 1;
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let AppMode::CombFinder {
                targets,
                filter,
                selected,
                ..
            } = &mut app.mode
            {
                let filtered_count = app::filter_comb_finder_targets(targets, filter).len();
                if *selected + 1 < filtered_count {
                    *selected += 1;
                }
            }
        }
        KeyCode::Enter => {
            let mode = std::mem::replace(&mut app.mode, AppMode::Normal);
            if let AppMode::CombFinder {
                targets,
                filter,
                selected,
                ..
            } = mode
            {
                let filtered = app::filter_comb_finder_targets(&targets, &filter);
                if let Some(target) = filtered.get(selected) {
                    match app.reveal_comb(&target.hive_dir_name, &target.comb_id) {
                        Ok(true) => {
                            app.status_message = Some(format!("Jumped to '{}'", target.comb_name));
                        }
                        Ok(false) => {
                            app.status_message = Some("Comb not found".to_string());
                        }
                        Err(e) => {
                            app.status_message = Some(format!("Jump failed: {}", e));
                        }
                    }
                } else {
                    app.status_message = Some("No matching combs".to_string());
                }
            }
        }
        KeyCode::Left => {
            if let AppMode::CombFinder { filter_cursor, .. } = &mut app.mode {
                if *filter_cursor > 0 {
                    *filter_cursor -= 1;
                }
            }
        }
        KeyCode::Right => {
            if let AppMode::CombFinder {
                filter,
                filter_cursor,
                ..
            } = &mut app.mode
            {
                if *filter_cursor < filter.chars().count() {
                    *filter_cursor += 1;
                }
            }
        }
        KeyCode::Char(c) => {
            if let AppMode::CombFinder {
                filter,
                filter_cursor,
                selected,
                ..
            } = &mut app.mode
            {
                let byte_pos = filter
                    .char_indices()
                    .nth(*filter_cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(filter.len());
                filter.insert(byte_pos, c);
                *filter_cursor += 1;
                *selected = 0;
            }
        }
        KeyCode::Backspace => {
            if let AppMode::CombFinder {
                filter,
                filter_cursor,
                selected,
                ..
            } = &mut app.mode
            {
                if *filter_cursor > 0 {
                    *filter_cursor -= 1;
                    let byte_pos = filter
                        .char_indices()
                        .nth(*filter_cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(filter.len());
                    filter.remove(byte_pos);
                    *selected = 0;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_delete_mode(
    app: &mut App,
    key: crossterm::event::KeyEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
            app.status_message = Some("Delete cancelled".to_string());
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.status_message = None;
            app.move_delete_selection_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.status_message = None;
            app.move_delete_selection_down();
        }
        KeyCode::Char('d') | KeyCode::Char('D') => {
            app.status_message = None;
            app.toggle_delete_selection();
        }
        KeyCode::Enter => {
            let targets = app.selected_delete_targets();
            if targets.is_empty() {
                app.status_message = Some("No combs selected for delete".to_string());
            } else {
                app.mode = AppMode::Normal;
                start_async_delete(app, targets)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_branch_picker(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    terminal: &mut Term,
) -> Result<(), Box<dyn std::error::Error>> {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Up => {
            if let AppMode::BranchPicker {
                selected,
                filter,
                branches,
                ..
            } = &mut app.mode
            {
                let filtered_count = fuzzy::fuzzy_filter_strings(branches, filter).len();
                if *selected > 0 {
                    *selected -= 1;
                } else if filtered_count > 0 {
                    *selected = filtered_count - 1;
                }
            }
        }
        KeyCode::Down => {
            if let AppMode::BranchPicker {
                selected,
                filter,
                branches,
                ..
            } = &mut app.mode
            {
                let filtered_count = fuzzy::fuzzy_filter_strings(branches, filter).len();
                if *selected + 1 < filtered_count {
                    *selected += 1;
                }
            }
        }
        KeyCode::Enter => {
            // Extract data from mode before replacing it
            let mode = std::mem::replace(&mut app.mode, AppMode::Normal);
            if let AppMode::BranchPicker {
                hive_dir_name,
                comb_name,
                branches,
                default_branch,
                filter,
                selected,
                ..
            } = mode
            {
                let filtered = fuzzy::fuzzy_filter_strings(&branches, &filter);
                let branch = if filtered.is_empty() {
                    if filter.is_empty() {
                        default_branch
                    } else {
                        filter
                    }
                } else {
                    filtered
                        .get(selected)
                        .map(|s| s.to_string())
                        .unwrap_or(default_branch)
                };

                start_async_clone(app, terminal, hive_dir_name, comb_name, branch)?;
            }
        }
        KeyCode::Left => {
            if let AppMode::BranchPicker { filter_cursor, .. } = &mut app.mode {
                if *filter_cursor > 0 {
                    *filter_cursor -= 1;
                }
            }
        }
        KeyCode::Right => {
            if let AppMode::BranchPicker {
                filter,
                filter_cursor,
                ..
            } = &mut app.mode
            {
                if *filter_cursor < filter.chars().count() {
                    *filter_cursor += 1;
                }
            }
        }
        KeyCode::Char(c) => {
            if let AppMode::BranchPicker {
                filter,
                filter_cursor,
                selected,
                ..
            } = &mut app.mode
            {
                let byte_pos = filter
                    .char_indices()
                    .nth(*filter_cursor)
                    .map(|(i, _)| i)
                    .unwrap_or(filter.len());
                filter.insert(byte_pos, c);
                *filter_cursor += 1;
                *selected = 0;
            }
        }
        KeyCode::Backspace => {
            if let AppMode::BranchPicker {
                filter,
                filter_cursor,
                selected,
                ..
            } = &mut app.mode
            {
                if *filter_cursor > 0 {
                    *filter_cursor -= 1;
                    let byte_pos = filter
                        .char_indices()
                        .nth(*filter_cursor)
                        .map(|(i, _)| i)
                        .unwrap_or(filter.len());
                    filter.remove(byte_pos);
                    *selected = 0;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_moving_comb(
    app: &mut App,
    key: crossterm::event::KeyEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let moving_comb_id = match &app.mode {
        AppMode::MovingComb { moving_comb_id, .. } => moving_comb_id.clone(),
        _ => return Ok(()),
    };
    let _ = app.select_comb_by_id(&moving_comb_id);

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.move_comb_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.move_comb_down();
        }
        KeyCode::Char('m') | KeyCode::Enter => {
            // Confirm: save the new order to disk
            let mode = std::mem::replace(&mut app.mode, AppMode::Normal);
            if let AppMode::MovingComb { hive_dir_name, .. } = mode {
                let comb_ids = app.comb_order_for_hive(&hive_dir_name);
                match reorder_combs(&app.beehive_dir, &hive_dir_name, &comb_ids) {
                    Ok(()) => {
                        app.status_message = Some("Order saved".to_string());
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Failed to save order: {}", e));
                    }
                }
            }
            // Flush any deferred refresh from operations that completed during the move
            if app.needs_refresh {
                app.needs_refresh = false;
                app.refresh();
            }
        }
        KeyCode::Esc => {
            // Cancel: restore original item order
            let mode = std::mem::replace(&mut app.mode, AppMode::Normal);
            if let AppMode::MovingComb {
                original_items,
                original_selected,
                ..
            } = mode
            {
                app.items = original_items;
                app.selected = if app.items.is_empty() {
                    0
                } else {
                    original_selected.min(app.items.len() - 1)
                };
                app.status_message = Some("Move cancelled".to_string());
            }
            // Flush any deferred refresh from operations that completed during the move
            if app.needs_refresh {
                app.needs_refresh = false;
                app.refresh();
            }
        }
        _ => {}
    }
    Ok(())
}

fn start_async_clone(
    app: &mut App,
    terminal: &mut Term,
    hive_dir_name: String,
    comb_name: String,
    branch: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Re-validate against the latest state (guards against TOCTOU race from branch picker delay)
    let mut state = load_hive_state(&app.beehive_dir, &hive_dir_name)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    if let Err(e) = validate_comb_name(&comb_name, &state.combs) {
        app.status_message = Some(e);
        return Ok(());
    }

    // Write a cloning:true placeholder to state BEFORE spawning the thread.
    // This acts as a name reservation and gives the sidebar something to show.
    let hive_dir = Path::new(&app.beehive_dir).join(&hive_dir_name);
    let comb_dir = hive_dir.join(&comb_name);
    let comb_id = uuid::Uuid::new_v4().to_string();

    let placeholder = Comb {
        id: comb_id.clone(),
        name: comb_name.clone(),
        branch: branch.clone(),
        path: comb_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        panes: vec![],
        cloning: true,
    };
    state.combs.push(placeholder);
    save_hive_state(&app.beehive_dir, &hive_dir_name, &state)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    app.status_message = Some(format!("Cloning '{}'...", comb_name));
    app.refresh();
    terminal.draw(|frame| {
        ui::render(frame, app);
    })?;

    let slot: Arc<Mutex<Option<CloneResult>>> = Arc::new(Mutex::new(None));
    let slot_clone = Arc::clone(&slot);
    let beehive_dir = app.beehive_dir.clone();
    let hive_dir_clone = hive_dir_name.clone();
    let comb_name_clone = comb_name.clone();
    let comb_id_clone = comb_id.clone();
    let branch_clone = branch.clone();

    std::thread::spawn(move || {
        let result = create_comb_sync(
            &beehive_dir,
            &hive_dir_clone,
            &comb_name_clone,
            &comb_id_clone,
            &branch_clone,
        );
        let mut guard = slot_clone.lock().unwrap();
        *guard = Some(CloneResult {
            comb: result,
            comb_name: comb_name_clone,
            hive_dir_name: hive_dir_clone,
            is_copy: false,
        });
    });

    app.pending_clones.push(app::PendingClone {
        slot,
        activity: format!("Cloning '{}'", comb_name),
    });
    Ok(())
}

fn start_async_copy(
    app: &mut App,
    terminal: &mut Term,
    hive_dir_name: String,
    comb_name: String,
    source_comb_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load fresh state and validate
    let mut state = load_hive_state(&app.beehive_dir, &hive_dir_name)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    if let Err(e) = validate_comb_name(&comb_name, &state.combs) {
        app.status_message = Some(e);
        return Ok(());
    }

    // Write cloning:true placeholder (reused for copy operations too)
    let hive_dir = Path::new(&app.beehive_dir).join(&hive_dir_name);
    let comb_dir = hive_dir.join(&comb_name);
    let comb_id = uuid::Uuid::new_v4().to_string();

    let placeholder = Comb {
        id: comb_id.clone(),
        name: comb_name.clone(),
        branch: String::new(), // filled in after copy
        path: comb_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        panes: vec![],
        cloning: true,
    };
    state.combs.push(placeholder);
    save_hive_state(&app.beehive_dir, &hive_dir_name, &state)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    app.status_message = Some(format!("Copying to '{}'...", comb_name));
    app.refresh();
    terminal.draw(|frame| {
        ui::render(frame, app);
    })?;

    let slot: Arc<Mutex<Option<CloneResult>>> = Arc::new(Mutex::new(None));
    let slot_clone = Arc::clone(&slot);
    let beehive_dir = app.beehive_dir.clone();
    let hive_dir_clone = hive_dir_name.clone();
    let comb_name_clone = comb_name.clone();
    let comb_id_clone = comb_id.clone();

    std::thread::spawn(move || {
        let result = copy_comb_sync(
            &beehive_dir,
            &hive_dir_clone,
            &comb_name_clone,
            &comb_id_clone,
            &source_comb_path,
        );
        let mut guard = slot_clone.lock().unwrap();
        *guard = Some(CloneResult {
            comb: result,
            comb_name: comb_name_clone,
            hive_dir_name: hive_dir_clone,
            is_copy: true,
        });
    });

    app.pending_clones.push(app::PendingClone {
        slot,
        activity: format!("Copying '{}'", comb_name),
    });
    Ok(())
}

fn delete_activity(targets: &[DeleteTarget]) -> String {
    if targets.len() == 1 {
        match &targets[0] {
            DeleteTarget::Comb { comb_name, .. } => format!("Deleting '{}'", comb_name),
            DeleteTarget::Hive { repo_name, .. } => format!("Deleting hive '{}'", repo_name),
        }
    } else {
        format!("Deleting {} combs", targets.len())
    }
}

fn delete_targets_sync(beehive_dir: &str, targets: Vec<DeleteTarget>) -> DeleteResult {
    let mut deleted_comb_names = Vec::new();
    let mut deleted_hive_names = Vec::new();
    let mut errors = Vec::new();

    for target in targets {
        match target {
            DeleteTarget::Comb {
                hive_dir_name,
                comb_id,
                comb_name,
            } => {
                let mut state = match load_hive_state(beehive_dir, &hive_dir_name) {
                    Ok(state) => state,
                    Err(e) => {
                        errors.push(format!("Failed to load '{}': {}", comb_name, e));
                        continue;
                    }
                };

                let Some(pos) = state.combs.iter().position(|comb| comb.id == comb_id) else {
                    errors.push(format!("Comb '{}' no longer exists", comb_name));
                    continue;
                };

                let comb = state.combs.remove(pos);
                if Path::new(&comb.path).exists() {
                    if let Err(e) = std::fs::remove_dir_all(&comb.path) {
                        errors.push(format!("Failed to delete '{}': {}", comb_name, e));
                        continue;
                    }
                }

                if let Err(e) = save_hive_state(beehive_dir, &hive_dir_name, &state) {
                    errors.push(format!("Failed to update '{}': {}", comb_name, e));
                    continue;
                }

                deleted_comb_names.push(comb_name);
            }
            DeleteTarget::Hive {
                dir_name,
                repo_name,
            } => {
                let hive_dir = Path::new(beehive_dir).join(&dir_name);
                if hive_dir.exists() {
                    if let Err(e) = std::fs::remove_dir_all(&hive_dir) {
                        errors.push(format!("Failed to delete hive '{}': {}", repo_name, e));
                        continue;
                    }
                }

                deleted_hive_names.push(repo_name);
            }
        }
    }

    DeleteResult {
        deleted_comb_names,
        deleted_hive_names,
        errors,
    }
}

fn start_async_delete(
    app: &mut App,
    targets: Vec<DeleteTarget>,
) -> Result<(), Box<dyn std::error::Error>> {
    if targets.is_empty() {
        app.status_message = Some("Nothing selected for delete".to_string());
        return Ok(());
    }

    for target in &targets {
        match target {
            DeleteTarget::Comb { comb_id, .. } => {
                app.deleting_comb_ids.insert(comb_id.clone());
                app.remove_terminal(comb_id);
            }
            DeleteTarget::Hive { dir_name, .. } => {
                app.deleting_hive_dir_names.insert(dir_name.clone());
                app.remove_hive_terminals(dir_name);
            }
        }
    }

    let slot: Arc<Mutex<Option<DeleteResult>>> = Arc::new(Mutex::new(None));
    let slot_clone = Arc::clone(&slot);
    let beehive_dir = app.beehive_dir.clone();
    let activity = delete_activity(&targets);

    std::thread::spawn(move || {
        let result = delete_targets_sync(&beehive_dir, targets);
        let mut guard = slot_clone.lock().unwrap();
        *guard = Some(result);
    });

    app.pending_refresh = None;
    app.pending_deletes
        .push(app::PendingDelete { slot, activity });
    app.status_message = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn make_terminal() -> Term {
        Terminal::new(CrosstermBackend::new(io::stdout())).unwrap()
    }

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

    fn make_app(items: Vec<app::NavItem>, selected: usize) -> App {
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
    fn moving_comb_escape_restores_original_selection_and_items() {
        let original_items = vec![
            app::NavItem::Hive {
                info: hive("repo_api", "api"),
                expanded: true,
                comb_count: 2,
            },
            app::NavItem::Comb {
                hive_dir_name: "repo_api".to_string(),
                comb: comb("a", "alpha", "main"),
            },
            app::NavItem::Comb {
                hive_dir_name: "repo_api".to_string(),
                comb: comb("b", "beta", "main"),
            },
        ];
        let mut moved_items = original_items.clone();
        moved_items.swap(1, 2);

        let mut app = make_app(moved_items, 1);
        app.mode = AppMode::MovingComb {
            hive_dir_name: "repo_api".to_string(),
            moving_comb_id: "b".to_string(),
            original_items: original_items.clone(),
            original_selected: 2,
        };

        handle_moving_comb(&mut app, crossterm::event::KeyEvent::from(KeyCode::Esc)).unwrap();

        assert!(matches!(app.mode, AppMode::Normal));
        assert_eq!(app.selected, 2);
        match &app.items[2] {
            app::NavItem::Comb { comb, .. } => assert_eq!(comb.id, "b"),
            _ => panic!("expected selected comb"),
        }
    }

    #[test]
    fn delete_mode_enter_requires_a_selection() {
        let mut app = make_app(
            vec![
                app::NavItem::Hive {
                    info: hive("repo_api", "api"),
                    expanded: true,
                    comb_count: 2,
                },
                app::NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("a", "alpha", "main"),
                },
                app::NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("b", "beta", "main"),
                },
            ],
            1,
        );
        app.mode = AppMode::DeleteCombSelection {
            hive_dir_name: "repo_api".to_string(),
            selected_comb_ids: HashSet::new(),
        };

        handle_delete_mode(&mut app, crossterm::event::KeyEvent::from(KeyCode::Enter)).unwrap();

        assert!(matches!(app.mode, AppMode::DeleteCombSelection { .. }));
        assert_eq!(
            app.status_message.as_deref(),
            Some("No combs selected for delete")
        );
    }

    #[test]
    fn delete_mode_d_toggles_current_comb() {
        let mut app = make_app(
            vec![
                app::NavItem::Hive {
                    info: hive("repo_api", "api"),
                    expanded: true,
                    comb_count: 1,
                },
                app::NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("a", "alpha", "main"),
                },
            ],
            1,
        );
        app.mode = AppMode::DeleteCombSelection {
            hive_dir_name: "repo_api".to_string(),
            selected_comb_ids: HashSet::new(),
        };

        handle_delete_mode(
            &mut app,
            crossterm::event::KeyEvent::from(KeyCode::Char('d')),
        )
        .unwrap();
        assert!(app.is_marked_for_delete("a"));

        handle_delete_mode(
            &mut app,
            crossterm::event::KeyEvent::from(KeyCode::Char('d')),
        )
        .unwrap();
        assert!(!app.is_marked_for_delete("a"));
    }

    #[test]
    fn comb_finder_enter_jumps_to_matching_visible_comb() {
        let mut app = make_app(
            vec![
                app::NavItem::Hive {
                    info: hive("repo_api", "api"),
                    expanded: true,
                    comb_count: 2,
                },
                app::NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("a", "alpha", "main"),
                },
                app::NavItem::Comb {
                    hive_dir_name: "repo_api".to_string(),
                    comb: comb("b", "beta", "main"),
                },
            ],
            0,
        );
        app.mode = AppMode::CombFinder {
            targets: vec![
                app::CombFinderTarget {
                    hive_dir_name: "repo_api".to_string(),
                    hive_repo_name: "api".to_string(),
                    comb_id: "a".to_string(),
                    comb_name: "alpha".to_string(),
                    branch: "main".to_string(),
                },
                app::CombFinderTarget {
                    hive_dir_name: "repo_api".to_string(),
                    hive_repo_name: "api".to_string(),
                    comb_id: "b".to_string(),
                    comb_name: "beta".to_string(),
                    branch: "main".to_string(),
                },
            ],
            filter: "bet".to_string(),
            filter_cursor: 3,
            selected: 0,
        };

        handle_comb_finder(&mut app, crossterm::event::KeyEvent::from(KeyCode::Enter)).unwrap();

        assert!(matches!(app.mode, AppMode::Normal));
        assert_eq!(app.selected, 2);
        assert_eq!(app.status_message.as_deref(), Some("Jumped to 'beta'"));
    }

    #[test]
    fn ctrl_space_during_input_keeps_sidebar_focus_and_overlay() {
        let mut app = make_app(vec![], 0);
        let mut terminal = make_terminal();
        app.focus = Focus::Sidebar;
        app.mode = AppMode::Input {
            prompt: "Comb name".to_string(),
            value: String::new(),
            cursor: 0,
            action: InputAction::AddHiveUrl,
        };

        handle_key(
            &mut app,
            crossterm::event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
            &mut terminal,
        )
        .unwrap();

        assert!(matches!(app.mode, AppMode::Input { .. }));
        assert!(matches!(app.focus, Focus::Sidebar));
    }

    #[test]
    fn input_overlay_handles_escape_even_if_focus_was_terminal() {
        let mut app = make_app(vec![], 0);
        let mut terminal = make_terminal();
        app.focus = Focus::Terminal;
        app.mode = AppMode::Input {
            prompt: "Comb name".to_string(),
            value: "draft".to_string(),
            cursor: 5,
            action: InputAction::AddHiveUrl,
        };

        handle_key(
            &mut app,
            crossterm::event::KeyEvent::from(KeyCode::Esc),
            &mut terminal,
        )
        .unwrap();

        assert!(matches!(app.mode, AppMode::Normal));
    }
}

fn handle_input_submit(
    app: &mut App,
    terminal: &mut Term,
) -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::mem::replace(&mut app.mode, AppMode::Normal);

    if let AppMode::Input { value, action, .. } = mode {
        match action {
            InputAction::NewCombName { hive_dir_name } => {
                let state = load_hive_state(&app.beehive_dir, &hive_dir_name)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
                if let Err(e) = validate_comb_name(&value, &state.combs) {
                    app.status_message = Some(e);
                    return Ok(());
                }

                // Fetch branches and show picker
                app.status_message = Some("Loading branches...".to_string());
                terminal.draw(|frame| {
                    ui::render(frame, app);
                })?;

                match list_branches(&app.beehive_dir, &hive_dir_name) {
                    Ok((branches, default_branch)) => {
                        app.status_message = None;
                        // Pre-select the default branch in the picker
                        let selected = branches
                            .iter()
                            .position(|b| b == &default_branch)
                            .unwrap_or(0);
                        app.enter_sidebar_mode(AppMode::BranchPicker {
                            hive_dir_name,
                            comb_name: value,
                            branches,
                            default_branch,
                            filter: String::new(),
                            filter_cursor: 0,
                            selected,
                        });
                    }
                    Err(_) => {
                        // Fallback: just use text input for branch
                        let default_branch = state
                            .info
                            .default_branch
                            .unwrap_or_else(|| "main".to_string());
                        app.status_message =
                            Some("Could not fetch branches, type manually".to_string());
                        app.enter_sidebar_mode(AppMode::Input {
                            prompt: format!("Branch [{}]", default_branch),
                            value: String::new(),
                            cursor: 0,
                            action: InputAction::NewCombName { hive_dir_name },
                        });
                    }
                }
            }
            InputAction::AddHiveUrl => {
                app.status_message = Some("Adding hive...".to_string());
                terminal.draw(|frame| {
                    ui::render(frame, app);
                })?;

                match add_hive(&app.beehive_dir, &value) {
                    Ok(name) => app.status_message = Some(format!("Added '{}'", name)),
                    Err(e) => app.status_message = Some(format!("Failed: {}", e)),
                }
                app.refresh();
            }
            InputAction::RenameCombName {
                hive_dir_name,
                comb_id,
                current_name,
            } => {
                match rename_comb(&app.beehive_dir, &hive_dir_name, &comb_id, &value) {
                    Ok(comb) => {
                        app.status_message =
                            Some(format!("Renamed '{}' to '{}'", current_name, comb.name));
                        app.refresh();
                    }
                    Err(e) => {
                        app.status_message = Some(e);
                    }
                }
            }
            InputAction::CopyCombName {
                hive_dir_name,
                source_comb_path,
                ..
            } => {
                start_async_copy(app, terminal, hive_dir_name, value, source_comb_path)?;
            }
        }
    }
    Ok(())
}

fn handle_confirm(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::mem::replace(&mut app.mode, AppMode::Normal);
    if let AppMode::Confirm { action, .. } = mode {
        match action {
            ConfirmAction::DeleteComb {
                hive_dir_name,
                comb_id,
                comb_name,
            } => {
                start_async_delete(
                    app,
                    vec![DeleteTarget::Comb {
                        hive_dir_name,
                        comb_id,
                        comb_name,
                    }],
                )?;
            }
            ConfirmAction::DeleteHive {
                dir_name,
                repo_name,
            } => {
                start_async_delete(
                    app,
                    vec![DeleteTarget::Hive {
                        dir_name,
                        repo_name,
                    }],
                )?;
            }
            ConfirmAction::ResetConfig => match reset_config() {
                Ok(()) => {
                    app.status_message = Some("Config reset. Restart to reconfigure.".to_string());
                }
                Err(e) => {
                    app.status_message = Some(format!("Reset failed: {}", e));
                }
            },
            ConfirmAction::Quit => {
                app.should_quit = true;
            }
        }
    }
    Ok(())
}

// --- Sync clone operation (runs in background thread) ---

fn create_comb_sync(
    beehive_dir: &str,
    hive_dir_name: &str,
    comb_name: &str,
    comb_id: &str,
    branch: &str,
) -> Result<Comb, String> {
    let hive_dir = Path::new(beehive_dir).join(hive_dir_name);
    let comb_dir = hive_dir.join(comb_name);

    // Helper: on any failure, remove the placeholder from state and clean up the directory.
    let cleanup = |err_msg: String| -> String {
        if let Ok(mut st) = load_hive_state(beehive_dir, hive_dir_name) {
            st.combs.retain(|c| c.id != comb_id);
            let _ = save_hive_state(beehive_dir, hive_dir_name, &st);
        }
        let _ = std::fs::remove_dir_all(&comb_dir);
        err_msg
    };

    // Load state to get repo_url (the placeholder was written by the caller)
    let state = load_hive_state(beehive_dir, hive_dir_name)
        .map_err(|e| cleanup(format!("Failed to read state: {}", e)))?;

    let clone_output = cmd_with_path("git")
        .args(["clone", &state.info.repo_url, &comb_dir.to_string_lossy()])
        .output()
        .map_err(|e| cleanup(format!("Clone failed: {}", e)))?;

    if !clone_output.status.success() {
        return Err(cleanup(format!(
            "Clone failed: {}",
            String::from_utf8_lossy(&clone_output.stderr)
                .lines()
                .next()
                .unwrap_or("unknown")
        )));
    }

    let checkout = cmd_with_path("git")
        .args(["checkout", branch])
        .current_dir(&comb_dir)
        .output()
        .map_err(|e| cleanup(format!("Checkout failed: {}", e)))?;

    if !checkout.status.success() {
        let checkout_new = cmd_with_path("git")
            .args(["checkout", "-b", branch])
            .current_dir(&comb_dir)
            .output()
            .map_err(|e| cleanup(format!("Checkout -b failed: {}", e)))?;
        if !checkout_new.status.success() {
            return Err(cleanup(format!("Branch '{}' failed", branch)));
        }
    }

    // Success: flip the placeholder to cloning: false
    let mut state = load_hive_state(beehive_dir, hive_dir_name)
        .map_err(|e| cleanup(format!("Failed to update state: {}", e)))?;
    let comb = if let Some(c) = state.combs.iter_mut().find(|c| c.id == comb_id) {
        c.cloning = false;
        c.clone()
    } else {
        // Placeholder was removed externally (e.g. user deleted it) — nothing to do
        return Err("Comb was removed during cloning".to_string());
    };
    save_hive_state(beehive_dir, hive_dir_name, &state)
        .map_err(|e| format!("Clone succeeded but failed to save state: {}", e))?;
    Ok(comb)
}

// --- Sync copy operation (runs in background thread) ---

fn copy_comb_sync(
    beehive_dir: &str,
    hive_dir_name: &str,
    comb_name: &str,
    comb_id: &str,
    source_path: &str,
) -> Result<Comb, String> {
    let hive_dir = Path::new(beehive_dir).join(hive_dir_name);
    let comb_dir = hive_dir.join(comb_name);

    // Helper: on any failure, remove the placeholder from state and clean up the directory.
    let cleanup = |err_msg: String| -> String {
        if let Ok(mut st) = load_hive_state(beehive_dir, hive_dir_name) {
            st.combs.retain(|c| c.id != comb_id);
            let _ = save_hive_state(beehive_dir, hive_dir_name, &st);
        }
        let _ = std::fs::remove_dir_all(&comb_dir);
        err_msg
    };

    if comb_dir.exists() {
        return Err(cleanup(format!(
            "Directory '{}' already exists on disk",
            comb_name
        )));
    }

    // Recursive copy
    let status = cmd_with_path("cp")
        .args(["-r", source_path, &comb_dir.to_string_lossy()])
        .status()
        .map_err(|e| cleanup(format!("Copy failed: {}", e)))?;

    if !status.success() {
        return Err(cleanup("Copy failed".to_string()));
    }

    // Read the branch from the copied directory
    let branch =
        config::get_git_branch(&comb_dir.to_string_lossy()).unwrap_or_else(|| "main".to_string());

    // Success: flip the placeholder to cloning: false and fill in the branch
    let mut state = load_hive_state(beehive_dir, hive_dir_name)
        .map_err(|e| cleanup(format!("Failed to update state: {}", e)))?;
    let comb = if let Some(c) = state.combs.iter_mut().find(|c| c.id == comb_id) {
        c.cloning = false;
        c.branch = branch;
        c.clone()
    } else {
        return Err("Comb was removed during copying".to_string());
    };
    save_hive_state(beehive_dir, hive_dir_name, &state)
        .map_err(|e| format!("Copy succeeded but failed to save state: {}", e))?;
    Ok(comb)
}

fn add_hive(beehive_dir: &str, url: &str) -> Result<String, String> {
    let (owner, repo_name) = parse_repo_url(url)?;
    let repo_spec = format!("{}/{}", owner, repo_name);

    let json_output = run_cmd(
        "gh",
        &[
            "repo",
            "view",
            &repo_spec,
            "--json",
            "name,owner,description,defaultBranchRef,sshUrl,url",
        ],
    )?;

    let parsed: serde_json::Value =
        serde_json::from_str(&json_output).map_err(|e| format!("Parse: {}", e))?;
    let description = parsed["description"].as_str().map(|s| s.to_string());
    let default_branch = parsed["defaultBranchRef"]["name"]
        .as_str()
        .map(|s| s.to_string());

    let clone_url = if url.starts_with("git@") {
        parsed["sshUrl"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("git@github.com:{}/{}.git", owner, repo_name))
    } else {
        parsed["url"]
            .as_str()
            .map(|s| format!("{}.git", s))
            .unwrap_or_else(|| format!("https://github.com/{}/{}.git", owner, repo_name))
    };

    let dir_name = format!("repo_{}", repo_name);
    let hive_dir = Path::new(beehive_dir).join(&dir_name);

    if hive_dir.join(".hive").join("state.json").exists() {
        return Err(format!("'{}' already exists", repo_name));
    }

    std::fs::create_dir_all(&hive_dir).map_err(|e| format!("Mkdir: {}", e))?;
    let dot_hive = hive_dir.join(".hive");
    std::fs::create_dir_all(&dot_hive).map_err(|e| format!("Mkdir: {}", e))?;

    let info = HiveInfo {
        dir_name,
        repo_url: clone_url,
        repo_name: repo_name.clone(),
        owner,
        description,
        default_branch,
        custom_buttons: vec![],
    };
    let state = HiveState {
        info,
        combs: vec![],
    };
    let json = serde_json::to_string_pretty(&state).map_err(|e| format!("Serialize: {}", e))?;
    std::fs::write(dot_hive.join("state.json"), &json).map_err(|e| format!("Write: {}", e))?;
    Ok(repo_name)
}
