import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { HiveInfo } from "../types";

interface Props {
  beehiveDir: string;
  onSelectHive: (hive: HiveInfo) => void;
  onSettings: () => void;
}

export function HiveListScreen({ beehiveDir, onSelectHive, onSettings }: Props) {
  const [hives, setHives] = useState<HiveInfo[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [repoUrl, setRepoUrl] = useState("");
  const [loading, setLoading] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [error, setError] = useState("");

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
    // Basic client-side format check
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

  async function handleDelete(dirName: string, repoName: string) {
    if (!confirm(`Delete hive "${repoName}"? This removes all combs and workspaces.`)) {
      return;
    }
    setDeleting(dirName);
    try {
      await invoke("delete_hive", { beehiveDir, dirName });
      await loadHives();
    } catch (e) {
      console.error("Failed to delete hive:", e);
      setError(`Delete failed: ${e}`);
    }
    setDeleting(null);
  }

  return (
    <div className="screen-center">
      <div className="card" style={{ maxWidth: 600, width: "100%" }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
          <h1>&#x2B21; Hives</h1>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-primary" onClick={() => { setShowAdd(!showAdd); setError(""); }}>
              + Add Hive
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
              <button
                className="btn-icon danger"
                onClick={(e) => {
                  e.stopPropagation();
                  handleDelete(hive.dirName, hive.repoName);
                }}
                disabled={deleting === hive.dirName}
                title="Delete hive"
              >
                {deleting === hive.dirName ? "..." : "X"}
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
