import { useState, useRef, useEffect } from "react";
import type { HiveInfo, Comb } from "../types";

interface Props {
  hives: HiveInfo[];
  activeHive: HiveInfo | null;
  combs: Comb[];
  activeCombId: string | null;
  onSelectHive: (hive: HiveInfo) => void;
  onSelectComb: (comb: Comb) => void;
  onNewComb: () => void;
  onManageHives: () => void;
  onSettings: () => void;
  onHelp: () => void;
  onDeleteComb: (combId: string) => void;
}

export function Sidebar({
  hives,
  activeHive,
  combs,
  activeCombId,
  onSelectHive,
  onSelectComb,
  onNewComb,
  onManageHives,
  onSettings,
  onHelp,
  onDeleteComb,
}: Props) {
  const [hiveDropdownOpen, setHiveDropdownOpen] = useState(false);
  const hiveDropdownRef = useRef<HTMLDivElement>(null);

  // Delete confirmation: just track which combId is in "confirm" state
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  // Close hive dropdown on outside click
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (hiveDropdownRef.current && !hiveDropdownRef.current.contains(e.target as Node)) {
        setHiveDropdownOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  return (
    <div className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-logo">&#x2B21; Beehive</div>
      </div>

      {/* Hive selector */}
      <div className="sidebar-section">
        <div className="sidebar-section-label">Hive</div>
        <div className="sidebar-hive-select" ref={hiveDropdownRef}>
          <button
            className="sidebar-hive-trigger"
            onClick={() => setHiveDropdownOpen(!hiveDropdownOpen)}
          >
            <span>{activeHive ? activeHive.repoName : "Select a hive"}</span>
            <span className="custom-select-arrow">{hiveDropdownOpen ? "\u25B2" : "\u25BC"}</span>
          </button>
          {hiveDropdownOpen && (
            <div className="sidebar-hive-dropdown">
              {hives.map((h) => (
                <div
                  key={h.dirName}
                  className={`sidebar-hive-option ${h.dirName === activeHive?.dirName ? "active" : ""}`}
                  onClick={() => {
                    onSelectHive(h);
                    setHiveDropdownOpen(false);
                  }}
                >
                  <span className="sidebar-hive-name">{h.repoName}</span>
                  <span className="sidebar-hive-owner">{h.owner}</span>
                </div>
              ))}
              {hives.length === 0 && (
                <div className="sidebar-hive-empty">No hives added</div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* Comb list */}
      <div className="sidebar-section sidebar-combs">
        <div className="sidebar-section-label">Combs</div>
        <div className="sidebar-comb-list">
          {combs.map((comb) => (
            <div
              key={comb.id}
              className={`sidebar-comb-item ${comb.id === activeCombId ? "active" : ""}`}
              onClick={() => onSelectComb(comb)}
            >
              <div className="sidebar-comb-info">
                <span className="sidebar-comb-name">{comb.name}</span>
                <span className="sidebar-comb-branch">{comb.branch}</span>
              </div>
              {confirmDeleteId === comb.id ? (
                <div className="delete-confirm-inline" onClick={(e) => e.stopPropagation()}>
                  <button
                    className="btn-sm btn-danger"
                    onClick={() => {
                      setConfirmDeleteId(null);
                      onDeleteComb(comb.id);
                    }}
                  >
                    Sure?
                  </button>
                  <button className="btn-sm" onClick={() => setConfirmDeleteId(null)}>
                    No
                  </button>
                </div>
              ) : (
                <button
                  className="sidebar-comb-delete"
                  onClick={(e) => {
                    e.stopPropagation();
                    setConfirmDeleteId(comb.id);
                  }}
                  title="Delete comb"
                >
                  x
                </button>
              )}
            </div>
          ))}
          {activeHive && combs.length === 0 && (
            <div className="sidebar-empty">No combs yet</div>
          )}
        </div>
        {activeHive && (
          <button className="sidebar-add-comb" onClick={onNewComb}>
            + New Comb
          </button>
        )}
      </div>

      {/* Footer */}
      <div className="sidebar-footer">
        <button className="sidebar-footer-btn" onClick={onManageHives}>
          Manage Hives
        </button>
        <button className="sidebar-footer-btn" onClick={onSettings}>
          Settings
        </button>
        <button className="sidebar-footer-btn" onClick={onHelp}>
          Help
        </button>
      </div>
    </div>
  );
}
