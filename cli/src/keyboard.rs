//! Keyboard protocol state tracking for inner PTY applications.
//!
//! Detects when an inner application (zellij, opencode, claude, etc.) negotiates
//! the kitty keyboard protocol via escape sequences in PTY output, and adjusts
//! the key encoding sent to the PTY accordingly.
//!
//! Protocol sequences detected:
//! - `CSI > flags u`  — push enhancement flags (enable)
//! - `CSI < u`        — pop enhancement flags (disable)
//! - `CSI < N u`      — pop N levels
//! - `CSI ? u`        — query support (we respond)

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Tracks the keyboard protocol state negotiated by the inner application.
/// One instance per EmbeddedTerminal.
pub struct KeyboardProtocol {
    /// Enhancement flags requested by the inner app via CSI > N u.
    /// 0 = legacy mode, >0 = enhanced mode with that flag value.
    /// Uses a simple top-of-stack model (not a full stack) for simplicity.
    flags: Arc<AtomicU8>,
    /// Force enhanced mode regardless of inner app negotiation.
    /// Set via BEEHIVE_FORCE_ENHANCED_KEYS=1 env var.
    force_enhanced: bool,
}

impl KeyboardProtocol {
    pub fn new() -> Self {
        let force = std::env::var("BEEHIVE_FORCE_ENHANCED_KEYS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self {
            flags: Arc::new(AtomicU8::new(0)),
            force_enhanced: force,
        }
    }

    /// Get a clone of the atomic flags ref for the background reader thread.
    pub fn flags_ref(&self) -> Arc<AtomicU8> {
        Arc::clone(&self.flags)
    }

    /// Whether the inner app is in enhanced keyboard mode.
    pub fn is_enhanced(&self) -> bool {
        self.force_enhanced || self.flags.load(Ordering::SeqCst) > 0
    }

    /// Current flags value (0 = legacy).
    pub fn flags_value(&self) -> u8 {
        self.flags.load(Ordering::SeqCst)
    }

    /// Set force-enhanced override.
    #[allow(dead_code)]
    pub fn set_force_enhanced(&mut self, force: bool) {
        self.force_enhanced = force;
    }
}

/// Detect keyboard protocol negotiation sequences in raw PTY output.
///
/// Called from the background reader thread. Updates the shared atomic flags
/// when the inner app pushes or pops keyboard enhancement.
///
/// Sequences:
/// - `ESC [ > N u` — push flags N (enable enhanced mode)
/// - `ESC [ < u`   — pop one level (disable enhanced mode)
/// - `ESC [ < N u` — pop N levels (disable enhanced mode)
///
/// Note: `ESC [ ? u` (query) requires a response which we handle separately.
pub fn detect_keyboard_protocol(data: &[u8], flags: &AtomicU8) {
    let len = data.len();
    let mut i = 0;

    while i + 3 < len {
        // Look for ESC [
        if data[i] != 0x1b || data[i + 1] != b'[' {
            i += 1;
            continue;
        }
        i += 2;

        if i >= len {
            break;
        }

        match data[i] {
            b'>' => {
                // Push: ESC [ > N u
                i += 1;
                let mut num: u8 = 0;
                let mut has_digits = false;
                while i < len && data[i].is_ascii_digit() {
                    num = num.saturating_mul(10).saturating_add(data[i] - b'0');
                    has_digits = true;
                    i += 1;
                }
                if i < len && data[i] == b'u' && has_digits {
                    flags.store(num.max(1), Ordering::SeqCst);
                    i += 1;
                }
            }
            b'<' => {
                // Pop: ESC [ < u  OR  ESC [ < N u
                i += 1;
                if i < len && data[i] == b'u' {
                    flags.store(0, Ordering::SeqCst);
                    i += 1;
                } else {
                    // Skip digits
                    while i < len && data[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i < len && data[i] == b'u' {
                        flags.store(0, Ordering::SeqCst);
                        i += 1;
                    }
                }
            }
            b'=' => {
                // Set mode: ESC [ = flags ; mode u  (kitty spec alternate form)
                // Some apps use this instead of push/pop
                i += 1;
                let mut num: u8 = 0;
                while i < len && data[i].is_ascii_digit() {
                    num = num.saturating_mul(10).saturating_add(data[i] - b'0');
                    i += 1;
                }
                // Skip optional ; mode
                if i < len && data[i] == b';' {
                    i += 1;
                    while i < len && data[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if i < len && data[i] == b'u' {
                    flags.store(num.max(1), Ordering::SeqCst);
                    i += 1;
                }
            }
            _ => {
                // Not a keyboard protocol sequence, continue scanning
            }
        }
    }
}

/// Generate a response to a `CSI ? u` query from the inner app.
/// Returns the response bytes: `CSI ? flags u`
/// This tells the inner app what enhancement flags we support.
pub fn query_response(enhanced_outer: bool) -> Vec<u8> {
    if enhanced_outer {
        // We support disambiguate + report event types
        b"\x1b[?3u".to_vec() // flags = 3 (0b11)
    } else {
        // Legacy outer terminal: we can still encode CSI u on the output side,
        // but we can't guarantee all modifier combos from the outer terminal.
        // Report flag 1 (disambiguate only).
        b"\x1b[?1u".to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_push() {
        let flags = AtomicU8::new(0);
        detect_keyboard_protocol(b"\x1b[>1u", &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_detect_push_flags_3() {
        let flags = AtomicU8::new(0);
        detect_keyboard_protocol(b"\x1b[>3u", &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_detect_pop() {
        let flags = AtomicU8::new(3);
        detect_keyboard_protocol(b"\x1b[<u", &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_detect_pop_n() {
        let flags = AtomicU8::new(3);
        detect_keyboard_protocol(b"\x1b[<1u", &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_detect_push_in_stream() {
        let flags = AtomicU8::new(0);
        // Embedded in a stream of other output
        let mut data = Vec::new();
        data.extend_from_slice(b"Hello world\r\n");
        data.extend_from_slice(b"\x1b[>1u");
        data.extend_from_slice(b"more output");
        detect_keyboard_protocol(&data, &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_detect_set_mode() {
        let flags = AtomicU8::new(0);
        detect_keyboard_protocol(b"\x1b[=1;1u", &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_no_false_positive() {
        let flags = AtomicU8::new(0);
        // Regular CSI u (cursor restore) should not trigger
        detect_keyboard_protocol(b"\x1b[u", &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_no_false_positive_csi_number_u() {
        let flags = AtomicU8::new(0);
        // CSI 13;2u is a key event, not a protocol negotiation
        detect_keyboard_protocol(b"\x1b[13;2u", &flags);
        assert_eq!(flags.load(Ordering::SeqCst), 0);
    }
}
