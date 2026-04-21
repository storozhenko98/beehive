use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::{AppHandle, Emitter};

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
pub struct Nest {
    pub id: String,
    pub name: String,
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
    pub nest_id: Option<String>,
    #[serde(default)]
    pub panes: Vec<PaneConfig>,
    #[serde(default)]
    pub cloning: bool, // deprecated, use operation instead
    #[serde(default)]
    pub operation: Option<String>, // "cloning" | "copying" | "deleting"
}

// Event payloads for operation completion
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombOperationResult {
    pub hive_dir_name: String,
    pub comb_id: String,
    pub op_type: String, // "cloning" | "copying" | "deleting"
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HiveOperationResult {
    pub hive_dir_name: String,
    pub op_type: String, // "deleting"
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HiveState {
    pub info: HiveInfo,
    #[serde(default)]
    pub nests: Vec<Nest>,
    pub combs: Vec<Comb>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombNestAssignment {
    pub comb_id: String,
    pub nest_id: Option<String>,
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

fn full_path() -> String {
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

fn cmd_with_path(cmd: &str) -> Command {
    let mut c = Command::new(cmd);
    c.env("PATH", full_path());
    c
}

fn run_cmd(cmd: &str, args: &[&str]) -> Result<String, String> {
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
            result
                .messages
                .push("git is not installed. Install it from https://git-scm.com".to_string());
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
            result.messages.push(
                "gh CLI is not installed. Install it from https://cli.github.com".to_string(),
            );
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
                result
                    .messages
                    .push("gh is not authenticated. Run: gh auth login".to_string());
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
    #[serde(default)]
    pub mux_preference: Option<String>,
    #[serde(default)]
    pub cli_command: Option<String>,
    #[serde(default)]
    pub comb_startup_command: Option<String>,
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u16,
}

fn default_sidebar_width() -> u16 {
    28
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
        return Ok(AppConfig {
            beehive_dir: None,
            mux_preference: None,
            cli_command: None,
            comb_startup_command: None,
            sidebar_width: default_sidebar_width(),
        });
    }
    let data =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read app config: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse app config: {}", e))
}

#[tauri::command]
pub async fn save_app_config(config: AppConfig) -> Result<(), String> {
    let path = app_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create ~/.beehive: {}", e))?;
    }
    let json =
        serde_json::to_string_pretty(&config).map_err(|e| format!("Failed to serialize: {}", e))?;
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
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
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
    let data =
        fs::read_to_string(&config_path).map_err(|e| format!("Failed to read config: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse config: {}", e))
}

#[tauri::command]
pub async fn verify_repo(repo_url: String) -> Result<HiveInfo, String> {
    let (owner, repo_name) = parse_repo_url(&repo_url)?;

    // Use gh to get repo info
    let repo_spec = format!("{}/{}", owner, repo_name);
    let json_output = run_cmd(
        "gh",
        &[
            "repo",
            "view",
            &repo_spec,
            "--json",
            "name,owner,description,defaultBranchRef,sshUrl,url",
        ],
    )
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
    run_cmd("git", &["ls-remote", "--heads", &clone_url]).map_err(|e| {
        format!(
            "Repository '{}' is not accessible via git. Check your SSH keys or credentials.\n{}",
            repo_spec, e
        )
    })?;

    let dir_name = format!("repo_{}", repo_name);

    Ok(HiveInfo {
        dir_name,
        repo_url: clone_url,
        repo_name,
        owner,
        description,
        default_branch,
        custom_buttons: vec![],
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
        nests: vec![],
        combs: vec![],
    };
    let state_json =
        serde_json::to_string_pretty(&state).map_err(|e| format!("Failed to serialize: {}", e))?;

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

/// Phase 1: Check if hive can be deleted (no active comb operations)
#[tauri::command]
pub async fn delete_hive_start(beehive_dir: String, dir_name: String) -> Result<(), String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;

    // Check if any combs have active operations
    let active_ops: Vec<&str> = state
        .combs
        .iter()
        .filter_map(|c| c.operation.as_deref())
        .collect();

    if !active_ops.is_empty() {
        return Err("Cannot delete hive while comb operations are in progress".to_string());
    }

    Ok(())
}

/// Phase 2: Execute the actual hive deletion (runs in background, emits event)
#[tauri::command]
pub async fn delete_hive_run(
    app: AppHandle,
    beehive_dir: String,
    dir_name: String,
) -> Result<(), String> {
    let beehive_dir_clone = beehive_dir.clone();
    let dir_name_clone = dir_name.clone();

    tokio::task::spawn_blocking(move || {
        let hive_dir = Path::new(&beehive_dir_clone).join(&dir_name_clone);

        let delete_result = if hive_dir.exists() {
            fs::remove_dir_all(&hive_dir).map_err(|e| format!("Failed to delete: {}", e))
        } else {
            Ok(())
        };

        match delete_result {
            Ok(()) => {
                let _ = app.emit(
                    "hive-operation-done",
                    HiveOperationResult {
                        hive_dir_name: dir_name_clone,
                        op_type: "deleting".to_string(),
                        success: true,
                        error: None,
                    },
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "hive-operation-done",
                    HiveOperationResult {
                        hive_dir_name: dir_name_clone,
                        op_type: "deleting".to_string(),
                        success: false,
                        error: Some(e),
                    },
                );
            }
        }
    });

    Ok(())
}

/// Legacy single-phase delete (kept for backward compatibility)
#[tauri::command]
pub async fn delete_hive(beehive_dir: String, dir_name: String) -> Result<(), String> {
    let hive_dir = Path::new(&beehive_dir).join(&dir_name);
    if hive_dir.exists() {
        fs::remove_dir_all(&hive_dir).map_err(|e| format!("Failed to delete: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn list_branches(
    beehive_dir: String,
    dir_name: String,
) -> Result<Vec<RepoBranch>, String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;
    let repo_spec = format!("{}/{}", state.info.owner, state.info.repo_name);

    let output = run_cmd(
        "gh",
        &[
            "api",
            &format!("repos/{}/branches?per_page=100", repo_spec),
            "--paginate",
            "--jq",
            ".[].name",
        ],
    )
    .map_err(|e| format!("Failed to list branches: {}", e))?;

    let default_branch = state
        .info
        .default_branch
        .unwrap_or_else(|| "main".to_string());

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
pub async fn create_comb_start(
    beehive_dir: String,
    dir_name: String,
    name: String,
    branch: String,
) -> Result<Comb, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    validate_comb_name(&name, &state.combs)?;

    let comb_id = uuid::Uuid::new_v4().to_string();
    let hive_dir = Path::new(&beehive_dir).join(&dir_name);
    let comb_dir = hive_dir.join(&name);

    let comb = Comb {
        id: comb_id,
        name: name.clone(),
        branch,
        path: comb_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        nest_id: None,
        panes: vec![],
        cloning: true, // for backward compat
        operation: Some("cloning".to_string()),
    };

    state.combs.push(comb.clone());
    save_hive_state(&beehive_dir, &dir_name, &state)?;

    Ok(comb)
}

#[tauri::command]
pub async fn create_comb_clone(
    app: AppHandle,
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
) -> Result<(), String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;

    let comb = state
        .combs
        .iter()
        .find(|c| c.id == comb_id)
        .ok_or_else(|| format!("Comb '{}' not found", comb_id))?
        .clone();

    let repo_url = state.info.repo_url.clone();
    let dir_name_clone = dir_name.clone();
    let beehive_dir_clone = beehive_dir.clone();
    let comb_id_clone = comb_id.clone();

    // Spawn blocking task for git operations
    tokio::task::spawn_blocking(move || {
        let comb_dir = Path::new(&comb.path);

        // Clone the repo into the comb directory
        let clone_output = cmd_with_path("git")
            .args(["clone", &repo_url, comb_dir.to_str().unwrap()])
            .output();

        let clone_result = match clone_output {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(format!(
                "Git clone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(e) => Err(format!("Clone failed: {}", e)),
        };

        if let Err(error) = clone_result {
            // Clean up: remove comb from state and directory
            if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone) {
                state.combs.retain(|c| c.id != comb_id_clone);
                let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
            }
            let _ = fs::remove_dir_all(comb_dir);
            let _ = app.emit(
                "comb-operation-done",
                CombOperationResult {
                    hive_dir_name: dir_name_clone,
                    comb_id: comb_id_clone,
                    op_type: "cloning".to_string(),
                    success: false,
                    error: Some(error),
                },
            );
            return;
        }

        // Checkout the branch
        let checkout_output = cmd_with_path("git")
            .args(["checkout", &comb.branch])
            .current_dir(comb_dir)
            .output();

        let checkout_ok = match checkout_output {
            Ok(output) if output.status.success() => true,
            _ => {
                // Try creating the branch if it doesn't exist remotely
                let checkout_new = cmd_with_path("git")
                    .args(["checkout", "-b", &comb.branch])
                    .current_dir(comb_dir)
                    .output();

                match checkout_new {
                    Ok(output) if output.status.success() => true,
                    Ok(output) => {
                        // Clean up: remove comb from state and directory
                        if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone)
                        {
                            state.combs.retain(|c| c.id != comb_id_clone);
                            let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
                        }
                        let _ = fs::remove_dir_all(comb_dir);
                        let _ = app.emit(
                            "comb-operation-done",
                            CombOperationResult {
                                hive_dir_name: dir_name_clone,
                                comb_id: comb_id_clone,
                                op_type: "cloning".to_string(),
                                success: false,
                                error: Some(format!(
                                    "Failed to checkout branch '{}': {}",
                                    comb.branch,
                                    String::from_utf8_lossy(&output.stderr)
                                )),
                            },
                        );
                        return;
                    }
                    Err(e) => {
                        if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone)
                        {
                            state.combs.retain(|c| c.id != comb_id_clone);
                            let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
                        }
                        let _ = fs::remove_dir_all(comb_dir);
                        let _ = app.emit(
                            "comb-operation-done",
                            CombOperationResult {
                                hive_dir_name: dir_name_clone,
                                comb_id: comb_id_clone,
                                op_type: "cloning".to_string(),
                                success: false,
                                error: Some(format!("Checkout -b failed: {}", e)),
                            },
                        );
                        return;
                    }
                }
            }
        };

        if checkout_ok {
            // Success: mark operation complete
            if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone) {
                if let Some(c) = state.combs.iter_mut().find(|c| c.id == comb_id_clone) {
                    c.cloning = false;
                    c.operation = None;
                }
                let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
            }
            let _ = app.emit(
                "comb-operation-done",
                CombOperationResult {
                    hive_dir_name: dir_name_clone,
                    comb_id: comb_id_clone,
                    op_type: "cloning".to_string(),
                    success: true,
                    error: None,
                },
            );
        }
    });

    Ok(())
}

fn get_git_branch(path: &str) -> Option<String> {
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

#[tauri::command]
pub async fn get_hive_state(beehive_dir: String, dir_name: String) -> Result<HiveState, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;
    let mut changed = false;
    for comb in &mut state.combs {
        if let Some(branch) = get_git_branch(&comb.path) {
            if branch != comb.branch {
                comb.branch = branch;
                changed = true;
            }
        }
    }
    if changed {
        save_hive_state(&beehive_dir, &dir_name, &state)?;
    }
    Ok(state)
}

#[tauri::command]
pub async fn list_combs(beehive_dir: String, dir_name: String) -> Result<Vec<Comb>, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;
    let mut changed = false;
    for comb in &mut state.combs {
        if let Some(branch) = get_git_branch(&comb.path) {
            if branch != comb.branch {
                comb.branch = branch;
                changed = true;
            }
        }
    }
    if changed {
        save_hive_state(&beehive_dir, &dir_name, &state)?;
    }
    Ok(state.combs)
}

/// Phase 1: Mark comb as deleting (returns immediately)
#[tauri::command]
pub async fn delete_comb_start(
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
) -> Result<(), String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    if let Some(comb) = state.combs.iter_mut().find(|c| c.id == comb_id) {
        // Don't allow deleting if already has an operation in progress
        if comb.operation.is_some() {
            return Err("Comb already has an operation in progress".to_string());
        }
        comb.operation = Some("deleting".to_string());
        save_hive_state(&beehive_dir, &dir_name, &state)?;
    }

    Ok(())
}

/// Phase 2: Execute the actual deletion (runs in background, emits event)
#[tauri::command]
pub async fn delete_comb_run(
    app: AppHandle,
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
) -> Result<(), String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;

    let comb = state
        .combs
        .iter()
        .find(|c| c.id == comb_id)
        .ok_or_else(|| format!("Comb '{}' not found", comb_id))?
        .clone();

    let comb_path = comb.path.clone();
    let dir_name_clone = dir_name.clone();
    let beehive_dir_clone = beehive_dir.clone();
    let comb_id_clone = comb_id.clone();

    tokio::task::spawn_blocking(move || {
        let path = Path::new(&comb_path);

        let delete_result = if path.exists() {
            fs::remove_dir_all(path).map_err(|e| format!("Failed to delete comb directory: {}", e))
        } else {
            Ok(())
        };

        match delete_result {
            Ok(()) => {
                // Success: remove comb from state
                if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone) {
                    state.combs.retain(|c| c.id != comb_id_clone);
                    let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
                }
                let _ = app.emit(
                    "comb-operation-done",
                    CombOperationResult {
                        hive_dir_name: dir_name_clone,
                        comb_id: comb_id_clone,
                        op_type: "deleting".to_string(),
                        success: true,
                        error: None,
                    },
                );
            }
            Err(e) => {
                // Failed: revert operation flag
                if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone) {
                    if let Some(c) = state.combs.iter_mut().find(|c| c.id == comb_id_clone) {
                        c.operation = None;
                    }
                    let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
                }
                let _ = app.emit(
                    "comb-operation-done",
                    CombOperationResult {
                        hive_dir_name: dir_name_clone,
                        comb_id: comb_id_clone,
                        op_type: "deleting".to_string(),
                        success: false,
                        error: Some(e),
                    },
                );
            }
        }
    });

    Ok(())
}

