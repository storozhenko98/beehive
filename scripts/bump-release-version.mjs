#!/usr/bin/env node

import path from "node:path";

import {
  bumpPatchVersion,
  readPackageVersion,
  setReleaseVersion,
} from "./release-version.mjs";

const args = parseArgs(process.argv.slice(2));
const repoDir = path.resolve(args.repo ?? process.cwd());
const currentVersion = await readPackageVersion(path.join(repoDir, "package.json"));
const nextVersion = bumpPatchVersion(currentVersion);
await setReleaseVersion(repoDir, nextVersion);

console.log(`Updated release version from ${currentVersion} to ${nextVersion}.`);

function parseArgs(argv) {
  const parsed = {};

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      throw new Error(`Unexpected argument: ${argument}`);
    }

    const key = argument.slice(2);
    const value = argv[index + 1];

    if (!value || value.startsWith("--")) {
      throw new Error(`Missing value for --${key}`);
    }

    parsed[key] = value;
    index += 1;
  }

  return parsed;
}
