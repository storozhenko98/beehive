use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BeehiveConfig {
    pub version: u32,
    pub beehive_dir: String,
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
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiveState {
    pub info: HiveInfo,
    pub combs: Vec<Comb>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightResult {
    pub ok: bool,
    pub git_available: bool,
    pub gh_available: bool,
    pub gh_authenticated: bool,
    pub messages: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoBranch {
    pub name: String,
    pub is_default: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", cmd, e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[tauri::command]
pub async fn preflight_check() -> Result<PreflightResult, String> {
    let mut result = PreflightResult {
        ok: true,
        git_available: false,
        gh_available: false,
        gh_authenticated: false,
        messages: vec![],
    };

    // Check git
    match run_cmd("git", &["--version"]) {
        Ok(v) => {
            result.git_available = true;
            result.messages.push(format!("git: {}", v));
        }
        Err(_) => {
            result.ok = false;
            result.messages.push("git is not installed. Install it from https://git-scm.com".to_string());
        }
    }

    // Check gh
    match run_cmd("gh", &["--version"]) {
        Ok(v) => {
            result.gh_available = true;
            let first_line = v.lines().next().unwrap_or(&v);
            result.messages.push(format!("gh: {}", first_line));
        }
        Err(_) => {
            result.ok = false;
            result.messages.push("gh CLI is not installed. Install it from https://cli.github.com".to_string());
        }
    }

    // Check gh auth
    if result.gh_available {
        match run_cmd("gh", &["auth", "status"]) {
            Ok(_) => {
                result.gh_authenticated = true;
                result.messages.push("gh: authenticated".to_string());
            }
            Err(_) => {
                result.ok = false;
                result.messages.push("gh is not authenticated. Run: gh auth login".to_string());
            }
        }
    }

    Ok(result)
}

/// App-level config stored at ~/.beehive/config.json
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub beehive_dir: Option<String>,
}

fn app_config_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/"))
        .join(".beehive")
        .join("config.json")
}

#[tauri::command]
pub async fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Cannot determine home directory".to_string())
}

#[tauri::command]
pub async fn load_app_config() -> Result<AppConfig, String> {
    let path = app_config_path();
    if !path.exists() {
        return Ok(AppConfig { beehive_dir: None });
    }
    let data = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read app config: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse app config: {}", e))
}

#[tauri::command]
pub async fn save_app_config(config: AppConfig) -> Result<(), String> {
    let path = app_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create ~/.beehive: {}", e))?;
    }
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write app config: {}", e))
}

#[tauri::command]
pub async fn reset_app() -> Result<(), String> {
    let path = app_config_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete config: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_app_config_path() -> Result<String, String> {
    Ok(app_config_path().to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_dirs(path: String) -> Result<Vec<DirEntry>, String> {
    let target = if path.is_empty() {
        dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"))
    } else {
        std::path::PathBuf::from(&path)
    };

    if !target.is_dir() {
        // If path doesn't exist yet, list the parent
        if let Some(parent) = target.parent() {
            if parent.is_dir() {
                return list_dir_entries(parent);
            }
        }
        return Ok(vec![]);
    }

    list_dir_entries(&target)
}

fn list_dir_entries(dir: &std::path::Path) -> Result<Vec<DirEntry>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Cannot read directory: {}", e))?;

    let mut results: Vec<DirEntry> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
        })
        .filter(|e| {
            // Hide hidden dirs unless parent is home
            let name = e.file_name().to_string_lossy().to_string();
            !name.starts_with('.')
        })
        .map(|e| DirEntry {
            name: e.file_name().to_string_lossy().to_string(),
            path: e.path().to_string_lossy().to_string(),
            is_dir: true,
        })
        .collect();

    results.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(results)
}

