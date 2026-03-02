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

pub fn copy_comb(
    beehive_dir: &str,
    hive_dir_name: &str,
    source_path: &str,
    new_name: &str,
) -> Result<Comb, String> {
    let hive_dir = Path::new(beehive_dir).join(hive_dir_name);
    let dest_dir = hive_dir.join(new_name);

    if dest_dir.exists() {
        return Err(format!("Directory '{}' already exists", new_name));
    }

    // Recursive copy
    let status = Command::new("cp")
        .args(["-r", source_path, &dest_dir.to_string_lossy()])
        .status()
        .map_err(|e| format!("Copy failed: {}", e))?;

    if !status.success() {
        let _ = fs::remove_dir_all(&dest_dir);
        return Err("Copy failed".to_string());
    }

    // Read the branch from the copied directory
    let branch = get_git_branch(&dest_dir.to_string_lossy())
        .unwrap_or_else(|| "main".to_string());

    let comb = Comb {
        id: uuid::Uuid::new_v4().to_string(),
        name: new_name.to_string(),
        branch,
        path: dest_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        panes: vec![],
        cloning: false,
    };

    let mut state = load_hive_state(beehive_dir, hive_dir_name)?;
    state.combs.push(comb.clone());
    save_hive_state(beehive_dir, hive_dir_name, &state)?;
    Ok(comb)
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

pub fn save_hive_state(
    beehive_dir: &str,
    dir_name: &str,
    state: &HiveState,
) -> Result<(), String> {
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
        Some(
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string(),
        )
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

pub fn chrono_now() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}
