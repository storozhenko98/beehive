use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use crate::config::full_path;

/// Detect DECSET 1004 (focus reporting) requests in the PTY output stream.
/// Scans for \x1b[?1004h (enable) and \x1b[?1004l (disable).
fn detect_focus_reporting(data: &[u8], flag: &AtomicBool) {
    let enable = b"\x1b[?1004h";
    let disable = b"\x1b[?1004l";
    if data.len() >= enable.len() {
        if data.windows(enable.len()).any(|w| w == enable) {
            flag.store(true, Ordering::SeqCst);
        }
        if data.windows(disable.len()).any(|w| w == disable) {
            flag.store(false, Ordering::SeqCst);
        }
    }
}

pub struct EmbeddedTerminal {
    /// vt100 parser — owned exclusively by the main thread (no locks needed).
    /// Background reader sends raw bytes via `output_rx` channel instead.
    parser: vt100::Parser,
    /// Channel receiving raw PTY output from the background reader thread.
    output_rx: mpsc::Receiver<Vec<u8>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    #[allow(dead_code)]
    alive: Arc<AtomicBool>,
    /// Whether the inner app requested focus reporting (DECSET 1004).
    focus_reporting: Arc<AtomicBool>,
}

impl EmbeddedTerminal {
    pub fn new(cwd: &str, rows: u16, cols: u16) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l");
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("PATH", full_path());
        cmd.env("BEEHIVE_COMB", cwd);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn: {}", e))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take writer: {}", e))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone reader: {}", e))?;

        let parser = vt100::Parser::new(rows, cols, 1000);
        let alive = Arc::new(AtomicBool::new(true));
        let focus_reporting = Arc::new(AtomicBool::new(false));
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));

        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>();

        // Background reader: PTY output → channel + OSC 52 clipboard + focus detect
        let alive_clone = Arc::clone(&alive);
        let focus_clone = Arc::clone(&focus_reporting);
        std::thread::spawn(move || {
            let mut buf = [0u8; 16384];
            let mut osc52 = Osc52Detector::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &buf[..n];
                        osc52.process(data);
                        detect_focus_reporting(data, &focus_clone);
                        if output_tx.send(data.to_vec()).is_err() {
                            break; // receiver dropped
                        }
                    }
                    Err(_) => break,
                }
            }
            alive_clone.store(false, Ordering::SeqCst);
        });

        Ok(Self {
            parser,
            output_rx,
            writer,
            master,
            _child: child,
            alive,
            focus_reporting,
        })
    }

    /// Drain all pending PTY output from the channel and feed it to the parser.
    /// Call this on the main thread before rendering. Returns true if any output
    /// was processed (i.e. the screen may have changed).
    pub fn process_pending_output(&mut self) -> bool {
        let mut got_data = false;
        while let Ok(data) = self.output_rx.try_recv() {
            self.parser.process(&data);
            got_data = true;
        }
        got_data
    }

    pub fn with_screen<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&vt100::Screen) -> R,
    {
        f(self.parser.screen())
    }

    pub fn write_input(&self, data: &[u8]) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(data);
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == 0 || cols == 0 {
            return;
        }
        if let Ok(m) = self.master.lock() {
            let _ = m.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        self.parser.set_size(rows, cols);
    }

    #[allow(dead_code)]
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Whether the inner application requested focus reporting via DECSET 1004.
    pub fn focus_reporting(&self) -> bool {
        self.focus_reporting.load(Ordering::SeqCst)
    }

    pub fn application_cursor(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    pub fn mouse_protocol_mode(&self) -> vt100::MouseProtocolMode {
        self.parser.screen().mouse_protocol_mode()
    }

    pub fn mouse_protocol_encoding(&self) -> vt100::MouseProtocolEncoding {
        self.parser.screen().mouse_protocol_encoding()
    }

    pub fn bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste()
    }
}

/// Compute the xterm modifier parameter from crossterm KeyModifiers.
/// Encoding: value = 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)
/// Returns 0 if no modifiers are set (meaning: no modifier parameter needed).
fn xterm_modifier(mods: crossterm::event::KeyModifiers) -> u8 {
    use crossterm::event::KeyModifiers;
    let mut m: u8 = 0;
    if mods.contains(KeyModifiers::SHIFT) {
        m += 1;
    }
    if mods.contains(KeyModifiers::ALT) {
        m += 2;
    }
    if mods.contains(KeyModifiers::CONTROL) {
        m += 4;
    }
    if m > 0 {
        m + 1
    } else {
        0
    }
}

