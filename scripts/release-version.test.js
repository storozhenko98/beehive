import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  assertReleaseVersion,
  bumpReleaseVersion,
  bumpPatchVersion,
  readPackageVersion,
  setReleaseVersion,
} from "./release-version.js";

test("setReleaseVersion updates every release version file", async (t) => {
  const repoDir = await fs.mkdtemp(
    path.join(os.tmpdir(), "beehive-release-version-"),
  );

  t.after(async () => {
    await fs.rm(repoDir, { recursive: true, force: true });
  });

  await writeRepoFile(
    repoDir,
    "package.json",
    JSON.stringify({ name: "beehive", version: "1.2.3" }, null, 2) + "\n",
  );
  await writeRepoFile(
    repoDir,
    "package-lock.json",
    JSON.stringify(
      {
        name: "beehive",
        version: "1.2.3",
        lockfileVersion: 3,
        requires: true,
        packages: {
          "": {
            name: "beehive",
            version: "1.2.3",
          },
        },
      },
      null,
      2,
    ) + "\n",
  );
  await writeRepoFile(
    repoDir,
    "cli/Cargo.toml",
    ['[package]', 'name = "beehive-tui"', 'version = "1.2.3"', ""].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "src-tauri/Cargo.toml",
    ['[package]', 'name = "beehive"', 'version = "1.2.3"', ""].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "cli/Cargo.lock",
    [
      "[[package]]",
      'name = "beehive-tui"',
      'version = "1.2.3"',
      "",
      "[[package]]",
      'name = "other-cli-dep"',
      'version = "9.9.9"',
      "",
    ].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "src-tauri/Cargo.lock",
    [
      "[[package]]",
      'name = "beehive"',
      'version = "1.2.3"',
      "",
      "[[package]]",
      'name = "other-gui-dep"',
      'version = "9.9.9"',
      "",
    ].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "src-tauri/tauri.conf.json",
    JSON.stringify({ productName: "Beehive", version: "1.2.3" }, null, 2) + "\n",
  );

  await setReleaseVersion(repoDir, "1.2.4");

  assert.equal(
    await readPackageVersion(path.join(repoDir, "package.json")),
    "1.2.4",
  );

  const packageLock = JSON.parse(
    await fs.readFile(path.join(repoDir, "package-lock.json"), "utf8"),
  );
  assert.equal(packageLock.version, "1.2.4");
  assert.equal(packageLock.packages[""].version, "1.2.4");

  const cliCargoToml = await fs.readFile(
    path.join(repoDir, "cli/Cargo.toml"),
    "utf8",
  );
  assert.match(cliCargoToml, /version = "1\.2\.4"/);
  assert.doesNotMatch(cliCargoToml, /version = "1\.2\.3"/);

  const tauriCargoToml = await fs.readFile(
    path.join(repoDir, "src-tauri/Cargo.toml"),
    "utf8",
  );
  assert.match(tauriCargoToml, /version = "1\.2\.4"/);
  assert.doesNotMatch(tauriCargoToml, /version = "1\.2\.3"/);

  const cliCargoLock = await fs.readFile(
    path.join(repoDir, "cli/Cargo.lock"),
    "utf8",
  );
  assert.match(cliCargoLock, /name = "beehive-tui"[\s\S]*version = "1\.2\.4"/);
  assert.match(cliCargoLock, /name = "other-cli-dep"[\s\S]*version = "9\.9\.9"/);

  const tauriCargoLock = await fs.readFile(
    path.join(repoDir, "src-tauri/Cargo.lock"),
    "utf8",
  );
  assert.match(tauriCargoLock, /name = "beehive"[\s\S]*version = "1\.2\.4"/);
  assert.match(
    tauriCargoLock,
    /name = "other-gui-dep"[\s\S]*version = "9\.9\.9"/,
  );

  const tauriConfig = JSON.parse(
    await fs.readFile(path.join(repoDir, "src-tauri/tauri.conf.json"), "utf8"),
  );
  assert.equal(tauriConfig.version, "1.2.4");
});

test("bumpPatchVersion increments the patch segment", () => {
  assert.equal(bumpPatchVersion("0.1.106"), "0.1.107");
});

test("assertReleaseVersion rejects invalid versions", () => {
  assert.equal(assertReleaseVersion("1.2.3"), "1.2.3");
  assert.throws(() => assertReleaseVersion("foo"), /Unsupported version format/);
});

test("bumpReleaseVersion increments the current repo version", async (t) => {
  const repoDir = await fs.mkdtemp(
    path.join(os.tmpdir(), "beehive-release-version-bump-"),
  );

  t.after(async () => {
    await fs.rm(repoDir, { recursive: true, force: true });
  });

  await writeRepoFile(
    repoDir,
    "package.json",
    JSON.stringify({ name: "beehive", version: "1.2.3" }, null, 2) + "\n",
  );
  await writeRepoFile(
    repoDir,
    "package-lock.json",
    JSON.stringify(
      {
        name: "beehive",
        version: "1.2.3",
        packages: { "": { version: "1.2.3" } },
      },
      null,
      2,
    ) + "\n",
  );
  await writeRepoFile(
    repoDir,
    "cli/Cargo.toml",
    ['[package]', 'name = "beehive-tui"', 'version = "1.2.3"', ""].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "src-tauri/Cargo.toml",
    ['[package]', 'name = "beehive"', 'version = "1.2.3"', ""].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "cli/Cargo.lock",
    ['[[package]]', 'name = "beehive-tui"', 'version = "1.2.3"', ""].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "src-tauri/Cargo.lock",
    ['[[package]]', 'name = "beehive"', 'version = "1.2.3"', ""].join("\n"),
  );
  await writeRepoFile(
    repoDir,
    "src-tauri/tauri.conf.json",
    JSON.stringify({ productName: "Beehive", version: "1.2.3" }, null, 2) + "\n",
  );

  const result = await bumpReleaseVersion(repoDir);

  assert.deepEqual(result, {
    currentVersion: "1.2.3",
    nextVersion: "1.2.4",
  });
  assert.equal(
    await readPackageVersion(path.join(repoDir, "package.json")),
    "1.2.4",
  );
});

async function writeRepoFile(repoDir, relativePath, content) {
  const absolutePath = path.join(repoDir, relativePath);
  await fs.mkdir(path.dirname(absolutePath), { recursive: true });
  await fs.writeFile(absolutePath, content);
}
