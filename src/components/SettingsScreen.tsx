import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

interface PreflightResult {
  ok: boolean;
  gitAvailable: boolean;
  ghAvailable: boolean;
  ghAuthenticated: boolean;
  messages: string[];
}

interface AppConfig {
  beehiveDir: string | null;
  muxPreference?: string | null;
  cliCommand?: string | null;
  combStartupCommand?: string | null;
  sidebarWidth?: number;
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
  const [cliInstalled, setCliInstalled] = useState<boolean | undefined>(undefined);
  const [cliCmdName, setCliCmdName] = useState<string | null>(null);
  const [cliPath, setCliPath] = useState<string | null>(null);
  const [cliChoice, setCliChoice] = useState<"bh" | "beehive">("bh");
  const [cliLoading, setCliLoading] = useState(false);
  const [cliError, setCliError] = useState("");
  const [updateStatus, setUpdateStatus] = useState<
    "checking" | "up-to-date" | "available" | "downloading" | "restarting" | "error"
  >("checking");
  const [updateInfo, setUpdateInfo] = useState<Update | null>(null);
  const [updateError, setUpdateError] = useState("");
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
  const [combStartupCommand, setCombStartupCommand] = useState("");
  const [startupSaving, setStartupSaving] = useState(false);
  const [startupError, setStartupError] = useState("");
  const [startupSaved, setStartupSaved] = useState(false);

  const checkForUpdates = useCallback(async () => {
    setUpdateStatus("checking");
    setUpdateError("");
    try {
      const update = await check();
      if (update) {
        setUpdateInfo(update);
        setUpdateStatus("available");
      } else {
        setUpdateStatus("up-to-date");
      }
    } catch (e) {
      setUpdateError(`${e}`);
      setUpdateStatus("error");
    }
  }, []);

  useEffect(() => {
    invoke<string>("get_app_config_path").then(setConfigPath);
    invoke<PreflightResult>("preflight_check").then(setPreflight);
    invoke<AppConfig>("load_app_config").then((config) => {
      setAppConfig(config);
      setCombStartupCommand(config.combStartupCommand ?? "");
    });
    getVersion().then(setAppVersion);
    checkCli();
    checkForUpdates();
  }, [checkForUpdates]);

  async function handleSaveStartupCommand() {
    setStartupSaving(true);
    setStartupError("");
    setStartupSaved(false);

    const trimmed = combStartupCommand.trim();
    const nextConfig: AppConfig = appConfig
      ? { ...appConfig, combStartupCommand: trimmed || null }
      : { beehiveDir, combStartupCommand: trimmed || null };

    try {
      await invoke("save_app_config", { config: nextConfig });
      setAppConfig(nextConfig);
      setCombStartupCommand(trimmed);
      setStartupSaved(true);
    } catch (e) {
      setStartupError(`${e}`);
    }

    setStartupSaving(false);
  }

  async function checkCli() {
    try {
      const result = await invoke<{ installed: boolean; cmdName: string | null; path: string | null }>("cli_status");
      setCliInstalled(result.installed);
      setCliCmdName(result.cmdName);
      setCliPath(result.path);
      if (result.cmdName) {
        setCliChoice(result.cmdName as "bh" | "beehive");
      }
    } catch {
      setCliInstalled(false);
    }
  }

  async function handleInstallCli() {
    setCliLoading(true);
    setCliError("");
    try {
      await invoke<string>("install_cli", { cmdName: cliChoice });
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

  async function handleDownloadUpdate() {
    if (!updateInfo) return;
    setUpdateStatus("downloading");
    setDownloadProgress(0);
    try {
      let totalBytes = 0;
      let downloadedBytes = 0;
      await updateInfo.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          totalBytes = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          if (totalBytes > 0) {
            setDownloadProgress(Math.round((downloadedBytes / totalBytes) * 100));
          }
        } else if (event.event === "Finished") {
          setDownloadProgress(100);
        }
      });
      setUpdateStatus("restarting");
      await relaunch();
    } catch (e) {
      setUpdateError(`${e}`);
      setUpdateStatus("error");
    }
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
          <h3>Beehive v{appVersion || "..."}</h3>
          {updateStatus === "checking" && (
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>
              Checking for updates...
            </p>
          )}
          {updateStatus === "up-to-date" && (
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <p style={{ color: "var(--success)", fontSize: 12 }}>
                Up to date
              </p>
              <button className="btn-text" onClick={checkForUpdates}>
                Check again
              </button>
            </div>
          )}
          {updateStatus === "available" && updateInfo && (
            <>
              <p style={{ color: "var(--accent)", fontSize: 12, marginBottom: 6 }}>
                v{updateInfo.version} available
              </p>
              <div style={{ display: "flex", gap: 8 }}>
                <button className="btn btn-primary" onClick={handleDownloadUpdate}>
                  Download & Install
                </button>
                <button className="btn btn-secondary" onClick={checkForUpdates}>
                  Check Again
                </button>
              </div>
            </>
          )}
          {updateStatus === "downloading" && (
            <div>
              <div style={{
                height: 6,
                borderRadius: 3,
                background: "var(--bg-surface)",
                overflow: "hidden",
                marginBottom: 6,
              }}>
                <div style={{
                  height: "100%",
                  width: `${downloadProgress}%`,
                  background: "var(--accent)",
                  borderRadius: 3,
                  transition: "width 0.2s",
                }} />
              </div>
              <p style={{ color: "var(--text-muted)", fontSize: 12 }}>
                Downloading... {downloadProgress}%
              </p>
            </div>
          )}
          {updateStatus === "restarting" && (
            <p style={{ color: "var(--accent)", fontSize: 12 }}>
              Restarting...
            </p>
          )}
          {updateStatus === "error" && (
            <>
              <p style={{ color: "var(--warning)", fontSize: 12 }}>
                Could not check for updates.
              </p>
              {updateError && (
                <p style={{ color: "var(--warning)", fontSize: 11, marginTop: 4, fontFamily: "'Menlo', monospace", opacity: 0.7 }}>
                  {updateError}
                </p>
              )}
              <button
                className="btn btn-secondary"
                onClick={checkForUpdates}
                style={{ marginTop: 6 }}
              >
                Retry
              </button>
            </>
          )}
        </div>