/// Check if the modifier combination requires CSI u encoding because
/// legacy terminal encoding cannot represent it (e.g. Ctrl+Shift+letter).
fn needs_csi_u(mods: crossterm::event::KeyModifiers) -> bool {
    use crossterm::event::KeyModifiers;
    let has_ctrl = mods.contains(KeyModifiers::CONTROL);
    let has_shift = mods.contains(KeyModifiers::SHIFT);
    let has_alt = mods.contains(KeyModifiers::ALT);
    // Any two-or-more modifier combo needs CSI u for character keys
    (has_ctrl && has_shift) || (has_ctrl && has_alt) || (has_alt && has_shift)
}

/// Generate CSI u sequence: ESC [ codepoint ; modifier u
fn csi_u(codepoint: u32, modifier: u8) -> Vec<u8> {
    format!("\x1b[{};{}u", codepoint, modifier).into_bytes()
}

/// Generate a modified special key sequence.
/// For keys encoded as ESC [ <letter> (arrow keys, Home, End):
///   ESC [ 1 ; <modifier> <letter>
/// For keys encoded as ESC [ <code> ~ (PageUp, Delete, etc.):
///   ESC [ <code> ; <modifier> ~
/// For keys encoded as ESC O <letter> (F1-F4):
///   ESC [ 1 ; <modifier> <letter>  (promoted to CSI format with modifiers)
fn modified_special_key_csi(suffix: u8, modifier: u8) -> Vec<u8> {
    // CSI 1 ; modifier <suffix>
    format!("\x1b[1;{}{}", modifier, suffix as char).into_bytes()
}

fn modified_special_key_tilde(code: u16, modifier: u8) -> Vec<u8> {
    // CSI code ; modifier ~
    format!("\x1b[{};{}~", code, modifier).into_bytes()
}

