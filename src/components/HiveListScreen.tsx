import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { HiveInfo } from "../types";

interface Props {
  beehiveDir: string;
  onSelectHive: (hive: HiveInfo) => void;
  onSettings: () => void;
  onHelp: () => void;
  onBack?: () => void;
  backLabel?: string;
}

export function HiveListScreen({ beehiveDir, onSelectHive, onSettings, onHelp, onBack, backLabel }: Props) {
  const [hives, setHives] = useState<HiveInfo[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [repoUrl, setRepoUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  // Simple confirm delete: track which dirName is pending confirmation
  const [confirmDeleteDir, setConfirmDeleteDir] = useState<string | null>(null);

  useEffect(() => {
    loadHives();
  }, [beehiveDir]);

  async function loadHives() {
    try {
      const list = await invoke<HiveInfo[]>("list_hives", { beehiveDir });
      setHives(list);
    } catch (e) {
      console.error("Failed to list hives:", e);
    }
  }

  async function handleAdd() {
    const trimmed = repoUrl.trim();
    if (!trimmed) {
      setError("Please enter a repository URL");
      return;
    }
    if (!trimmed.includes("/") || trimmed.endsWith("/")) {
      setError("Use format: owner/repo, https://github.com/owner/repo, or git@github.com:owner/repo.git");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await invoke<HiveInfo>("create_hive", {
        beehiveDir,
        repoUrl: trimmed,
      });
      setRepoUrl("");
      setShowAdd(false);
      await loadHives();
    } catch (e) {
      setError(`${e}`);
    }
    setLoading(false);
  }

  const executeDelete = useCallback(async (dirName: string) => {
    setConfirmDeleteDir(null);
    try {
      await invoke("delete_hive", { beehiveDir, dirName });
      await loadHives();
    } catch (e) {
      console.error("Failed to delete hive:", e);
      setError(`Delete failed: ${e}`);
    }
  }, [beehiveDir]);

  return (
    <div className="screen-center">
      <div className="card" style={{ maxWidth: 600, width: "100%" }}>
        {onBack && (
          <button className="btn-text" onClick={onBack} style={{ marginBottom: 8 }}>
            &larr; {backLabel ?? "Back"}
          </button>
        )}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
          <h1>&#x2B21; Hives</h1>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-primary" onClick={() => { setShowAdd(!showAdd); setError(""); }}>
              + Add Hive
            </button>
            <button className="btn btn-secondary" onClick={onHelp} title="Help">
              ?
            </button>
            <button className="btn btn-secondary" onClick={onSettings} title="Settings">
              &#x2699;
            </button>
          </div>
        </div>

        {showAdd && (
          <div className="add-form" style={{ marginBottom: 20 }}>
            <div className="form-group">
              <label>Repository URL</label>
              <input
                type="text"
                value={repoUrl}
                onChange={(e) => { setRepoUrl(e.target.value); setError(""); }}
                placeholder="owner/repo or git@github.com:owner/repo.git"
                onKeyDown={(e) => e.key === "Enter" && !loading && handleAdd()}
                autoComplete="off"
                spellCheck={false}
                autoCorrect="off"
                autoCapitalize="off"
                data-form-type="other"
                autoFocus
              />
              <span className="form-hint">
                Accepts: owner/repo, GitHub HTTPS URL, or SSH URL
              </span>
            </div>
            {error && <div className="error-box">{error}</div>}
            <div style={{ display: "flex", gap: 8 }}>
              <button
                className="btn btn-primary"
                onClick={handleAdd}
                disabled={loading}
              >
                {loading ? "Verifying & adding..." : "Add"}
              </button>
              <button
                className="btn btn-secondary"
                onClick={() => { setShowAdd(false); setError(""); setRepoUrl(""); }}
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {hives.length === 0 && !showAdd && (
          <div className="empty-state">
            <p>No hives yet</p>
            <p style={{ fontSize: 12 }}>Add a repository to get started</p>
          </div>
        )}

        <div className="hive-list">
          {hives.map((hive) => (
            <div
              key={hive.dirName}
              className="hive-item"
              onClick={() => onSelectHive(hive)}
            >
              <div className="hive-item-info">
                <span className="hive-name">{hive.repoName}</span>
                <span className="hive-owner">{hive.owner}/{hive.repoName}</span>
                {hive.description && (
                  <span className="hive-desc">{hive.description}</span>
                )}
              </div>
              {confirmDeleteDir === hive.dirName ? (
                <div className="delete-confirm-inline" onClick={(e) => e.stopPropagation()}>
                  <button
                    className="btn-sm btn-danger"
                    onClick={() => executeDelete(hive.dirName)}
                  >
                    Are you sure?
                  </button>
                  <button
                    className="btn-sm"
                    onClick={() => setConfirmDeleteDir(null)}
                  >
                    No
                  </button>
                </div>
              ) : (
                <button
                  className="btn-icon danger"
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirmDeleteDir(hive.dirName);
                  }}
                  title="Delete hive"
                >
                  X
                </button>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
