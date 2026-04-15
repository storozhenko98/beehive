#!/usr/bin/env node

import path from "node:path";
import { parseArgs } from "node:util";

import { bumpReleaseVersion } from "./release-version.js";

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    repo: { type: "string" },
  },
  allowPositionals: false,
});

const repoDir = path.resolve(values.repo ?? process.cwd());
const { currentVersion, nextVersion } = await bumpReleaseVersion(repoDir);
console.log(`Updated release version from ${currentVersion} to ${nextVersion}.`);
