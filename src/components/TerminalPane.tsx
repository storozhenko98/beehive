import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";

interface DragDropPayload {
  paths: string[];
  position: { x: number; y: number };
}

interface TerminalPaneProps {
  id: string;
  cwd: string;
  cmd?: string;
  args?: string[];
  isVisible: boolean;
  onExit?: () => void;
}

export function TerminalPane({ id, cwd, cmd, args, isVisible, onExit }: TerminalPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  // Track which PTY session is "ours" to ignore stale exit events
  const activeSessionRef = useRef<string | null>(null);
  // Drag-drop visual feedback (ref avoids stale closures in event callbacks)
  const [isDragOver, setIsDragOver] = useState(false);
  const isDragOverRef = useRef(false);
  function updateDragOver(value: boolean) {
    isDragOverRef.current = value;
    setIsDragOver(value);
  }

  useEffect(() => {
    if (!containerRef.current) return;

    // Generate a unique session ID for this effect invocation.
    // This prevents React StrictMode double-mount from leaking
    // exit events between the killed first PTY and the live second one.
    const sessionId = `${id}-${crypto.randomUUID()}`;
    activeSessionRef.current = sessionId;

    const terminal = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: '"MesloLGS NF", "Hack Nerd Font", "FiraCode Nerd Font", "JetBrainsMono Nerd Font", Menlo, Monaco, "Courier New", monospace',
      allowProposedApi: true,
      theme: {
        background: "#1e1e2e",
        foreground: "#cdd6f4",
        cursor: "#f5e0dc",
        selectionBackground: "#585b7066",
        black: "#45475a",
        red: "#f38ba8",
        green: "#a6e3a1",
        yellow: "#f9e2af",
        blue: "#89b4fa",
        magenta: "#f5c2e7",
        cyan: "#94e2d5",
        white: "#bac2de",
        brightBlack: "#585b70",
        brightRed: "#f38ba8",
        brightGreen: "#a6e3a1",
        brightYellow: "#f9e2af",
        brightBlue: "#89b4fa",
        brightMagenta: "#f5c2e7",
        brightCyan: "#94e2d5",
        brightWhite: "#a6adc8",
      },
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.loadAddon(new WebLinksAddon());
    const unicode11 = new Unicode11Addon();
    terminal.loadAddon(unicode11);
    terminal.unicode.activeVersion = "11";
    terminal.open(containerRef.current);

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    // Copy xterm.js selection to system clipboard (mouse selection)
    const onSelDisposable = terminal.onSelectionChange(() => {
      const sel = terminal.getSelection();
      if (sel) {
        navigator.clipboard.writeText(sel).catch(() => {});
      }
    });

    // Handle OSC 52 clipboard sequences from programs like zellij/tmux
    terminal.parser.registerOscHandler(52, (data) => {
      const idx = data.indexOf(";");
      if (idx === -1) return true;
      const payload = data.slice(idx + 1);
      if (payload && payload !== "?") {
        try {
          const text = atob(payload);
          navigator.clipboard.writeText(text).catch(() => {});
        } catch {
          // invalid base64
        }
      }
      return true;
    });

    // Small delay to ensure the container is properly sized before fitting
    requestAnimationFrame(() => {
      fitAddon.fit();

      // Create the PTY using the unique session ID
      invoke("create_pty", {
        id: sessionId,
        cwd,
        cmd: cmd ?? null,
        args: args ?? null,
        rows: terminal.rows,
        cols: terminal.cols,
      }).catch((err) => {
        terminal.write(`\r\nFailed to create PTY: ${err}\r\n`);
      });
    });

    // Listen for PTY output using the session-specific event
    const unlistenOutput = listen<number[]>(`pty-output-${sessionId}`, (event) => {
      terminal.write(new Uint8Array(event.payload));
    });

    // Listen for PTY exit
    const unlistenExit = listen(`pty-exit-${sessionId}`, () => {
      terminal.write("\r\n\x1b[90m[Process exited]\x1b[0m\r\n");
      onExit?.();
    });

    // Send user input to PTY
    const onDataDisposable = terminal.onData((data) => {
      invoke("write_to_pty", { id: sessionId, data }).catch(() => {
        // PTY may have been closed
      });
    });

    // Handle resize
    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      invoke("resize_pty", {
        id: sessionId,
        rows: terminal.rows,
        cols: terminal.cols,
      }).catch(() => {
        // PTY may have been closed
      });
    });
    resizeObserver.observe(containerRef.current);

    // --- Drag-drop: listen for Tauri's global drag events ---
    // Hit-test helper: check if window-relative position falls inside this pane
    function isOverThisPane(x: number, y: number): boolean {
      const el = document.elementFromPoint(x, y);
      return !!containerRef.current && containerRef.current.contains(el);
    }

    const unlistenDragOver = listen<DragDropPayload>("tauri://drag-over", (event) => {
      const { x, y } = event.payload.position;
      const over = isOverThisPane(x, y);
      if (over !== isDragOverRef.current) {
        updateDragOver(over);
      }
    });

    const unlistenDrop = listen<DragDropPayload>("tauri://drag-drop", (event) => {
      updateDragOver(false);

      const { paths, position } = event.payload;
      if (!isOverThisPane(position.x, position.y)) return;
      if (!terminalRef.current) return;
      if (paths.length === 0) return;

      // Paste the raw path(s) through xterm.js so bracketed paste mode
      // is handled correctly. This mimics how real terminals (iTerm2, etc.)
      // handle file drops — the running application (OpenCode, Claude Code)
      // receives a paste event and can detect the file path as an image.
      terminalRef.current.paste(paths.join(" "));
    });

    const unlistenDragLeave = listen("tauri://drag-leave", () => {
      updateDragOver(false);
    });

    return () => {
      resizeObserver.disconnect();
      onDataDisposable.dispose();
      onSelDisposable.dispose();
      unlistenOutput.then((fn) => fn());
      unlistenExit.then((fn) => fn());
      unlistenDragOver.then((fn) => fn());
      unlistenDrop.then((fn) => fn());
      unlistenDragLeave.then((fn) => fn());
      invoke("close_pty", { id: sessionId }).catch(() => {});
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, [id, cwd, cmd]);

  // Re-fit when visibility changes
  useEffect(() => {
    if (isVisible && fitAddonRef.current && terminalRef.current) {
      const currentSession = activeSessionRef.current;
      // Delay to allow layout to settle after display change
      requestAnimationFrame(() => {
        fitAddonRef.current?.fit();
        if (terminalRef.current && currentSession) {
          invoke("resize_pty", {
            id: currentSession,
            rows: terminalRef.current.rows,
            cols: terminalRef.current.cols,
          }).catch(() => {});
        }
      });
    }
  }, [isVisible]);

  return (
    <div
      ref={containerRef}
      style={{ width: "100%", height: "100%", padding: "4px", position: "relative" }}
    >
      {isDragOver && <div className="drop-overlay">Drop file to paste path</div>}
    </div>
  );
}