/// Legacy single-phase delete (kept for backward compatibility)
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
pub async fn rename_comb(
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
    new_name: String,
) -> Result<Comb, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    let Some(index) = state.combs.iter().position(|comb| comb.id == comb_id) else {
        return Err(format!("Comb '{}' not found", comb_id));
    };

    if state.combs[index].cloning || state.combs[index].operation.is_some() {
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
    validate_comb_name(&new_name, &existing_combs)?;

    state.combs[index].name = new_name;
    let renamed = state.combs[index].clone();

    save_hive_state(&beehive_dir, &dir_name, &state)?;

    Ok(renamed)
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

#[tauri::command]
pub async fn save_custom_buttons(
    beehive_dir: String,
    dir_name: String,
    buttons: Vec<CustomButton>,
) -> Result<(), String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;
    state.info.custom_buttons = buttons;
    save_hive_state(&beehive_dir, &dir_name, &state)?;
    Ok(())
}

fn validate_comb_name(name: &str, existing_combs: &[Comb]) -> Result<(), String> {
    if name.is_empty() {
        return Err("Comb name cannot be empty".to_string());
    }
    if name.len() > 40 {
        return Err("Comb name must be 40 characters or fewer".to_string());
    }
    if name.starts_with('.') || name.starts_with('-') {
        return Err("Comb name cannot start with '.' or '-'".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(
            "Comb name can only contain letters, numbers, hyphens, and underscores".to_string(),
        );
    }
    if name == ".hive" {
        return Err("'.hive' is a reserved name".to_string());
    }
    if existing_combs
        .iter()
        .any(|c| c.name == name || comb_path_basename(c) == Some(name))
    {
        return Err(format!("A comb named '{}' already exists", name));
    }
    Ok(())
}

fn comb_path_basename(comb: &Comb) -> Option<&str> {
    Path::new(&comb.path)
        .file_name()
        .and_then(|name| name.to_str())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("Failed to create directory {:?}: {}", dst, e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read {:?}: {}", src, e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed to copy {:?}: {}", src_path, e))?;
        }
    }
    Ok(())
}

