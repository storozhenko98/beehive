use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use crate::config::full_path;
use crate::keyboard::{self, KeyboardProtocol};

/// Tracks DECSET modes we need to mirror outside the vt100 parser.
///
/// We keep a short tail buffer so mode sequences split across PTY reads are still
/// detected, and we parse parameter lists so combined DECSET/DECRST sequences
/// like `CSI ? 1000 ; 1004 h` are handled correctly.
struct DecModeDetector {
    tail: Vec<u8>,
}

impl DecModeDetector {
    fn new() -> Self {
        Self { tail: Vec::new() }
    }

    fn process(
        &mut self,
        data: &[u8],
        focus_reporting: &AtomicBool,
        alternate_scroll: &AtomicBool,
    ) {
        let mut buf = Vec::with_capacity(self.tail.len() + data.len());
        buf.extend_from_slice(&self.tail);
        buf.extend_from_slice(data);
        apply_dec_modes(&buf, focus_reporting, alternate_scroll);

        const MAX_TAIL: usize = 64;
        let keep = buf.len().min(MAX_TAIL);
        self.tail.clear();
        self.tail.extend_from_slice(&buf[buf.len() - keep..]);
    }
}

fn apply_dec_modes(data: &[u8], focus_reporting: &AtomicBool, alternate_scroll: &AtomicBool) {
    let len = data.len();
    let mut i = 0;

    while i + 3 < len {
        if data[i] != 0x1b || data[i + 1] != b'[' || data[i + 2] != b'?' {
            i += 1;
            continue;
        }
        i += 3;

        let params_start = i;
        while i < len {
            let b = data[i];
            if b.is_ascii_digit() || b == b';' {
                i += 1;
                continue;
            }
            if b == b'h' || b == b'l' {
                let enabled = b == b'h';
                for param in data[params_start..i].split(|&ch| ch == b';') {
                    if let Some(mode) = std::str::from_utf8(param)
                        .ok()
                        .and_then(|s| s.parse::<u16>().ok())
                    {
                        match mode {
                            1004 => focus_reporting.store(enabled, Ordering::SeqCst),
                            1007 => alternate_scroll.store(enabled, Ordering::SeqCst),
                            _ => {}
                        }
                    }
                }
                i += 1;
                break;
            }

            // Not a DEC private mode we understand; continue scanning from the
            // next byte so we can recover from unrelated CSI sequences.
            i = params_start;
            break;
        }
    }
}

fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn resolve_login_shell_with_candidates(shell_env: Option<&str>, fallbacks: &[&str]) -> String {
    if let Some(shell) = shell_env.map(str::trim).filter(|shell| !shell.is_empty()) {
        if is_executable_file(Path::new(shell)) {
            return shell.to_string();
        }
    }

    for fallback in fallbacks {
        if is_executable_file(Path::new(fallback)) {
            return (*fallback).to_string();
        }
    }

    shell_env
        .map(str::trim)
        .filter(|shell| !shell.is_empty())
        .unwrap_or("/bin/sh")
        .to_string()
}

fn resolve_login_shell() -> String {
    let shell_env = std::env::var("SHELL").ok();
    resolve_login_shell_with_candidates(shell_env.as_deref(), &["/bin/bash", "/bin/sh"])
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
    /// Keyboard protocol state negotiated by the inner app (legacy vs enhanced).
    keyboard_protocol: KeyboardProtocol,
    /// Whether the outer terminal supports kitty keyboard enhancement.
    outer_keyboard_enhanced: bool,
    /// Whether the inner app requested alternate scroll mode (DECSET 1007).
    alternate_scroll: Arc<AtomicBool>,
    /// Tail buffer for terminal query sequences that may be split across PTY reads.
    query_tail: Vec<u8>,
}