/// Translate a crossterm key event into the byte sequence to send to a PTY.
/// Uses CSI u encoding for multi-modifier character combos (Ctrl+Shift, Ctrl+Alt, etc.)
/// that legacy terminal encoding cannot represent. Single-modifier keys use legacy encoding
/// for maximum backward compatibility.
pub fn key_to_bytes(key: &crossterm::event::KeyEvent, app_cursor: bool) -> Vec<u8> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mods = key.modifiers;
    let xmod = xterm_modifier(mods);

    match key.code {
        KeyCode::Char(c) => {
            // Multi-modifier combos (Ctrl+Shift, Ctrl+Alt, etc.) need CSI u
            if needs_csi_u(mods) {
                let codepoint = c.to_ascii_lowercase() as u32;
                return csi_u(codepoint, xmod);
            }

            // Single-modifier legacy encoding
            if mods.contains(KeyModifiers::CONTROL) {
                if c.is_ascii_alphabetic() {
                    vec![(c.to_ascii_lowercase() as u8) & 0x1f]
                } else if c == ' ' {
                    vec![0x00]
                } else if c == '[' {
                    vec![0x1b]
                } else if c == '\\' {
                    vec![0x1c]
                } else if c == ']' {
                    vec![0x1d]
                } else {
                    // For other Ctrl+<char>, use CSI u since legacy has no mapping
                    let codepoint = c as u32;
                    csi_u(codepoint, xmod)
                }
            } else if mods.contains(KeyModifiers::ALT) {
                let mut bytes = vec![0x1b];
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                bytes
            } else {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf);
                buf[..c.len_utf8()].to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => {
            if xmod > 0 {
                // Backspace = codepoint 127
                csi_u(127, xmod)
            } else {
                vec![0x7f]
            }
        }
        KeyCode::Tab => {
            if mods.contains(KeyModifiers::SHIFT) {
                vec![0x1b, b'[', b'Z'] // BackTab
            } else {
                vec![b'\t']
            }
        }
        KeyCode::BackTab => vec![0x1b, b'[', b'Z'],
        KeyCode::Esc => vec![0x1b],

        // Arrow keys — with modifier support
        KeyCode::Up => {
            if xmod > 0 {
                modified_special_key_csi(b'A', xmod)
            } else if app_cursor {
                vec![0x1b, b'O', b'A']
            } else {
                vec![0x1b, b'[', b'A']
            }
        }
        KeyCode::Down => {
            if xmod > 0 {
                modified_special_key_csi(b'B', xmod)
            } else if app_cursor {
                vec![0x1b, b'O', b'B']
            } else {
                vec![0x1b, b'[', b'B']
            }
        }
        KeyCode::Right => {
            if xmod > 0 {
                modified_special_key_csi(b'C', xmod)
            } else if app_cursor {
                vec![0x1b, b'O', b'C']
            } else {
                vec![0x1b, b'[', b'C']
            }
        }
        KeyCode::Left => {
            if xmod > 0 {
                modified_special_key_csi(b'D', xmod)
            } else if app_cursor {
                vec![0x1b, b'O', b'D']
            } else {
                vec![0x1b, b'[', b'D']
            }
        }

        // Navigation keys — with modifier support
        KeyCode::Home => {
            if xmod > 0 {
                modified_special_key_csi(b'H', xmod)
            } else {
                vec![0x1b, b'[', b'H']
            }
        }
        KeyCode::End => {
            if xmod > 0 {
                modified_special_key_csi(b'F', xmod)
            } else {
                vec![0x1b, b'[', b'F']
            }
        }
        KeyCode::PageUp => {
            if xmod > 0 {
                modified_special_key_tilde(5, xmod)
            } else {
                vec![0x1b, b'[', b'5', b'~']
            }
        }
        KeyCode::PageDown => {
            if xmod > 0 {
                modified_special_key_tilde(6, xmod)
            } else {
                vec![0x1b, b'[', b'6', b'~']
            }
        }
        KeyCode::Insert => {
            if xmod > 0 {
                modified_special_key_tilde(2, xmod)
            } else {
                vec![0x1b, b'[', b'2', b'~']
            }
        }
        KeyCode::Delete => {
            if xmod > 0 {
                modified_special_key_tilde(3, xmod)
            } else {
                vec![0x1b, b'[', b'3', b'~']
            }
        }

        // Function keys — with modifier support
        // F1-F4 use SS3 format without modifiers, CSI format with modifiers
        KeyCode::F(1) => {
            if xmod > 0 {
                modified_special_key_csi(b'P', xmod)
            } else {
                vec![0x1b, b'O', b'P']
            }
        }
        KeyCode::F(2) => {
            if xmod > 0 {
                modified_special_key_csi(b'Q', xmod)
            } else {
                vec![0x1b, b'O', b'Q']
            }
        }
        KeyCode::F(3) => {
            if xmod > 0 {
                modified_special_key_csi(b'R', xmod)
            } else {
                vec![0x1b, b'O', b'R']
            }
        }
        KeyCode::F(4) => {
            if xmod > 0 {
                modified_special_key_csi(b'S', xmod)
            } else {
                vec![0x1b, b'O', b'S']
            }
        }
        // F5-F12 use tilde format
        KeyCode::F(5) => {
            if xmod > 0 {
                modified_special_key_tilde(15, xmod)
            } else {
                vec![0x1b, b'[', b'1', b'5', b'~']
            }
        }
        KeyCode::F(6) => {
            if xmod > 0 {
                modified_special_key_tilde(17, xmod)
            } else {
                vec![0x1b, b'[', b'1', b'7', b'~']
            }
        }
        KeyCode::F(7) => {
            if xmod > 0 {
                modified_special_key_tilde(18, xmod)
            } else {
                vec![0x1b, b'[', b'1', b'8', b'~']
            }
        }
        KeyCode::F(8) => {
            if xmod > 0 {
                modified_special_key_tilde(19, xmod)
            } else {
                vec![0x1b, b'[', b'1', b'9', b'~']
            }
        }
        KeyCode::F(9) => {
            if xmod > 0 {
                modified_special_key_tilde(20, xmod)
            } else {
                vec![0x1b, b'[', b'2', b'0', b'~']
            }
        }
        KeyCode::F(10) => {
            if xmod > 0 {
                modified_special_key_tilde(21, xmod)
            } else {
                vec![0x1b, b'[', b'2', b'1', b'~']
            }
        }
        KeyCode::F(11) => {
            if xmod > 0 {
                modified_special_key_tilde(23, xmod)
            } else {
                vec![0x1b, b'[', b'2', b'3', b'~']
            }
        }
        KeyCode::F(12) => {
            if xmod > 0 {
                modified_special_key_tilde(24, xmod)
            } else {
                vec![0x1b, b'[', b'2', b'4', b'~']
            }
        }
        _ => vec![],
    }
}

