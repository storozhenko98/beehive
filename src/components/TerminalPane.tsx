import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";

interface TerminalPaneProps {
  id: string;
  cwd: string;
  cmd?: string;
  args?: string[];
  onExit?: () => void;
}

export function TerminalPane({ id, cwd, cmd, args, onExit }: TerminalPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const terminal = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
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
    terminal.open(containerRef.current);

    // Small delay to ensure the container is properly sized before fitting
    requestAnimationFrame(() => {
      fitAddon.fit();

      // Create the PTY after we know the terminal dimensions
      invoke("create_pty", {
        id,
        cwd,
        cmd: cmd ?? null,
        args: args ?? null,
        rows: terminal.rows,
        cols: terminal.cols,
      }).catch((err) => {
        terminal.write(`\r\nFailed to create PTY: ${err}\r\n`);
      });
    });

    // Listen for PTY output
    const unlistenOutput = listen<number[]>(`pty-output-${id}`, (event) => {
      terminal.write(new Uint8Array(event.payload));
    });

    // Listen for PTY exit
    const unlistenExit = listen(`pty-exit-${id}`, () => {
      terminal.write("\r\n\x1b[90m[Process exited]\x1b[0m\r\n");
      onExit?.();
    });

    // Send user input to PTY
    const onDataDisposable = terminal.onData((data) => {
      invoke("write_to_pty", { id, data }).catch(() => {
        // PTY may have been closed
      });
    });

    // Handle resize
    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      invoke("resize_pty", {
        id,
        rows: terminal.rows,
        cols: terminal.cols,
      }).catch(() => {
        // PTY may have been closed
      });
    });
    resizeObserver.observe(containerRef.current);

    return () => {
      resizeObserver.disconnect();
      onDataDisposable.dispose();
      unlistenOutput.then((fn) => fn());
      unlistenExit.then((fn) => fn());
      invoke("close_pty", { id }).catch(() => {});
      terminal.dispose();
    };
  }, [id, cwd, cmd]);

  return (
    <div
      ref={containerRef}
      style={{ width: "100%", height: "100%", padding: "4px" }}
    />
  );
}
