use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// --- Data types (shared with GUI, camelCase serde) ---

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub beehive_dir: Option<String>,
    #[serde(default)]
    pub mux_preference: Option<String>,
    #[serde(default)]
    pub cli_command: Option<String>,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,
}

fn default_sidebar_width() -> u16 {
    28
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BeehiveConfig {
    pub version: u32,
    pub beehive_dir: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomButton {
    pub label: String,
    pub cmd: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HiveInfo {
    pub dir_name: String,
    pub repo_url: String,
    pub repo_name: String,
    pub owner: String,
    pub description: Option<String>,
    pub default_branch: Option<String>,
    #[serde(default)]
    pub custom_buttons: Vec<CustomButton>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PaneConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub pane_type: String,
    pub cmd: Option<String>,
    pub args: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Comb {
    pub id: String,
    pub name: String,
    pub branch: String,
    pub path: String,
    pub created_at: String,
    #[serde(default)]
    pub panes: Vec<PaneConfig>,
    #[serde(default)]
    pub cloning: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiveState {
    pub info: HiveInfo,
    pub combs: Vec<Comb>,
}

// --- Preflight ---

pub struct PreflightResult {
    pub git: Option<String>,
    pub gh: Option<String>,
    pub gh_auth: bool,
}

pub fn preflight() -> PreflightResult {
    let git = run_cmd("git", &["--version"])
        .ok()
        .map(|v| v.trim().to_string());

    let gh = run_cmd("gh", &["--version"])
        .ok()
        .map(|v| v.lines().next().unwrap_or("").trim().to_string());

    let gh_auth = gh.is_some() && run_cmd("gh", &["auth", "status"]).is_ok();

    PreflightResult { git, gh, gh_auth }
}

pub fn reset_config() -> Result<(), String> {
    let path = app_config_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Failed to remove config: {}", e))?;
    }
    Ok(())
}

// --- Path/command helpers ---

pub fn full_path() -> String {
    let extra = [
        "/opt/homebrew/bin",
        "/opt/homebrew/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
    ];
    let system_path = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<&str> = extra.to_vec();
    for p in system_path.split(':') {
        if !parts.contains(&p) {
            parts.push(p);
        }
    }
    parts.join(":")
}

pub fn cmd_with_path(cmd: &str) -> Command {
    let mut c = Command::new(cmd);
    c.env("PATH", full_path());
    c
}

pub fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = cmd_with_path(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

// --- Config I/O ---

pub fn app_config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".beehive")
        .join("config.json")
}

pub fn load_app_config() -> Result<AppConfig, String> {
    let path = app_config_path();
    if !path.exists() {
        return Ok(AppConfig {
            beehive_dir: None,
            mux_preference: None,
            cli_command: None,
            sidebar_width: 28,
        });
    }
    let data =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read app config: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse app config: {}", e))
}

pub fn save_app_config(config: &AppConfig) -> Result<(), String> {
    let path = app_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create ~/.beehive: {}", e))?;
    }
    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write app config: {}", e))
}

pub fn init_beehive(dir: &str) -> Result<(), String> {
    let path = Path::new(dir);
    fs::create_dir_all(path).map_err(|e| format!("Failed to create directory: {}", e))?;
    let config_path = path.join("beehive.json");
    if !config_path.exists() {
        let config = BeehiveConfig {
            version: 1,
            beehive_dir: dir.to_string(),
        };
        let json = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        fs::write(&config_path, json).map_err(|e| format!("Failed to write config: {}", e))?;
    }
    Ok(())
}

// --- Hive/Comb I/O ---

pub fn load_hive_state(beehive_dir: &str, dir_name: &str) -> Result<HiveState, String> {
    let state_path = Path::new(beehive_dir)
        .join(dir_name)
        .join(".hive")
        .join("state.json");
    let data =
        fs::read_to_string(&state_path).map_err(|e| format!("Failed to read hive state: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse hive state: {}", e))
}

pub fn save_hive_state(beehive_dir: &str, dir_name: &str, state: &HiveState) -> Result<(), String> {
    let state_path = Path::new(beehive_dir)
        .join(dir_name)
        .join(".hive")
        .join("state.json");
    let json =
        serde_json::to_string_pretty(state).map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&state_path, json).map_err(|e| format!("Failed to write state: {}", e))
}

pub fn list_hives(beehive_dir: &str) -> Result<Vec<HiveInfo>, String> {
    let base = Path::new(beehive_dir);
    if !base.exists() {
        return Ok(vec![]);
    }
    let mut hives = vec![];
    let entries = fs::read_dir(base).map_err(|e| format!("Failed to read dir: {}", e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("repo_") {
            let state_path = entry.path().join(".hive").join("state.json");
            if state_path.exists() {
                if let Ok(data) = fs::read_to_string(&state_path) {
                    if let Ok(state) = serde_json::from_str::<HiveState>(&data) {
                        if !state.info.repo_name.is_empty() {
                            hives.push(state.info);
                        }
                    }
                }
            }
        }
    }
    hives.sort_by(|a, b| a.repo_name.to_lowercase().cmp(&b.repo_name.to_lowercase()));
    Ok(hives)
}

pub fn get_combs(beehive_dir: &str, dir_name: &str) -> Result<Vec<Comb>, String> {
    let mut state = load_hive_state(beehive_dir, dir_name)?;
    for comb in &mut state.combs {
        if let Some(branch) = get_git_branch(&comb.path) {
            if branch != comb.branch {
                comb.branch = branch;
            }
        }
    }
    Ok(state.combs)
}

pub fn get_git_branch(path: &str) -> Option<String> {
    let p = Path::new(path);
    if !p.exists() {
        return None;
    }
    let output = cmd_with_path("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(p)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn list_branches(beehive_dir: &str, dir_name: &str) -> Result<(Vec<String>, String), String> {
    let state = load_hive_state(beehive_dir, dir_name)?;
    let repo_spec = format!("{}/{}", state.info.owner, state.info.repo_name);
    let default_branch = state
        .info
        .default_branch
        .unwrap_or_else(|| "main".to_string());

    let output = run_cmd(
        "gh",
        &[
            "api",
            &format!("repos/{}/branches?per_page=100", repo_spec),
            "--paginate",
            "--jq",
            ".[].name",
        ],
    )?;

    let branches: Vec<String> = output.lines().map(|l| l.to_string()).collect();
    Ok((branches, default_branch))
}

/// Reorder the combs in state.json to match the given ID order.
/// IDs not in the list are appended at the end (preserves combs added externally).
pub fn reorder_combs(
    beehive_dir: &str,
    hive_dir_name: &str,
    comb_ids: &[String],
) -> Result<(), String> {
    let mut state = load_hive_state(beehive_dir, hive_dir_name)?;

    let mut ordered: Vec<Comb> = Vec::with_capacity(state.combs.len());
    for id in comb_ids {
        if let Some(pos) = state.combs.iter().position(|c| c.id == *id) {
            ordered.push(state.combs.remove(pos));
        }
    }
    // Append any combs not in the ID list (e.g. added by another process during the move)
    ordered.append(&mut state.combs);
    state.combs = ordered;

    save_hive_state(beehive_dir, hive_dir_name, &state)
}

pub fn rename_comb(
    beehive_dir: &str,
    hive_dir_name: &str,
    comb_id: &str,
    new_name: &str,
) -> Result<Comb, String> {
    let mut state = load_hive_state(beehive_dir, hive_dir_name)?;

    let Some(index) = state.combs.iter().position(|comb| comb.id == comb_id) else {
        return Err(format!("Comb '{}' not found", comb_id));
    };

    if state.combs[index].cloning {
        return Err("Cannot rename a comb that is still in progress".to_string());
    }

    let current_name = state.combs[index].name.clone();
    if new_name == current_name {
        return Err("Comb already has that name".to_string());
    }

    let existing_combs: Vec<Comb> = state
        .combs
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, comb)| comb.clone())
        .collect();
    validate_comb_name(new_name, &existing_combs)?;

    let old_path = PathBuf::from(&state.combs[index].path);
    let Some(parent) = old_path.parent() else {
        return Err(format!("Invalid comb path '{}'", state.combs[index].path));
    };
    let new_path = parent.join(new_name);

    if rename_target_conflicts(&old_path, &new_path)? {
        return Err(format!("Directory '{}' already exists", new_name));
    }

    fs::rename(&old_path, &new_path).map_err(|e| format!("Failed to rename comb directory: {}", e))?;

    state.combs[index].name = new_name.to_string();
    state.combs[index].path = new_path.to_string_lossy().to_string();
    let renamed = state.combs[index].clone();

    if let Err(e) = save_hive_state(beehive_dir, hive_dir_name, &state) {
        let _ = fs::rename(&new_path, &old_path);
        return Err(format!("Failed to save renamed comb: {}", e));
    }

    Ok(renamed)
}

fn rename_target_conflicts(old_path: &Path, new_path: &Path) -> Result<bool, String> {
    if !new_path.exists() {
        return Ok(false);
    }

    Ok(
        fs::canonicalize(old_path)
            .map_err(|e| format!("Failed to inspect current comb directory: {}", e))?
            != fs::canonicalize(new_path)
                .map_err(|e| format!("Failed to inspect rename target: {}", e))?,
    )
}

pub fn validate_comb_name(name: &str, existing_combs: &[Comb]) -> Result<(), String> {
    if name.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if name.len() > 40 {
        return Err("Max 40 characters".to_string());
    }
    if name.starts_with('.') || name.starts_with('-') {
        return Err("Cannot start with '.' or '-'".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Letters, numbers, hyphens, underscores only".to_string());
    }
    if name == ".hive" {
        return Err("Reserved name".to_string());
    }
    if existing_combs.iter().any(|c| c.name == name) {
        return Err(format!("'{}' already exists", name));
    }
    Ok(())
}

pub fn parse_repo_url(url: &str) -> Result<(String, String), String> {
    let cleaned = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let (owner, repo_name) = if cleaned.contains(':') && cleaned.starts_with("git@") {
        let after_colon = cleaned.split(':').last().ok_or("Invalid SSH URL")?;
        let parts: Vec<&str> = after_colon.split('/').collect();
        if parts.len() >= 2 {
            (
                parts[parts.len() - 2].to_string(),
                parts[parts.len() - 1].to_string(),
            )
        } else {
            return Err(format!("Cannot parse: {}", url));
        }
    } else if cleaned.contains("github.com/") {
        let after_gh = cleaned
            .split("github.com/")
            .last()
            .ok_or("Invalid GitHub URL")?;
        let parts: Vec<&str> = after_gh.split('/').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            return Err(format!("Cannot parse: {}", url));
        }
    } else {
        let parts: Vec<&str> = cleaned.split('/').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            return Err(format!("Use owner/repo or full URL: {}", url));
        }
    };
    if owner.is_empty() || repo_name.is_empty() {
        return Err(format!("Invalid: {}", url));
    }
    Ok((owner, repo_name))
}

