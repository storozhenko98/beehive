mod app;
mod config;
mod terminal;
mod ui;

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppMode, CloneResult, ConfirmAction, Focus, InputAction};
use config::*;
use terminal::key_to_bytes;

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
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app =
        App::new(beehive_dir).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    if let Some(warn) = warnings.first() {
        app.status_message = Some(warn.clone());
    }

    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
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
    })
    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    Ok(dir)
}

fn run_app(terminal: &mut Term, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let mut tick_count: u32 = 0;
    loop {
        // Check for completed async clone
        check_pending_clone(app);

        // Refresh branch labels every ~5 seconds (312 * 16ms)
        tick_count = tick_count.wrapping_add(1);
        if tick_count % 312 == 0 {
            app.refresh();
        }

        let mut term_area = ratatui::layout::Rect::default();
        terminal.draw(|frame| {
            term_area = ui::render(frame, app);
        })?;

        if let Some(t) = app.active_terminal() {
            let new_size = (term_area.width, term_area.height);
            if new_size != app.last_term_size && new_size.0 > 0 && new_size.1 > 0 {
                t.resize(new_size.1, new_size.0);
                app.last_term_size = new_size;
            }
        }

        if event::poll(Duration::from_millis(16))? {
            match event::read()? {
                Event::Key(key) => {
                    handle_key(app, key, terminal)?;
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn check_pending_clone(app: &mut App) {
    if let Some(ref slot) = app.pending_clone {
        let mut guard = slot.lock().unwrap();
        if let Some(result) = guard.take() {
            drop(guard);
            app.pending_clone = None;
            match result.comb {
                Ok(comb) => {
                    app.status_message = Some(format!("Created '{}'", result.comb_name));
                    let id = comb.id.clone();
                    let path = comb.path.clone();
                    app.active_comb_id = Some(id.clone());
                    app.refresh();
                    app.open_terminal(&id, &path);
                }
                Err(e) => {
                    app.status_message = Some(format!("Clone failed: {}", e));
                    app.refresh();
                }
            }
        }
    }
}

fn is_focus_toggle(key: &crossterm::event::KeyEvent) -> bool {
    key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn handle_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    terminal: &mut Term,
) -> Result<(), Box<dyn std::error::Error>> {
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
                    app.mode = AppMode::Confirm {
                        message: "Reset config? (repos stay on disk)".to_string(),
                        action: ConfirmAction::ResetConfig,
                    };
                }
                _ => {}
            }
            return Ok(());
        }
        AppMode::BranchPicker { .. } => {
            handle_branch_picker(app, key, terminal)?;
            return Ok(());
        }
        _ => {}
    }

    // Global: Ctrl+C
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if app.focus == Focus::Terminal {
            if let Some(t) = app.active_terminal() {
                t.write_input(&[0x03]);
            }
            return Ok(());
        }
        app.should_quit = true;
        return Ok(());
    }

    // Focus toggle
    if is_focus_toggle(&key) {
        app.focus = match app.focus {
            Focus::Sidebar => {
                if app.active_terminal().is_some() {
                    Focus::Terminal
                } else {
                    Focus::Sidebar
                }
            }
            Focus::Terminal => Focus::Sidebar,
        };
        return Ok(());
    }

    match app.focus {
        Focus::Terminal => {
            if let Some(t) = app.active_terminal() {
                let app_cursor = t.application_cursor();
                let bytes = key_to_bytes(&key, app_cursor);
                if !bytes.is_empty() {
                    t.write_input(&bytes);
                }
            }
        }
        Focus::Sidebar => {
            match &app.mode {
                AppMode::Normal => {
                    app.status_message = None;
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
                        KeyCode::Char('c') => app.start_copy_comb(),
                        KeyCode::Char('a') => app.start_add_hive(),
                        KeyCode::Char('d') => app.start_delete(),
                        KeyCode::Char('r') => {
                            app.refresh();
                            app.status_message = Some("Refreshed".to_string());
                        }
                        KeyCode::Char('s') => app.open_settings(),
                        KeyCode::Char('?') => app.open_help(),
                        _ => {}
                    }
                }
                AppMode::Input { .. } => match key.code {
                    KeyCode::Esc => {
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Enter => {
                        handle_input_submit(app, terminal)?;
                    }
                    KeyCode::Char(c) => {
                        if let AppMode::Input { value, .. } = &mut app.mode {
                            value.push(c);
                        }
                    }
                    KeyCode::Backspace => {
                        if let AppMode::Input { value, .. } = &mut app.mode {
                            value.pop();
                        }
                    }
                    _ => {}
                },
                AppMode::Confirm { .. } => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        handle_confirm(app)?;
                    }
                    _ => {
                        app.mode = AppMode::Normal;
                    }
                },
                AppMode::Help | AppMode::Settings { .. } | AppMode::BranchPicker { .. } => {}
            }
        }
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
            if let AppMode::BranchPicker { selected, filter, branches, .. } = &mut app.mode {
                let filtered_count = branches.iter().filter(|b| b.contains(filter.as_str())).count();
                if *selected > 0 {
                    *selected -= 1;
                } else if filtered_count > 0 {
                    *selected = filtered_count - 1;
                }
            }
        }
        KeyCode::Down => {
            if let AppMode::BranchPicker { selected, filter, branches, .. } = &mut app.mode {
                let filtered_count = branches.iter().filter(|b| b.contains(filter.as_str())).count();
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
            } = mode
            {
                let filtered: Vec<&String> = branches.iter().filter(|b| b.contains(&filter)).collect();
                let branch = if filtered.is_empty() {
                    if filter.is_empty() { default_branch } else { filter }
                } else {
                    filtered.get(selected).map(|s| s.to_string()).unwrap_or(default_branch)
                };

                start_async_clone(app, terminal, hive_dir_name, comb_name, branch)?;
            }
        }
        KeyCode::Char(c) => {
            if let AppMode::BranchPicker { filter, selected, .. } = &mut app.mode {
                filter.push(c);
                *selected = 0;
            }
        }
        KeyCode::Backspace => {
            if let AppMode::BranchPicker { filter, selected, .. } = &mut app.mode {
                filter.pop();
                *selected = 0;
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
    let state = load_hive_state(&app.beehive_dir, &hive_dir_name)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    app.status_message = Some(format!("Cloning '{}'...", comb_name));
    terminal.draw(|frame| {
        ui::render(frame, app);
    })?;

    let slot: Arc<Mutex<Option<CloneResult>>> = Arc::new(Mutex::new(None));
    let slot_clone = Arc::clone(&slot);
    let beehive_dir = app.beehive_dir.clone();
    let hive_dir_clone = hive_dir_name.clone();
    let comb_name_clone = comb_name.clone();
    let branch_clone = branch.clone();

    std::thread::spawn(move || {
        let result = create_comb_sync(&beehive_dir, &hive_dir_clone, &comb_name_clone, &branch_clone, &state);
        let mut guard = slot_clone.lock().unwrap();
        *guard = Some(CloneResult {
            comb: result,
            comb_name: comb_name_clone,
            hive_dir_name: hive_dir_clone,
        });
    });

    app.pending_clone = Some(slot);
    Ok(())
}

fn handle_input_submit(
    app: &mut App,
    terminal: &mut Term,
) -> Result<(), Box<dyn std::error::Error>> {
    let mode = std::mem::replace(&mut app.mode, AppMode::Normal);

    if let AppMode::Input {
        value, action, ..
    } = mode
    {
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
                        app.mode = AppMode::BranchPicker {
                            hive_dir_name,
                            comb_name: value,
                            branches,
                            default_branch,
                            filter: String::new(),
                            selected: 0,
                        };
                    }
                    Err(_) => {
                        // Fallback: just use text input for branch
                        let default_branch = state
                            .info
                            .default_branch
                            .unwrap_or_else(|| "main".to_string());
                        app.status_message = Some("Could not fetch branches, type manually".to_string());
                        app.mode = AppMode::Input {
                            prompt: format!("Branch [{}]", default_branch),
                            value: String::new(),
                            action: InputAction::NewCombName { hive_dir_name },
                        };
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
            InputAction::CopyCombName {
                hive_dir_name,
                source_comb_path,
                ..
            } => {
                let state = load_hive_state(&app.beehive_dir, &hive_dir_name)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
                if let Err(e) = validate_comb_name(&value, &state.combs) {
                    app.status_message = Some(e);
                    return Ok(());
                }

                app.status_message = Some(format!("Copying to '{}'...", value));
                terminal.draw(|frame| {
                    ui::render(frame, app);
                })?;

                match copy_comb(&app.beehive_dir, &hive_dir_name, &source_comb_path, &value) {
                    Ok(comb) => {
                        app.status_message = Some(format!("Copied to '{}'", value));
                        let id = comb.id.clone();
                        let path = comb.path.clone();
                        app.active_comb_id = Some(id.clone());
                        app.refresh();
                        app.open_terminal(&id, &path);
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Copy failed: {}", e));
                    }
                }
                app.refresh();
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
                let mut state = load_hive_state(&app.beehive_dir, &hive_dir_name)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
                if let Some(pos) = state.combs.iter().position(|c| c.id == comb_id) {
                    let comb = state.combs.remove(pos);
                    if Path::new(&comb.path).exists() {
                        std::fs::remove_dir_all(&comb.path)?;
                    }
                    save_hive_state(&app.beehive_dir, &hive_dir_name, &state)
                        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
                    app.status_message = Some(format!("Deleted '{}'", comb_name));
                    app.remove_terminal(&comb_id);
                }
                app.refresh();
            }
            ConfirmAction::DeleteHive {
                dir_name,
                repo_name,
            } => {
                let hive_dir = Path::new(&app.beehive_dir).join(&dir_name);
                if hive_dir.exists() {
                    std::fs::remove_dir_all(&hive_dir)?;
                }
                app.status_message = Some(format!("Deleted '{}'", repo_name));
                app.remove_hive_terminals(&dir_name);
                app.refresh();
            }
            ConfirmAction::ResetConfig => {
                match reset_config() {
                    Ok(()) => {
                        app.status_message = Some("Config reset. Restart to reconfigure.".to_string());
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Reset failed: {}", e));
                    }
                }
            }
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
    branch: &str,
    state: &HiveState,
) -> Result<Comb, String> {
    let hive_dir = Path::new(beehive_dir).join(hive_dir_name);
    let comb_dir = hive_dir.join(comb_name);
    let comb_id = uuid::Uuid::new_v4().to_string();

    let clone_output = cmd_with_path("git")
        .args(["clone", &state.info.repo_url, &comb_dir.to_string_lossy()])
        .output()
        .map_err(|e| format!("Clone failed: {}", e))?;

    if !clone_output.status.success() {
        let _ = std::fs::remove_dir_all(&comb_dir);
        return Err(format!(
            "Clone failed: {}",
            String::from_utf8_lossy(&clone_output.stderr)
                .lines()
                .next()
                .unwrap_or("unknown")
        ));
    }

    let checkout = cmd_with_path("git")
        .args(["checkout", branch])
        .current_dir(&comb_dir)
        .output()
        .map_err(|e| format!("Checkout failed: {}", e))?;

    if !checkout.status.success() {
        let checkout_new = cmd_with_path("git")
            .args(["checkout", "-b", branch])
            .current_dir(&comb_dir)
            .output()
            .map_err(|e| format!("Checkout -b failed: {}", e))?;
        if !checkout_new.status.success() {
            let _ = std::fs::remove_dir_all(&comb_dir);
            return Err(format!("Branch '{}' failed", branch));
        }
    }

    let comb = Comb {
        id: comb_id,
        name: comb_name.to_string(),
        branch: branch.to_string(),
        path: comb_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        panes: vec![],
        cloning: false,
    };

    let mut state = load_hive_state(beehive_dir, hive_dir_name)?;
    state.combs.push(comb.clone());
    save_hive_state(beehive_dir, hive_dir_name, &state)?;
    Ok(comb)
}

fn add_hive(beehive_dir: &str, url: &str) -> Result<String, String> {
    let (owner, repo_name) = parse_repo_url(url)?;
    let repo_spec = format!("{}/{}", owner, repo_name);

    let json_output = run_cmd(
        "gh",
        &[
            "repo", "view", &repo_spec, "--json",
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