#[tauri::command]
pub async fn init_beehive(dir: String) -> Result<BeehiveConfig, String> {
    let path = Path::new(&dir);
    fs::create_dir_all(path).map_err(|e| format!("Failed to create directory: {}", e))?;

    let config = BeehiveConfig {
        version: 1,
        beehive_dir: dir.clone(),
    };

    let config_path = path.join("beehive.json");
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    fs::write(&config_path, json).map_err(|e| format!("Failed to write config: {}", e))?;

    Ok(config)
}

#[tauri::command]
pub async fn load_beehive(dir: String) -> Result<BeehiveConfig, String> {
    let config_path = Path::new(&dir).join("beehive.json");
    if !config_path.exists() {
        return Err("No beehive.json found".to_string());
    }
    let data = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse config: {}", e))
}

#[tauri::command]
pub async fn verify_repo(repo_url: String) -> Result<HiveInfo, String> {
    let (owner, repo_name) = parse_repo_url(&repo_url)?;

    // Use gh to get repo info
    let repo_spec = format!("{}/{}", owner, repo_name);
    let json_output = run_cmd("gh", &["repo", "view", &repo_spec, "--json", "name,owner,description,defaultBranchRef,sshUrl,url"])
        .map_err(|e| format!("Cannot access repo '{}': {}", repo_spec, e))?;

    let parsed: serde_json::Value = serde_json::from_str(&json_output)
        .map_err(|e| format!("Failed to parse gh output: {}", e))?;

    let description = parsed["description"].as_str().map(|s| s.to_string());
    let default_branch = parsed["defaultBranchRef"]["name"]
        .as_str()
        .map(|s| s.to_string());

    // Build a proper clone URL — prefer SSH if the original was SSH, otherwise HTTPS
    let clone_url = if repo_url.starts_with("git@") {
        // User provided SSH URL, use SSH
        parsed["sshUrl"]
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("git@github.com:{}/{}.git", owner, repo_name))
    } else {
        // Use HTTPS
        parsed["url"]
            .as_str()
            .map(|s| format!("{}.git", s))
            .unwrap_or_else(|| format!("https://github.com/{}/{}.git", owner, repo_name))
    };

    // Verify the repo is actually cloneable with git ls-remote
    run_cmd("git", &["ls-remote", "--heads", &clone_url])
        .map_err(|e| format!(
            "Repository '{}' is not accessible via git. Check your SSH keys or credentials.\n{}",
            repo_spec, e
        ))?;

    let dir_name = format!("repo_{}", repo_name);

    Ok(HiveInfo {
        dir_name,
        repo_url: clone_url,
        repo_name,
        owner,
        description,
        default_branch,
    })
}

#[tauri::command]
pub async fn create_hive(beehive_dir: String, repo_url: String) -> Result<HiveInfo, String> {
    // Validate URL format before hitting the network
    let trimmed = repo_url.trim().to_string();
    if trimmed.is_empty() {
        return Err("Repository URL cannot be empty".to_string());
    }

    let info = verify_repo(trimmed).await?;

    // Check for existing hive with same name
    let hive_dir = Path::new(&beehive_dir).join(&info.dir_name);
    if hive_dir.join(".hive").join("state.json").exists() {
        return Err(format!(
            "Hive for '{}' already exists. Delete it first if you want to re-add.",
            info.repo_name
        ));
    }

    fs::create_dir_all(&hive_dir).map_err(|e| format!("Failed to create hive dir: {}", e))?;

    let dot_hive = hive_dir.join(".hive");
    fs::create_dir_all(&dot_hive).map_err(|e| format!("Failed to create .hive dir: {}", e))?;

    // Write hive state
    let state = HiveState {
        info: info.clone(),
        combs: vec![],
    };
    let state_json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize: {}", e))?;

    if let Err(e) = fs::write(dot_hive.join("state.json"), &state_json) {
        // Clean up on failure
        let _ = fs::remove_dir_all(&hive_dir);
        return Err(format!("Failed to write state: {}", e));
    }

    Ok(info)
}

