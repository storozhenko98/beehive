import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PreflightResult {
  ok: boolean;
  gitAvailable: boolean;
  ghAvailable: boolean;
  ghAuthenticated: boolean;
  messages: string[];
}

interface Props {
  beehiveDir: string;
  onBack: () => void;
  onReset: () => void;
  backLabel?: string;
}

export function SettingsScreen({ beehiveDir, onBack, onReset, backLabel }: Props) {
  const [configPath, setConfigPath] = useState("");
  const [preflight, setPreflight] = useState<PreflightResult | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);

  useEffect(() => {
    invoke<string>("get_app_config_path").then(setConfigPath);
    invoke<PreflightResult>("preflight_check").then(setPreflight);
  }, []);

  function handleReset() {
    if (!confirmReset) {
      setConfirmReset(true);
      return;
    }
    onReset();
  }

  return (
    <div className="screen-center">
      <div className="card" style={{ maxWidth: 560, width: "100%" }}>
        <div style={{ marginBottom: 24 }}>
          <button className="btn-text" onClick={onBack}>
            &larr; {backLabel ?? "Back"}
          </button>
          <h1 style={{ marginTop: 8 }}>Settings</h1>
        </div>

        <div className="settings-section">
          <h3>Paths</h3>
          <div className="settings-row">
            <span className="settings-label">Beehive directory</span>
            <code className="settings-value">{beehiveDir}</code>
          </div>
          <div className="settings-row">
            <span className="settings-label">App config file</span>
            <code className="settings-value">{configPath}</code>
          </div>
        </div>

        <div className="settings-section">
          <h3>Dependencies</h3>
          {preflight ? (
            <div className="settings-checks">
              <div className="settings-row">
                <span className="settings-label">git</span>
                <span className={`settings-status ${preflight.gitAvailable ? "ok" : "fail"}`}>
                  {preflight.gitAvailable ? "Installed" : "Not found"}
                </span>
              </div>
              <div className="settings-row">
                <span className="settings-label">gh CLI</span>
                <span className={`settings-status ${preflight.ghAvailable ? "ok" : "fail"}`}>
                  {preflight.ghAvailable ? "Installed" : "Not found"}
                </span>
              </div>
              <div className="settings-row">
                <span className="settings-label">gh auth</span>
                <span className={`settings-status ${preflight.ghAuthenticated ? "ok" : "fail"}`}>
                  {preflight.ghAuthenticated ? "Authenticated" : "Not authenticated"}
                </span>
              </div>
            </div>
          ) : (
            <p style={{ color: "var(--text-muted)" }}>Checking...</p>
          )}
        </div>

        <div className="settings-section">
          <h3>Danger Zone</h3>
          <p style={{ color: "var(--text-secondary)", fontSize: 12, marginBottom: 12 }}>
            Reset removes the app config file at <code>{configPath}</code>.
            Your hives and combs on disk are not deleted — only the reference to
            the beehive directory is cleared. Next launch will show the setup screen.
          </p>
          <button
            className={`btn ${confirmReset ? "btn-danger" : "btn-secondary"}`}
            onClick={handleReset}
          >
            {confirmReset ? "Are you sure? Click again to reset" : "Reset Beehive Setup"}
          </button>
          {confirmReset && (
            <button
              className="btn btn-secondary"
              onClick={() => setConfirmReset(false)}
              style={{ marginLeft: 8 }}
            >
              Cancel
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