/// Phase 1: Create comb entry for copying (returns immediately)
#[tauri::command]
pub async fn copy_comb_start(
    beehive_dir: String,
    dir_name: String,
    source_comb_id: String,
    new_name: String,
) -> Result<Comb, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    let source = state
        .combs
        .iter()
        .find(|c| c.id == source_comb_id)
        .ok_or_else(|| format!("Source comb '{}' not found", source_comb_id))?
        .clone();

    // Don't allow copying a comb that's being deleted
    if source.operation.as_deref() == Some("deleting") {
        return Err("Cannot copy a comb that is being deleted".to_string());
    }

    validate_comb_name(&new_name, &state.combs)?;

    let hive_dir = Path::new(&beehive_dir).join(&dir_name);
    let new_dir = hive_dir.join(&new_name);

    let comb = Comb {
        id: uuid::Uuid::new_v4().to_string(),
        name: new_name.clone(),
        branch: source.branch.clone(),
        path: new_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        nest_id: source.nest_id.clone(),
        panes: vec![],
        cloning: false,
        operation: Some("copying".to_string()),
    };

    state.combs.push(comb.clone());
    save_hive_state(&beehive_dir, &dir_name, &state)?;

    Ok(comb)
}

/// Phase 2: Execute the actual copy (runs in background, emits event)
#[tauri::command]
pub async fn copy_comb_run(
    app: AppHandle,
    beehive_dir: String,
    dir_name: String,
    comb_id: String,
    source_comb_id: String,
) -> Result<(), String> {
    let state = load_hive_state(&beehive_dir, &dir_name)?;

    let source = state
        .combs
        .iter()
        .find(|c| c.id == source_comb_id)
        .ok_or_else(|| format!("Source comb '{}' not found", source_comb_id))?
        .clone();

    let new_comb = state
        .combs
        .iter()
        .find(|c| c.id == comb_id)
        .ok_or_else(|| format!("New comb '{}' not found", comb_id))?
        .clone();

    let source_path = source.path.clone();
    let new_path = new_comb.path.clone();
    let dir_name_clone = dir_name.clone();
    let beehive_dir_clone = beehive_dir.clone();
    let comb_id_clone = comb_id.clone();

    tokio::task::spawn_blocking(move || {
        let source_dir = Path::new(&source_path);
        let new_dir = Path::new(&new_path);

        if !source_dir.exists() {
            // Clean up: remove comb from state
            if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone) {
                state.combs.retain(|c| c.id != comb_id_clone);
                let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
            }
            let _ = app.emit(
                "comb-operation-done",
                CombOperationResult {
                    hive_dir_name: dir_name_clone,
                    comb_id: comb_id_clone,
                    op_type: "copying".to_string(),
                    success: false,
                    error: Some(format!(
                        "Source comb directory does not exist: {}",
                        source_path
                    )),
                },
            );
            return;
        }

        match copy_dir_recursive(source_dir, new_dir) {
            Ok(()) => {
                // Success: mark operation complete
                if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone) {
                    if let Some(c) = state.combs.iter_mut().find(|c| c.id == comb_id_clone) {
                        c.operation = None;
                    }
                    let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
                }
                let _ = app.emit(
                    "comb-operation-done",
                    CombOperationResult {
                        hive_dir_name: dir_name_clone,
                        comb_id: comb_id_clone,
                        op_type: "copying".to_string(),
                        success: true,
                        error: None,
                    },
                );
            }
            Err(e) => {
                // Clean up: remove comb from state and partial directory
                if let Ok(mut state) = load_hive_state(&beehive_dir_clone, &dir_name_clone) {
                    state.combs.retain(|c| c.id != comb_id_clone);
                    let _ = save_hive_state(&beehive_dir_clone, &dir_name_clone, &state);
                }
                let _ = fs::remove_dir_all(new_dir);
                let _ = app.emit(
                    "comb-operation-done",
                    CombOperationResult {
                        hive_dir_name: dir_name_clone,
                        comb_id: comb_id_clone,
                        op_type: "copying".to_string(),
                        success: false,
                        error: Some(e),
                    },
                );
            }
        }
    });

    Ok(())
}

