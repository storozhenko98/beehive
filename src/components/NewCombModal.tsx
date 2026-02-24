import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Comb, HiveInfo } from "../types";

interface RepoBranch {
  name: string;
  isDefault: boolean;
}

interface Props {
  beehiveDir: string;
  hive: HiveInfo;
  existingNames: string[];
  onCreated: (comb: Comb) => void;
  onClose: () => void;
}

function validateCombName(name: string, existingNames: string[]): string {
  if (!name) return "";
  if (name.length > 40) return "Max 40 characters";
  if (name.startsWith(".") || name.startsWith("-")) return "Cannot start with '.' or '-'";
  if (!/^[a-zA-Z0-9_-]+$/.test(name)) return "Only letters, numbers, hyphens, underscores";
  if (existingNames.includes(name)) return `'${name}' already exists`;
  return "";
}

export function NewCombModal({ beehiveDir, hive, existingNames, onCreated, onClose }: Props) {
  const [name, setName] = useState("");
  const [branch, setBranch] = useState("");
  const [branches, setBranches] = useState<RepoBranch[]>([]);
  const [branchesLoading, setBranchesLoading] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  // Custom dropdown state
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [dropdownFilter, setDropdownFilter] = useState("");
  const [dropdownPos, setDropdownPos] = useState<{ top: number; left: number; width: number; maxHeight: number } | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const filterInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadBranches();
  }, [hive.dirName]);

  // Close dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      const target = e.target as Node;
      if (
        dropdownRef.current && !dropdownRef.current.contains(target) &&
        triggerRef.current && !triggerRef.current.contains(target)
      ) {
        setDropdownOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const updateDropdownPos = useCallback(() => {
    if (triggerRef.current) {
      const rect = triggerRef.current.getBoundingClientRect();
      const viewportPadding = 12;
      const maxHeight = Math.max(150, window.innerHeight - rect.bottom - 2 - viewportPadding);
      setDropdownPos({
        top: rect.bottom + 2,
        left: rect.left,
        width: rect.width,
        maxHeight,
      });
    }
  }, []);

  // Reposition on scroll/resize while open
  useEffect(() => {
    if (!dropdownOpen) return;
    updateDropdownPos();
    window.addEventListener("resize", updateDropdownPos);
    return () => window.removeEventListener("resize", updateDropdownPos);
  }, [dropdownOpen, updateDropdownPos]);

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

  const nameValidation = validateCombName(name.trim(), existingNames);

  async function handleCreate() {
    if (!name.trim()) {
      setError("Comb name is required");
      return;
    }
    if (nameValidation) {
      setError(nameValidation);
      return;
    }
    if (!branch) {
      setError("Please select a branch");
      return;
    }
    setLoading(true);
    setError("");
    try {
      const comb = await invoke<Comb>("create_comb_start", {
        beehiveDir,
        dirName: hive.dirName,
        name: name.trim(),
        branch,
      });
      onCreated(comb);
    } catch (e) {
      setError(`${e}`);
      setLoading(false);
    }
  }

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
          <h1>New Comb</h1>
          <button className="close-btn" onClick={onClose} style={{ fontSize: 16, padding: "4px 8px" }}>x</button>
        </div>

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
            maxLength={40}
            onKeyDown={(e) => e.key === "Enter" && !loading && handleCreate()}
          />
          {nameValidation && (
            <span style={{ fontSize: 11, color: "var(--danger)", marginTop: 4, display: "block" }}>
              {nameValidation}
            </span>
          )}
        </div>

        <div className="form-group">
          <label>Branch</label>
          <div className="custom-select">
            <button
              ref={triggerRef}
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
          </div>
        </div>

        {error && <div className="error-box">{error}</div>}
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn btn-primary" onClick={handleCreate} disabled={loading || !!nameValidation}>
            {loading ? "Creating..." : "Create Comb"}
          </button>
          <button className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>

      {dropdownOpen && dropdownPos && (
        <div
          ref={dropdownRef}
          className="custom-select-dropdown-fixed"
          style={{
            top: dropdownPos.top,
            left: dropdownPos.left,
            width: dropdownPos.width,
          }}
          onClick={(e) => e.stopPropagation()}
        >
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
          <div className="custom-select-options" style={{ maxHeight: Math.max(120, dropdownPos.maxHeight - 75) }}>
            {filteredBranches.length === 0 && (
              <div className="custom-select-empty">No branches found</div>
            )}
            {filteredBranches.map((b) => (
              <div
                key={b.name}
                className={`custom-select-option ${b.name === branch ? "selected" : ""}`}
                onClick={() => selectBranch(b.name)}
                title={b.name}
              >
                <span className="branch-name">{b.name}</span>
                {b.isDefault && <span className="branch-default-tag">default</span>}
              </div>
            ))}
          </div>
          {branches.length > 0 && (
            <div className="custom-select-count">
              {filteredBranches.length} of {branches.length} branches
            </div>
          )}
        </div>
      )}
    </div>
  );
}
