use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

pub struct PtySession {
    master: Arc<std::sync::Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub struct PtyManager {
    pub sessions: HashMap<String, PtySession>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

pub type PtyState = Arc<Mutex<PtyManager>>;

#[tauri::command]
pub async fn create_pty(
    id: String,
    cwd: String,
    cmd: Option<String>,
    args: Option<Vec<String>>,
    rows: u16,
    cols: u16,
    app: AppHandle,
    state: State<'_, PtyState>,
) -> Result<(), String> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let mut command = match &cmd {
        Some(c) => {
            let mut cb = CommandBuilder::new(c);
            if let Some(ref a) = args {
                for arg in a {
                    cb.arg(arg);
                }
            }
            cb
        }
        None => {
            // Default to user's shell
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
            CommandBuilder::new(shell)
        }
    };

    command.cwd(&cwd);
    command.env("TERM", "xterm-256color");

    // Ensure Homebrew paths are visible inside bundled .app (hardened runtime strips PATH)
    let extra_paths = ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin", "/usr/local/sbin"];
    let system_path = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<&str> = extra_paths.to_vec();
    for p in system_path.split(':') {
        if !parts.contains(&p) {
            parts.push(p);
        }
    }
    command.env("PATH", parts.join(":"));

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| format!("Failed to spawn: {}", e))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take writer: {}", e))?;

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone reader: {}", e))?;

    let master = Arc::new(std::sync::Mutex::new(pair.master));
    let writer = Arc::new(std::sync::Mutex::new(writer));

    {
        let mut manager = state.lock().await;
        manager.sessions.insert(
            id.clone(),
            PtySession {
                master: master.clone(),
                writer: writer.clone(),
                child,
            },
        );
    }

    // Spawn background reader thread
    let output_event = format!("pty-output-{}", id);
    let exit_event = format!("pty-exit-{}", id);

    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 16384];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    let _ = app.emit(&exit_event, ());
                    break;
                }
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let _ = app.emit(&output_event, data);
                }
                Err(_) => {
                    let _ = app.emit(&exit_event, ());
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn write_to_pty(
    id: String,
    data: String,
    state: State<'_, PtyState>,
) -> Result<(), String> {
    // Clone the Arc — hold global lock only for HashMap lookup
    let writer = {
        let manager = state.lock().await;
        let session = manager
            .sessions
            .get(&id)
            .ok_or_else(|| format!("PTY session '{}' not found", id))?;
        session.writer.clone() // Arc::clone is cheap
    }; // global manager lock dropped here — other sessions can proceed concurrently

    let mut writer = writer
        .lock()
        .map_err(|e| format!("Failed to lock writer: {}", e))?;

    writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Failed to write: {}", e))?;

    // No flush needed — PTY master fds are unbuffered at the OS level.
    // Data is immediately available to the slave process after write().

    Ok(())
}

#[tauri::command]
pub async fn write_to_pty_binary(
    id: String,
    data: Vec<u8>,
    state: State<'_, PtyState>,
) -> Result<(), String> {
    let writer = {
        let manager = state.lock().await;
        let session = manager
            .sessions
            .get(&id)
            .ok_or_else(|| format!("PTY session '{}' not found", id))?;
        session.writer.clone()
    };

    let mut writer = writer
        .lock()
        .map_err(|e| format!("Failed to lock writer: {}", e))?;

    writer
        .write_all(&data)
        .map_err(|e| format!("Failed to write: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn resize_pty(
    id: String,
    rows: u16,
    cols: u16,
    state: State<'_, PtyState>,
) -> Result<(), String> {
    // Clone the Arc — hold global lock only for HashMap lookup
    let master = {
        let manager = state.lock().await;
        let session = manager
            .sessions
            .get(&id)
            .ok_or_else(|| format!("PTY session '{}' not found", id))?;
        session.master.clone()
    }; // global manager lock dropped here

    let master = master
        .lock()
        .map_err(|e| format!("Failed to lock master: {}", e))?;

    master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to resize: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn close_pty(id: String, state: State<'_, PtyState>) -> Result<(), String> {
    let mut manager = state.lock().await;
    if let Some(mut session) = manager.sessions.remove(&id) {
        session.child.kill().ok();
    }
    Ok(())
}