/// Legacy single-phase copy (kept for backward compatibility, now just calls the two-phase)
#[tauri::command]
pub async fn copy_comb(
    beehive_dir: String,
    dir_name: String,
    source_comb_id: String,
    new_name: String,
) -> Result<Comb, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    let source = state
        .combs
        .iter()
        .find(|c| c.id == source_comb_id)
        .ok_or_else(|| format!("Source comb '{}' not found", source_comb_id))?
        .clone();

    validate_comb_name(&new_name, &state.combs)?;

    let hive_dir = Path::new(&beehive_dir).join(&dir_name);
    let source_dir = Path::new(&source.path);
    let new_dir = hive_dir.join(&new_name);

    if !source_dir.exists() {
        return Err(format!(
            "Source comb directory does not exist: {}",
            source.path
        ));
    }

    copy_dir_recursive(source_dir, &new_dir)?;

    let comb = Comb {
        id: uuid::Uuid::new_v4().to_string(),
        name: new_name.clone(),
        branch: source.branch.clone(),
        path: new_dir.to_string_lossy().to_string(),
        created_at: chrono_now(),
        nest_id: source.nest_id.clone(),
        panes: vec![],
        cloning: false,
        operation: None,
    };

    state.combs.push(comb.clone());
    save_hive_state(&beehive_dir, &dir_name, &state)?;

    Ok(comb)
}

