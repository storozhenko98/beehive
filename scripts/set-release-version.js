#!/usr/bin/env node

import path from "node:path";
import { parseArgs } from "node:util";

import {
  readReleaseVersion,
  setReleaseVersion,
} from "./release-version.js";

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    repo: { type: "string" },
    version: { type: "string" },
  },
  allowPositionals: false,
});

const repoDir = path.resolve(values.repo ?? process.cwd());
const nextVersion = values.version;

if (!nextVersion) {
  console.error("Missing required --version argument.");
  process.exit(1);
}

const currentVersion = await readReleaseVersion(repoDir);
await setReleaseVersion(repoDir, nextVersion);

console.log(`Updated release version from ${currentVersion} to ${nextVersion}.`);
