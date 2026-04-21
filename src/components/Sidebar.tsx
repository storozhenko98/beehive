import { useState, useRef, useEffect } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import type { HiveInfo, Comb, Nest } from "../types";
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

function validateNestName(name: string, currentNestId: string, nests: Nest[]): string {
  const trimmed = name.trim();
  if (!trimmed) return "Nest name is required";
  if (trimmed.length > 40) return "Max 40 characters";
  if (
    nests.some(
      (nest) =>
        nest.id !== currentNestId &&
        nest.name.trim().toLowerCase() === trimmed.toLowerCase()
    )
  ) {
    return `'${trimmed}' already exists`;
  }
  return "";
}

interface Props {
  hives: HiveInfo[];
  activeHive: HiveInfo | null;
  nests: Nest[];
  combs: Comb[];
  activeCombId: string | null;
  onSelectHive: (hive: HiveInfo) => void;
  onSelectComb: (comb: Comb) => void;
  onNewComb: () => void;
  onManageHives: () => void;
  onSettings: () => void;
  onHelp: () => void;
  onNewNest: (combId?: string) => void;
  onAssignCombToNest: (combId: string, nestId?: string) => Promise<void>;
  onRenameNest: (nestId: string, newName: string) => Promise<void>;
  onDeleteNest: (nestId: string) => Promise<void>;
  onDeleteComb: (combId: string) => void;
  onRenameComb: (combId: string, newName: string) => Promise<void>;
  onCopyComb: (combId: string) => void;
  onReorderCombs: (combs: Comb[]) => void;
}

interface CombSection {
  key: string;
  label: string;
  combs: Comb[];
  collapsible: boolean;
  emptyLabel?: string;
}

function buildCombSections(combs: Comb[], nests: Nest[]): CombSection[] {
  const sections: CombSection[] = [];
  const nestIdSet = new Set(nests.map((nest) => nest.id));
  const ungroupedCombs = combs.filter((comb) => !comb.nestId || !nestIdSet.has(comb.nestId));

  if (ungroupedCombs.length > 0 || nests.length === 0) {
    sections.push({
      key: "ungrouped",
      label: nests.length === 0 ? "All Combs" : "Ungrouped",
      combs: ungroupedCombs,
      collapsible: false,
    });
  }

  for (const nest of nests) {
    sections.push({
      key: nest.id,
      label: nest.name,
      combs: combs.filter((comb) => comb.nestId === nest.id),
      collapsible: true,
      emptyLabel: "Empty nest",
    });
  }

  return sections;
}

function sectionKeyForComb(comb: Comb, nestIds: Set<string>): string {
  return comb.nestId && nestIds.has(comb.nestId) ? comb.nestId : "ungrouped";
}

interface VisibleCombEntry {
  comb: Comb;
  sectionKey: string;
}

interface SortableCombProps {
  ref: (el: HTMLDivElement | null) => void;
  onPointerDown: (e: ReactPointerEvent) => void;
  style: CSSProperties | undefined;
  isDragged: boolean;
}

interface SortableCombSectionProps {
  section: CombSection;
  activeCombId: string | null;
  collapsed: boolean;
  editingNestId: string | null;
  editingNestName: string;
  nestRenameError: string;
  savingNestRename: boolean;
  confirmDeleteId: string | null;
  confirmDeleteNestId: string | null;
  editingCombId: string | null;
  editingName: string;
  renameError: string;
  savingRename: boolean;
  onToggleNest: (nestId: string) => void;
  onStartRenameNest: (section: CombSection) => void;
  onStopRenameNest: () => void;
  onSubmitRenameNest: (section: CombSection) => Promise<void>;
  onEditingNestNameChange: (name: string) => void;
  onClearNestRenameError: () => void;
  onSelectComb: (comb: Comb) => void;
  onStartRename: (comb: Comb) => void;
  onStopRename: () => void;
  onSubmitRename: (comb: Comb) => Promise<void>;
  onEditingNameChange: (name: string) => void;
  onClearRenameError: () => void;
  onContextMenu: (combId: string, x: number, y: number) => void;
  onConfirmDelete: (combId: string | null) => void;
  onConfirmDeleteNest: (nestId: string | null) => void;
  onDeleteNest: (nestId: string) => void;
  onDeleteComb: (combId: string) => void;
  onCopyComb: (combId: string) => void;
  isDragging: boolean;
  getCombProps: (combId: string) => SortableCombProps;
  setSectionRef: (sectionKey: string, el: HTMLDivElement | null) => void;
}

