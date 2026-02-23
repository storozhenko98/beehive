import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PreflightScreen } from "./components/PreflightScreen";
import { SetupScreen } from "./components/SetupScreen";
import { MainLayout } from "./components/MainLayout";
import "./App.css";

interface AppConfig {
  beehiveDir: string | null;
}

type Screen =
  | { name: "loading" }
  | { name: "preflight" }
  | { name: "setup" }
  | { name: "main" };

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
      setScreen({ name: "main" });
    } else {
      setScreen({ name: "setup" });
    }
  }

  async function handleSetup(dir: string) {
    setBeehiveDir(dir);
    await invoke("save_app_config", { config: { beehiveDir: dir } });
    setScreen({ name: "main" });
  }

  async function handleReset() {
    await invoke("reset_app");
    setBeehiveDir("");
    setScreen({ name: "setup" });
  }

  if (screen.name === "loading") {
    return (
      <div className="screen-center">
        <p style={{ color: "var(--text-muted)" }}>Loading...</p>
      </div>
    );
  }

  if (screen.name === "preflight") {
    return <PreflightScreen onPass={handlePreflightPass} />;
  }

  if (screen.name === "setup") {
    return <SetupScreen onSetup={handleSetup} />;
  }

  // main
  return <MainLayout beehiveDir={beehiveDir} onReset={handleReset} />;
}

export default App;
