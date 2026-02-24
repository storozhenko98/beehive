import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";
import { SetupScreen } from "./components/SetupScreen";
import { MainLayout } from "./components/MainLayout";
import "./App.css";

interface AppConfig {
  beehiveDir: string | null;
}

interface PreflightResult {
  ok: boolean;
  gitAvailable: boolean;
  ghAvailable: boolean;
  ghAuthenticated: boolean;
  messages: string[];
}

type Screen =
  | { name: "loading" }
  | { name: "setup" }
  | { name: "main" };

function App() {
  const [screen, setScreen] = useState<Screen>({ name: "loading" });
  const [beehiveDir, setBeehiveDir] = useState<string>("");
  const [preflightWarnings, setPreflightWarnings] = useState<string[]>([]);
  const [warningDismissed, setWarningDismissed] = useState(false);
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [updateDismissed, setUpdateDismissed] = useState(false);

  useEffect(() => {
    loadConfig();
  }, []);

  async function loadConfig() {
    try {
      const config = await invoke<AppConfig>("load_app_config");
      if (config.beehiveDir) {
        setBeehiveDir(config.beehiveDir);
        setScreen({ name: "main" });
      } else {
        setScreen({ name: "setup" });
      }
    } catch {
      setScreen({ name: "setup" });
    }

    // Run preflight in background — show warning if issues found
    try {
      const r = await invoke<PreflightResult>("preflight_check");
      if (!r.ok) {
        const warnings: string[] = [];
        if (!r.gitAvailable) warnings.push("Install git: https://git-scm.com");
        if (!r.ghAvailable) warnings.push("Install gh CLI: https://cli.github.com");
        else if (!r.ghAuthenticated) warnings.push("Authenticate gh: run gh auth login");
        setPreflightWarnings(warnings);
      }
    } catch {
      // ignore
    }

    // Check for updates in background
    try {
      const update = await check();
      if (update) {
        setUpdateVersion(update.version);
      }
    } catch {
      // ignore
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

  if (screen.name === "setup") {
    return <SetupScreen onSetup={handleSetup} />;
  }

  // main
  return (
    <>
      <MainLayout beehiveDir={beehiveDir} onReset={handleReset} />
      {updateVersion && !updateDismissed && (
        <div className="update-banner">
          <span>Beehive v{updateVersion} is available</span>
          <span className="update-banner-hint">Update in Settings</span>
          <button onClick={() => setUpdateDismissed(true)}>x</button>
        </div>
      )}
      {preflightWarnings.length > 0 && !warningDismissed && (
        <div className="preflight-warning">
          <div className="preflight-warning-content">
            <strong>Missing dependencies</strong>
            {preflightWarnings.map((w, i) => (
              <span key={i}>{w}</span>
            ))}
          </div>
          <button onClick={() => setWarningDismissed(true)}>x</button>
        </div>
      )}
    </>
  );
}

export default App;