function SortableCombSection({
  section,
  activeCombId,
  collapsed,
  editingNestId,
  editingNestName,
  nestRenameError,
  savingNestRename,
  confirmDeleteId,
  confirmDeleteNestId,
  editingCombId,
  editingName,
  renameError,
  savingRename,
  onToggleNest,
  onStartRenameNest,
  onStopRenameNest,
  onSubmitRenameNest,
  onEditingNestNameChange,
  onClearNestRenameError,
  onSelectComb,
  onStartRename,
  onStopRename,
  onSubmitRename,
  onEditingNameChange,
  onClearRenameError,
  onContextMenu,
  onConfirmDelete,
  onConfirmDeleteNest,
  onDeleteNest,
  onDeleteComb,
  onCopyComb,
  isDragging,
  getCombProps,
  setSectionRef,
}: SortableCombSectionProps) {
  return (
    <div
      ref={(el) => setSectionRef(section.key, el)}
      className={`sidebar-nest-section ${isDragging ? "is-dragging" : ""}`}
    >
      <div
        className={`sidebar-nest-header ${section.collapsible ? "collapsible" : ""}`}
        onClick={() => {
          if (section.collapsible) onToggleNest(section.key);
        }}
        onDoubleClick={(e) => {
          if (!section.collapsible) return;
          e.stopPropagation();
          onStartRenameNest(section);
        }}
        role={section.collapsible ? "button" : undefined}
        tabIndex={section.collapsible ? 0 : undefined}
        aria-expanded={section.collapsible ? !collapsed : undefined}
      >
        <span className="sidebar-nest-title">
          {section.collapsible && (
            <span className="sidebar-nest-arrow">{collapsed ? "▸" : "▾"}</span>
          )}
          {editingNestId === section.key ? (
            <input
              className={`sidebar-nest-name-input ${nestRenameError ? "error" : ""}`}
              type="text"
              value={editingNestName}
              onChange={(e) => {
                onEditingNestNameChange(e.target.value);
                if (nestRenameError) onClearNestRenameError();
              }}
              onClick={(e) => e.stopPropagation()}
              onPointerDown={(e) => e.stopPropagation()}
              onDoubleClick={(e) => e.stopPropagation()}
              onBlur={() => {
                if (!savingNestRename) {
                  void onSubmitRenameNest(section);
                }
              }}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") {
                  e.preventDefault();
                  void onSubmitRenameNest(section);
                }
                if (e.key === "Escape") {
                  e.preventDefault();
                  onStopRenameNest();
                }
              }}
              autoFocus
              maxLength={40}
              spellCheck={false}
            />
          ) : (
            <span className="sidebar-nest-name">{section.label}</span>
          )}
        </span>
        {section.collapsible ? (
          confirmDeleteNestId === section.key ? (
            <div className="delete-confirm-inline" onClick={(e) => e.stopPropagation()}>
              <button
                className="btn-sm btn-danger"
                onClick={() => {
                  onConfirmDeleteNest(null);
                  onDeleteNest(section.key);
                }}
              >
                Sure?
              </button>
              <button className="btn-sm" onClick={() => onConfirmDeleteNest(null)}>
                No
              </button>
            </div>
          ) : (
            <div className="sidebar-nest-actions">
              <span className="sidebar-nest-count">{section.combs.length}</span>
              <button
                className="sidebar-nest-delete"
                onClick={(e) => {
                  e.stopPropagation();
                  onConfirmDelete(null);
                  onConfirmDeleteNest(section.key);
                }}
                title="Delete nest"
              >
                x
              </button>
            </div>
          )
        ) : (
          <span className="sidebar-nest-count">{section.combs.length}</span>
        )}
      </div>
      {editingNestId === section.key && nestRenameError ? (
        <div className="sidebar-nest-error">{nestRenameError}</div>
      ) : null}

      {!collapsed && section.combs.length === 0 && section.emptyLabel ? (
        <div className="sidebar-nest-empty">{section.emptyLabel}</div>
      ) : null}

      {!collapsed && section.combs.map((comb) => {
        const { ref, onPointerDown, style, isDragged } = getCombProps(comb.id);
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
              onContextMenu(comb.id, e.clientX, e.clientY);
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
                      onEditingNameChange(e.target.value);
                      if (renameError) onClearRenameError();
                    }}
                    onClick={(e) => e.stopPropagation()}
                    onPointerDown={(e) => e.stopPropagation()}
                    onDoubleClick={(e) => e.stopPropagation()}
                    onBlur={() => {
                      if (!savingRename) {
                        const trimmedName = editingName.trim();
                        if (trimmedName === comb.name) {
                          onStopRename();
                        } else {
                          void onSubmitRename(comb);
                        }
                      }
                    }}
                    onKeyDown={(e) => {
                      e.stopPropagation();
                      if (e.key === "Enter") {
                        e.preventDefault();
                        void onSubmitRename(comb);
                      }
                      if (e.key === "Escape") {
                        e.preventDefault();
                        onStopRename();
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
                      onStartRename(comb);
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
                        onConfirmDelete(null);
                        onDeleteComb(comb.id);
                      }}
                    >
                      Sure?
                    </button>
                    <button className="btn-sm" onClick={() => onConfirmDelete(null)}>
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
                        onConfirmDelete(comb.id);
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
    </div>
  );
}

