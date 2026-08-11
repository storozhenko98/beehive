use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

const OPENCODE_SENTINEL: &str = "__beehive_opencode__";
const PI_SENTINEL: &str = "__beehive_pi__";

pub struct PtySession {
    master: Arc<std::sync::Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    attention_listener_alive: Option<Arc<AtomicBool>>,
}

pub struct PtyManager {
    pub sessions: HashMap<String, PtySession>,
    pub startup_initialized_combs: HashSet<String>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            startup_initialized_combs: HashSet::new(),
        }
    }
}

pub type PtyState = Arc<Mutex<PtyManager>>;

fn resolve_login_shell() -> String {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let trimmed = shell.trim();
    if trimmed.is_empty() {
        "/bin/zsh".to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_startup_command(startup_command: Option<&str>) -> Option<&str> {
    startup_command.and_then(|cmd| {
        let trimmed = cmd.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn startup_wrapper_script() -> &'static str {
    "eval \"$BEEHIVE_STARTUP_COMMAND\"\nexec \"$SHELL\" -l"
}

fn opencode_wrapper_script(port: u16) -> String {
    format!(
        "opencode --hostname 127.0.0.1 --port {}\nexec \"$SHELL\" -l",
        port
    )
}

fn pi_status_extension() -> &'static str {
    include_str!("../../shared/pi-status-extension.ts")
}

fn pi_status_process_exit_reporter() -> &'static str {
    include_str!("../../shared/pi-status-process-exit.js")
}

fn pi_wrapper_script() -> &'static str {
    r#"extension_dir="$(mktemp -d "${TMPDIR:-/tmp}/beehive-pi-status.XXXXXX")" || exit 1
extension_path="$extension_dir/index.ts"
printf '%s' "$BEEHIVE_PI_STATUS_EXTENSION" > "$extension_path" || exit 1
pi -e "$extension_path"
node -e "$BEEHIVE_PI_STATUS_REPORTER"
rm -f "$extension_path"
rmdir "$extension_dir"
exec "$SHELL" -l"#
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeAttentionEvent {
    pub attention_key: String,
    pub status: String,
    pub event_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiAttentionMessage {
    status: String,
    event_type: String,
}

fn reserve_local_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to reserve OpenCode port: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to read OpenCode port: {}", e))?
        .port();
    drop(listener);
    Ok(port)
}

fn json_event_type(data: &str) -> Option<String> {
    let value: Value = serde_json::from_str(data).ok()?;
    value
        .get("type")
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn classify_opencode_event(event_type: &str, data: &str) -> Option<&'static str> {
    match event_type {
        "permission.asked" | "question.asked" => Some("input"),
        "permission.replied" | "question.replied" | "question.rejected" => Some("clear"),
        "session.error" => Some("error"),
        "session.idle" => Some("idle"),
        "session.status" => {
            let value: Value = serde_json::from_str(data).ok()?;
            let status_type = value
                .pointer("/properties/status/type")
                .or_else(|| value.pointer("/status/type"))
                .and_then(Value::as_str);
            match status_type {
                Some("idle") => Some("idle"),
                Some("busy") | Some("retry") => Some("clear"),
                _ => None,
            }
        }
        _ => None,
    }
}

fn emit_opencode_attention(app: &AppHandle, attention_key: &str, status: &str, event_type: &str) {
    let _ = app.emit(
        "opencode-attention",
        OpenCodeAttentionEvent {
            attention_key: attention_key.to_string(),
            status: status.to_string(),
            event_type: event_type.to_string(),
        },
    );
}

fn start_opencode_event_listener(app: AppHandle, attention_key: String, port: u16) {
    std::thread::spawn(move || {
        let mut stream = None;
        for _ in 0..300 {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }

        let Some(mut stream) = stream else {
            emit_opencode_attention(
                &app,
                &attention_key,
                "error",
                "beehive.opencode.connect_failed",
            );
            return;
        };

        let request = format!(
            "GET /event HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n",
            port
        );
        if stream.write_all(request.as_bytes()).is_err() {
            emit_opencode_attention(
                &app,
                &attention_key,
                "error",
                "beehive.opencode.subscribe_failed",
            );
            return;
        }

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) if line == "\r\n" || line == "\n" => break,
                Ok(_) => continue,
                Err(_) => return,
            }
        }

        let mut event_name: Option<String> = None;
        let mut data = String::new();
        loop {
            line.clear();
            let read = match reader.read_line(&mut line) {
                Ok(read) => read,
                Err(_) => return,
            };
            if read == 0 {
                return;
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                if !data.is_empty() {
                    let event_type = event_name.clone().or_else(|| json_event_type(&data));
                    if let Some(event_type) = event_type {
                        if let Some(status) = classify_opencode_event(&event_type, &data) {
                            emit_opencode_attention(&app, &attention_key, status, &event_type);
                        }
                    }
                }
                event_name = None;
                data.clear();
            } else if let Some(value) = trimmed.strip_prefix("event:") {
                event_name = Some(value.trim().to_string());
            } else if let Some(value) = trimmed.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.trim_start());
            }
        }
    });
}

