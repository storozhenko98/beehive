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
  const [cliTarget, setCliTarget] = useState<string | null | undefined>(undefined);
  const [cliLoading, setCliLoading] = useState(false);
  const [cliError, setCliError] = useState("");
  const [updateStatus, setUpdateStatus] = useState<
    "checking" | "up-to-date" | "available" | "downloading" | "restarting" | "error"
  >("checking");
  const [updateInfo, setUpdateInfo] = useState<Update | null>(null);
  const [updateError, setUpdateError] = useState("");
  const [downloadProgress, setDownloadProgress] = useState(0);

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
    getVersion().then(setAppVersion);
    checkCli();
    checkForUpdates();
  }, [checkForUpdates]);

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
          <h3>CLI Command</h3>
          <p style={{ color: "var(--text-secondary)", fontSize: 12, marginBottom: 8 }}>
            Launch Beehive from any terminal with the <code>beehive</code> command.
          </p>
          {cliTarget === undefined ? (
            <p style={{ color: "var(--text-muted)", fontSize: 12 }}>Checking...</p>
          ) : cliTarget ? (
            <>
              <div className="settings-row">
                <span className="settings-label">Symlink</span>
                <code className="settings-value">/usr/local/bin/beehive</code>
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
            <button
              className="btn btn-primary"
              onClick={handleInstallCli}
              disabled={cliLoading}
            >
              {cliLoading ? "Installing..." : "Install"}
            </button>
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
