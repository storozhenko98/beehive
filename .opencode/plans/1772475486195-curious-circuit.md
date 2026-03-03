# Fix: Terminal Keybinding Passthrough + Input Latency

## Problem Summary

1. **Custom Zellij keybindings (Ctrl+Shift+T, etc.) don't work** in both the GUI (Tauri) and CLI (TUI) versions
2. **Terminal input is occasionally laggy**, especially with multiple terminals open

## Root Cause Analysis

### Keybinding Issue — CLI (`cli/src/terminal.rs:162-165`)

The `key_to_bytes` function drops the Shift modifier when Ctrl is also held:

```rust
if mods.contains(KeyModifiers::CONTROL) {
    if c.is_ascii_alphabetic() {
        vec![(c.to_ascii_lowercase() as u8) & 0x1f]  // Shift is LOST
    }
}
```

Ctrl+T and Ctrl+Shift+T both produce `0x14`. Zellij cannot distinguish them. Legacy terminal encoding simply cannot represent Ctrl+Shift+letter — it needs CSI u format (`\x1b[<codepoint>;<modifier>u`).

Same issue exists for: Ctrl+Alt+letter, Alt+Shift+letter, and any multi-modifier combo. Also, arrow keys / F-keys with modifiers (Shift+Up, Ctrl+Arrow, etc.) are not encoded with modifier parameters.

### Keybinding Issue — GUI (`src/components/TerminalPane.tsx`)

- No `attachCustomKeyEventHandler` is set, so the Tauri WebView may consume modifier key combos (Ctrl+Shift+T) before xterm.js sees them
- xterm.js 5.5 doesn't support the kitty keyboard protocol (added in 6.1.0), so even if keys reach xterm, Ctrl+Shift+T generates the same `\x14` as Ctrl+T
- No `terminal.onBinary()` handler exists for non-UTF-8 escape sequences

### Input Latency — GUI (`src-tauri/src/pty.rs:140-165`)

- **Global mutex contention**: `write_to_pty` holds the `PtyManager` mutex for the entire write+flush cycle. All terminals serialize even though sessions are independent
- **Flush per keystroke**: `writer.flush()` called on every character adds syscall overhead
- Same contention issue in `resize_pty`

### Input Latency — CLI (`cli/src/terminal.rs:97-101`)

- `write_input` calls `flush()` on every keystroke — same overhead

---

## Implementation Plan

### 1. CLI: Fix `key_to_bytes` modifier encoding (`cli/src/terminal.rs`)

Rewrite `key_to_bytes` to generate CSI u sequences for multi-modifier combos:

- **Ctrl+Shift+letter**: Generate `\x1b[<codepoint>;6u` (modifier 6 = 1+shift+ctrl)
- **Ctrl+Alt+letter**: Generate `\x1b[<codepoint>;7u`
- **Alt+Shift+letter**: Generate `\x1b[<codepoint>;4u`
- **Ctrl+Alt+Shift+letter**: Generate `\x1b[<codepoint>;8u`
- **Single Ctrl/Alt**: Keep legacy encoding (backward compatible)
- **Arrow/Home/End/F-keys with modifiers**: Add `\x1b[1;<mod>A` format (standard xterm modifier encoding)
- **PageUp/Delete/Insert with modifiers**: Add `\x1b[5;<mod>~` format

CSI u modifier formula: `value = 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)`

Also add modifier encoding for special keys using standard xterm format:
- Shift+Up: `\x1b[1;2A`, Ctrl+Up: `\x1b[1;5A`, Ctrl+Shift+Up: `\x1b[1;6A`, etc.

### 2. CLI: Remove flush per keystroke (`cli/src/terminal.rs:100`)

Remove `let _ = w.flush();` from `write_input`. PTY master fds are unbuffered — data is immediately available to the slave process without flushing.

### 3. GUI: Add `attachCustomKeyEventHandler` (`src/components/TerminalPane.tsx`)

Add after `terminal.open(containerRef.current)` (line 87):

- Return `false` for Cmd+C/V/A/Q (let macOS handle native copy/paste/quit)
- For Ctrl+Shift+letter, Ctrl+Alt+letter, and Alt+Shift+letter combos: manually generate CSI u sequences and write to PTY, then `return false` (prevent xterm.js from also generating a legacy sequence)
- Return `true` for everything else (let xterm.js handle normally)

Need a ref to sessionId accessible from the handler — add `sessionIdRef`.

### 4. GUI: Add global keydown capture listener (`src/components/TerminalPane.tsx`)

Add a `window.addEventListener("keydown", handler, { capture: true })` that calls `event.preventDefault()` for Ctrl+Shift and Ctrl+Alt combos when the terminal textarea is focused. This prevents the WebView from consuming these events before xterm.js sees them.

### 5. GUI: Add `terminal.onBinary()` handler (`src/components/TerminalPane.tsx`)

Add handler after `onData` handler. Converts binary string to byte array and sends via a new `write_to_pty_binary` Tauri command. This handles non-UTF-8 escape sequences (legacy mouse reports).

### 6. GUI: Add `write_to_pty_binary` command (`src-tauri/src/pty.rs`)

New command accepting `Vec<u8>` instead of `String`. Reuses the same pattern as `write_to_pty` but writes raw bytes. Register in `lib.rs`.

### 7. GUI: Reduce mutex contention in `write_to_pty` (`src-tauri/src/pty.rs`)

Clone the `Arc<Mutex<Writer>>` while holding the global lock, then drop global lock before writing:

```rust
let writer = {
    let manager = state.lock().await;
    manager.sessions.get(&id)...?.writer.clone()
}; // global lock dropped
let mut writer = writer.lock()...;
writer.write_all(data.as_bytes())...;
// No flush needed
```

### 8. GUI: Remove flush per keystroke (`src-tauri/src/pty.rs`)

Remove `writer.flush()` from `write_to_pty`. PTY master fds are unbuffered.

### 9. GUI: Same contention fix for `resize_pty` (`src-tauri/src/pty.rs`)

Clone `Arc<Mutex<Master>>`, drop global lock, then resize.

### 10. GUI: Increase reader buffer (`src-tauri/src/pty.rs:117`)

Increase from 4096 to 16384 bytes. Reduces IPC events during output bursts (TUI app redraws are 5-20KB).

---

## Files to Modify

| File | Changes |
|------|---------|
| `cli/src/terminal.rs` | Rewrite `key_to_bytes` for CSI u + modifier encoding; remove flush |
| `src/components/TerminalPane.tsx` | Add `attachCustomKeyEventHandler`, `onBinary`, global keydown listener, sessionId ref |
| `src-tauri/src/pty.rs` | Reduce mutex contention, remove flush, add `write_to_pty_binary`, increase buffer |
| `src-tauri/src/lib.rs` | Register `write_to_pty_binary` command |

## Verification

1. **CLI keybinding test**: Run Beehive CLI, open a comb, launch Zellij, press Ctrl+Shift+T then `+` — should create a new Zellij tab
2. **GUI keybinding test**: Same in the Tauri app
3. **Latency test**: Open 3-4 terminal panes, type rapidly in each — should feel responsive
4. **Regression test**: Verify basic typing, arrow keys, Ctrl+C, Ctrl+D, Alt+letter, mouse clicks in TUI apps, copy/paste, drag-drop all still work
5. **Build check**: `cargo check` in both `src-tauri/` and `cli/`, `npx tsc --noEmit` for frontend
