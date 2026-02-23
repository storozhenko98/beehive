import { useState, useCallback } from "react";
import { TerminalPane } from "./TerminalPane";
import type { Comb, HiveInfo } from "../types";

interface PaneState {
  id: string;
  type: "agent" | "terminal";
}

interface Props {
  beehiveDir: string;
  hive: HiveInfo;
  comb: Comb;
  onBack: () => void;
}

let paneCounter = 0;

export function WorkspaceScreen({ beehiveDir: _, hive, comb, onBack }: Props) {
  const [panes, setPanes] = useState<PaneState[]>([
    { id: `pane-${++paneCounter}`, type: "terminal" },
  ]);

  const addPane = useCallback((type: "agent" | "terminal") => {
    setPanes((prev) => [...prev, { id: `pane-${++paneCounter}`, type }]);
  }, []);

  const removePane = useCallback((id: string) => {
    setPanes((prev) => {
      const next = prev.filter((p) => p.id !== id);
      return next;
    });
  }, []);

  const cols = panes.length <= 1 ? 1 : panes.length <= 4 ? 2 : 3;

  return (
    <div className="workspace-layout">
      <div className="workspace-header">
        <button className="btn-text" onClick={onBack}>
          &larr; {hive.repoName}
        </button>
        <div className="workspace-title">
          <strong>{comb.name}</strong>
          <span className="workspace-branch">{comb.branch}</span>
        </div>
        <div className="workspace-actions">
          <button className="btn btn-sm" onClick={() => addPane("terminal")}>
            + Terminal
          </button>
          <button className="btn btn-sm" onClick={() => addPane("agent")}>
            + Agent
          </button>
        </div>
      </div>

      <div
        className="workspace-grid"
        style={{
          gridTemplateColumns: `repeat(${cols}, 1fr)`,
        }}
      >
        {panes.map((pane) => (
          <div key={pane.id} className="terminal-pane">
            <div className="pane-header">
              <span className="pane-title">
                <span className="pane-type-badge">
                  {pane.type === "agent" ? "AGENT" : "TERM"}
                </span>
              </span>
              <button
                className="close-btn"
                onClick={() => removePane(pane.id)}
                title="Close pane"
              >
                x
              </button>
            </div>
            <div className="pane-body">
              <TerminalPane
                id={pane.id}
                cwd={comb.path}
                cmd={pane.type === "agent" ? "claude" : undefined}
              />
            </div>
          </div>
        ))}

        {panes.length === 0 && (
          <div className="empty-state" style={{ gridColumn: "1 / -1" }}>
            <p>No panes open</p>
            <p style={{ fontSize: 12 }}>Add a terminal or agent pane above</p>
          </div>
        )}
      </div>
    </div>
  );
}
