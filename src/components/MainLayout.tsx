import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Sidebar } from "./Sidebar";
import { WorkspaceGrid } from "./WorkspaceGrid";
import { NewCombModal } from "./NewCombModal";
import { HiveListScreen } from "./HiveListScreen";
import { SettingsScreen } from "./SettingsScreen";
import type { HiveInfo, Comb, PaneConfig } from "../types";

interface Props {
  beehiveDir: string;
  onReset: () => void;
}

type Overlay =
  | null
  | { type: "newComb" }
  | { type: "manageHives" }
  | { type: "settings"; from: "sidebar" | "manageHives" };

// Per-hive runtime state (combs, opened combs, panes, active comb)
interface HiveRuntime {
  combs: Comb[];
  openedCombs: Set<string>;
  panesByComb: Map<string, PaneConfig[]>;
  activeCombId: string | null;
}

export function MainLayout({ beehiveDir, onReset }: Props) {
  const [hives, setHives] = useState<HiveInfo[]>([]);
  const [activeHiveDirName, setActiveHiveDirName] = useState<string | null>(null);
  // Per-hive state keyed by dirName — survives hive switches
  const [hiveRuntimes, setHiveRuntimes] = useState<Map<string, HiveRuntime>>(new Map());
  const [overlay, setOverlay] = useState<Overlay>({ type: "manageHives" });

  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const activeHive = hives.find((h) => h.dirName === activeHiveDirName) ?? null;
  const activeRuntime = activeHiveDirName ? hiveRuntimes.get(activeHiveDirName) : undefined;

  // Load hives on mount
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

  function getOrCreateRuntime(dirName: string): HiveRuntime {
    const existing = hiveRuntimes.get(dirName);
    if (existing) return existing;
    return { combs: [], openedCombs: new Set(), panesByComb: new Map(), activeCombId: null };
  }

  function updateRuntime(dirName: string, updater: (rt: HiveRuntime) => HiveRuntime) {
    setHiveRuntimes((prev) => {
      const current = prev.get(dirName) ?? { combs: [], openedCombs: new Set(), panesByComb: new Map(), activeCombId: null };
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
        const list = await invoke<Comb[]>("list_combs", {
          beehiveDir,
          dirName: hive.dirName,
        });
        updateRuntime(hive.dirName, (rt) => ({ ...rt, combs: list }));
      } catch (e) {
        console.error("Failed to list combs:", e);
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
    (combId: string, type: "agent" | "terminal") => {
      if (!activeHiveDirName) return;
      const hiveDirName = activeHiveDirName;
      updateRuntime(hiveDirName, (rt) => {
        const current = rt.panesByComb.get(combId) ?? [];
        const newPane: PaneConfig = {
          id: crypto.randomUUID(),
          type,
          cmd: type === "agent" ? "claude" : undefined,
        };
        const updated = [...current, newPane];
        debounceSavePanes(hiveDirName, combId, updated);
        return { ...rt, panesByComb: new Map(rt.panesByComb).set(combId, updated) };
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

    updateRuntime(hiveDirName, (rt) => {
      const newOpened = new Set(rt.openedCombs);
      newOpened.delete(combId);
      const newPanes = new Map(rt.panesByComb);
      newPanes.delete(combId);
      const remaining = rt.combs.filter((c) => c.id !== combId);
      const newActive = rt.activeCombId === combId
        ? (remaining.length > 0 ? remaining[0].id : null)
        : rt.activeCombId;
      return { ...rt, openedCombs: newOpened, panesByComb: newPanes, activeCombId: newActive };
    });

    try {
      await invoke("delete_comb", { beehiveDir, dirName: hiveDirName, combId });
      const list = await invoke<Comb[]>("list_combs", { beehiveDir, dirName: hiveDirName });
      updateRuntime(hiveDirName, (rt) => ({ ...rt, combs: list }));
    } catch (e) {
      console.error("Failed to delete comb:", e);
    }
  }

  function handleCombCreated(comb: Comb) {
    if (!activeHiveDirName) return;
    setOverlay(null);
    updateRuntime(activeHiveDirName, (rt) => ({
      ...rt,
      combs: [...rt.combs, comb],
    }));
    openComb(comb);
  }

  // Collect ALL opened combs across ALL hives for rendering
  const allOpenedCombs: { hive: HiveInfo; comb: Comb; panes: PaneConfig[]; isVisible: boolean }[] = [];
  for (const [dirName, runtime] of hiveRuntimes) {
    const hive = hives.find((h) => h.dirName === dirName);
    if (!hive) continue;
    for (const comb of runtime.combs) {
      if (!runtime.openedCombs.has(comb.id)) continue;
      allOpenedCombs.push({
        hive,
        comb,
        panes: runtime.panesByComb.get(comb.id) ?? [],
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
        combs={currentCombs}
        activeCombId={currentActiveCombId}
        onSelectHive={(hive) => {
          selectHive(hive);
        }}
        onSelectComb={openComb}
        onNewComb={() => setOverlay({ type: "newComb" })}
        onManageHives={() => setOverlay({ type: "manageHives" })}
        onSettings={() => setOverlay({ type: "settings", from: "sidebar" })}
        onDeleteComb={handleDeleteComb}
      />

      <div className="main-content">
        {allOpenedCombs.map(({ comb, panes, isVisible }) => (
          <WorkspaceGrid
            key={comb.id}
            comb={comb}
            panes={panes}
            isVisible={isVisible}
            onAddPane={(type) => addPane(comb.id, type)}
            onRemovePane={(paneId) => removePane(comb.id, paneId)}
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
          onCreated={handleCombCreated}
          onClose={() => setOverlay(null)}
        />
      )}

      {overlay?.type === "manageHives" && (
        <div className="fullscreen-overlay">
          <HiveListScreen
            beehiveDir={beehiveDir}
            onSelectHive={(hive) => {
              setOverlay(null);
              selectHive(hive);
            }}
            onSettings={() => setOverlay({ type: "settings", from: "manageHives" })}
            onBack={activeHive ? () => {
              setOverlay(null);
              loadHives();
            } : undefined}
            backLabel={activeHive ? `Back to ${activeHive.repoName}` : undefined}
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
    </div>
  );
}
