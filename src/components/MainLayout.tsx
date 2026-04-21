import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Sidebar } from "./Sidebar";
import { WorkspaceGrid } from "./WorkspaceGrid";
import { NewCombModal } from "./NewCombModal";
import { CopyCombModal } from "./CopyCombModal";
import { CreateNestModal } from "./CreateNestModal";
import { CustomButtonsModal } from "./CustomButtonsModal";
import { HiveListScreen } from "./HiveListScreen";
import { SettingsScreen } from "./SettingsScreen";
import { HelpScreen } from "./HelpScreen";
import { Toast } from "./Toast";
import type { HiveInfo, Comb, PaneConfig, CustomButton, CombOperationResult, HiveOperationResult, Nest, HiveState } from "../types";

// Normalize comb: convert legacy `cloning` to `operation`
function normalizeComb(comb: Comb): Comb {
  if (comb.cloning && !comb.operation) {
    return { ...comb, operation: "cloning" };
  }
  return comb;
}

function normalizeCombs(combs: Comb[]): Comb[] {
  return combs.map(normalizeComb);
}

interface Props {
  beehiveDir: string;
  onReset: () => void;
}

type Overlay =
  | null
  | { type: "newComb" }
  | { type: "manageHives" }
  | { type: "settings"; from: "sidebar" | "manageHives" }
  | { type: "help"; from: "sidebar" | "manageHives" }
  | { type: "customButtons" }
  | { type: "createNest"; combId?: string }
  | { type: "copyComb"; sourceCombId: string };

// Per-hive runtime state (combs, opened combs, panes, active comb)
interface HiveRuntime {
  nests: Nest[];
  combs: Comb[];
  openedCombs: Set<string>;
  panesByComb: Map<string, PaneConfig[]>;
  activeCombId: string | null;
  focusedPaneByComb: Map<string, string>;
}

function emptyRuntime(): HiveRuntime {
  return {
    nests: [],
    combs: [],
    openedCombs: new Set(),
    panesByComb: new Map(),
    activeCombId: null,
    focusedPaneByComb: new Map(),
  };
}

