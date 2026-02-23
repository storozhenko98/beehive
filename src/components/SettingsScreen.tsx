import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion, getName, getTauriVersion } from "@tauri-apps/api/app";

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
  const [appVersion, setAppVersion] = useState("");
  const [appName, setAppName] = useState("");
  const [tauriVersion, setTauriVersion] = useState("");
  const [cliTarget, setCliTarget] = useState<string | null | undefined>(undefined); // undefined=loading, null=not installed, string=target
  const [cliLoading, setCliLoading] = useState(false);
  const [cliError, setCliError] = useState("");

  useEffect(() => {
    invoke<string>("get_app_config_path").then(setConfigPath);
    invoke<PreflightResult>("preflight_check").then(setPreflight);
    getVersion().then(setAppVersion);
    getName().then(setAppName);
    getTauriVersion().then(setTauriVersion);
    checkCli();
  }, []);

  async function checkCli() {
    try {
      const target = await invoke<string | null>("cli_status");
      setCliTarget(target);
    } catch {
      setCliTarget(null);
    }
  }

  async function handleInstallCli() {
    setCliLoading(true);
    setCliError("");
    try {
      await invoke<string>("install_cli");
      await checkCli();
    } catch (e) {
      setCliError(`${e}`);
    }
    setCliLoading(false);
  }

  async function handleUninstallCli() {
    setCliLoading(true);
    setCliError("");
    try {
      await invoke("uninstall_cli");
      await checkCli();
    } catch (e) {
      setCliError(`${e}`);
    }
    setCliLoading(false);
  }

  function handleReset() {
    if (!confirmReset) {
      setConfirmReset(true);
      return;
    }
    onReset();
  }

  return (
    <div className="settings-page">
      <div className="settings-page-content">
        <div className="settings-page-header">
          <button className="btn-text" onClick={onBack}>
            &larr; {backLabel ?? "Back"}
          </button>
          <h1 style={{ marginTop: 8 }}>Settings</h1>
        </div>

        <div className="settings-section">
          <h3>About</h3>
          <div className="settings-row">
            <span className="settings-label">App</span>
            <span className="settings-value">{appName || "..."}</span>
          </div>
          <div className="settings-row">
            <span className="settings-label">Version</span>
            <span className="settings-value">{appVersion || "..."}</span>
          </div>
          <div className="settings-row">
            <span className="settings-label">Tauri</span>
            <span className="settings-value">{tauriVersion || "..."}</span>
          </div>
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
          <h3>CLI Command</h3>
          <p style={{ color: "var(--text-secondary)", fontSize: 12, marginBottom: 12 }}>
            Install the <code>beehive</code> command to launch the app from any terminal.
          </p>
          {cliTarget === undefined ? (
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>Checking...</p>
          ) : cliTarget ? (
            <>
              <div className="settings-row">
                <span className="settings-label">Status</span>
                <span className="settings-status ok">Installed</span>
              </div>
              <div className="settings-row">
                <span className="settings-label">Symlink</span>
                <code className="settings-value">/usr/local/bin/beehive</code>
              </div>
              <button
                className="btn btn-secondary"
                onClick={handleUninstallCli}
                disabled={cliLoading}
                style={{ marginTop: 8 }}
              >
                {cliLoading ? "Removing..." : "Uninstall CLI"}
              </button>
            </>
          ) : (
            <button
              className="btn btn-primary"
              onClick={handleInstallCli}
              disabled={cliLoading}
            >
              {cliLoading ? "Installing..." : "Install beehive command"}
            </button>
          )}
          {cliError && <div className="error-box" style={{ marginTop: 8 }}>{cliError}</div>}
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
