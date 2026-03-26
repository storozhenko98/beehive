import { useState, useRef, useEffect } from "react";
import type { HiveInfo, Comb } from "../types";
import { useSortable } from "../hooks/useSortable";

function validateCombName(name: string, currentCombId: string, combs: Comb[]): string {
  if (!name) return "Name is required";
  if (name.length > 40) return "Max 40 characters";
  if (name.startsWith(".") || name.startsWith("-")) return "Cannot start with '.' or '-'";
  if (!/^[a-zA-Z0-9_-]+$/.test(name)) return "Only letters, numbers, hyphens, underscores";
  if (combs.some((comb) => comb.id !== currentCombId && comb.name === name)) {
    return `'${name}' already exists`;
  }
  return "";
}

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
  onRenameComb: (combId: string, newName: string) => Promise<void>;
  onCopyComb: (combId: string) => void;
  onReorderCombs: (combIds: string[]) => void;
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
  onRenameComb,
  onCopyComb,
  onReorderCombs,
}: Props) {
  const [hiveDropdownOpen, setHiveDropdownOpen] = useState(false);
  const hiveDropdownRef = useRef<HTMLDivElement>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [editingCombId, setEditingCombId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState("");
  const [renameError, setRenameError] = useState("");
  const [savingRename, setSavingRename] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ combId: string; x: number; y: number } | null>(null);
  const contextMenuRef = useRef<HTMLDivElement>(null);

  const { isDragging, getItemProps } = useSortable(combs, onReorderCombs);

  function startRename(comb: Comb) {
    setConfirmDeleteId(null);
    setContextMenu(null);
    setEditingCombId(comb.id);
    setEditingName(comb.name);
    setRenameError("");
  }

  function stopRename() {
    setEditingCombId(null);
    setEditingName("");
    setRenameError("");
    setSavingRename(false);
  }

  async function submitRename(comb: Comb) {
    const trimmedName = editingName.trim();
    if (trimmedName === comb.name) {
      stopRename();
      return;
    }

    const error = validateCombName(trimmedName, comb.id, combs);
    if (error) {
      setRenameError(error);
      return;
    }

    setSavingRename(true);
    setRenameError("");
    try {
      await onRenameComb(comb.id, trimmedName);
      stopRename();
    } catch (e) {
      setRenameError(`${e}`);
      setSavingRename(false);
    }
  }

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (hiveDropdownRef.current && !hiveDropdownRef.current.contains(e.target as Node)) {
        setHiveDropdownOpen(false);
      }
      if (contextMenuRef.current && !contextMenuRef.current.contains(e.target as Node)) {
        setContextMenu(null);
      }
    }

    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  useEffect(() => {
    if (editingCombId && !combs.some((comb) => comb.id === editingCombId)) {
      stopRename();
    }
  }, [combs, editingCombId]);

  return (
    <div className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-logo">&#x2B21; Beehive</div>
      </div>

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
              {hives.length === 0 && <div className="sidebar-hive-empty">No hives added</div>}
            </div>
          )}
        </div>
      </div>

      <div className="sidebar-section sidebar-combs">
        <div className="sidebar-section-label">Combs</div>
        <div className={`sidebar-comb-list ${isDragging ? "is-dragging" : ""}`}>
          {combs.map((comb, idx) => {
            const { ref, onPointerDown, style, isDragged } = getItemProps(idx);

            const hasOperation = !!comb.operation || comb.cloning;
            const operationText = comb.operation === "cloning" || comb.cloning
              ? "Cloning..."
              : comb.operation === "copying"
              ? "Copying..."
              : comb.operation === "deleting"
              ? "Deleting..."
              : null;
            const isDeleting = comb.operation === "deleting";
            const canInteract = !hasOperation;
            const canCopy = canInteract && !isDeleting;
            const canRename = canInteract && !isDeleting;

            return (
              <div
                key={comb.id}
                ref={ref}
                className={`sidebar-comb-item ${comb.id === activeCombId ? "active" : ""} ${hasOperation ? "has-operation" : ""} ${isDeleting ? "deleting" : ""} ${isDragged ? "dragging" : ""}`}
                style={style}
                onPointerDown={(e) => {
                  if (canInteract && editingCombId !== comb.id) onPointerDown(e);
                }}
                onClick={() => {
                  if (canInteract && !isDragging && editingCombId !== comb.id) onSelectComb(comb);
                }}
                onContextMenu={(e) => {
                  if (!canRename) return;
                  e.preventDefault();
                  setConfirmDeleteId(null);
                  setContextMenu({ combId: comb.id, x: e.clientX, y: e.clientY });
                }}
              >
                <div className="sidebar-comb-info">
                  {editingCombId === comb.id ? (
                    <>
                      <input
                        className={`sidebar-comb-name-input ${renameError ? "error" : ""}`}
                        type="text"
                        value={editingName}
                        onChange={(e) => {
                          setEditingName(e.target.value);
                          if (renameError) setRenameError("");
                        }}
                        onClick={(e) => e.stopPropagation()}
                        onPointerDown={(e) => e.stopPropagation()}
                        onDoubleClick={(e) => e.stopPropagation()}
                        onBlur={() => {
                          if (!savingRename) {
                            const trimmedName = editingName.trim();
                            if (trimmedName === comb.name) {
                              stopRename();
                            } else {
                              void submitRename(comb);
                            }
                          }
                        }}
                        onKeyDown={(e) => {
                          e.stopPropagation();
                          if (e.key === "Enter") {
                            e.preventDefault();
                            void submitRename(comb);
                          }
                          if (e.key === "Escape") {
                            e.preventDefault();
                            stopRename();
                          }
                        }}
                        autoFocus
                        maxLength={40}
                        spellCheck={false}
                      />
                      {renameError ? (
                        <span className="sidebar-comb-error">{renameError}</span>
                      ) : (
                        <span className="sidebar-comb-branch">{comb.branch}</span>
                      )}
                    </>
                  ) : (
                    <>
                      <span
                        className="sidebar-comb-name"
                        onDoubleClick={(e) => {
                          if (!canRename) return;
                          e.stopPropagation();
                          startRename(comb);
                        }}
                      >
                        {comb.name}
                      </span>
                      {operationText ? (
                        <span className="sidebar-comb-operation">{operationText}</span>
                      ) : (
                        <span className="sidebar-comb-branch">{comb.branch}</span>
                      )}
                    </>
                  )}
                </div>
                {canInteract && editingCombId !== comb.id && (
                  <>
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
                      <div className="sidebar-comb-actions">
                        {canCopy && (
                          <button
                            className="sidebar-comb-copy"
                            onClick={(e) => {
                              e.stopPropagation();
                              onCopyComb(comb.id);
                            }}
                            title="Copy comb"
                          >
                            &#x2398;
                          </button>
                        )}
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
                      </div>
                    )}
                  </>
                )}
              </div>
            );
          })}
          {activeHive && combs.length === 0 && <div className="sidebar-empty">No combs yet</div>}
        </div>
        {activeHive && (
          <button className="sidebar-add-comb" onClick={onNewComb}>
            + New Comb
          </button>
        )}
      </div>

      {contextMenu && (
        <div
          ref={contextMenuRef}
          className="sidebar-context-menu"
          style={{ top: contextMenu.y, left: contextMenu.x }}
        >
          <button
            className="sidebar-context-menu-item"
            onClick={() => {
              const comb = combs.find((item) => item.id === contextMenu.combId);
              if (comb) startRename(comb);
            }}
          >
            Rename
          </button>
        </div>
      )}

      <div className="sidebar-footer">
        {(() => {
          const activeOps = combs.filter((c) => c.operation || c.cloning).length;
          if (activeOps > 0) {
            return (
              <div className="sidebar-ops-indicator">
                {activeOps} operation{activeOps > 1 ? "s" : ""} running
              </div>
            );
          }
          return null;
        })()}
        <div className="sidebar-footer-buttons">
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
    </div>
  );
}