export function MainLayout({ beehiveDir, onReset }: Props) {
  const [hives, setHives] = useState<HiveInfo[]>([]);
  const [activeHiveDirName, setActiveHiveDirName] = useState<string | null>(null);
  // Per-hive state keyed by dirName — survives hive switches
  const [hiveRuntimes, setHiveRuntimes] = useState<Map<string, HiveRuntime>>(new Map());
  const [overlay, setOverlay] = useState<Overlay>({ type: "manageHives" });
  
  // Track hives being deleted
  const [hivesDeleting, setHivesDeleting] = useState<Set<string>>(new Set());
  
  // Toast notifications for operation errors
  const [toasts, setToasts] = useState<Array<{ id: string; message: string; type: "error" | "success" }>>([]);

  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  
  const addToast = useCallback((message: string, type: "error" | "success" = "error") => {
    const id = crypto.randomUUID();
    setToasts((prev) => [...prev, { id, message, type }]);
    // Auto-dismiss after 5 seconds
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 5000);
  }, []);
  
  const dismissToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const activeHive = hives.find((h) => h.dirName === activeHiveDirName) ?? null;
  const activeRuntime = activeHiveDirName ? hiveRuntimes.get(activeHiveDirName) : undefined;

  // Load hives on mount
  useEffect(() => {
    loadHives();
  }, [beehiveDir]);
  
  // Listen for comb operation completion events
  useEffect(() => {
    const unlistenComb = listen<CombOperationResult>("comb-operation-done", (event) => {
      const { hiveDirName, combId, opType, success, error } = event.payload;
      
      if (opType === "deleting") {
        if (success) {
          // Remove comb from state
          updateRuntime(hiveDirName, (rt) => {
            const newOpened = new Set(rt.openedCombs);
            newOpened.delete(combId);
            const newPanes = new Map(rt.panesByComb);
            newPanes.delete(combId);
            const remaining = rt.combs.filter((c) => c.id !== combId);
            const newActive = rt.activeCombId === combId
              ? (remaining.length > 0 ? remaining[0].id : null)
              : rt.activeCombId;
            return { 
              ...rt, 
              combs: remaining,
              openedCombs: newOpened, 
              panesByComb: newPanes, 
              activeCombId: newActive 
            };
          });
        } else {
          // Revert operation flag on failure
          updateRuntime(hiveDirName, (rt) => ({
            ...rt,
            combs: rt.combs.map((c) =>
              c.id === combId ? { ...c, operation: undefined } : c
            ),
          }));
          if (error) {
            addToast(`Delete failed: ${error}`);
          }
        }
      } else {
        // cloning or copying
        if (success) {
          // Clear operation flag
          updateRuntime(hiveDirName, (rt) => ({
            ...rt,
            combs: rt.combs.map((c) =>
              c.id === combId ? { ...c, operation: undefined, cloning: false } : c
            ),
          }));
        } else {
          // Remove the failed comb from runtime
          updateRuntime(hiveDirName, (rt) => ({
            ...rt,
            combs: rt.combs.filter((c) => c.id !== combId),
          }));
          if (error) {
            addToast(`${opType === "cloning" ? "Clone" : "Copy"} failed: ${error}`);
          }
        }
      }
    });
    
    const unlistenHive = listen<HiveOperationResult>("hive-operation-done", (event) => {
      const { hiveDirName, success, error } = event.payload;
      
      // Remove from deleting set
      setHivesDeleting((prev) => {
        const next = new Set(prev);
        next.delete(hiveDirName);
        return next;
      });
      
      if (success) {
        // Remove hive from state
        setHives((prev) => prev.filter((h) => h.dirName !== hiveDirName));
        // Clear runtime for this hive
        setHiveRuntimes((prev) => {
          const next = new Map(prev);
          next.delete(hiveDirName);
          return next;
        });
        // If this was the active hive, clear it
        setActiveHiveDirName((prev) => prev === hiveDirName ? null : prev);
      } else {
        if (error) {
          addToast(`Delete hive failed: ${error}`);
        }
      }
    });
    
    return () => {
      unlistenComb.then((fn) => fn());
      unlistenHive.then((fn) => fn());
    };
  }, [addToast]);

  // Periodically refresh comb branches from git
  useEffect(() => {
    const interval = setInterval(async () => {
      if (!activeHiveDirName) return;
      try {
        const list = await invoke<Comb[]>("list_combs", {
          beehiveDir,
          dirName: activeHiveDirName,
        });
        const normalized = normalizeCombs(list);
        updateRuntime(activeHiveDirName, (rt) => ({
          ...rt,
          combs: rt.combs.map((c) => {
            const fresh = normalized.find((f) => f.id === c.id);
            // Preserve local operation state if not in fresh data
            return fresh ? { ...c, branch: fresh.branch, operation: fresh.operation || c.operation } : c;
          }),
        }));
      } catch { /* ignore */ }
    }, 5000);
    return () => clearInterval(interval);
  }, [activeHiveDirName, beehiveDir]);

  async function loadHives() {
    try {
      const list = await invoke<HiveInfo[]>("list_hives", { beehiveDir });
      setHives(list);
    } catch (e) {
      console.error("Failed to list hives:", e);
    }
  }

  function getOrCreateRuntime(dirName: string): HiveRuntime {
    const existing = hiveRuntimes.get(dirName);
    if (existing) return existing;
    return emptyRuntime();
  }

  function updateRuntime(dirName: string, updater: (rt: HiveRuntime) => HiveRuntime) {
    setHiveRuntimes((prev) => {
      const current = prev.get(dirName) ?? emptyRuntime();
      const updated = updater(current);
      const next = new Map(prev);
      next.set(dirName, updated);
      return next;
    });
  }

  async function selectHive(hive: HiveInfo) {
    setActiveHiveDirName(hive.dirName);

    // Load combs if not yet loaded for this hive
    const runtime = hiveRuntimes.get(hive.dirName);
    if (!runtime || runtime.combs.length === 0) {
      try {
        const state = await invoke<HiveState>("get_hive_state", {
          beehiveDir,
          dirName: hive.dirName,
        });
        updateRuntime(hive.dirName, (rt) => ({
          ...rt,
          nests: state.nests ?? [],
          combs: normalizeCombs(state.combs),
        }));
      } catch (e) {
        console.error("Failed to load hive state:", e);
      }
    }
  }

  async function openComb(comb: Comb) {
    if (!activeHiveDirName) return;

    const runtime = getOrCreateRuntime(activeHiveDirName);

    if (!runtime.openedCombs.has(comb.id)) {
      // First time opening — load saved panes or create default
      let panes: PaneConfig[];
      try {
        panes = await invoke<PaneConfig[]>("get_comb_panes", {
          beehiveDir,
          dirName: activeHiveDirName,
          combId: comb.id,
        });
      } catch {
        panes = [];
      }

      if (panes.length === 0) {
        panes = [{ id: crypto.randomUUID(), type: "terminal" }];
      }

      updateRuntime(activeHiveDirName, (rt) => ({
        ...rt,
        activeCombId: comb.id,
        openedCombs: new Set(rt.openedCombs).add(comb.id),
        panesByComb: new Map(rt.panesByComb).set(comb.id, panes),
      }));
    } else {
      updateRuntime(activeHiveDirName, (rt) => ({ ...rt, activeCombId: comb.id }));
    }
  }

  function debounceSavePanes(hiveDirName: string, combId: string, panes: PaneConfig[]) {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(() => {
      invoke("save_comb_panes", {
        beehiveDir,
        dirName: hiveDirName,
        combId,
        panes: panes.map((p) => ({
          id: p.id,
          type: p.type,
          cmd: p.cmd ?? null,
          args: p.args ?? null,
        })),
      }).catch((e) => console.error("Failed to save panes:", e));
    }, 500);
  }

  const addPane = useCallback(
    (combId: string, cmd?: string) => {
      if (!activeHiveDirName) return;
      const hiveDirName = activeHiveDirName;
      updateRuntime(hiveDirName, (rt) => {
        const current = rt.panesByComb.get(combId) ?? [];
        const newPane: PaneConfig = {
          id: crypto.randomUUID(),
          type: cmd ? "agent" : "terminal",
          cmd: cmd || undefined,
        };
        const updated = [...current, newPane];
        debounceSavePanes(hiveDirName, combId, updated);
        const newFocused = new Map(rt.focusedPaneByComb);
        newFocused.set(combId, newPane.id);
        return { ...rt, panesByComb: new Map(rt.panesByComb).set(combId, updated), focusedPaneByComb: newFocused };
      });
    },
    [activeHiveDirName, beehiveDir]
  );

  const removePane = useCallback(
    (combId: string, paneId: string) => {
      if (!activeHiveDirName) return;
      const hiveDirName = activeHiveDirName;
      updateRuntime(hiveDirName, (rt) => {
        const current = rt.panesByComb.get(combId) ?? [];
        const updated = current.filter((p) => p.id !== paneId);
        debounceSavePanes(hiveDirName, combId, updated);
        return { ...rt, panesByComb: new Map(rt.panesByComb).set(combId, updated) };
      });
    },
    [activeHiveDirName, beehiveDir]
  );

  async function handleDeleteComb(combId: string) {
    if (!activeHiveDirName) return;
    const hiveDirName = activeHiveDirName;

    // Check if comb already has an operation in progress
    const runtime = hiveRuntimes.get(hiveDirName);
    const comb = runtime?.combs.find((c) => c.id === combId);
    if (comb?.operation) {
      addToast("Cannot delete comb with operation in progress");
      return;
    }

    // Mark comb as deleting in UI immediately
    updateRuntime(hiveDirName, (rt) => ({
      ...rt,
      combs: rt.combs.map((c) =>
        c.id === combId ? { ...c, operation: "deleting" } : c
      ),
    }));

    // Close the comb if it's open (but don't remove from combs list yet)
    updateRuntime(hiveDirName, (rt) => {
      const newOpened = new Set(rt.openedCombs);
      newOpened.delete(combId);
      const newPanes = new Map(rt.panesByComb);
      newPanes.delete(combId);
      const newActive = rt.activeCombId === combId
        ? (rt.combs.filter((c) => c.id !== combId && !c.operation).find(() => true)?.id ?? null)
        : rt.activeCombId;
      return { ...rt, openedCombs: newOpened, panesByComb: newPanes, activeCombId: newActive };
    });

    try {
      // Phase 1: Mark in backend
      await invoke("delete_comb_start", { beehiveDir, dirName: hiveDirName, combId });
      // Phase 2: Fire background delete (event will handle completion)
      invoke("delete_comb_run", { beehiveDir, dirName: hiveDirName, combId });
    } catch (e) {
      console.error("Failed to start comb deletion:", e);
      // Revert UI state
      updateRuntime(hiveDirName, (rt) => ({
        ...rt,
        combs: rt.combs.map((c) =>
          c.id === combId ? { ...c, operation: undefined } : c
        ),
      }));
      addToast(`Failed to delete comb: ${e}`);
    }
  }

  async function handleRenameComb(combId: string, newName: string) {
    if (!activeHiveDirName) return;
    const renamed = await invoke<Comb>("rename_comb", {
      beehiveDir,
      dirName: activeHiveDirName,
      combId,
      newName,
    });
    updateRuntime(activeHiveDirName, (rt) => ({
      ...rt,
      combs: rt.combs.map((comb) => (comb.id === renamed.id ? renamed : comb)),
    }));
  }

  function handleCombCreated(comb: Comb) {
    if (!activeHiveDirName) return;
    const hiveDirName = activeHiveDirName;
    setOverlay(null);
    
    // Normalize the comb to use operation field
    const normalizedComb = normalizeComb(comb);
    
    updateRuntime(hiveDirName, (rt) => ({
      ...rt,
      combs: [...rt.combs, normalizedComb],
    }));
    
    // Fire background clone - event listener handles completion
    invoke("create_comb_clone", {
      beehiveDir,
      dirName: hiveDirName,
      combId: comb.id,
    }).catch((e) => {
      // This catch is for immediate invocation errors only
      // Actual clone failures are handled via events
      console.error("Failed to start clone:", e);
    });
  }

  const [copyCombError, setCopyCombError] = useState("");

  async function handleCopyComb(sourceCombId: string, newName: string) {
    if (!activeHiveDirName) return;
    const hiveDirName = activeHiveDirName;
    
    // Check if source comb is being deleted
    const runtime = hiveRuntimes.get(hiveDirName);
    const sourceComb = runtime?.combs.find((c) => c.id === sourceCombId);
    if (sourceComb?.operation === "deleting") {
      setCopyCombError("Cannot copy a comb that is being deleted");
      return;
    }
    
    setCopyCombError("");
    
    try {
      // Phase 1: Create comb entry (quick)
      const comb = await invoke<Comb>("copy_comb_start", {
        beehiveDir,
        dirName: hiveDirName,
        sourceCombId,
        newName,
      });
      
      // Close modal immediately
      setOverlay(null);
      
      // Add comb to runtime (with operation: "copying")
      const normalizedComb = normalizeComb(comb);
      updateRuntime(hiveDirName, (rt) => ({
        ...rt,
        combs: [...rt.combs, normalizedComb],
      }));
      
      // Phase 2: Fire background copy - event listener handles completion
      invoke("copy_comb_run", {
        beehiveDir,
        dirName: hiveDirName,
        combId: comb.id,
        sourceCombId,
      }).catch((e) => {
        console.error("Failed to start copy:", e);
      });
    } catch (e) {
      setCopyCombError(`${e}`);
    }
  }

  const handleReorderCombs = useCallback(async (nextCombs: Comb[]) => {
    if (!activeHiveDirName || !activeRuntime) return;
    const hiveDirName = activeHiveDirName;
    const combIds = nextCombs.map((comb) => comb.id);

    // Optimistic update
    updateRuntime(hiveDirName, (rt) => ({ ...rt, combs: nextCombs }));

    try {
      const state = await invoke<HiveState>("save_nests", {
        beehiveDir,
        dirName: hiveDirName,
        nests: activeRuntime.nests,
        assignments: nextCombs.map((comb) => ({
          combId: comb.id,
          nestId: comb.nestId ?? null,
        })),
      });
      await invoke("reorder_combs", { beehiveDir, dirName: hiveDirName, combIds });
      const byId = new Map(normalizeCombs(state.combs).map((comb) => [comb.id, comb]));
      const orderedCombs = nextCombs.map((comb) => byId.get(comb.id) ?? comb);
      updateRuntime(hiveDirName, (rt) => ({
        ...rt,
        nests: state.nests ?? [],
        combs: orderedCombs,
      }));
    } catch (e) {
      console.error("Failed to reorder combs:", e);
    }
  }, [activeHiveDirName, activeRuntime, beehiveDir]);

  async function persistNestState(nests: Nest[], combs: Comb[]) {
    if (!activeHiveDirName) {
      throw new Error("No active hive selected");
    }
    const state = await invoke<HiveState>("save_nests", {
      beehiveDir,
      dirName: activeHiveDirName,
      nests,
      assignments: combs.map((comb) => ({
        combId: comb.id,
        nestId: comb.nestId ?? null,
      })),
    });

    updateRuntime(activeHiveDirName, (rt) => ({
      ...rt,
      nests: state.nests ?? [],
      combs: normalizeCombs(state.combs),
    }));
    return state;
  }

  async function handleCreateNest(name: string, combId?: string) {
    if (!activeRuntime) {
      throw new Error("No active hive selected");
    }

    const nextNest: Nest = {
      id: crypto.randomUUID(),
      name,
    };
    const nextCombs = activeRuntime.combs.map((comb) =>
      comb.id === combId ? { ...comb, nestId: nextNest.id } : comb
    );

    await persistNestState([...activeRuntime.nests, nextNest], nextCombs);
    setOverlay(null);
  }

  async function handleAssignCombToNest(combId: string, nestId?: string) {
    if (!activeRuntime) return;

    const nextCombs = activeRuntime.combs.map((comb) =>
      comb.id === combId ? { ...comb, nestId } : comb
    );

    await persistNestState(activeRuntime.nests, nextCombs);
  }

  async function handleRenameNest(nestId: string, newName: string) {
    if (!activeRuntime) return;

    const nextNests = activeRuntime.nests.map((nest) =>
      nest.id === nestId ? { ...nest, name: newName } : nest
    );

    await persistNestState(nextNests, activeRuntime.combs);
  }

  async function handleDeleteNest(nestId: string) {
    if (!activeRuntime) return;

    const nextNests = activeRuntime.nests.filter((nest) => nest.id !== nestId);
    const nextCombs = activeRuntime.combs.map((comb) =>
      comb.nestId === nestId ? { ...comb, nestId: undefined } : comb
    );

    await persistNestState(nextNests, nextCombs);
  }

  async function saveCustomButtons(buttons: CustomButton[]) {
    if (!activeHiveDirName) return;
    try {
      await invoke("save_custom_buttons", {
        beehiveDir,
        dirName: activeHiveDirName,
        buttons,
      });
      setHives((prev) =>
        prev.map((h) =>
          h.dirName === activeHiveDirName
            ? { ...h, customButtons: buttons }
            : h
        )
      );
      setOverlay(null);
    } catch (e) {
      console.error("Failed to save custom buttons:", e);
    }
  }

  const handlePaneFocused = useCallback(
    (combId: string, paneId: string) => {
      if (!activeHiveDirName) return;
      updateRuntime(activeHiveDirName, (rt) => {
        const newFocused = new Map(rt.focusedPaneByComb);
        newFocused.set(combId, paneId);
        return { ...rt, focusedPaneByComb: newFocused };
      });
    },
    [activeHiveDirName]
  );
  
  // Async hive deletion
  async function handleDeleteHive(dirName: string) {
    // Check if any combs in this hive have active operations
    const runtime = hiveRuntimes.get(dirName);
    if (runtime?.combs.some((c) => c.operation)) {
      addToast("Cannot delete hive while comb operations are in progress");
      return;
    }
    
    // Mark hive as deleting
    setHivesDeleting((prev) => new Set([...prev, dirName]));
    
    try {
      // Phase 1: Check if hive can be deleted
      await invoke("delete_hive_start", { beehiveDir, dirName });
      // Phase 2: Fire background delete (event will handle completion)
      invoke("delete_hive_run", { beehiveDir, dirName });
    } catch (e) {
      console.error("Failed to start hive deletion:", e);
      setHivesDeleting((prev) => {
        const next = new Set(prev);
        next.delete(dirName);
        return next;
      });
      addToast(`Failed to delete hive: ${e}`);
    }
  }

  // Collect ALL opened combs across ALL hives for rendering
  const allOpenedCombs: { hive: HiveInfo; comb: Comb; panes: PaneConfig[]; focusedPaneId: string | null; isVisible: boolean }[] = [];
  for (const [dirName, runtime] of hiveRuntimes) {
    const hive = hives.find((h) => h.dirName === dirName);
    if (!hive) continue;
    for (const comb of runtime.combs) {
      if (!runtime.openedCombs.has(comb.id)) continue;
      const panes = runtime.panesByComb.get(comb.id) ?? [];
      const focusedPaneId = runtime.focusedPaneByComb.get(comb.id) ?? panes[0]?.id ?? null;
      allOpenedCombs.push({
        hive,
        comb,
        panes,
        focusedPaneId,
        isVisible: dirName === activeHiveDirName && comb.id === runtime.activeCombId,
      });
    }
  }

  const currentCombs = activeRuntime?.combs ?? [];
  const currentActiveCombId = activeRuntime?.activeCombId ?? null;

  return (
    <div className="main-layout">
      <Sidebar
        hives={hives}
        activeHive={activeHive}
        nests={activeRuntime?.nests ?? []}
        combs={currentCombs}
        activeCombId={currentActiveCombId}
        onSelectHive={(hive) => {
          selectHive(hive);
        }}
        onSelectComb={openComb}
        onNewComb={() => setOverlay({ type: "newComb" })}
        onManageHives={() => setOverlay({ type: "manageHives" })}
        onSettings={() => setOverlay({ type: "settings", from: "sidebar" })}
        onHelp={() => setOverlay({ type: "help", from: "sidebar" })}
        onNewNest={(combId) => setOverlay({ type: "createNest", combId })}
        onAssignCombToNest={handleAssignCombToNest}
        onRenameNest={handleRenameNest}
        onDeleteNest={handleDeleteNest}
        onDeleteComb={handleDeleteComb}
        onRenameComb={handleRenameComb}
        onCopyComb={(combId) => {
          setCopyCombError("");
          setOverlay({ type: "copyComb", sourceCombId: combId });
        }}
        onReorderCombs={handleReorderCombs}
      />

      <div className="main-content">
        {allOpenedCombs.map(({ hive, comb, panes, focusedPaneId, isVisible }) => (
          <WorkspaceGrid
            key={comb.id}
            comb={comb}
            panes={panes}
            customButtons={hive.customButtons ?? []}
            isVisible={isVisible}
            focusedPaneId={focusedPaneId}
            onAddPane={(cmd) => addPane(comb.id, cmd)}
            onRemovePane={(paneId) => removePane(comb.id, paneId)}
            onPaneFocused={(paneId) => handlePaneFocused(comb.id, paneId)}
            onConfigureButtons={() => {
              setActiveHiveDirName(hive.dirName);
              setOverlay({ type: "customButtons" });
            }}
          />
        ))}

        {(!activeHive || !currentActiveCombId) && (
          <div className="main-empty">
            <p>{activeHive ? "Select a comb to start working" : "Select a hive to get started"}</p>
            {activeHive && currentCombs.length === 0 && (
              <button
                className="btn btn-primary"
                onClick={() => setOverlay({ type: "newComb" })}
                style={{ marginTop: 12 }}
              >
                + New Comb
              </button>
            )}
          </div>
        )}
      </div>

      {/* Overlays — rendered ON TOP of the layout, keeping terminals alive underneath */}
      {overlay?.type === "newComb" && activeHive && (
        <NewCombModal
          beehiveDir={beehiveDir}
          hive={activeHive}
          existingNames={currentCombs.map((c) => c.name)}
          onCreated={handleCombCreated}
          onClose={() => setOverlay(null)}
        />
      )}

      {overlay?.type === "copyComb" && activeHive && (() => {
        const sourceComb = currentCombs.find((c) => c.id === overlay.sourceCombId);
        if (!sourceComb) return null;
        return (
          <CopyCombModal
            sourceCombName={sourceComb.name}
            existingNames={currentCombs.map((c) => c.name)}
            error={copyCombError}
            onCopy={(newName) => handleCopyComb(overlay.sourceCombId, newName)}
            onClose={() => setOverlay(null)}
          />
        );
      })()}

      {overlay?.type === "manageHives" && (
        <div className="fullscreen-overlay">
          <HiveListScreen
            beehiveDir={beehiveDir}
            onSelectHive={(hive) => {
              setOverlay(null);
              // Ensure hive is in local state (may have been added in the overlay)
              setHives((prev) =>
                prev.some((h) => h.dirName === hive.dirName) ? prev : [...prev, hive]
              );
              selectHive(hive);
            }}
            onSettings={() => setOverlay({ type: "settings", from: "manageHives" })}
            onHelp={() => setOverlay({ type: "help", from: "manageHives" })}
            onBack={activeHive ? () => {
              setOverlay(null);
              loadHives();
            } : undefined}
            backLabel={activeHive ? `Back to ${activeHive.repoName}` : undefined}
            hivesDeleting={hivesDeleting}
            onDeleteHive={handleDeleteHive}
          />
        </div>
      )}

      {overlay?.type === "settings" && (
        <div className="fullscreen-overlay">
          <SettingsScreen
            beehiveDir={beehiveDir}
            onBack={() => {
              if (overlay.from === "manageHives") {
                setOverlay({ type: "manageHives" });
              } else {
                setOverlay(null);
              }
            }}
            onReset={onReset}
            backLabel={overlay.from === "manageHives" ? "Back to Hives" : activeHive ? `Back to ${activeHive.repoName}` : "Back"}
          />
        </div>
      )}

      {overlay?.type === "help" && (
        <div className="fullscreen-overlay">
          <HelpScreen
            onBack={() => {
              if (overlay.from === "manageHives") {
                setOverlay({ type: "manageHives" });
              } else {
                setOverlay(null);
              }
            }}
            backLabel={overlay.from === "manageHives" ? "Back to Hives" : activeHive ? `Back to ${activeHive.repoName}` : "Back"}
          />
        </div>
      )}

      {overlay?.type === "customButtons" && activeHive && (
        <CustomButtonsModal
          buttons={activeHive.customButtons ?? []}
          suggestions={hives
            .filter((h) => h.dirName !== activeHiveDirName)
            .flatMap((h) =>
              (h.customButtons ?? []).map((b) => ({
                ...b,
                hiveName: h.repoName,
              }))
            )}
          onSave={saveCustomButtons}
          onClose={() => setOverlay(null)}
        />
      )}

      {overlay?.type === "createNest" && activeHive && activeRuntime && (
        <CreateNestModal
          existingNests={activeRuntime.nests}
          onCreate={(name) => handleCreateNest(name, overlay.combId)}
          onClose={() => setOverlay(null)}
        />
      )}
      
      {/* Toast notifications */}
      <Toast toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