/// Clean up stale `cloning: true` entries left by a crash or interrupted clone/copy.
/// For each cloning comb: if the directory is a valid git repo, mark it complete;
/// otherwise remove the entry and clean up any partial directory.
pub fn cleanup_stale_cloning(beehive_dir: &str) {
    let hives = match list_hives(beehive_dir) {
        Ok(h) => h,
        Err(_) => return,
    };
    for hive in hives {
        let mut state = match load_hive_state(beehive_dir, &hive.dir_name) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut changed = false;
        let mut cleaned_combs = Vec::new();
        for comb in state.combs.drain(..) {
            if comb.cloning {
                let comb_path = Path::new(&comb.path);
                let git_dir = comb_path.join(".git");
                if comb_path.exists() && git_dir.exists() {
                    // Clone/copy completed but flag wasn't flipped — recover it
                    let mut recovered = comb;
                    recovered.cloning = false;
                    cleaned_combs.push(recovered);
                } else {
                    // Incomplete — remove the directory if it exists and drop the entry
                    if comb_path.exists() {
                        let _ = fs::remove_dir_all(comb_path);
                    }
                }
                changed = true;
            } else {
                cleaned_combs.push(comb);
            }
        }
        if changed {
            state.combs = cleaned_combs;
            let _ = save_hive_state(beehive_dir, &hive.dir_name, &state);
        }
    }
}