impl EmbeddedTerminal {
    pub fn new(
        cwd: &str,
        rows: u16,
        cols: u16,
        outer_keyboard_enhanced: bool,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let shell = resolve_login_shell();
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
        let alternate_scroll = Arc::new(AtomicBool::new(false));
        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));

        let keyboard_protocol = KeyboardProtocol::new();
        let (output_tx, output_rx) = mpsc::channel::<Vec<u8>>();

        // Background reader: PTY output → channel + OSC 52 clipboard + focus detect + keyboard protocol detect
        let alive_clone = Arc::clone(&alive);
        let focus_clone = Arc::clone(&focus_reporting);
        let alternate_scroll_clone = Arc::clone(&alternate_scroll);
        let kb_flags_clone = keyboard_protocol.flags_ref();
        std::thread::spawn(move || {
            let mut buf = [0u8; 16384];
            let mut osc52 = Osc52Detector::new();
            let mut dec_mode_detector = DecModeDetector::new();
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = &buf[..n];
                        osc52.process(data);
                        dec_mode_detector.process(data, &focus_clone, &alternate_scroll_clone);
                        keyboard::detect_keyboard_protocol(data, &kb_flags_clone);
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
            keyboard_protocol,
            outer_keyboard_enhanced,
            alternate_scroll,
            query_tail: Vec::new(),
        })
    }

    /// Drain all pending PTY output from the channel and feed it to the parser.
    /// Call this on the main thread before rendering. Returns true if any output
    /// was processed (i.e. the screen may have changed).
    pub fn process_pending_output(&mut self) -> bool {
        let mut got_data = false;
        while let Ok(data) = self.output_rx.try_recv() {
            self.process_output_chunk(&data);
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

    pub fn alternate_screen(&self) -> bool {
        self.parser.screen().alternate_screen()
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

    pub fn alternate_scroll(&self) -> bool {
        self.alternate_scroll.load(Ordering::SeqCst)
    }

    /// Whether the inner application has requested enhanced keyboard mode
    /// via the kitty keyboard protocol (CSI > N u).
    pub fn keyboard_enhanced(&self) -> bool {
        self.keyboard_protocol.is_enhanced()
    }

    fn process_output_chunk(&mut self, data: &[u8]) {
        let mut buf = Vec::with_capacity(self.query_tail.len() + data.len());
        buf.extend_from_slice(&self.query_tail);
        buf.extend_from_slice(data);

        let mut responses = Vec::new();
        let mut i = 0;
        let mut parsed_until = 0;
        let mut tail_start = buf.len();

        while i < buf.len() {
            if buf[i] != 0x1b {
                i += 1;
                continue;
            }

            if i > parsed_until {
                self.parser.process(&buf[parsed_until..i]);
                parsed_until = i;
            }

            let Some((consumed, response)) = self.match_terminal_query(&buf[i..]) else {
                if terminal_query_needs_more_bytes(&buf[i..]) {
                    tail_start = i;
                    break;
                }
                i += 1;
                continue;
            };

            if let Some(response) = response {
                responses.extend_from_slice(&response);
            }
            let end = i + consumed;
            self.parser.process(&buf[i..end]);
            parsed_until = end;
            i = end;
        }

        let parse_end = tail_start.min(buf.len());
        if parsed_until < parse_end {
            self.parser.process(&buf[parsed_until..parse_end]);
        }

        self.query_tail.clear();
        if tail_start < buf.len() {
            let keep = (buf.len() - tail_start).min(64);
            self.query_tail
                .extend_from_slice(&buf[buf.len() - keep..]);
        }

        if !responses.is_empty() {
            self.write_input(&responses);
        }
    }

    fn match_terminal_query(&self, data: &[u8]) -> Option<(usize, Option<Vec<u8>>)> {
        if data.len() < 2 {
            return None;
        }

        if data[1] == b'[' {
            return self.match_csi_query(data);
        }

        None
    }

    fn match_csi_query(&self, data: &[u8]) -> Option<(usize, Option<Vec<u8>>)> {
        if data.len() < 3 {
            return None;
        }

        match data[2] {
            b'?' => self.match_private_csi_query(data),
            b'>' => self.match_secondary_csi_query(data),
            _ => self.match_standard_csi_query(data),
        }
    }

    fn match_private_csi_query(&self, data: &[u8]) -> Option<(usize, Option<Vec<u8>>)> {
        if data.len() < 4 {
            return None;
        }

        if data[3] == b'u' {
            return Some((
                4,
                Some(keyboard::query_response(self.outer_keyboard_enhanced)),
            ));
        }

        let mut i = 3;
        while i < data.len() && data[i].is_ascii_digit() {
            i += 1;
        }

        if i == data.len() {
            return None;
        }

        let param = std::str::from_utf8(&data[3..i])
            .ok()
            .and_then(|s| s.parse::<u16>().ok());

        match data[i] {
            b'$' => {
                if data.get(i + 1) == Some(&b'p') {
                    Some((
                        i + 2,
                        Some(dec_mode_response(
                            param.unwrap_or_default(),
                            self.focus_reporting(),
                            self.alternate_screen(),
                            self.mouse_protocol_mode(),
                            self.mouse_protocol_encoding(),
                            self.bracketed_paste(),
                            self.alternate_scroll(),
                        )),
                    ))
                } else if i + 1 >= data.len() {
                    None
                } else {
                    scan_csi_sequence_len(data).map(|len| (len, None))
                }
            }
            _ => scan_csi_sequence_len(data).map(|len| (len, None)),
        }
    }

    fn match_standard_csi_query(&self, data: &[u8]) -> Option<(usize, Option<Vec<u8>>)> {
        let mut i = 2;
        while i < data.len() && data[i].is_ascii_digit() {
            i += 1;
        }

        if i == data.len() {
            return None;
        }

        let param = std::str::from_utf8(&data[2..i])
            .ok()
            .and_then(|s| s.parse::<u16>().ok());

        match data[i] {
            b'n' if param == Some(6) => {
                let (row, col) = self.parser.screen().cursor_position();
                Some((i + 1, Some(cursor_position_response(row, col))))
            }
            _ => scan_csi_sequence_len(data).map(|len| (len, None)),
        }
    }

    fn match_secondary_csi_query(&self, data: &[u8]) -> Option<(usize, Option<Vec<u8>>)> {
        scan_csi_sequence_len(data).map(|len| (len, None))
    }
}

fn scan_csi_sequence_len(data: &[u8]) -> Option<usize> {
    if data.len() < 3 || data[0] != 0x1b || data[1] != b'[' {
        return None;
    }

    let mut i = 2;
    while i < data.len() {
        let b = data[i];
        if (0x40..=0x7e).contains(&b) {
            return Some(i + 1);
        }
        i += 1;
    }

    None
}

fn terminal_query_needs_more_bytes(data: &[u8]) -> bool {
    data.len() < 2 || (data[0] == 0x1b && data[1] == b'[')
}

fn cursor_position_response(row: u16, col: u16) -> Vec<u8> {
    format!("\x1b[{};{}R", row + 1, col + 1).into_bytes()
}

fn dec_mode_response(
    mode: u16,
    focus_reporting: bool,
    alternate_screen: bool,
    mouse_protocol_mode: vt100::MouseProtocolMode,
    mouse_protocol_encoding: vt100::MouseProtocolEncoding,
    bracketed_paste: bool,
    alternate_scroll: bool,
) -> Vec<u8> {
    use vt100::{MouseProtocolEncoding, MouseProtocolMode};

    let code = match mode {
        1000 => enabled_mode_code(matches!(
            mouse_protocol_mode,
            MouseProtocolMode::Press
                | MouseProtocolMode::PressRelease
                | MouseProtocolMode::ButtonMotion
                | MouseProtocolMode::AnyMotion
        )),
        1002 => enabled_mode_code(matches!(
            mouse_protocol_mode,
            MouseProtocolMode::ButtonMotion | MouseProtocolMode::AnyMotion
        )),
        1003 => enabled_mode_code(matches!(mouse_protocol_mode, MouseProtocolMode::AnyMotion)),
        1004 => enabled_mode_code(focus_reporting),
        1006 => enabled_mode_code(matches!(mouse_protocol_encoding, MouseProtocolEncoding::Sgr)),
        1007 => enabled_mode_code(alternate_scroll),
        1049 => enabled_mode_code(alternate_screen),
        2004 => enabled_mode_code(bracketed_paste),
        _ => 0,
    };

    format!("\x1b[?{};{}$y", mode, code).into_bytes()
}

fn enabled_mode_code(enabled: bool) -> u8 {
    if enabled { 1 } else { 2 }
}

/// Compute the kitty keyboard protocol modifier parameter from crossterm KeyModifiers.
/// Encoding per kitty spec: value = 1 + (shift:1 | alt:2 | ctrl:4 | super:8 | hyper:16 | meta:32)
/// Returns 0 if no modifiers are set (meaning: omit modifier parameter).
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
    if mods.contains(KeyModifiers::SUPER) {
        m += 8;
    }
    if mods.contains(KeyModifiers::HYPER) {
        m += 16;
    }
    if mods.contains(KeyModifiers::META) {
        m += 32;
    }
    if m > 0 {
        m + 1
    } else {
        0
    }
}

/// Check if the modifier combination requires CSI u encoding because
/// legacy terminal encoding cannot represent it.
/// Returns true for:
/// - Any combo involving SUPER, HYPER, or META (no legacy representation at all)
/// - Any two-or-more traditional modifier combo (Ctrl+Shift, Ctrl+Alt, Alt+Shift)
fn needs_csi_u(mods: crossterm::event::KeyModifiers) -> bool {
    use crossterm::event::KeyModifiers;
    // SUPER/HYPER/META have no legacy encoding — always need CSI u
    if mods.intersects(KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META) {
        return true;
    }
    let has_ctrl = mods.contains(KeyModifiers::CONTROL);
    let has_shift = mods.contains(KeyModifiers::SHIFT);
    let has_alt = mods.contains(KeyModifiers::ALT);
    // Any two-or-more traditional modifier combo needs CSI u for character keys
    (has_ctrl && has_shift) || (has_ctrl && has_alt) || (has_alt && has_shift)
}

/// Generate CSI u sequence: ESC [ codepoint ; modifier u
/// When modifier is 0, the semicolon and modifier are omitted: ESC [ codepoint u
fn csi_u(codepoint: u32, modifier: u8) -> Vec<u8> {
    if modifier > 0 {
        format!("\x1b[{};{}u", codepoint, modifier).into_bytes()
    } else {
        format!("\x1b[{}u", codepoint).into_bytes()
    }
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
///
/// When `inner_enhanced` is true (inner app requested kitty keyboard protocol),
/// ambiguous keys (Enter, Tab, Backspace, Esc, Space) are ALWAYS encoded as CSI u,
/// even without modifiers, so the inner app can fully disambiguate them.
///
/// When `inner_enhanced` is false (legacy mode):
/// - Character keys with multi-modifier or SUPER/META: CSI u encoding
/// - Character keys with single legacy modifier (Ctrl/Alt): legacy encoding for backward compat
/// - Enter/Backspace/Tab/Esc with ANY modifier: CSI u (no legacy representation for modified forms)
/// - Unmodified ambiguous keys: legacy encoding (e.g. Enter = \r)
///
/// Arrow/Home/End/PageUp/PageDown/Insert/Delete/F-keys always use standard xterm modified sequences.
/// All modifier calculations include SUPER/HYPER/META bits per kitty protocol spec.
pub fn key_to_bytes(
    key: &crossterm::event::KeyEvent,
    app_cursor: bool,
    inner_enhanced: bool,
) -> Vec<u8> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mods = key.modifiers;
    let xmod = xterm_modifier(mods);

    match key.code {
        // --- Character keys ---
        KeyCode::Char(c) => {
            // Multi-modifier combos or SUPER/META need CSI u (no legacy representation)
            if needs_csi_u(mods) {
                let codepoint = c.to_ascii_lowercase() as u32;
                return csi_u(codepoint, xmod);
            }

            // Single-modifier legacy encoding (backward compatible)
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

        // --- Ambiguous keys: CSI u when modified, OR when inner app is in enhanced mode ---
        // In enhanced mode, even unmodified Enter/Tab/Backspace/Esc use CSI u
        // so the inner app can fully disambiguate them (e.g. Ctrl+I vs Tab).
        KeyCode::Enter => {
            if xmod > 0 || inner_enhanced {
                csi_u(13, xmod) // ESC[13u or ESC[13;{mod}u
            } else {
                vec![b'\r']
            }
        }
        KeyCode::Backspace => {
            if xmod > 0 || inner_enhanced {
                csi_u(127, xmod) // ESC[127u or ESC[127;{mod}u
            } else {
                vec![0x7f]
            }
        }
        KeyCode::Tab => {
            if inner_enhanced {
                // Enhanced mode: always CSI u (inner app can distinguish Tab vs Ctrl+I)
                csi_u(9, xmod)
            } else if mods == KeyModifiers::SHIFT {
                // Legacy: pure Shift+Tab uses BackTab for maximum compatibility
                vec![0x1b, b'[', b'Z']
            } else if xmod > 0 {
                // Legacy: other modifiers use CSI u
                csi_u(9, xmod)
            } else {
                vec![b'\t']
            }
        }
        KeyCode::BackTab => {
            if inner_enhanced {
                // Enhanced mode: CSI u with shift modifier
                let mod_val = if xmod > 0 { xmod } else { 2 }; // BackTab always has shift
                csi_u(9, mod_val)
            } else if xmod > 2 {
                // Legacy: additional modifiers beyond shift use CSI u
                csi_u(9, xmod)
            } else {
                vec![0x1b, b'[', b'Z']
            }
        }
        KeyCode::Esc => {
            if xmod > 0 || inner_enhanced {
                csi_u(27, xmod) // ESC[27u or ESC[27;{mod}u
            } else {
                vec![0x1b]
            }
        }

        // --- Arrow keys: standard xterm modified encoding ---
        // Includes SUPER/META via xmod which now encodes all 6 modifiers.
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

        // --- Navigation keys: standard xterm modified encoding ---
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

        // --- Function keys: standard xterm modified encoding ---
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
            let inner_enhanced = terminal.keyboard_enhanced();
            key_to_bytes(key, app_cursor, inner_enhanced)
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
        return alternate_scroll_bytes(mouse.kind, terminal);
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

fn alternate_scroll_bytes(
    kind: crossterm::event::MouseEventKind,
    terminal: &EmbeddedTerminal,
) -> Vec<u8> {
    use crossterm::event::MouseEventKind;

    if !terminal.alternate_screen() || !terminal.alternate_scroll() {
        return vec![];
    }

    match kind {
        MouseEventKind::ScrollUp => {
            arrow_key_bytes(crossterm::event::KeyCode::Up, terminal.application_cursor())
        }
        MouseEventKind::ScrollDown => {
            arrow_key_bytes(crossterm::event::KeyCode::Down, terminal.application_cursor())
        }
        MouseEventKind::ScrollLeft => {
            arrow_key_bytes(crossterm::event::KeyCode::Left, terminal.application_cursor())
        }
        MouseEventKind::ScrollRight => {
            arrow_key_bytes(crossterm::event::KeyCode::Right, terminal.application_cursor())
        }
        _ => vec![],
    }
}

fn arrow_key_bytes(code: crossterm::event::KeyCode, app_cursor: bool) -> Vec<u8> {
    use crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    key_to_bytes(
        &KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        },
        app_cursor,
        false,
    )
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

/// Extract the URL at the given screen position, if one exists.
/// Scans the row for http:// or https:// URLs and checks if `col` falls within one.
pub fn url_at_position(screen: &vt100::Screen, row: u16, col: u16) -> Option<String> {
    let (screen_rows, screen_cols) = screen.size();
    if row >= screen_rows || col >= screen_cols {
        return None;
    }

    // Reconstruct the row text from individual cells
    let mut row_text = String::with_capacity(screen_cols as usize);
    for c in 0..screen_cols {
        if let Some(cell) = screen.cell(row, c) {
            let contents = cell.contents();
            if contents.is_empty() {
                row_text.push(' ');
            } else {
                row_text.push_str(&contents);
            }
        } else {
            row_text.push(' ');
        }
    }

    // Scan for URLs (http:// or https://) and check if col falls within one
    let bytes = row_text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for "http://" or "https://"
        let prefix_len = if bytes[i..].starts_with(b"https://") {
            8
        } else if bytes[i..].starts_with(b"http://") {
            7
        } else {
            i += 1;
            continue;
        };

        let start = i;
        let mut end = start + prefix_len;

        // Extend to the end of the URL (stop at whitespace and common delimiters)
        while end < bytes.len() {
            match bytes[end] {
                b' ' | b'\t' | b'"' | b'\'' | b'<' | b'>' | b'|' | b'{' | b'}' => break,
                _ => end += 1,
            }
        }

        // Strip trailing punctuation that's unlikely to be part of the URL
        while end > start + prefix_len {
            match bytes[end - 1] {
                b'.' | b',' | b';' | b':' | b'!' | b'?' | b')' | b']' => end -= 1,
                _ => break,
            }
        }

        if end > start + prefix_len && (col as usize) >= start && (col as usize) < end {
            return Some(row_text[start..end].to_string());
        }

        i = end;
    }

    None
}

/// Open a URL in the system browser.
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(not(target_os = "macos"))]
    let cmd = "xdg-open";

    let _ = std::process::Command::new(cmd)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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

// =============================================================================
// Tests — key encoding matrix covering all issue #3 keys + modifier combos
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use std::fs;
    use std::path::PathBuf;

    /// Helper: construct a key event with given code + modifiers
    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: mods,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    fn write_temp_shell(name: &str, executable: bool) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "beehive-terminal-test-{}-{}-{}",
            std::process::id(),
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if executable { 0o755 } else { 0o644 };
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        }

        path
    }

    #[test]
    fn test_resolve_login_shell_prefers_valid_shell_env() {
        let shell = write_temp_shell("preferred", true);
        let resolved =
            resolve_login_shell_with_candidates(Some(shell.to_str().unwrap()), &["/does/not/exist"]);
        assert_eq!(resolved, shell.to_string_lossy());
        fs::remove_file(shell).unwrap();
    }

    #[test]
    fn test_resolve_login_shell_falls_back_when_shell_env_invalid() {
        let fallback = write_temp_shell("fallback", true);
        let resolved = resolve_login_shell_with_candidates(
            Some("/does/not/exist"),
            &[fallback.to_str().unwrap()],
        );
        assert_eq!(resolved, fallback.to_string_lossy());
        fs::remove_file(fallback).unwrap();
    }

    #[test]
    fn test_resolve_login_shell_skips_non_executable_shell_env() {
        let shell = write_temp_shell("nonexec", false);
        let fallback = write_temp_shell("fallback2", true);
        let resolved = resolve_login_shell_with_candidates(
            Some(shell.to_str().unwrap()),
            &[fallback.to_str().unwrap()],
        );
        assert_eq!(resolved, fallback.to_string_lossy());
        fs::remove_file(shell).unwrap();
        fs::remove_file(fallback).unwrap();
    }

    // --- xterm_modifier tests ---

    #[test]
    fn test_xmod_none() {
        assert_eq!(xterm_modifier(KeyModifiers::NONE), 0);
    }

    #[test]
    fn test_xmod_shift() {
        assert_eq!(xterm_modifier(KeyModifiers::SHIFT), 2); // 1 + 1
    }

    #[test]
    fn test_xmod_ctrl() {
        assert_eq!(xterm_modifier(KeyModifiers::CONTROL), 5); // 1 + 4
    }

    #[test]
    fn test_xmod_alt() {
        assert_eq!(xterm_modifier(KeyModifiers::ALT), 3); // 1 + 2
    }

    #[test]
    fn test_xmod_super() {
        assert_eq!(xterm_modifier(KeyModifiers::SUPER), 9); // 1 + 8
    }

    #[test]
    fn test_xmod_ctrl_shift() {
        assert_eq!(
            xterm_modifier(KeyModifiers::CONTROL | KeyModifiers::SHIFT),
            6 // 1 + 1 + 4
        );
    }

    #[test]
    fn test_xmod_shift_super() {
        assert_eq!(
            xterm_modifier(KeyModifiers::SHIFT | KeyModifiers::SUPER),
            10 // 1 + 1 + 8
        );
    }

    #[test]
    fn test_xmod_meta() {
        assert_eq!(xterm_modifier(KeyModifiers::META), 33); // 1 + 32
    }

    // --- needs_csi_u tests ---

    #[test]
    fn test_csi_u_needed_for_super() {
        assert!(needs_csi_u(KeyModifiers::SUPER));
    }

    #[test]
    fn test_csi_u_needed_for_meta() {
        assert!(needs_csi_u(KeyModifiers::META));
    }

    #[test]
    fn test_csi_u_needed_for_ctrl_shift() {
        assert!(needs_csi_u(KeyModifiers::CONTROL | KeyModifiers::SHIFT));
    }

    #[test]
    fn test_csi_u_not_needed_for_ctrl_alone() {
        assert!(!needs_csi_u(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_csi_u_not_needed_for_shift_alone() {
        assert!(!needs_csi_u(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_dec_mode_detector_handles_split_sequences() {
        let focus = AtomicBool::new(false);
        let alternate_scroll = AtomicBool::new(false);
        let mut detector = DecModeDetector::new();

        detector.process(b"\x1b[?100", &focus, &alternate_scroll);
        assert!(!focus.load(Ordering::SeqCst));

        detector.process(b"4h", &focus, &alternate_scroll);
        assert!(focus.load(Ordering::SeqCst));

        detector.process(b"\x1b[?1004l", &focus, &alternate_scroll);
        assert!(!focus.load(Ordering::SeqCst));
    }

    #[test]
    fn test_dec_mode_detector_tracks_alternate_scroll() {
        let focus = AtomicBool::new(false);
        let alternate_scroll = AtomicBool::new(false);
        let mut detector = DecModeDetector::new();

        detector.process(b"\x1b[?1007h", &focus, &alternate_scroll);
        assert!(alternate_scroll.load(Ordering::SeqCst));

        detector.process(b"\x1b[?1007l", &focus, &alternate_scroll);
        assert!(!alternate_scroll.load(Ordering::SeqCst));
    }

    #[test]
    fn test_scan_csi_sequence_len_consumes_complete_standard_query() {
        assert_eq!(super::scan_csi_sequence_len(b"\x1b[6n"), Some(4));
    }

    #[test]
    fn test_scan_csi_sequence_len_consumes_complete_private_query() {
        assert_eq!(super::scan_csi_sequence_len(b"\x1b[?1004$p"), Some(9));
    }

    #[test]
    fn test_scan_csi_sequence_len_waits_for_complete_sequence() {
        assert_eq!(super::scan_csi_sequence_len(b"\x1b[>"), None);
    }

    #[test]
    fn test_terminal_query_tail_only_buffers_csi_sequences() {
        assert!(super::terminal_query_needs_more_bytes(b"\x1b["));
        assert!(super::terminal_query_needs_more_bytes(b"\x1b[?1004"));
        assert!(!super::terminal_query_needs_more_bytes(b"\x1b]10;?\x1b\\"));
        assert!(!super::terminal_query_needs_more_bytes(b"\x1b(B"));
    }

    #[test]
    fn test_cursor_position_response_uses_standard_cpr_format() {
        assert_eq!(super::cursor_position_response(4, 9), b"\x1b[5;10R");
    }

    #[test]
    fn test_dec_mode_response_reports_enabled_focus() {
        let bytes = super::dec_mode_response(
            1004,
            true,
            false,
            vt100::MouseProtocolMode::None,
            vt100::MouseProtocolEncoding::Default,
            false,
            false,
        );
        assert_eq!(bytes, b"\x1b[?1004;1$y");
    }

    #[test]
    fn test_dec_mode_response_reports_unsupported_unknown_mode() {
        let bytes = super::dec_mode_response(
            42,
            false,
            false,
            vt100::MouseProtocolMode::None,
            vt100::MouseProtocolEncoding::Default,
            false,
            false,
        );
        assert_eq!(bytes, b"\x1b[?42;0$y");
    }

    #[test]
    fn test_dec_mode_response_reports_alternate_scroll() {
        let bytes = super::dec_mode_response(
            1007,
            false,
            false,
            vt100::MouseProtocolMode::None,
            vt100::MouseProtocolEncoding::Default,
            false,
            true,
        );
        assert_eq!(bytes, b"\x1b[?1007;1$y");
    }

    #[test]
    fn test_arrow_key_bytes_respects_application_cursor_mode() {
        assert_eq!(
            super::arrow_key_bytes(crossterm::event::KeyCode::Up, false),
            b"\x1b[A"
        );
        assert_eq!(
            super::arrow_key_bytes(crossterm::event::KeyCode::Up, true),
            b"\x1bOA"
        );
    }

    // --- Issue #3 keys: Cmd+A, Cmd+Right, Shift+Cmd+Right, Shift+Enter ---

    #[test]
    fn test_cmd_a() {
        // Cmd+A (SUPER+a) → CSI u: ESC[97;9u
        let ev = key(KeyCode::Char('a'), KeyModifiers::SUPER);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[97;9u");
    }

    #[test]
    fn test_cmd_right() {
        // Cmd+Right → modified arrow: ESC[1;9C
        let ev = key(KeyCode::Right, KeyModifiers::SUPER);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[1;9C");
    }

    #[test]
    fn test_shift_cmd_right() {
        // Shift+Cmd+Right → modified arrow: ESC[1;10C  (1 + shift:1 + super:8 = 10)
        let ev = key(KeyCode::Right, KeyModifiers::SHIFT | KeyModifiers::SUPER);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[1;10C");
    }

    #[test]
    fn test_shift_enter_legacy() {
        // Shift+Enter in legacy mode → CSI u: ESC[13;2u
        let ev = key(KeyCode::Enter, KeyModifiers::SHIFT);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[13;2u");
    }

    #[test]
    fn test_shift_enter_enhanced() {
        // Shift+Enter in enhanced mode → CSI u: ESC[13;2u
        let ev = key(KeyCode::Enter, KeyModifiers::SHIFT);
        let bytes = key_to_bytes(&ev, false, true);
        assert_eq!(bytes, b"\x1b[13;2u");
    }

    // --- Unmodified ambiguous keys: legacy vs enhanced ---

    #[test]
    fn test_enter_legacy() {
        let ev = key(KeyCode::Enter, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![b'\r']);
    }

    #[test]
    fn test_enter_enhanced() {
        // Enhanced mode: unmodified Enter → CSI u: ESC[13u
        let ev = key(KeyCode::Enter, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, true);
        assert_eq!(bytes, b"\x1b[13u");
    }

    #[test]
    fn test_tab_legacy() {
        let ev = key(KeyCode::Tab, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![b'\t']);
    }

    #[test]
    fn test_tab_enhanced() {
        let ev = key(KeyCode::Tab, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, true);
        assert_eq!(bytes, b"\x1b[9u");
    }

    #[test]
    fn test_esc_legacy() {
        let ev = key(KeyCode::Esc, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![0x1b]);
    }

    #[test]
    fn test_esc_enhanced() {
        let ev = key(KeyCode::Esc, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, true);
        assert_eq!(bytes, b"\x1b[27u");
    }

    #[test]
    fn test_backspace_legacy() {
        let ev = key(KeyCode::Backspace, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![0x7f]);
    }

    #[test]
    fn test_backspace_enhanced() {
        let ev = key(KeyCode::Backspace, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, true);
        assert_eq!(bytes, b"\x1b[127u");
    }

    // --- Modified special keys ---

    #[test]
    fn test_ctrl_shift_t() {
        // Ctrl+Shift+T → CSI u: ESC[116;6u  (codepoint 116 = 't', mod 6 = 1+1+4)
        let ev = key(
            KeyCode::Char('T'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[116;6u");
    }

    #[test]
    fn test_ctrl_c() {
        // Ctrl+C → legacy: 0x03
        let ev = key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![0x03]);
    }

    #[test]
    fn test_alt_a() {
        // Alt+A → legacy: ESC + a
        let ev = key(KeyCode::Char('a'), KeyModifiers::ALT);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![0x1b, b'a']);
    }

    #[test]
    fn test_shift_tab_legacy() {
        // Shift+Tab → legacy BackTab
        let ev = key(KeyCode::Tab, KeyModifiers::SHIFT);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![0x1b, b'[', b'Z']);
    }

    #[test]
    fn test_ctrl_tab() {
        // Ctrl+Tab → CSI u: ESC[9;5u
        let ev = key(KeyCode::Tab, KeyModifiers::CONTROL);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[9;5u");
    }

    #[test]
    fn test_ctrl_enter() {
        // Ctrl+Enter → CSI u: ESC[13;5u
        let ev = key(KeyCode::Enter, KeyModifiers::CONTROL);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[13;5u");
    }

    #[test]
    fn test_cmd_enter() {
        // Cmd+Enter → CSI u: ESC[13;9u
        let ev = key(KeyCode::Enter, KeyModifiers::SUPER);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[13;9u");
    }

    // --- Arrow keys with modifiers ---

    #[test]
    fn test_shift_up() {
        let ev = key(KeyCode::Up, KeyModifiers::SHIFT);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[1;2A");
    }

    #[test]
    fn test_ctrl_right() {
        let ev = key(KeyCode::Right, KeyModifiers::CONTROL);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[1;5C");
    }

    // --- Plain keys (no regressions) ---

    #[test]
    fn test_plain_a() {
        let ev = key(KeyCode::Char('a'), KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![b'a']);
    }

    #[test]
    fn test_plain_enter() {
        let ev = key(KeyCode::Enter, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, vec![b'\r']);
    }

    #[test]
    fn test_arrow_up_normal() {
        let ev = key(KeyCode::Up, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[A");
    }

    #[test]
    fn test_arrow_up_app_cursor() {
        let ev = key(KeyCode::Up, KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, true, false);
        assert_eq!(bytes, b"\x1bOA");
    }

    #[test]
    fn test_f1_plain() {
        let ev = key(KeyCode::F(1), KeyModifiers::NONE);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1bOP");
    }

    #[test]
    fn test_f5_ctrl() {
        let ev = key(KeyCode::F(5), KeyModifiers::CONTROL);
        let bytes = key_to_bytes(&ev, false, false);
        assert_eq!(bytes, b"\x1b[15;5~");
    }

    // --- URL detection tests ---

    /// Helper: create a vt100 screen with the given text on row 0.
    fn screen_with_text(text: &str, cols: u16) -> vt100::Parser {
        let mut parser = vt100::Parser::new(2, cols, 0);
        parser.process(text.as_bytes());
        parser
    }

    #[test]
    fn test_url_at_position_finds_https() {
        let parser = screen_with_text("Visit https://example.com/path for info", 60);
        let screen = parser.screen();
        // Click on the 'e' of 'example' (col 14)
        assert_eq!(
            url_at_position(screen, 0, 14),
            Some("https://example.com/path".to_string())
        );
    }

    #[test]
    fn test_url_at_position_finds_http() {
        let parser = screen_with_text("Go to http://test.org now", 40);
        let screen = parser.screen();
        assert_eq!(
            url_at_position(screen, 0, 10),
            Some("http://test.org".to_string())
        );
    }

    #[test]
    fn test_url_at_position_returns_none_outside_url() {
        let parser = screen_with_text("Visit https://example.com for info", 60);
        let screen = parser.screen();
        // Click on "Visit" (col 2)
        assert_eq!(url_at_position(screen, 0, 2), None);
        // Click on "for" (col 28)
        assert_eq!(url_at_position(screen, 0, 28), None);
    }

    #[test]
    fn test_url_at_position_strips_trailing_punctuation() {
        let parser = screen_with_text("See https://example.com/page.", 40);
        let screen = parser.screen();
        assert_eq!(
            url_at_position(screen, 0, 10),
            Some("https://example.com/page".to_string())
        );
    }

    #[test]
    fn test_url_at_position_out_of_bounds() {
        let parser = screen_with_text("https://x.com", 20);
        let screen = parser.screen();
        assert_eq!(url_at_position(screen, 5, 0), None); // row out of bounds
        assert_eq!(url_at_position(screen, 0, 25), None); // col out of bounds
    }
}