#[tauri::command]
pub async fn reorder_combs(
    beehive_dir: String,
    dir_name: String,
    comb_ids: Vec<String>,
) -> Result<(), String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    let mut ordered: Vec<Comb> = Vec::with_capacity(state.combs.len());
    for id in &comb_ids {
        if let Some(pos) = state.combs.iter().position(|c| c.id == *id) {
            ordered.push(state.combs.remove(pos));
        }
    }
    ordered.append(&mut state.combs);
    state.combs = ordered;

    save_hive_state(&beehive_dir, &dir_name, &state)
}

fn validate_nest_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Nest name cannot be empty".to_string());
    }
    if trimmed.len() > 40 {
        return Err("Nest name must be 40 characters or fewer".to_string());
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err("Nest name cannot contain control characters".to_string());
    }
    Ok(())
}

fn validate_nests(nests: &[Nest]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();

    for nest in nests {
        if nest.id.trim().is_empty() {
            return Err("Nest id cannot be empty".to_string());
        }
        if !ids.insert(nest.id.clone()) {
            return Err(format!("Duplicate nest id '{}'", nest.id));
        }

        validate_nest_name(&nest.name)?;

        let folded = nest.name.trim().to_lowercase();
        if !names.insert(folded) {
            return Err(format!("Nest '{}' already exists", nest.name.trim()));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn save_nests(
    beehive_dir: String,
    dir_name: String,
    nests: Vec<Nest>,
    assignments: Vec<CombNestAssignment>,
) -> Result<HiveState, String> {
    let mut state = load_hive_state(&beehive_dir, &dir_name)?;

    validate_nests(&nests)?;

    let nest_ids: std::collections::HashSet<String> =
        nests.iter().map(|nest| nest.id.clone()).collect();
    let comb_ids: std::collections::HashSet<String> =
        state.combs.iter().map(|comb| comb.id.clone()).collect();
    let mut assignment_map = std::collections::HashMap::new();

    for assignment in assignments {
        if !comb_ids.contains(&assignment.comb_id) {
            return Err(format!("Comb '{}' not found", assignment.comb_id));
        }
        if let Some(nest_id) = assignment.nest_id.as_ref() {
            if !nest_ids.contains(nest_id) {
                return Err(format!("Nest '{}' not found", nest_id));
            }
        }
        assignment_map.insert(assignment.comb_id, assignment.nest_id);
    }

    for comb in &mut state.combs {
        if let Some(nest_id) = assignment_map.get(&comb.id) {
            comb.nest_id = nest_id.clone();
        }
        if comb
            .nest_id
            .as_ref()
            .is_some_and(|nest_id| !nest_ids.contains(nest_id))
        {
            comb.nest_id = None;
        }
    }

    state.nests = nests
        .into_iter()
        .map(|nest| Nest {
            id: nest.id,
            name: nest.name.trim().to_string(),
        })
        .collect();

    save_hive_state(&beehive_dir, &dir_name, &state)?;
    Ok(state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatusResult {
    pub installed: bool,
    pub cmd_name: Option<String>,
    pub path: Option<String>,
}

#[tauri::command]
pub async fn install_cli(cmd_name: String) -> Result<String, String> {
    // Validate command name
    if cmd_name != "bh" && cmd_name != "beehive" {
        return Err("Command name must be 'bh' or 'beehive'".to_string());
    }

    let version = env!("CARGO_PKG_VERSION");
    let url = format!(
        "https://github.com/storozhenko98/beehive/releases/download/v{}/beehive-tui-darwin-arm64",
        version
    );
    let install_path = format!("/usr/local/bin/{}", cmd_name);

    // Download to temp file
    let tmp = std::env::temp_dir().join("beehive-cli-install");
    let output = Command::new("curl")
        .args(["-fsSL", "-o", &tmp.to_string_lossy(), &url])
        .output()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Download failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("unknown error")
        ));
    }

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755));
    }

    // Ensure /usr/local/bin exists
    let parent = Path::new("/usr/local/bin");
    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create /usr/local/bin: {}", e))?;
    }

    // Remove any existing file at the target path
    let dest = Path::new(&install_path);
    if dest.exists() || dest.symlink_metadata().is_ok() {
        fs::remove_file(dest).map_err(|e| {
            format!(
                "Failed to remove existing {}: {}. Try: sudo rm {}",
                install_path, e, install_path
            )
        })?;
    }

    // Move binary into place
    fs::rename(&tmp, &install_path)
        .or_else(|_| fs::copy(&tmp, &install_path).map(|_| ()))
        .map_err(|e| {
            format!(
                "Failed to install to {}: {}. Check permissions.",
                install_path, e
            )
        })?;
    let _ = fs::remove_file(&tmp);

    // Save command name preference to config
    let mut config = load_app_config().await?;
    config.cli_command = Some(cmd_name.clone());
    save_app_config(config).await?;

    Ok(install_path)
}