        <div className="settings-section">
          <h3>Comb startup</h3>
          <p style={{ color: "var(--text-secondary)", fontSize: 12, marginBottom: 8 }}>
            Runs once per comb when Beehive opens that comb for the first time after launch, then drops into an interactive shell.
          </p>
          <input
            type="text"
            value={combStartupCommand}
            onChange={(e) => {
              setCombStartupCommand(e.target.value);
              setStartupSaved(false);
              if (startupError) setStartupError("");
            }}
            placeholder='e.g. tmux new-session -A -s "$(basename "$BEEHIVE_COMB")"'
            spellCheck={false}
            style={{ width: "100%", marginBottom: 8 }}
          />
          <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <button
              className="btn btn-primary"
              onClick={handleSaveStartupCommand}
              disabled={startupSaving}
            >
              {startupSaving ? "Saving..." : "Save startup command"}
            </button>
            {combStartupCommand && (
              <button
                className="btn btn-secondary"
                onClick={() => {
                  setCombStartupCommand("");
                  setStartupSaved(false);
                }}
                disabled={startupSaving}
              >
                Clear
              </button>
            )}
            {startupSaved && (
              <span style={{ color: "var(--success)", fontSize: 12 }}>
                Saved
              </span>
            )}
          </div>
          <p style={{ color: "var(--text-muted)", fontSize: 11, marginTop: 6 }}>
            Available in the shell as <code>BEEHIVE_COMB</code>.
          </p>
          {startupError && <div className="error-box" style={{ marginTop: 6 }}>{startupError}</div>}
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
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>Checking...</p>
          )}
        </div>

        <div className="settings-section">
          <h3>CLI / TUI</h3>
          <p style={{ color: "var(--text-secondary)", fontSize: 12, marginBottom: 8 }}>
            Install the Beehive TUI for terminal use. Same workspaces, shared config.
          </p>
          {cliInstalled === undefined ? (
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>Checking...</p>
          ) : cliInstalled ? (
            <>
              <div className="settings-row">
                <span className="settings-label">Command</span>
                <code className="settings-value">{cliCmdName}</code>
              </div>
              <div className="settings-row">
                <span className="settings-label">Path</span>
                <code className="settings-value">{cliPath}</code>
              </div>
              <button
                className="btn btn-secondary"
                onClick={handleUninstallCli}
                disabled={cliLoading}
                style={{ marginTop: 6 }}
              >
                {cliLoading ? "Removing..." : "Uninstall"}
              </button>
            </>
          ) : (
            <>
              <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
                <button
                  className={`btn ${cliChoice === "bh" ? "btn-primary" : "btn-secondary"}`}
                  onClick={() => setCliChoice("bh")}
                  style={{ padding: "4px 14px", fontSize: 12 }}
                >
                  bh
                </button>
                <button
                  className={`btn ${cliChoice === "beehive" ? "btn-primary" : "btn-secondary"}`}
                  onClick={() => setCliChoice("beehive")}
                  style={{ padding: "4px 14px", fontSize: 12 }}
                >
                  beehive
                </button>
                <span style={{ color: "var(--text-muted)", fontSize: 11, alignSelf: "center" }}>
                  installed as /usr/local/bin/{cliChoice}
                </span>
              </div>
              <button
                className="btn btn-primary"
                onClick={handleInstallCli}
                disabled={cliLoading}
              >
                {cliLoading ? "Downloading..." : "Install"}
              </button>
            </>
          )}
          {cliError && <div className="error-box" style={{ marginTop: 6 }}>{cliError}</div>}
        </div>

        <div className="settings-section">
          <h3>Feedback</h3>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              className="btn btn-primary"
              onClick={() => openUrl("https://github.com/storozhenko98/beehive/issues/new")}
            >
              Report Issue
            </button>
            <button
              className="btn btn-secondary"
              onClick={() => openUrl("https://github.com/storozhenko98/beehive")}
            >
              GitHub
            </button>
          </div>
        </div>

        <div className="settings-section">
          <h3>Advanced</h3>
          <div className="settings-row">
            <span className="settings-label">Beehive directory</span>
            <code className="settings-value">{beehiveDir}</code>
          </div>
          <div className="settings-row">
            <span className="settings-label">Config file</span>
            <code className="settings-value">{configPath}</code>
          </div>
        </div>

        <div className="settings-section">
          <h3>Reset</h3>
          <p style={{ color: "var(--text-secondary)", fontSize: 12, marginBottom: 8 }}>
            Clears the app config. Your repos and workspaces stay on disk.
          </p>
          <button
            className={`btn ${confirmReset ? "btn-danger" : "btn-secondary"}`}
            onClick={handleReset}
          >
            {confirmReset ? "Confirm Reset" : "Reset Beehive"}
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
