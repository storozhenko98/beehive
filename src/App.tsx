import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PreflightScreen } from "./components/PreflightScreen";
import { SetupScreen } from "./components/SetupScreen";
import { HiveListScreen } from "./components/HiveListScreen";
import { CombListScreen } from "./components/CombListScreen";
import { WorkspaceScreen } from "./components/WorkspaceScreen";
import { SettingsScreen } from "./components/SettingsScreen";
import type { Comb, HiveInfo } from "./types";
import "./App.css";

interface AppConfig {
  beehiveDir: string | null;
}

type Screen =
  | { name: "loading" }
  | { name: "preflight" }
  | { name: "setup" }
  | { name: "hives" }
  | { name: "combs"; hive: HiveInfo }
  | { name: "workspace"; hive: HiveInfo; comb: Comb }
  | { name: "settings" };

function App() {
  const [screen, setScreen] = useState<Screen>({ name: "loading" });
  const [beehiveDir, setBeehiveDir] = useState<string>("");

  useEffect(() => {
    loadConfig();
  }, []);

  async function loadConfig() {
    try {
      const config = await invoke<AppConfig>("load_app_config");
      if (config.beehiveDir) {
        setBeehiveDir(config.beehiveDir);
      }
      setScreen({ name: "preflight" });
    } catch {
      setScreen({ name: "preflight" });
    }
  }

  function handlePreflightPass() {
    if (beehiveDir) {
      setScreen({ name: "hives" });
    } else {
      setScreen({ name: "setup" });
    }
  }

  async function handleSetup(dir: string) {
    setBeehiveDir(dir);
    await invoke("save_app_config", { config: { beehiveDir: dir } });
    setScreen({ name: "hives" });
  }

  async function handleReset() {
    await invoke("reset_app");
    setBeehiveDir("");
    setScreen({ name: "setup" });
  }

  function handleSelectHive(hive: HiveInfo) {
    setScreen({ name: "combs", hive });
  }

  function handleSelectComb(hive: HiveInfo, comb: Comb) {
    setScreen({ name: "workspace", hive, comb });
  }

  if (screen.name === "loading") {
    return (
      <div className="screen-center">
        <p style={{ color: "var(--text-muted)" }}>Loading...</p>
      </div>
    );
  }

  if (screen.name === "settings") {
    return (
      <SettingsScreen
        beehiveDir={beehiveDir}
        onBack={() => setScreen({ name: "hives" })}
        onReset={handleReset}
      />
    );
  }

  if (screen.name === "preflight") {
    return <PreflightScreen onPass={handlePreflightPass} />;
  }

  if (screen.name === "setup") {
    return <SetupScreen onSetup={handleSetup} />;
  }

  if (screen.name === "workspace") {
    return (
      <WorkspaceScreen
        beehiveDir={beehiveDir}
        hive={screen.hive}
        comb={screen.comb}
        onBack={() => setScreen({ name: "combs", hive: screen.hive })}
      />
    );
  }

  if (screen.name === "combs") {
    return (
      <CombListScreen
        beehiveDir={beehiveDir}
        hive={screen.hive}
        onBack={() => setScreen({ name: "hives" })}
        onSelectComb={(comb) => handleSelectComb(screen.hive, comb)}
      />
    );
  }

  // hives
  return (
    <HiveListScreen
      beehiveDir={beehiveDir}
      onSelectHive={handleSelectHive}
      onSettings={() => setScreen({ name: "settings" })}
    />
  );
}

export default App;
