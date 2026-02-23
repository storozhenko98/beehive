import { TerminalPane } from "./TerminalPane";
import type { Comb, PaneConfig } from "../types";

interface Props {
  comb: Comb;
  panes: PaneConfig[];
  isVisible: boolean;
  onAddPane: (type: "agent" | "terminal") => void;
  onRemovePane: (id: string) => void;
}

export function WorkspaceGrid({ comb, panes, isVisible, onAddPane, onRemovePane }: Props) {
  const cols = panes.length <= 1 ? 1 : panes.length <= 4 ? 2 : 3;

  return (
    <div className="workspace-container" style={{ display: isVisible ? "flex" : "none" }}>
      <div className="workspace-header">
        <div className="workspace-title">
          <strong>{comb.name}</strong>
          <span className="workspace-branch">{comb.branch}</span>
        </div>
        <div className="workspace-actions">
          <button className="btn btn-sm" onClick={() => onAddPane("terminal")}>
            + Terminal
          </button>
          <button className="btn btn-sm" onClick={() => onAddPane("agent")}>
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
                onClick={() => onRemovePane(pane.id)}
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
                isVisible={isVisible}
                onExit={() => onRemovePane(pane.id)}
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