#[tauri::command]
pub async fn uninstall_cli() -> Result<(), String> {
    // Check config for stored preference
    let config = load_app_config().await?;
    let mut removed = false;

    if let Some(ref name) = config.cli_command {
        let path = format!("/usr/local/bin/{}", name);
        let p = Path::new(&path);
        if p.exists() {
            fs::remove_file(p)
                .map_err(|e| format!("Failed to remove {}: {}. Try: sudo rm {}", path, e, path))?;
            removed = true;
        }
    }

    // Also check the other name as fallback
    if !removed {
        for name in &["bh", "beehive"] {
            let path = format!("/usr/local/bin/{}", name);
            let p = Path::new(&path);
            if p.exists() {
                fs::remove_file(p).map_err(|e| {
                    format!("Failed to remove {}: {}. Try: sudo rm {}", path, e, path)
                })?;
                break;
            }
        }
    }

    // Clear config preference
    let mut config = load_app_config().await?;
    config.cli_command = None;
    save_app_config(config).await?;

    Ok(())
}

#[tauri::command]
pub async fn cli_status() -> Result<CliStatusResult, String> {
    let config = load_app_config().await?;

    // Check config preference first
    if let Some(ref name) = config.cli_command {
        let path = format!("/usr/local/bin/{}", name);
        if Path::new(&path).exists() {
            return Ok(CliStatusResult {
                installed: true,
                cmd_name: Some(name.clone()),
                path: Some(path),
            });
        }
    }

    // Fallback: check both names
    for name in &["bh", "beehive"] {
        let path = format!("/usr/local/bin/{}", name);
        if Path::new(&path).exists() {
            return Ok(CliStatusResult {
                installed: true,
                cmd_name: Some(name.to_string()),
                path: Some(path),
            });
        }
    }

    Ok(CliStatusResult {
        installed: false,
        cmd_name: None,
        path: None,
    })
}

