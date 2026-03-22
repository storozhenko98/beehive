export interface BeehiveConfig {
  version: number;
  beehiveDir: string;
  hives: string[]; // list of hive directory names (repo_*)
}

export interface CustomButton {
  label: string;
  cmd: string;
}

export interface HiveInfo {
  dirName: string; // repo_myapp
  repoUrl: string; // git@github.com:user/myapp.git
  repoName: string; // myapp
  owner: string; // user
  description?: string;
  defaultBranch?: string;
  customButtons?: CustomButton[];
}

export interface PaneConfig {
  id: string;
  type: "agent" | "terminal";
  cmd?: string;
  args?: string[];
}

// Operation types for concurrent comb operations
export type CombOperationType = "cloning" | "copying" | "deleting";

export interface Comb {
  id: string;
  name: string; // user-chosen name
  branch: string; // git branch to pull from
  path: string; // absolute path to the workspace clone
  createdAt: string;
  panes: PaneConfig[];
  cloning?: boolean; // deprecated, use operation instead
  operation?: CombOperationType; // current operation in progress
}

// Event payload from backend for operation completion
export interface CombOperationResult {
  hiveDirName: string;
  combId: string;
  opType: CombOperationType;
  success: boolean;
  error?: string;
}

// Event payload for hive-level operations
export interface HiveOperationResult {
  hiveDirName: string;
  opType: "deleting";
  success: boolean;
  error?: string;
}

export interface HiveState {
  info: HiveInfo;
  combs: Comb[];
}

export type AppView =
  | { screen: "loading" }
  | { screen: "setup" } // first launch, no beehive dir
  | { screen: "preflight-fail"; missing: string[] } // git/gh not found
  | { screen: "hive-list" } // show all hives + add button
  | { screen: "comb-list"; hiveDirName: string } // show combs for a hive
  | { screen: "workspace"; hiveDirName: string; combId: string }; // terminal workspace
