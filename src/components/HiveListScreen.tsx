import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Comb, HiveInfo } from "../types";

type AddMode = "existing" | "new";

interface FreshHiveResult {
  hive: HiveInfo;
  comb: Comb;
}

function validateFreshRepoSpec(value: string): string {
  const cleaned = value.trim().replace(/\.git$/, "").replace(/\/$/, "");
  if (!cleaned) return "Repository name is required";
  if (cleaned.startsWith("git@") || cleaned.includes("github.com/")) return "";

  const parts = cleaned.split("/");
  if (parts.length > 2 || parts.some((part) => part.length === 0)) {
    return "Use repo-name or owner/repo";
  }
  if (parts.length === 2 && !/^[a-zA-Z0-9-]+$/.test(parts[0])) {
    return "Owner can only contain letters, numbers, and hyphens";
  }

  const repoName = parts[parts.length - 1];
  if (!/^[a-zA-Z0-9._-]+$/.test(repoName)) {
    return "Repo name can only contain letters, numbers, dots, hyphens, and underscores";
  }
  if (repoName === "." || repoName === "..") return "Repository name is reserved";
  return "";
}

function validateCombName(name: string): string {
  if (!name) return "Comb name is required";
  if (name.length > 40) return "Comb name must be 40 characters or fewer";
  if (name.startsWith(".") || name.startsWith("-")) return "Comb name cannot start with '.' or '-'";
  if (!/^[a-zA-Z0-9_-]+$/.test(name)) {
    return "Comb name can only contain letters, numbers, hyphens, and underscores";
  }
  return "";
}

