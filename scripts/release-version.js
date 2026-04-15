import fs from "node:fs/promises";
import path from "node:path";

export const versionFiles = [
  "package.json",
  "package-lock.json",
  "cli/Cargo.toml",
  "cli/Cargo.lock",
  "src-tauri/Cargo.toml",
  "src-tauri/Cargo.lock",
  "src-tauri/tauri.conf.json",
];

export async function readPackageVersion(filePath) {
  const fileContents = await fs.readFile(filePath, "utf8");
  return JSON.parse(fileContents).version;
}

export async function readReleaseVersion(repoDir) {
  return readPackageVersion(path.join(repoDir, "package.json"));
}

export async function setReleaseVersion(repoDir, nextVersion) {
  assertReleaseVersion(nextVersion);

  for (const file of versionFiles) {
    await updateVersionFile(repoDir, file, nextVersion);
  }
}

export async function bumpReleaseVersion(repoDir) {
  const currentVersion = await readReleaseVersion(repoDir);
  const nextVersion = bumpPatchVersion(currentVersion);
  await setReleaseVersion(repoDir, nextVersion);
  return { currentVersion, nextVersion };
}

export function bumpPatchVersion(version) {
  const parts = parseVersion(version);
  return `${parts[0]}.${parts[1]}.${parts[2] + 1}`;
}

export function assertReleaseVersion(version) {
  parseVersion(version);
  return version;
}

function parseVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`Unsupported version format: ${version}`);
  }

  return match.slice(1).map(Number);
}

async function updateVersionFile(repoDir, relativePath, nextVersion) {
  const absolutePath = path.join(repoDir, relativePath);
  const current = await fs.readFile(absolutePath, "utf8");
  const updated = transformVersionFile(relativePath, current, nextVersion);

  if (updated !== current) {
    await fs.writeFile(absolutePath, updated);
  }
}

function transformVersionFile(relativePath, content, nextVersion) {
  switch (relativePath) {
    case "package.json": {
      const packageJson = JSON.parse(content);
      packageJson.version = nextVersion;
      return `${JSON.stringify(packageJson, null, 2)}\n`;
    }
    case "package-lock.json": {
      const packageLock = JSON.parse(content);
      packageLock.version = nextVersion;
      if (packageLock.packages?.[""]) {
        packageLock.packages[""].version = nextVersion;
      }
      return `${JSON.stringify(packageLock, null, 2)}\n`;
    }
    case "src-tauri/tauri.conf.json": {
      const tauriConfig = JSON.parse(content);
      tauriConfig.version = nextVersion;
      return `${JSON.stringify(tauriConfig, null, 2)}\n`;
    }
    case "cli/Cargo.toml":
      return replacePackageVersion(content, "beehive-tui", nextVersion);
    case "src-tauri/Cargo.toml":
      return replacePackageVersion(content, "beehive", nextVersion);
    case "cli/Cargo.lock":
      return replaceLockVersion(content, "beehive-tui", nextVersion);
    case "src-tauri/Cargo.lock":
      return replaceLockVersion(content, "beehive", nextVersion);
    default:
      throw new Error(`Unsupported version file: ${relativePath}`);
  }
}

function replacePackageVersion(content, packageName, nextVersion) {
  const packageBlockPattern = new RegExp(
    `(\\[package\\][\\s\\S]*?name = "${escapeRegExp(packageName)}"[\\s\\S]*?version = ")([^"]+)(")`,
  );

  return replaceOnce(content, packageBlockPattern, `$1${nextVersion}$3`, packageName);
}

function replaceLockVersion(content, packageName, nextVersion) {
  const lockBlockPattern = new RegExp(
    `(\\[\\[package\\]\\][\\s\\S]*?name = "${escapeRegExp(packageName)}"[\\s\\S]*?version = ")([^"]+)(")`,
  );

  return replaceOnce(content, lockBlockPattern, `$1${nextVersion}$3`, packageName);
}

function replaceOnce(content, pattern, replacement, label) {
  if (!pattern.test(content)) {
    throw new Error(`Could not find version block for ${label}.`);
  }

  return content.replace(pattern, replacement);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
