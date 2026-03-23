use std::fs;

use crate::config::{cmd_with_path, run_cmd};

const REPO: &str = "storozhenko98/beehive";
const INSTALL_CMD: &str =
    "curl -fsSL https://raw.githubusercontent.com/storozhenko98/beehive/main/install.sh | bash";

fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("beehive-tui-darwin-arm64"),
        ("linux", "x86_64") => Some("beehive-tui-linux-x64"),
        _ => None,
    }
}

/// Check GitHub for a newer release. Returns `Some(version)` if an update is available.
pub fn check_for_update() -> Option<String> {
    let output = run_cmd(
        "gh",
        &[
            "api",
            &format!("repos/{}/releases/latest", REPO),
            "--jq",
            ".tag_name",
        ],
    )
    .ok()?;

    let remote_tag = output.trim().trim_start_matches('v');
    let current = env!("CARGO_PKG_VERSION");

    if version_newer(remote_tag, current) {
        Some(remote_tag.to_string())
    } else {
        None
    }
}

/// Try to self-update the binary in place. Returns Ok(()) on success,
/// or Err with a user-friendly message (including the manual install command).
pub fn self_update(version: &str) -> Result<(), String> {
    let tag = format!("v{}", version);
    let asset = asset_name().ok_or_else(|| {
        format!(
            "Auto-update is not available on this platform yet. Update manually:\n  {}",
            INSTALL_CMD
        )
    })?;
    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        REPO, tag, asset
    );

    // Download to a temp file
    let tmp = std::env::temp_dir().join("beehive-tui-update");
    let output = cmd_with_path("curl")
        .args(["-fsSL", "-o", &tmp.to_string_lossy(), &url])
        .output()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Download failed. Update manually:\n  {}",
            INSTALL_CMD
        ));
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
    }

    // Find current executable path
    let current_exe = std::env::current_exe().map_err(|_| {
        format!(
            "Cannot find current binary. Update manually:\n  {}",
            INSTALL_CMD
        )
    })?;

    // Try to replace
    match fs::rename(&tmp, &current_exe) {
        Ok(()) => Ok(()),
        Err(_) => {
            // rename failed (cross-device or permissions) — try copy
            match fs::copy(&tmp, &current_exe) {
                Ok(_) => {
                    let _ = fs::remove_file(&tmp);
                    Ok(())
                }
                Err(_) => {
                    let _ = fs::remove_file(&tmp);
                    Err(format!(
                        "Permission denied. Update manually:\n  {}",
                        INSTALL_CMD
                    ))
                }
            }
        }
    }
}

/// Compare two semver-like version strings (e.g. "0.2.0" > "0.1.5").
fn version_newer(remote: &str, current: &str) -> bool {
    let parse =
        |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse::<u32>().ok()).collect() };
    let r = parse(remote);
    let c = parse(current);
    for i in 0..r.len().max(c.len()) {
        let rv = r.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if rv > cv {
            return true;
        }
        if rv < cv {
            return false;
        }
    }
    false
}