function validateBranchName(name: string): string {
  if (!name) return "Branch name is required";
  if (
    name.startsWith("-") ||
    name.includes("..") ||
    name.endsWith(".lock") ||
    /\s/.test(name) ||
    /[~^:?*[\\]/.test(name)
  ) {
    return "Branch name is not valid for git";
  }
  return "";
}

interface Props {
  beehiveDir: string;
  onSelectHive: (hive: HiveInfo) => void;
  onSettings: () => void;
  onHelp: () => void;
  onBack?: () => void;
  backLabel?: string;
  hivesDeleting?: Set<string>;
  onDeleteHive?: (dirName: string) => void;
}

export function HiveListScreen({ beehiveDir, onSelectHive, onSettings, onHelp, onBack, backLabel, hivesDeleting, onDeleteHive }: Props) {
  const [hives, setHives] = useState<HiveInfo[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [addMode, setAddMode] = useState<AddMode>("existing");
  const [repoUrl, setRepoUrl] = useState("");
  const [newRepoSpec, setNewRepoSpec] = useState("");
  const [newRepoDescription, setNewRepoDescription] = useState("");
  const [newRepoPrivate, setNewRepoPrivate] = useState(true);
  const [newCombName, setNewCombName] = useState("main");
  const [newBranch, setNewBranch] = useState("main");
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

  async function handleAddExisting() {
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

  async function handleCreateFresh() {
    const repoSpec = newRepoSpec.trim();
    const combName = newCombName.trim();
    const branch = newBranch.trim();
    const validation =
      validateFreshRepoSpec(repoSpec) ||
      validateCombName(combName) ||
      validateBranchName(branch);
    if (validation) {
      setError(validation);
      return;
    }

    setLoading(true);
    setError("");
    try {
      const result = await invoke<FreshHiveResult>("create_fresh_hive", {
        beehiveDir,
        repoSpec,
        description: newRepoDescription.trim(),
        private: newRepoPrivate,
        combName,
        branch,
      });
      setNewRepoSpec("");
      setNewRepoDescription("");
      setNewRepoPrivate(true);
      setNewCombName("main");
      setNewBranch("main");
      setShowAdd(false);
      await loadHives();
      onSelectHive(result.hive);
    } catch (e) {
      setError(`${e}`);
    }
    setLoading(false);
  }

  function handleSubmitAdd() {
    if (addMode === "existing") {
      void handleAddExisting();
    } else {
      void handleCreateFresh();
    }
  }

  const executeDelete = useCallback(async (dirName: string) => {
    setConfirmDeleteDir(null);
    
    // Use async deletion if handler provided
    if (onDeleteHive) {
      onDeleteHive(dirName);
      return;
    }
    
    // Fallback to synchronous deletion
    try {
      await invoke("delete_hive", { beehiveDir, dirName });
      await loadHives();
    } catch (e) {
      console.error("Failed to delete hive:", e);
      setError(`Delete failed: ${e}`);
    }
  }, [beehiveDir, onDeleteHive]);

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
            <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
              <button
                className={`btn ${addMode === "existing" ? "btn-primary" : "btn-secondary"}`}
                onClick={() => { setAddMode("existing"); setError(""); }}
                disabled={loading}
              >
                Existing Repo
              </button>
              <button
                className={`btn ${addMode === "new" ? "btn-primary" : "btn-secondary"}`}
                onClick={() => { setAddMode("new"); setError(""); }}
                disabled={loading}
              >
                New GitHub Repo
              </button>
            </div>

            {addMode === "existing" ? (
              <div className="form-group">
                <label>Repository URL</label>
                <input
                  type="text"
                  value={repoUrl}
                  onChange={(e) => { setRepoUrl(e.target.value); setError(""); }}
                  placeholder="owner/repo or git@github.com:owner/repo.git"
                  onKeyDown={(e) => e.key === "Enter" && !loading && handleSubmitAdd()}
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
            ) : (
              <>
                <div className="form-group">
                  <label>New Repository</label>
                  <input
                    type="text"
                    value={newRepoSpec}
                    onChange={(e) => { setNewRepoSpec(e.target.value); setError(""); }}
                    placeholder="repo-name or owner/repo"
                    onKeyDown={(e) => e.key === "Enter" && !loading && handleSubmitAdd()}
                    autoComplete="off"
                    spellCheck={false}
                    autoCorrect="off"
                    autoCapitalize="off"
                    data-form-type="other"
                    autoFocus
                  />
                  <span className="form-hint">
                    Creates the GitHub repo, initializes a local comb, commits README.md, and pushes the branch.
                  </span>
                </div>
                <div className="form-group">
                  <label>Description</label>
                  <input
                    type="text"
                    value={newRepoDescription}
                    onChange={(e) => setNewRepoDescription(e.target.value)}
                    placeholder="Optional"
                    onKeyDown={(e) => e.key === "Enter" && !loading && handleSubmitAdd()}
                  />
                </div>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
                  <div className="form-group">
                    <label>Initial Comb</label>
                    <input
                      type="text"
                      value={newCombName}
                      onChange={(e) => { setNewCombName(e.target.value); setError(""); }}
                      onKeyDown={(e) => e.key === "Enter" && !loading && handleSubmitAdd()}
                    />
                  </div>
                  <div className="form-group">
                    <label>Branch</label>
                    <input
                      type="text"
                      value={newBranch}
                      onChange={(e) => { setNewBranch(e.target.value); setError(""); }}
                      onKeyDown={(e) => e.key === "Enter" && !loading && handleSubmitAdd()}
                    />
                  </div>
                </div>
                <label className="checkbox-row">
                  <input
                    type="checkbox"
                    checked={newRepoPrivate}
                    onChange={(e) => setNewRepoPrivate(e.target.checked)}
                  />
                  Private repository
                </label>
              </>
            )}
            {error && <div className="error-box">{error}</div>}
            <div style={{ display: "flex", gap: 8 }}>
              <button
                className="btn btn-primary"
                onClick={handleSubmitAdd}
                disabled={loading}
              >
                {loading
                  ? (addMode === "existing" ? "Verifying & adding..." : "Creating repo...")
                  : (addMode === "existing" ? "Add" : "Create Repo")}
              </button>
              <button
                className="btn btn-secondary"
                onClick={() => {
                  setShowAdd(false);
                  setError("");
                  setRepoUrl("");
                  setNewRepoSpec("");
                  setNewRepoDescription("");
                  setNewCombName("main");
                  setNewBranch("main");
                }}
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
          {hives.map((hive) => {
            const isDeleting = hivesDeleting?.has(hive.dirName);
            return (
              <div
                key={hive.dirName}
                className={`hive-item ${isDeleting ? "hive-item-deleting" : ""}`}
                onClick={() => !isDeleting && onSelectHive(hive)}
                style={isDeleting ? { opacity: 0.5, pointerEvents: "none" } : undefined}
              >
                <div className="hive-item-info">
                  <span className="hive-name">{hive.repoName}</span>
                  <span className="hive-owner">{hive.owner}/{hive.repoName}</span>
                  {hive.description && (
                    <span className="hive-desc">{hive.description}</span>
                  )}
                  {isDeleting && (
                    <span className="hive-status">Deleting...</span>
                  )}
                </div>
                {isDeleting ? null : confirmDeleteDir === hive.dirName ? (
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
            );
          })}
        </div>
      </div>
    </div>
  );
}