/// Translate any crossterm Event into bytes to forward to the PTY.
/// Handles key, mouse, paste, and focus events.
/// `term_area` is the Rect of the terminal pane content area (for mouse coordinate adjustment).
pub fn event_to_bytes(
    event: &crossterm::event::Event,
    terminal: &EmbeddedTerminal,
    term_area: ratatui::layout::Rect,
) -> Vec<u8> {
    use crossterm::event::Event;

    match event {
        Event::Key(key) => {
            let app_cursor = terminal.application_cursor();
            key_to_bytes(key, app_cursor)
        }
        Event::Mouse(mouse) => mouse_to_bytes(mouse, terminal, term_area),
        Event::Paste(text) => {
            if terminal.bracketed_paste() {
                let mut bytes = Vec::with_capacity(text.len() + 12);
                bytes.extend_from_slice(b"\x1b[200~");
                bytes.extend_from_slice(text.as_bytes());
                bytes.extend_from_slice(b"\x1b[201~");
                bytes
            } else {
                text.as_bytes().to_vec()
            }
        }
        Event::FocusGained => {
            if terminal.focus_reporting() {
                b"\x1b[I".to_vec()
            } else {
                vec![]
            }
        }
        Event::FocusLost => {
            if terminal.focus_reporting() {
                b"\x1b[O".to_vec()
            } else {
                vec![]
            }
        }
        Event::Resize(_, _) => vec![],
    }
}

/// Encode a crossterm MouseEvent to bytes for the PTY, respecting the inner app's
/// mouse protocol mode and encoding. Coordinates are adjusted relative to term_area.
fn mouse_to_bytes(
    mouse: &crossterm::event::MouseEvent,
    terminal: &EmbeddedTerminal,
    term_area: ratatui::layout::Rect,
) -> Vec<u8> {
    use crossterm::event::{MouseButton, MouseEventKind};
    use vt100::MouseProtocolMode;

    let mode = terminal.mouse_protocol_mode();
    if mode == MouseProtocolMode::None {
        return vec![];
    }

    // Adjust coordinates relative to the terminal pane area
    let col = mouse.column as i32 - term_area.x as i32;
    let row = mouse.row as i32 - term_area.y as i32;

    // Ignore mouse events outside the terminal pane
    if col < 0 || row < 0 || col >= term_area.width as i32 || row >= term_area.height as i32 {
        return vec![];
    }

    let cx = col as u16;
    let cy = row as u16;

    // Determine the button code (cb) per X10/SGR convention
    let (cb, is_release) = match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => (0u8, false),
        MouseEventKind::Down(MouseButton::Middle) => (1, false),
        MouseEventKind::Down(MouseButton::Right) => (2, false),
        MouseEventKind::Up(MouseButton::Left) => (0, true),
        MouseEventKind::Up(MouseButton::Middle) => (1, true),
        MouseEventKind::Up(MouseButton::Right) => (2, true),
        MouseEventKind::Drag(MouseButton::Left) => (32, false), // 0 + 32 (motion flag)
        MouseEventKind::Drag(MouseButton::Middle) => (33, false), // 1 + 32
        MouseEventKind::Drag(MouseButton::Right) => (34, false), // 2 + 32
        MouseEventKind::Moved => (35, false),                   // 3 + 32 (no button + motion)
        MouseEventKind::ScrollUp => (64, false),
        MouseEventKind::ScrollDown => (65, false),
        MouseEventKind::ScrollLeft => (66, false),
        MouseEventKind::ScrollRight => (67, false),
    };

    // Filter events based on the active mouse mode
    let dominated_by_motion = matches!(mouse.kind, MouseEventKind::Drag(_) | MouseEventKind::Moved);
    let is_scroll = matches!(
        mouse.kind,
        MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::ScrollLeft
            | MouseEventKind::ScrollRight
    );

    match mode {
        MouseProtocolMode::None => return vec![],
        MouseProtocolMode::Press => {
            // Only report button press events (and scrolls)
            if is_release || dominated_by_motion {
                return vec![];
            }
        }
        MouseProtocolMode::PressRelease => {
            // Report press and release, but not motion
            if dominated_by_motion {
                return vec![];
            }
        }
        MouseProtocolMode::ButtonMotion => {
            // Report press, release, and drag — but not bare Moved
            if matches!(mouse.kind, MouseEventKind::Moved) {
                return vec![];
            }
        }
        MouseProtocolMode::AnyMotion => {
            // Report everything
        }
    }

    // Add modifier bits
    let mut cb_with_mods = cb;
    if mouse
        .modifiers
        .contains(crossterm::event::KeyModifiers::SHIFT)
    {
        cb_with_mods |= 4;
    }
    if mouse
        .modifiers
        .contains(crossterm::event::KeyModifiers::ALT)
    {
        cb_with_mods |= 8;
    }
    if mouse
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        cb_with_mods |= 16;
    }

    let encoding = terminal.mouse_protocol_encoding();
    match encoding {
        vt100::MouseProtocolEncoding::Sgr => {
            // SGR format: \x1b[<cb;cx;cyM (press/move) or \x1b[<cb;cx;cym (release)
            // SGR uses 1-based coordinates
            let suffix = if is_release { 'm' } else { 'M' };
            format!("\x1b[<{};{};{}{}", cb_with_mods, cx + 1, cy + 1, suffix).into_bytes()
        }
        _ => {
            // Default / UTF-8 encoding: \x1b[M cb cx cy
            // cb is button + 32, cx and cy are coordinate + 32 + 1 (1-based, offset by 32)
            if is_release && !is_scroll {
                // In default encoding, release is button code 3 (meaning "no button")
                let release_cb = 3u8 + 32;
                let enc_x = (cx as u8).saturating_add(32 + 1);
                let enc_y = (cy as u8).saturating_add(32 + 1);
                vec![0x1b, b'[', b'M', release_cb, enc_x, enc_y]
            } else {
                let enc_cb = cb_with_mods.saturating_add(32);
                let enc_x = (cx as u8).saturating_add(32 + 1);
                let enc_y = (cy as u8).saturating_add(32 + 1);
                vec![0x1b, b'[', b'M', enc_cb, enc_x, enc_y]
            }
        }
    }
}