fn start_pi_event_listener(
    app: AppHandle,
    attention_key: String,
    listener: TcpListener,
    alive: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        if listener.set_nonblocking(true).is_err() {
            emit_opencode_attention(&app, &attention_key, "error", "beehive.pi.listen_failed");
            return;
        }

        while alive.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut line = String::new();
                    if BufReader::new(stream).read_line(&mut line).is_ok() {
                        if let Ok(message) = serde_json::from_str::<PiAttentionMessage>(&line) {
                            if matches!(
                                message.status.as_str(),
                                "clear" | "idle" | "error" | "stopped"
                            ) {
                                emit_opencode_attention(
                                    &app,
                                    &attention_key,
                                    &message.status,
                                    &format!("pi.{}", message.event_type),
                                );
                            }
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => {
                    emit_opencode_attention(
                        &app,
                        &attention_key,
                        "error",
                        "beehive.pi.listen_failed",
                    );
                    return;
                }
            }
        }
    });
}

#[tauri::command]
pub async fn create_pty(
    id: String,
    cwd: String,
    cmd: Option<String>,
    args: Option<Vec<String>>,
    rows: u16,
    cols: u16,
    attention_key: Option<String>,
    app: AppHandle,
    state: State<'_, PtyState>,
) -> Result<(), String> {
    let pty_system = native_pty_system();
    let opencode_port = if matches!(cmd.as_deref(), Some(OPENCODE_SENTINEL)) {
        Some(reserve_local_port()?)
    } else {
        None
    };
    let pi_listener = if matches!(cmd.as_deref(), Some(PI_SENTINEL)) {
        Some(
            TcpListener::bind("127.0.0.1:0")
                .map_err(|e| format!("Failed to reserve Pi status port: {}", e))?,
        )
    } else {
        None
    };
    let pi_port = pi_listener
        .as_ref()
        .map(|listener| {
            listener
                .local_addr()
                .map(|addr| addr.port())
                .map_err(|e| format!("Failed to read Pi status port: {}", e))
        })
        .transpose()?;

    let startup_command = if cmd.is_none() && opencode_port.is_none() && pi_port.is_none() {
        let config = crate::hive::load_app_config().await?;
        normalize_startup_command(config.comb_startup_command.as_deref()).map(str::to_string)
    } else {
        None
    };

    let startup_command = if let Some(startup_command) = startup_command {
        let manager = state.lock().await;
        if manager.startup_initialized_combs.contains(&cwd) {
            None
        } else {
            Some(startup_command)
        }
    } else {
        None
    };

    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let mut command = match (&cmd, opencode_port, pi_port) {
        (_, Some(port), _) => {
            let shell = resolve_login_shell();
            let mut cb = CommandBuilder::new(&shell);
            cb.arg("-lc");
            cb.arg(opencode_wrapper_script(port));
            cb.env("SHELL", &shell);
            cb
        }
        (_, None, Some(port)) => {
            let shell = resolve_login_shell();
            let mut cb = CommandBuilder::new(&shell);
            cb.arg("-lc");
            cb.arg(pi_wrapper_script());
            cb.env("SHELL", &shell);
            cb.env("BEEHIVE_PI_STATUS_EXTENSION", pi_status_extension());
            cb.env(
                "BEEHIVE_PI_STATUS_REPORTER",
                pi_status_process_exit_reporter(),
            );
            cb.env("BEEHIVE_PI_STATUS_PORT", port.to_string());
            cb
        }
        (Some(c), None, None) => {
            let mut cb = CommandBuilder::new(c);
            if let Some(ref a) = args {
                for arg in a {
                    cb.arg(arg);
                }
            }
            cb
        }
        (None, None, None) => {
            let shell = resolve_login_shell();
            let mut cb = CommandBuilder::new(&shell);
            if let Some(startup_command) = startup_command.as_deref() {
                cb.arg("-lc");
                cb.arg(startup_wrapper_script());
                cb.env("SHELL", &shell);
                cb.env("BEEHIVE_STARTUP_COMMAND", startup_command);
            }
            cb
        }
    };

    command.cwd(&cwd);
    command.env("TERM", "xterm-256color");

    // Ensure Homebrew paths are visible inside bundled .app (hardened runtime strips PATH)
    let extra_paths = [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
    ];
    let system_path = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<&str> = extra_paths.to_vec();
    for p in system_path.split(':') {
        if !parts.contains(&p) {
            parts.push(p);
        }
    }
    command.env("PATH", parts.join(":"));
    command.env("BEEHIVE_COMB", &cwd);

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| format!("Failed to spawn: {}", e))?;

    if let (Some(port), Some(key)) = (opencode_port, attention_key.clone()) {
        start_opencode_event_listener(app.clone(), key, port);
    }

    let attention_listener_alive = if let (Some(listener), Some(key)) = (pi_listener, attention_key)
    {
        let alive = Arc::new(AtomicBool::new(true));
        start_pi_event_listener(app.clone(), key, listener, Arc::clone(&alive));
        Some(alive)
    } else {
        None
    };

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
        if startup_command.is_some() {
            manager.startup_initialized_combs.insert(cwd.clone());
        }
        manager.sessions.insert(
            id.clone(),
            PtySession {
                master: master.clone(),
                writer: writer.clone(),
                child,
                attention_listener_alive: attention_listener_alive.clone(),
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
                    if let Some(alive) = &attention_listener_alive {
                        alive.store(false, Ordering::Relaxed);
                    }
                    let _ = app.emit(&exit_event, ());
                    break;
                }
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let _ = app.emit(&output_event, data);
                }
                Err(_) => {
                    if let Some(alive) = &attention_listener_alive {
                        alive.store(false, Ordering::Relaxed);
                    }
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
        if let Some(alive) = &session.attention_listener_alive {
            alive.store(false, Ordering::Relaxed);
        }
        session.child.kill().ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_startup_command, pi_status_extension, pi_wrapper_script, startup_wrapper_script,
    };

    #[test]
    fn normalize_startup_command_ignores_blank_values() {
        assert_eq!(normalize_startup_command(None), None);
        assert_eq!(normalize_startup_command(Some("")), None);
        assert_eq!(normalize_startup_command(Some("   ")), None);
    }

    #[test]
    fn startup_wrapper_returns_to_shell_after_command() {
        let script = startup_wrapper_script();
        assert!(script.contains("BEEHIVE_STARTUP_COMMAND"));
        assert!(script.contains("exec \"$SHELL\" -l"));
    }

    #[test]
    fn pi_launcher_uses_a_scoped_status_extension_and_returns_to_the_shell() {
        let script = pi_wrapper_script();
        assert!(script.contains("BEEHIVE_PI_STATUS_EXTENSION"));
        assert!(script.contains("pi -e \"$extension_path\""));
        assert!(script.contains("rm -f \"$extension_path\""));
        assert!(script.contains("rmdir \"$extension_dir\""));
        assert!(script.contains("exec \"$SHELL\" -l"));
    }

    #[test]
    fn pi_status_extension_reports_busy_and_settled_lifecycle_events() {
        let extension = pi_status_extension();
        assert!(extension.contains("pi.on(\"agent_start\""));
        assert!(extension.contains("notify(\"clear\", \"agent_start\")"));
        assert!(extension.contains("pi.on(\"agent_settled\""));
        assert!(extension.contains("notify(\"idle\", \"agent_settled\")"));
    }
}
