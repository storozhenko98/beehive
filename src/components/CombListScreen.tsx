import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Comb, HiveInfo } from "../types";

interface RepoBranch {
  name: string;
  isDefault: boolean;
}

interface Props {
  beehiveDir: string;
  hive: HiveInfo;
  onBack: () => void;
  onSelectComb: (comb: Comb) => void;
}

export function CombListScreen({ beehiveDir, hive, onBack, onSelectComb }: Props) {
  const [combs, setCombs] = useState<Comb[]>([]);
  const [showAdd, setShowAdd] = useState(false);
  const [name, setName] = useState("");
  const [branch, setBranch] = useState("");
  const [branches, setBranches] = useState<RepoBranch[]>([]);
  const [branchesLoading, setBranchesLoading] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  // Custom dropdown state
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [dropdownFilter, setDropdownFilter] = useState("");
  const dropdownRef = useRef<HTMLDivElement>(null);
  const filterInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadCombs();
  }, [hive.dirName]);

  // Close dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  async function loadCombs() {
    try {
      const list = await invoke<Comb[]>("list_combs", {
        beehiveDir,
        dirName: hive.dirName,
      });
      setCombs(list);
    } catch (e) {
      console.error("Failed to list combs:", e);
    }
  }

  async function loadBranches() {
    setBranchesLoading(true);
    try {
      const list = await invoke<RepoBranch[]>("list_branches", {
        beehiveDir,
        dirName: hive.dirName,
      });
      setBranches(list);
      const defaultBranch = list.find((b) => b.isDefault);
      if (defaultBranch) {
        setBranch(defaultBranch.name);
      } else if (list.length > 0) {
        setBranch(list[0].name);
      }
    } catch (e) {
      console.error("Failed to list branches:", e);
    }
    setBranchesLoading(false);
  }

  function openAdd() {
    setShowAdd(true);
    setError("");
    setName("");
    setBranch("");
    setDropdownFilter("");
    loadBranches();
  }

  function selectBranch(branchName: string) {
    setBranch(branchName);
    setDropdownOpen(false);
    setDropdownFilter("");
  }

  function toggleDropdown() {
    const next = !dropdownOpen;
    setDropdownOpen(next);
    if (next) {
      setDropdownFilter("");
      setTimeout(() => filterInputRef.current?.focus(), 0);
    }
  }

  const filteredBranches = branches.filter((b) =>
    b.name.toLowerCase().includes(dropdownFilter.toLowerCase())
  );

  async function handleAdd() {
    if (!name.trim()) {
      setError("Comb name is required");
      return;
    }
    if (!branch) {
      setError("Please select a branch");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await invoke<Comb>("create_comb", {
        beehiveDir,
        dirName: hive.dirName,
        name: name.trim(),
        branch,
      });
      setName("");
      setBranch("");
      setShowAdd(false);
      await loadCombs();
    } catch (e) {
      setError(`${e}`);
    }
    setLoading(false);
  }

  async function handleDelete(combId: string) {
    try {
      await invoke("delete_comb", {
        beehiveDir,
        dirName: hive.dirName,
        combId,
      });
      await loadCombs();
    } catch (e) {
      console.error("Failed to delete comb:", e);
    }
  }

  return (
    <div className="screen-center">
      <div className="card" style={{ maxWidth: 600, width: "100%" }}>
        <div style={{ marginBottom: 24 }}>
          <button className="btn-text" onClick={onBack}>
            &larr; Back to Hives
          </button>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 8 }}>
            <div>
              <h1>{hive.repoName}</h1>
              <span style={{ color: "var(--text-muted)", fontSize: 12 }}>
                {hive.owner}/{hive.repoName}
              </span>
            </div>
            <button className="btn btn-primary" onClick={openAdd}>
              + New Comb
            </button>
          </div>
        </div>

        {showAdd && (
          <div className="add-form" style={{ marginBottom: 20 }}>
            <div className="form-group">
              <label>Comb name</label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. fix-auth, experiment-v2"
                autoComplete="off"
                spellCheck={false}
                autoFocus
              />
            </div>

            <div className="form-group">
              <label>Branch</label>
              <div className="custom-select" ref={dropdownRef}>
                <button
                  className="custom-select-trigger"
                  onClick={toggleDropdown}
                  type="button"
                >
                  <span className={branch ? "" : "placeholder"}>
                    {branchesLoading
                      ? "Loading branches..."
                      : branch || "Select a branch"}
                  </span>
                  <span className="custom-select-arrow">{dropdownOpen ? "\u25B2" : "\u25BC"}</span>
                </button>

                {dropdownOpen && (
                  <div className="custom-select-dropdown">
                    <div className="custom-select-search">
                      <input
                        ref={filterInputRef}
                        type="text"
                        value={dropdownFilter}
                        onChange={(e) => setDropdownFilter(e.target.value)}
                        placeholder="Filter branches..."
                        autoComplete="off"
                        spellCheck={false}
                        onKeyDown={(e) => {
                          if (e.key === "Escape") setDropdownOpen(false);
                          if (e.key === "Enter" && filteredBranches.length === 1) {
                            selectBranch(filteredBranches[0].name);
                          }
                        }}
                      />
                    </div>
                    <div className="custom-select-options">
                      {filteredBranches.length === 0 && (
                        <div className="custom-select-empty">No branches found</div>
                      )}
                      {filteredBranches.map((b) => (
                        <div
                          key={b.name}
                          className={`custom-select-option ${b.name === branch ? "selected" : ""}`}
                          onClick={() => selectBranch(b.name)}
                        >
                          <span className="branch-name">{b.name}</span>
                          {b.isDefault && <span className="branch-default-tag">default</span>}
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </div>

            {error && <div className="error-box">{error}</div>}
            <div style={{ display: "flex", gap: 8 }}>
              <button className="btn btn-primary" onClick={handleAdd} disabled={loading}>
                {loading ? "Cloning..." : "Create Comb"}
              </button>
              <button className="btn btn-secondary" onClick={() => setShowAdd(false)}>
                Cancel
              </button>
            </div>
          </div>
        )}

        {combs.length === 0 && !showAdd && (
          <div className="empty-state">
            <p>No combs yet</p>
            <p style={{ fontSize: 12 }}>Create a comb to start working</p>
          </div>
        )}

        <div className="comb-list">
          {combs.map((comb) => (
            <div
              key={comb.id}
              className="comb-item"
              onClick={() => onSelectComb(comb)}
            >
              <div className="comb-item-info">
                <span className="comb-name">{comb.name}</span>
                <span className="comb-branch">{comb.branch}</span>
              </div>
              <button
                className="btn-icon danger"
                onClick={(e) => {
                  e.stopPropagation();
                  if (confirm(`Delete comb "${comb.name}"? This removes the workspace.`)) {
                    handleDelete(comb.id);
                  }
                }}
                title="Delete comb"
              >
                X
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
