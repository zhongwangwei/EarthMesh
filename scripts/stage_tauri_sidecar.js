#!/usr/bin/env node
"use strict";

const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const cargo = process.env.CARGO || "cargo";
const rustc = process.env.RUSTC || "rustc";
const target =
  process.env.TAURI_ENV_TARGET_TRIPLE ||
  execFileSync(rustc, ["--print", "host-tuple"], { encoding: "utf8" }).trim();
const extension = target.includes("windows") ? ".exe" : "";
const targetDir = path.resolve(root, process.env.CARGO_TARGET_DIR || "target");
const source = path.join(
  targetDir,
  target,
  "release",
  `earthmesh_cli${extension}`,
);
const destination = path.join(
  root,
  "gui-tauri",
  "src-tauri",
  "binaries",
  `earthmesh_cli-${target}${extension}`,
);

execFileSync(
  cargo,
  [
    "build",
    "--manifest-path",
    path.join(root, "rust", "earthmesh_cli", "Cargo.toml"),
    "--release",
    "--locked",
    "--features",
    "static-netcdf",
    "--target",
    target,
  ],
  { cwd: root, env: process.env, stdio: "inherit" },
);
fs.mkdirSync(path.dirname(destination), { recursive: true });
fs.copyFileSync(source, destination);
if (process.platform !== "win32") fs.chmodSync(destination, 0o755);
console.log(`staged Tauri sidecar: ${destination}`);