// --- helpers ---

fn parse_repo_url(url: &str) -> Result<(String, String), String> {
    // Handle: git@github.com:owner/repo.git
    //         https://github.com/owner/repo.git
    //         https://github.com/owner/repo
    //         owner/repo
    let cleaned = url.trim().trim_end_matches('/').trim_end_matches(".git");

    let (owner, repo_name) = if cleaned.contains(':') && cleaned.starts_with("git@") {
        // SSH format: git@github.com:owner/repo
        let after_colon = cleaned.split(':').last().ok_or("Invalid SSH URL format")?;
        let parts: Vec<&str> = after_colon.split('/').collect();
        if parts.len() >= 2 {
            (
                parts[parts.len() - 2].to_string(),
                parts[parts.len() - 1].to_string(),
            )
        } else {
            return Err(format!(
                "Cannot parse SSH URL: {}. Expected format: git@github.com:owner/repo",
                url
            ));
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
            return Err(format!(
                "Cannot parse GitHub URL: {}. Expected format: https://github.com/owner/repo",
                url
            ));
        }
    } else {
        // Try owner/repo format
        let parts: Vec<&str> = cleaned.split('/').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            return Err(format!(
                "Cannot parse '{}'. Use owner/repo, a GitHub URL, or SSH URL.",
                url
            ));
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
    let data =
        fs::read_to_string(&state_path).map_err(|e| format!("Failed to read hive state: {}", e))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse hive state: {}", e))
}

fn save_hive_state(beehive_dir: &str, dir_name: &str, state: &HiveState) -> Result<(), String> {
    let state_path = Path::new(beehive_dir)
        .join(dir_name)
        .join(".hive")
        .join("state.json");
    let json =
        serde_json::to_string_pretty(state).map_err(|e| format!("Failed to serialize: {}", e))?;
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
