import { useState } from "react";
import type { CustomButton } from "../types";

interface Suggestion extends CustomButton {
  hiveName: string;
}

interface Props {
  buttons: CustomButton[];
  suggestions: Suggestion[];
  onSave: (buttons: CustomButton[]) => void;
  onClose: () => void;
}

export function CustomButtonsModal({ buttons, suggestions, onSave, onClose }: Props) {
  const [items, setItems] = useState<CustomButton[]>([...buttons]);
  const [label, setLabel] = useState("");
  const [cmd, setCmd] = useState("");
  const [error, setError] = useState("");

  const canAdd = items.length < 2;

  function handleAdd() {
    const trimmedLabel = label.trim();
    const trimmedCmd = cmd.trim();
    if (!trimmedLabel) {
      setError("Label is required");
      return;
    }
    if (trimmedLabel.length > 10) {
      setError("Label must be 10 characters or fewer");
      return;
    }
    if (!trimmedCmd) {
      setError("Command is required");
      return;
    }
    setError("");
    setItems([...items, { label: trimmedLabel, cmd: trimmedCmd }]);
    setLabel("");
    setCmd("");
  }

  function handleRemove(index: number) {
    setItems(items.filter((_, i) => i !== index));
  }

  function handleSuggestionClick(s: Suggestion) {
    setLabel(s.label);
    setCmd(s.cmd);
  }

  function handleSave() {
    onSave(items);
  }

  // Filter suggestions: exclude buttons already in items
  const filteredSuggestions = suggestions.filter(
    (s) => !items.some((b) => b.label === s.label && b.cmd === s.cmd)
  );

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
          <h1>Custom Buttons</h1>
          <button className="close-btn" onClick={onClose} style={{ fontSize: 16, padding: "4px 8px" }}>x</button>
        </div>

        {items.length > 0 && (
          <div className="custom-buttons-list">
            {items.map((item, i) => (
              <div key={i} className="custom-button-item">
                <div className="custom-button-item-info">
                  <span className="custom-button-item-label">{item.label}</span>
                  <span className="custom-button-item-cmd">{item.cmd}</span>
                </div>
                <button
                  className="close-btn"
                  onClick={() => handleRemove(i)}
                  title="Remove"
                >
                  x
                </button>
              </div>
            ))}
          </div>
        )}

        {items.length === 0 && (
          <p style={{ color: "var(--text-muted)", fontSize: 12, marginBottom: 16 }}>
            No custom buttons configured. Add up to 2 buttons that appear in the workspace header.
          </p>
        )}

        {canAdd && (
          <div className="add-form">
            <div className="form-group">
              <label>Label</label>
              <input
                type="text"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="e.g. Agent"
                maxLength={10}
                autoComplete="off"
                spellCheck={false}
                autoFocus
                onKeyDown={(e) => e.key === "Enter" && handleAdd()}
              />
            </div>
            <div className="form-group">
              <label>Command</label>
              <input
                type="text"
                value={cmd}
                onChange={(e) => setCmd(e.target.value)}
                placeholder="e.g. claude, opencode, nix develop && bun i"
                autoComplete="off"
                spellCheck={false}
                onKeyDown={(e) => e.key === "Enter" && handleAdd()}
              />
            </div>
            {error && <div className="error-box">{error}</div>}
            <button className="btn btn-sm" onClick={handleAdd}>
              + Add
            </button>

            {filteredSuggestions.length > 0 && (
              <div className="previously-used-section">
                <h3>Previously used</h3>
                {filteredSuggestions.map((s, i) => (
                  <div
                    key={i}
                    className="previously-used-item"
                    onClick={() => handleSuggestionClick(s)}
                  >
                    <span className="previously-used-item-label">{s.label}</span>
                    <span className="previously-used-item-cmd">{s.cmd}</span>
                    <span className="previously-used-item-source">{s.hiveName}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        <div style={{ display: "flex", gap: 8, marginTop: 20 }}>
          <button className="btn btn-primary" onClick={handleSave}>
            Save
          </button>
          <button className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