export function Sidebar({
  hives,
  activeHive,
  nests,
  combs,
  activeCombId,
  onSelectHive,
  onSelectComb,
  onNewComb,
  onManageHives,
  onSettings,
  onHelp,
  onNewNest,
  onAssignCombToNest,
  onRenameNest,
  onDeleteNest,
  onDeleteComb,
  onRenameComb,
  onCopyComb,
  onReorderCombs,
}: Props) {
  const [hiveDropdownOpen, setHiveDropdownOpen] = useState(false);
  const hiveDropdownRef = useRef<HTMLDivElement>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [confirmDeleteNestId, setConfirmDeleteNestId] = useState<string | null>(null);
  const [editingCombId, setEditingCombId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState("");
  const [renameError, setRenameError] = useState("");
  const [savingRename, setSavingRename] = useState(false);
  const [editingNestId, setEditingNestId] = useState<string | null>(null);
  const [editingNestName, setEditingNestName] = useState("");
  const [nestRenameError, setNestRenameError] = useState("");
  const [savingNestRename, setSavingNestRename] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ combId: string; x: number; y: number; showNestPicker?: boolean } | null>(null);
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const [collapsedNestIds, setCollapsedNestIds] = useState<Set<string>>(() => new Set());
  const sectionElsByKey = useRef<Map<string, HTMLDivElement>>(new Map());

  const sections = buildCombSections(combs, nests);
  const nestIds = new Set(nests.map((nest) => nest.id));
  const visibleCombEntries: VisibleCombEntry[] = sections.flatMap((section) =>
    collapsedNestIds.has(section.key)
      ? []
      : section.combs.map((comb) => ({ comb, sectionKey: section.key }))
  );
  const visibleCombs = visibleCombEntries.map((entry) => entry.comb);
  const visibleIndexByCombId = new Map(visibleCombs.map((comb, index) => [comb.id, index]));

  function handleReorderVisibleCombs(
    reorderedVisibleCombIds: string[],
    meta: { from: number; to: number; movedId: string; clientY: number },
  ) {
    const combById = new Map(combs.map((comb) => [comb.id, comb]));
    const movedComb = combById.get(meta.movedId);
    if (!movedComb) return;
    const sectionAtDrop = [...sectionElsByKey.current.entries()].find(([, el]) => {
      const rect = el.getBoundingClientRect();
      return meta.clientY >= rect.top && meta.clientY <= rect.bottom;
    })?.[0];
    const destinationSectionKey =
      sectionAtDrop ??
      visibleCombEntries[meta.to]?.sectionKey ??
      visibleCombEntries[meta.from]?.sectionKey ??
      sectionKeyForComb(movedComb, nestIds);
    const destinationNestId = destinationSectionKey === "ungrouped" ? undefined : destinationSectionKey;
    const reorderedVisibleCombs = reorderedVisibleCombIds
      .map((id) => combById.get(id))
      .filter(Boolean)
      .map((comb) =>
        comb!.id === meta.movedId ? { ...comb!, nestId: destinationNestId } : comb!
      );
    const originalVisibleIds = new Set(visibleCombEntries.map((entry) => entry.comb.id));
    const visibleIterator = reorderedVisibleCombs[Symbol.iterator]();
    const nextCombs = combs.map((comb) =>
      originalVisibleIds.has(comb.id) ? visibleIterator.next().value ?? comb : comb
    );
    onReorderCombs(nextCombs);
  }
  const { isDragging, getItemProps } = useSortable(visibleCombs, handleReorderVisibleCombs);

  function getCombProps(combId: string): SortableCombProps {
    const index = visibleIndexByCombId.get(combId);
    if (index === undefined) {
      return {
        ref: () => {},
        onPointerDown: () => {},
        style: undefined,
        isDragged: false,
      };
    }
    return getItemProps(index);
  }

  function setSectionRef(sectionKey: string, el: HTMLDivElement | null) {
    if (el) {
      sectionElsByKey.current.set(sectionKey, el);
    } else {
      sectionElsByKey.current.delete(sectionKey);
    }
  }

  function toggleNestCollapsed(nestId: string) {
    setCollapsedNestIds((current) => {
      const next = new Set(current);
      if (next.has(nestId)) {
        next.delete(nestId);
      } else {
        next.add(nestId);
      }
      return next;
    });
  }

  function startRename(comb: Comb) {
    setConfirmDeleteId(null);
    setConfirmDeleteNestId(null);
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

  function startRenameNest(section: CombSection) {
    if (!section.collapsible) return;
    setConfirmDeleteId(null);
    setConfirmDeleteNestId(null);
    setContextMenu(null);
    setEditingNestId(section.key);
    setEditingNestName(section.label);
    setNestRenameError("");
  }

  function stopRenameNest() {
    setEditingNestId(null);
    setEditingNestName("");
    setNestRenameError("");
    setSavingNestRename(false);
  }

  async function submitRenameNest(section: CombSection) {
    const trimmedName = editingNestName.trim();
    if (trimmedName === section.label) {
      stopRenameNest();
      return;
    }

    const error = validateNestName(trimmedName, section.key, nests);
    if (error) {
      setNestRenameError(error);
      return;
    }

    setSavingNestRename(true);
    setNestRenameError("");
    try {
      await onRenameNest(section.key, trimmedName);
      stopRenameNest();
    } catch (e) {
      setNestRenameError(`${e}`);
      setSavingNestRename(false);
    }
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

  useEffect(() => {
    if (editingNestId && !nests.some((nest) => nest.id === editingNestId)) {
      stopRenameNest();
    }
    if (confirmDeleteNestId && !nests.some((nest) => nest.id === confirmDeleteNestId)) {
      setConfirmDeleteNestId(null);
    }
  }, [nests, editingNestId, confirmDeleteNestId]);

  useEffect(() => {
    const liveNestIds = new Set(nests.map((nest) => nest.id));
    setCollapsedNestIds((current) => {
      const next = new Set([...current].filter((id) => liveNestIds.has(id)));
      return next.size === current.size ? current : next;
    });
  }, [nests]);

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
        <div className="sidebar-section-header">
          <div className="sidebar-section-label">Combs</div>
          {activeHive && (
            <button
              className="sidebar-section-action"
              onClick={() => onNewNest()}
              title="Create nest"
            >
              + Nest
            </button>
          )}
        </div>
        <div className="sidebar-comb-list">
          {sections.map((section) => (
            <SortableCombSection
              key={section.key}
              section={section}
              activeCombId={activeCombId}
              collapsed={collapsedNestIds.has(section.key)}
              editingNestId={editingNestId}
              editingNestName={editingNestName}
              nestRenameError={nestRenameError}
              savingNestRename={savingNestRename}
              confirmDeleteId={confirmDeleteId}
              confirmDeleteNestId={confirmDeleteNestId}
              editingCombId={editingCombId}
              editingName={editingName}
              renameError={renameError}
              savingRename={savingRename}
              onToggleNest={toggleNestCollapsed}
              onStartRenameNest={startRenameNest}
              onStopRenameNest={stopRenameNest}
              onSubmitRenameNest={submitRenameNest}
              onEditingNestNameChange={setEditingNestName}
              onClearNestRenameError={() => setNestRenameError("")}
              onSelectComb={onSelectComb}
              onStartRename={startRename}
              onStopRename={stopRename}
              onSubmitRename={submitRename}
              onEditingNameChange={setEditingName}
              onClearRenameError={() => setRenameError("")}
              onContextMenu={(combId, x, y) => {
                setConfirmDeleteId(null);
                setConfirmDeleteNestId(null);
                setContextMenu({ combId, x, y });
              }}
              onConfirmDelete={(combId) => {
                setConfirmDeleteNestId(null);
                setConfirmDeleteId(combId);
              }}
              onConfirmDeleteNest={setConfirmDeleteNestId}
              onDeleteNest={(nestId) => {
                setConfirmDeleteNestId(null);
                void onDeleteNest(nestId);
              }}
              onDeleteComb={onDeleteComb}
              onCopyComb={onCopyComb}
              isDragging={isDragging}
              getCombProps={getCombProps}
              setSectionRef={setSectionRef}
            />
          ))}
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
          <button
            className="sidebar-context-menu-item"
            onClick={() => {
              setContextMenu((prev) => prev ? { ...prev, showNestPicker: !prev.showNestPicker } : prev);
            }}
          >
            Add to Nest
          </button>
          {contextMenu.showNestPicker && (
            <div className="sidebar-context-submenu">
              <button
                className="sidebar-context-menu-item sidebar-context-subitem"
                onClick={() => {
                  void onAssignCombToNest(contextMenu.combId, undefined).catch(console.error);
                  setContextMenu(null);
                }}
              >
                Ungrouped
              </button>
              {nests.map((nest) => {
                const comb = combs.find((item) => item.id === contextMenu.combId);
                const isSelected = comb?.nestId === nest.id;
                return (
                  <button
                    key={nest.id}
                    className={`sidebar-context-menu-item sidebar-context-subitem ${isSelected ? "selected" : ""}`}
                    onClick={() => {
                      void onAssignCombToNest(contextMenu.combId, nest.id).catch(console.error);
                      setContextMenu(null);
                    }}
                  >
                    {isSelected ? "• " : ""}
                    {nest.name}
                  </button>
                );
              })}
              <button
                className="sidebar-context-menu-item sidebar-context-subitem"
                onClick={() => {
                  onNewNest(contextMenu.combId);
                  setContextMenu(null);
                }}
              >
                + Create Nest
              </button>
            </div>
          )}
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
