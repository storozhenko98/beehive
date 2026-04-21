import { useState } from "react";
import type { Nest } from "../types";

interface Props {
  existingNests: Nest[];
  onCreate: (name: string) => Promise<void>;
  onClose: () => void;
}

function validateNestName(name: string, existingNests: Nest[]): string {
  const trimmed = name.trim();
  if (!trimmed) return "Nest name is required";
  if (trimmed.length > 40) return "Max 40 characters";
  if (
    existingNests.some(
      (nest) => nest.name.trim().toLowerCase() === trimmed.toLowerCase()
    )
  ) {
    return `'${trimmed}' already exists`;
  }
  return "";
}

export function CreateNestModal({ existingNests, onCreate, onClose }: Props) {
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [submitted, setSubmitted] = useState(false);

  const validationError = validateNestName(name, existingNests);
  const showValidation = submitted || name.trim().length > 0;

  async function handleCreate() {
    setSubmitted(true);
    if (validationError) {
      return;
    }

    setSaving(true);
    setError("");
    try {
      await onCreate(name.trim());
    } catch (e) {
      setError(`${e}`);
      setSaving(false);
    }
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
          <h1>New Nest</h1>
          <button className="close-btn" onClick={onClose} style={{ fontSize: 16, padding: "4px 8px" }}>
            x
          </button>
        </div>

        <div className="form-group">
          <label>Nest name</label>
          <input
            type="text"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setSubmitted(false);
              if (error) setError("");
            }}
            placeholder="e.g. bugs, clients, experiments"
            autoFocus
            maxLength={40}
            spellCheck={false}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !saving) {
                e.preventDefault();
                void handleCreate();
              }
            }}
          />
          {showValidation && validationError && (
            <span style={{ fontSize: 11, color: "var(--danger)", marginTop: 4, display: "block" }}>
              {validationError}
            </span>
          )}
        </div>

        {error && <div className="error-box">{error}</div>}
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-primary" onClick={() => void handleCreate()} disabled={saving}>
            {saving ? "Creating..." : "Create Nest"}
          </button>
          <button className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
