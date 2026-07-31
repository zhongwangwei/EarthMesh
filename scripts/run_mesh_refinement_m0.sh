#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT=${EARTHMESH_M0_OUTPUT_DIR:-"$ROOT/target/mesh-refinement-m0"}
mkdir -p "$OUT"
export EARTHMESH_M0_OUTPUT_DIR="$OUT"

python3 - "$ROOT" "$OUT/measurement_manifest.json" <<'PY'
import hashlib
import json
import os
import pathlib
import platform
import subprocess
import sys
import time

root = pathlib.Path(sys.argv[1])
output = pathlib.Path(sys.argv[2])
iterations = os.environ.get("EARTHMESH_M0_ITERS", "0,50,500,5000")

def run(*args):
    return subprocess.run(args, cwd=root, text=True, check=True, capture_output=True).stdout.strip()

tracked = run("git", "ls-files", "-co", "--exclude-standard").splitlines()
source_suffixes = {".rs", ".toml", ".yaml", ".yml", ".nml", ".sh", ".js", ".ts"}
hasher = hashlib.sha256()
for name in sorted(tracked):
    path = root / name
    if path.is_file() and (path.suffix in source_suffixes or path.name == "Cargo.lock"):
        hasher.update(name.encode())
        hasher.update(b"\0")
        hasher.update(path.read_bytes())
        hasher.update(b"\0")

manifest = {
    "kind": "earthmesh_mesh_refinement_m0_manifest",
    "created_epoch_seconds": int(time.time()),
    "git_head": run("git", "rev-parse", "HEAD"),
    "git_status_porcelain_v2": run("git", "status", "--porcelain=v2"),
    "source_config_sha256": hasher.hexdigest(),
    "rustc": run("rustc", "--version"),
    "cargo": run("cargo", "--version"),
    "build_profile": "test",
    "platform": platform.platform(),
    "nxp": int(os.environ.get("EARTHMESH_M0_NXP", "81")),
    "global_niter": int(os.environ.get("EARTHMESH_M0_GLOBAL_NITER", "5000")),
    "hfield_g": float(os.environ.get("EARTHMESH_M0_HFIELD_G", "0.2")),
    "requested_hfield_g": float(os.environ.get("EARTHMESH_M0_HFIELD_G", "0.2")),
    "iterations": iterations,
    "iteration_policy": "fixed",
    "cases": os.environ.get("EARTHMESH_M0_CASES", "G-CIRCLE,G-FRAGMENT,R-BBOX"),
    "repeats": int(os.environ.get("EARTHMESH_M0_REPEATS", "2")),
    "topology_g_cap_variants": os.environ.get("EARTHMESH_M0_TOPOLOGY_G_CAPS", "off,on"),
    "m0_diagnostics": os.environ.get("EARTHMESH_M0_COLLECT_DIAGNOSTICS", "on"),
    "repair_trace": os.environ.get("EARTHMESH_M0_REPAIR_TRACE", "off"),
    "diagnostics_parity": os.environ.get("EARTHMESH_M0_DIAGNOSTICS_PARITY", "on"),
}
output.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
PY

cd "$ROOT"
set +e
cargo test --manifest-path rust/earthmesh_cli/Cargo.toml \
    --test mesh_refinement_m0 mesh_refinement_m0_measurements -- \
    --ignored --test-threads=1 --nocapture 2>&1 | tee "$OUT/cargo-test.log"
test_status=${PIPESTATUS[0]}
set -e

python3 - "$ROOT" "$OUT" <<'PY'
import hashlib
import json
import pathlib
import sys
import time

root = pathlib.Path(sys.argv[1])
output = pathlib.Path(sys.argv[2])

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

manifest_path = output / "measurement_manifest.json"
manifest = json.loads(manifest_path.read_text())
executables = list((root / "target/debug/deps").glob("mesh_refinement_m0-*"))
executable = max((path for path in executables if path.is_file() and path.stat().st_mode & 0o111),
                 key=lambda path: path.stat().st_mtime)
inputs = sorted(path for path in output.rglob("*")
                if path.is_file() and (path.name == "project.nml" or "sources" in path.parts or "threshold" in path.parts))
manifest.update({
    "completed_epoch_seconds": int(time.time()),
    "test_executable": str(executable),
    "test_executable_sha256": digest(executable),
    "inputs": [{"path": str(path.relative_to(output)), "sha256": digest(path)} for path in inputs],
    "cargo_test_log": "cargo-test.log",
})
manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")

measurements_path = output / "measurements.json"
if measurements_path.is_file():
    measurements = json.loads(measurements_path.read_text())
    for run in measurements["runs"]:
        gridfile = pathlib.Path(run.get("gridfile", ""))
        if gridfile.is_file():
            run["gridfile_sha256"] = digest(gridfile)
    measurements_path.write_text(json.dumps(measurements, ensure_ascii=False, indent=2) + "\n")
PY

echo "measurement_manifest=$OUT/measurement_manifest.json"
echo "measurements=$OUT/measurements.json"
exit "$test_status"