#[tauri::command]
pub async fn list_hives(beehive_dir: String) -> Result<Vec<HiveInfo>, String> {
    let base = Path::new(&beehive_dir);
    let mut hives = vec![];

    let entries = fs::read_dir(base).map_err(|e| format!("Failed to read dir: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("repo_") {
            let state_path = entry.path().join(".hive").join("state.json");
            if state_path.exists() {
                match fs::read_to_string(&state_path) {
                    Ok(data) => {
                        match serde_json::from_str::<HiveState>(&data) {
                            Ok(state) => {
                                // Validate the hive has a proper repo name
                                if !state.info.repo_name.is_empty() {
                                    hives.push(state.info);
                                } else {
                                    // Broken hive, clean up
                                    let _ = fs::remove_dir_all(entry.path());
                                }
                            }
                            Err(_) => {
                                // Corrupted state, clean up
                                let _ = fs::remove_dir_all(entry.path());
                            }
                        }
                    }
                    Err(_) => {
                        // Can't read, clean up
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            } else {
                // repo_ dir without state.json — orphaned, clean up
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    Ok(hives)
}

#[tauri::command]
pub async fn delete_hive(beehive_dir: String, dir_name: String) -> Result<(), String> {
    let hive_dir = Path::new(&beehive_dir).join(&dir_name);
    if hive_dir.exists() {
        fs::remove_dir_all(&hive_dir).map_err(|e| format!("Failed to delete: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_branches(beehive_dir: String, dir_name: String) -> Result<Vec<RepoBranch>, String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;
    let repo_spec = format!("{}/{}", state.info.owner, state.info.repo_name);

    let output = run_cmd("gh", &[
        "api",
        &format!("repos/{}/branches", repo_spec),
        "--jq", ".[].name",
    ]).map_err(|e| format!("Failed to list branches: {}", e))?;

    let default_branch = state.info.default_branch.unwrap_or_else(|| "main".to_string());

    let branches: Vec<RepoBranch> = output
        .lines()
        .map(|line| RepoBranch {
            name: line.to_string(),
            is_default: line == default_branch,
        })
        .collect();

    Ok(branches)
}

#[tauri::command]
pub async fn create_comb(
    beehive_dir: String,
    dir_name: String,
    name: String,
    branch: String,
) -> Result<Comb, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    let comb_id = uuid::Uuid::new_v4().to_string();
    let hive_dir = Path::new(&beehive_dir).join(&dir_name);
    let comb_dir = hive_dir.join(&name);

    // Clone the repo into the comb directory
    let clone_output = Command::new("git")
        .args(["clone", &state.info.repo_url, comb_dir.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Clone failed: {}", e))?;

    if !clone_output.status.success() {
        return Err(format!(
            "Git clone failed: {}",
            String::from_utf8_lossy(&clone_output.stderr)
        ));
    }

    // Checkout the branch
    let checkout_output = Command::new("git")
        .args(["checkout", &branch])
        .current_dir(&comb_dir)
        .output()
        .map_err(|e| format!("Checkout failed: {}", e))?;

    if !checkout_output.status.success() {
        // Try creating the branch if it doesn't exist remotely
        let checkout_new = Command::new("git")
            .args(["checkout", "-b", &branch])
            .current_dir(&comb_dir)
            .output()
            .map_err(|e| format!("Checkout -b failed: {}", e))?;

        if !checkout_new.status.success() {
            // Clean up failed clone
            let _ = fs::remove_dir_all(&comb_dir);
            return Err(format!(
                "Failed to checkout branch '{}': {}",
                branch,
                String::from_utf8_lossy(&checkout_new.stderr)
            ));
        }
    }

    let comb = Comb {
        id: comb_id,
        name: name.clone(),
        branch,
        path: comb_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        panes: vec![],
    };

    state.combs.push(comb.clone());
    save_hive_state(&beehive_dir, &dir_name, &state)?;

    Ok(comb)
}

#[tauri::command]
pub async fn list_combs(beehive_dir: String, dir_name: String) -> Result<Vec<Comb>, String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;
    Ok(state.combs)
}

#[tauri::command]
pub async fn delete_comb(
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
) -> Result<(), String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    if let Some(pos) = state.combs.iter().position(|c| c.id == comb_id) {
        let comb = state.combs.remove(pos);
        let comb_path = Path::new(&comb.path);
        if comb_path.exists() {
            fs::remove_dir_all(comb_path)
                .map_err(|e| format!("Failed to delete comb directory: {}", e))?;
        }
        save_hive_state(&beehive_dir, &dir_name, &state)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn save_comb_panes(
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
    panes: Vec<PaneConfig>,
) -> Result<(), String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;
    if let Some(comb) = state.combs.iter_mut().find(|c| c.id == comb_id) {
        comb.panes = panes;
        save_hive_state(&beehive_dir, &dir_name, &state)?;
        Ok(())
    } else {
        Err(format!("Comb '{}' not found", comb_id))
    }
}

#[tauri::command]
pub async fn get_comb_panes(
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
) -> Result<Vec<PaneConfig>, String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;
    if let Some(comb) = state.combs.iter().find(|c| c.id == comb_id) {
        Ok(comb.panes.clone())
    } else {
        Err(format!("Comb '{}' not found", comb_id))
    }
}

// --- helpers ---

fn parse_repo_url(url: &str) -> Result<(String, String), String> {
    // Handle: git@github.com:owner/repo.git
    //         https://github.com/owner/repo.git
    //         https://github.com/owner/repo
    //         owner/repo
    let cleaned = url
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git");

    let (owner, repo_name) = if cleaned.contains(':') && cleaned.starts_with("git@") {
        // SSH format: git@github.com:owner/repo
        let after_colon = cleaned.split(':').last()
            .ok_or("Invalid SSH URL format")?;
        let parts: Vec<&str> = after_colon.split('/').collect();
        if parts.len() >= 2 {
            (parts[parts.len() - 2].to_string(), parts[parts.len() - 1].to_string())
        } else {
            return Err(format!("Cannot parse SSH URL: {}. Expected format: git@github.com:owner/repo", url));
        }
    } else if cleaned.contains("github.com/") {
        let after_gh = cleaned.split("github.com/").last()
            .ok_or("Invalid GitHub URL")?;
        let parts: Vec<&str> = after_gh.split('/').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            return Err(format!("Cannot parse GitHub URL: {}. Expected format: https://github.com/owner/repo", url));
        }
    } else {
        // Try owner/repo format
        let parts: Vec<&str> = cleaned.split('/').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            return Err(format!("Cannot parse '{}'. Use owner/repo, a GitHub URL, or SSH URL.", url));
        }
    };

    // Validate both parts are non-empty
    if owner.is_empty() || repo_name.is_empty() {
        return Err(format!(
            "Invalid repo format: '{}'. Both owner and repo name are required (e.g. owner/repo).",
            url
        ));
    }

    Ok((owner, repo_name))
}

fn load_hive_state(beehive_dir: &str, dir_name: &str) -> Result<HiveState, String> {
    let state_path = Path::new(beehive_dir)
        .join(dir_name)
        .join(".hive")
        .join("state.json");
    let data = fs::read_to_string(&state_path)
        .map_err(|e| format!("Failed to read hive state: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse hive state: {}", e))
}

fn save_hive_state(beehive_dir: &str, dir_name: &str, state: &HiveState) -> Result<(), String> {
    let state_path = Path::new(beehive_dir)
        .join(dir_name)
        .join(".hive")
        .join("state.json");
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    fs::write(&state_path, json).map_err(|e| format!("Failed to write state: {}", e))
}

fn chrono_now() -> String {
    // Simple ISO 8601 without pulling in chrono crate
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", duration.as_secs())
}