pub fn chrono_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn unique_temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("beehive-config-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn rename_comb_updates_state_and_directory() {
        let beehive_dir = unique_temp_dir();
        let hive_dir_name = "repo_demo";
        let hive_dir = beehive_dir.join(hive_dir_name);
        let dot_hive = hive_dir.join(".hive");
        let old_dir = hive_dir.join("alpha");

        fs::create_dir_all(&dot_hive).unwrap();
        fs::create_dir_all(&old_dir).unwrap();

        let state = HiveState {
            info: HiveInfo {
                dir_name: hive_dir_name.to_string(),
                repo_url: "https://github.com/acme/demo.git".to_string(),
                repo_name: "demo".to_string(),
                owner: "acme".to_string(),
                description: None,
                default_branch: Some("main".to_string()),
                custom_buttons: vec![],
            },
            combs: vec![Comb {
                id: "comb-1".to_string(),
                name: "alpha".to_string(),
                branch: "main".to_string(),
                path: old_dir.to_string_lossy().to_string(),
                created_at: "0".to_string(),
                panes: vec![],
                cloning: false,
            }],
        };
        save_hive_state(beehive_dir.to_str().unwrap(), hive_dir_name, &state).unwrap();

        let renamed =
            rename_comb(beehive_dir.to_str().unwrap(), hive_dir_name, "comb-1", "beta").unwrap();

        let new_dir = hive_dir.join("beta");
        assert_eq!(renamed.name, "beta");
        assert_eq!(renamed.path, new_dir.to_string_lossy().to_string());
        assert!(!old_dir.exists());
        assert!(new_dir.exists());

        let saved = load_hive_state(beehive_dir.to_str().unwrap(), hive_dir_name).unwrap();
        assert_eq!(saved.combs[0].name, "beta");
        assert_eq!(saved.combs[0].path, new_dir.to_string_lossy().to_string());

        let _ = fs::remove_dir_all(&beehive_dir);
    }

    #[cfg(unix)]
    #[test]
    fn rename_target_conflicts_allows_same_entry() {
        let temp_dir = unique_temp_dir();
        let old_dir = temp_dir.join("alpha");
        let alias_dir = temp_dir.join("alpha-link");

        fs::create_dir_all(&old_dir).unwrap();
        symlink(&old_dir, &alias_dir).unwrap();

        assert!(!rename_target_conflicts(&old_dir, &alias_dir).unwrap());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn rename_target_conflicts_rejects_other_entry() {
        let temp_dir = unique_temp_dir();
        let old_dir = temp_dir.join("alpha");
        let other_dir = temp_dir.join("beta");

        fs::create_dir_all(&old_dir).unwrap();
        fs::create_dir_all(&other_dir).unwrap();

        assert!(rename_target_conflicts(&old_dir, &other_dir).unwrap());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
