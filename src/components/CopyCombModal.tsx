import { useState } from "react";

interface Props {
  sourceCombName: string;
  existingNames: string[];
  onCopy: (newName: string) => void;
  onClose: () => void;
  error: string;
}

function validateName(name: string, existingNames: string[]): string {
  if (!name) return "";
  if (name.length > 40) return "Max 40 characters";
  if (name.startsWith(".") || name.startsWith("-")) return "Cannot start with '.' or '-'";
  if (!/^[a-zA-Z0-9_-]+$/.test(name)) return "Only letters, numbers, hyphens, underscores";
  if (existingNames.includes(name)) return `'${name}' already exists`;
  return "";
}

export function CopyCombModal({ sourceCombName, existingNames, onCopy, onClose, error }: Props) {
  const [name, setName] = useState("");

  const validationError = validateName(name, existingNames);
  const canSubmit = name.length > 0 && !validationError;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
          <h1>Copy Comb</h1>
          <button className="close-btn" onClick={onClose} style={{ fontSize: 16, padding: "4px 8px" }}>x</button>
        </div>

        <p style={{ fontSize: 12, color: "var(--text-secondary)", marginBottom: 16 }}>
          Copying <strong>{sourceCombName}</strong>
        </p>

        <div className="form-group">
          <label>New comb name</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. fix-auth-copy, experiment-v2"
            autoComplete="off"
            spellCheck={false}
            autoFocus
            maxLength={40}
            onKeyDown={(e) => e.key === "Enter" && canSubmit && onCopy(name)}
          />
          {validationError && (
            <span style={{ fontSize: 11, color: "var(--danger)", marginTop: 4, display: "block" }}>
              {validationError}
            </span>
          )}
        </div>

        {error && <div className="error-box">{error}</div>}
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-primary" onClick={() => onCopy(name)} disabled={!canSubmit}>
            Copy Comb
          </button>
          <button className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
