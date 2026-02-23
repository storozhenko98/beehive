import { TerminalPane } from "./TerminalPane";
import type { Comb, PaneConfig, CustomButton } from "../types";

interface Props {
  comb: Comb;
  panes: PaneConfig[];
  customButtons: CustomButton[];
  isVisible: boolean;
  onAddPane: (cmd?: string) => void;
  onRemovePane: (id: string) => void;
  onConfigureButtons: () => void;
}

export function WorkspaceGrid({ comb, panes, customButtons, isVisible, onAddPane, onRemovePane, onConfigureButtons }: Props) {
  const cols = panes.length <= 1 ? 1 : panes.length <= 4 ? 2 : 3;

  return (
    <div className="workspace-container" style={{ display: isVisible ? "flex" : "none" }}>
      <div className="workspace-header">
        <div className="workspace-title">
          <strong>{comb.name}</strong>
          <span className="workspace-branch">{comb.branch}</span>
        </div>
        <div className="workspace-actions">
          <button className="btn btn-sm" onClick={() => onAddPane()}>
            + Terminal
          </button>
          {customButtons.map((btn, i) => (
            <button key={i} className="btn btn-sm" onClick={() => onAddPane(btn.cmd)}>
              + {btn.label}
            </button>
          ))}
          <button
            className="workspace-configure-btn"
            onClick={onConfigureButtons}
            title="Configure buttons"
          >
            &#9881;
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
                cmd={pane.cmd}
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