// --- OSC 52 clipboard support ---
// Detects OSC 52 escape sequences in PTY output and sets the system clipboard.
// Format: \x1b]52;<selection>;<base64-data>\x07  (or \x1b\\ as terminator)

const OSC52_PREFIX: &[u8] = b"\x1b]52;";

struct Osc52Detector {
    prefix_matched: usize,
    payload: Vec<u8>,
    collecting: bool,
}

impl Osc52Detector {
    fn new() -> Self {
        Self {
            prefix_matched: 0,
            payload: Vec::new(),
            collecting: false,
        }
    }

    fn process(&mut self, data: &[u8]) {
        for &b in data {
            if self.collecting {
                if b == 0x07 {
                    // BEL terminator — sequence complete
                    self.handle_complete();
                } else if b == b'\\' && self.payload.last() == Some(&0x1b) {
                    // ST terminator (ESC \) — remove the ESC we buffered, sequence complete
                    self.payload.pop();
                    self.handle_complete();
                } else {
                    self.payload.push(b);
                    if self.payload.len() > 100_000 {
                        // Safety limit
                        self.payload.clear();
                        self.collecting = false;
                    }
                }
            } else {
                if b == OSC52_PREFIX[self.prefix_matched] {
                    self.prefix_matched += 1;
                    if self.prefix_matched == OSC52_PREFIX.len() {
                        self.collecting = true;
                        self.prefix_matched = 0;
                    }
                } else {
                    // Reset, but check if this byte starts a new match
                    self.prefix_matched = if b == OSC52_PREFIX[0] { 1 } else { 0 };
                }
            }
        }
    }

    fn handle_complete(&mut self) {
        self.collecting = false;
        // Payload is: <selection>;<base64-data>
        // e.g., "c;SGVsbG8=" where c=clipboard
        if let Some(semi_pos) = self.payload.iter().position(|&b| b == b';') {
            let b64_data = &self.payload[semi_pos + 1..];
            if b64_data == b"?" {
                // Query request, not a set — ignore
                self.payload.clear();
                return;
            }
            if let Some(decoded) = base64_decode(b64_data) {
                set_clipboard(&decoded);
            }
        }
        self.payload.clear();
    }
}

fn base64_decode(input: &[u8]) -> Option<Vec<u8>> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &b in input {
        if b == b'=' || b == b'\n' || b == b'\r' || b == b' ' {
            continue;
        }
        let val = TABLE.iter().position(|&c| c == b)? as u32;
        buf = (buf << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(result)
}

fn set_clipboard(text: &[u8]) {
    use std::process::{Command, Stdio};

    if let Ok(mut child) = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(text);
        }
        let _ = child.wait();
    }
}
