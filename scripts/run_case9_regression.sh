#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}
LANDTYPE=${EARTHMESH_CASE9_LANDTYPE:-"$ROOT/input/landtype_igbp_update.nc"}
EXPECTED_LANDTYPE_SHA=${EARTHMESH_CASE9_LANDTYPE_SHA256:-89bde86be2436f8762bd9d2b9bcfa727193e74299941e9d1545222b54e41be2a}
EXPECTED_GRID_SHA=${EARTHMESH_CASE9_GRID_SHA256:-ecfd5366d9087df9c9208913aa27851e976f71184e4a0a5da76fc332eca79ef2}
OUT=${EARTHMESH_CASE9_OUTPUT:-"$ROOT/target/case9-regression-$(date +%s)"}

feature_args=(--features static-netcdf)
if [[ -n "${CLI_FEATURES:-}" ]]; then
    read -r -a feature_args <<<"$CLI_FEATURES"
fi

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

absolute_file() {
    local path=$1
    local directory
    directory=$(CDPATH= cd -- "$(dirname -- "$path")" && pwd -P)
    printf '%s/%s\n' "$directory" "$(basename -- "$path")"
}

[[ -f "$LANDTYPE" ]] || {
    echo "missing Case 9 landcover file: $LANDTYPE" >&2
    exit 2
}
LANDTYPE=$(absolute_file "$LANDTYPE")
landtype_sha=$(sha256 "$LANDTYPE")
[[ "$landtype_sha" == "$EXPECTED_LANDTYPE_SHA" ]] || {
    echo "Case 9 landcover sha256 mismatch: $landtype_sha" >&2
    exit 2
}

mkdir -p "$OUT"
OUT=$(CDPATH= cd -- "$OUT" && pwd -P)
landtype_json=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$LANDTYPE")

cat >"$OUT/project.yaml" <<EOF
schema_version: 3.0.0
metadata:
  name: global-tri-landcover-threshold
  authors: []
  description: Case 9 durable regression
domain: Global
target:
  kind: Ocean
  cell: Tri
  intent: CoastalOcean
  resolution: !ApproxKm 100.0
  model_format: Fvcom
data_layers:
- id: landcover
  role: LandType
  path: $landtype_json
  enabled: true
refinement:
  enabled: true
  threshold_enabled: true
  max_passes: 3
  threshold_criteria:
  - id: landcover
    enabled: true
    value: 12.0
  specified_circle: null
  specified_bbox: null
  specified_close: null
  hfield:
    enabled: true
    g: 0.2
    max_level: 0
    base_m: null
quality:
  min_angle_deg: 25.0
  auto_refine_batch_cells: 1
  on_violation: Warn
expert:
  nxp: null
  openmp: null
  niter: null
  niter_refine: 1
  max_iter_spc: 1
  max_iter_cal: 3
  halo: null
  max_transition_row: null
  set_dis_type: null
  num_rc: null
  vertex_pretect_layers: null
  spring_global_type: null
  spring_regional_type: null
  beta: null
  relax: null
  weak_concav_eliminate: null
hydro_coast: null
coupling: null
EOF

if [[ -n "${EARTHMESH_CASE9_CLI:-}" ]]; then
    CLI=$(absolute_file "$EARTHMESH_CASE9_CLI")
else
    (
        cd "$ROOT"
        "$CARGO" build --release --bin earthmesh_cli "${feature_args[@]}"
    )
    CLI="$ROOT/target/release/earthmesh_cli"
fi
[[ -x "$CLI" ]] || {
    echo "Case 9 CLI is not executable: $CLI" >&2
    exit 2
}

"$CLI" --project "$OUT/project.yaml" --max-tris 2000000 --quiet \
    2>&1 | tee "$OUT/run.log"

run_root=$(find "$OUT" -maxdepth 1 -type d -name 'project.yaml.earthmesh-run-*' -print -quit)
[[ -n "$run_root" ]] || {
    echo "Case 9 run directory was not created" >&2
    exit 1
}
result="$run_root/global-tri-landcover-threshold/result"
gridfile="$result/gridfile_NXP0081_tri.nc4"
quality="$result/quality_summary.json"
namelist="$result/namelist.save"
for required in "$gridfile" "$quality" "$namelist" "$gridfile.source-demand.json"; do
    [[ -f "$required" ]] || {
        echo "Case 9 output is missing: $required" >&2
        exit 1
    }
done

grid_sha=$(sha256 "$gridfile")
[[ "$grid_sha" == "$EXPECTED_GRID_SHA" ]] || {
    echo "Case 9 gridfile sha256 mismatch: $grid_sha" >&2
    exit 1
}

export EARTHMESH_P2_CONTROL_GRIDFILE="$gridfile"
export EARTHMESH_P2_CONTROL_NAMELIST="$namelist"
export EARTHMESH_P2_CONTROL_OUTPUT="$OUT/p2-negative-control.json"
(
    cd "$ROOT"
    "$CARGO" test --manifest-path rust/earthmesh_cli/Cargo.toml --lib \
        p2_primal_refine_research::satisfied_tri_control_is_a_topology_preserving_noop \
        -- --ignored --exact --test-threads=1
)

python3 - "$quality" "$OUT/p2-negative-control.json" "$OUT/case9-regression.json" \
    "$landtype_sha" "$(sha256 "$CLI")" "$grid_sha" <<'PY'
import json
import sys
from pathlib import Path

quality_path, p2_path, output_path, landtype_sha, cli_sha, grid_sha = sys.argv[1:]
quality = json.loads(Path(quality_path).read_text())
p2 = json.loads(Path(p2_path).read_text())
geometry = quality["geometry"]
topology = quality["topology"]
hfield = quality["hfield"]
gates = {gate["metric"]: gate for gate in quality["gates"]}

assert quality["cell_view"] == "tri"
assert quality["verdict"] in {"pass", "warn"}
assert geometry["cell_count"] == 210048
assert geometry["self_intersection_count"] == 0
assert geometry["invalid_polygon_count"] == 0
for field in (
    "non_manifold_vertex_fan_count",
    "invalid_vertex_index_count",
    "invalid_cell_index_count",
    "duplicate_edge_count",
    "dangling_edge_count",
    "misoriented_shared_edge_count",
    "neighbor_reciprocity_failure_count",
    "orphan_cell_count",
):
    assert topology[field] == 0, (field, topology[field])
assert hfield["target_above_actual_count"] == 0
assert hfield["actual_level_jump_gt_one_count"] == 0
assert gates["hfield_uncovered_hard_support_bin_count"]["value"] == 0
assert p2["cells"] == 210048
assert p2["active_hard_bins"] == 116
assert p2["adequately_covered_hard_bins"] == 116
assert p2["passes"] == 0
assert p2["changed"] is False

result = {
    "kind": "earthmesh_case9_regression",
    "build_profile": "release",
    "landtype_sha256": landtype_sha,
    "executable_sha256": cli_sha,
    "gridfile_sha256": grid_sha,
    "quality": {
        "verdict": quality["verdict"],
        "cells": geometry["cell_count"],
        "target_above_actual": hfield["target_above_actual_count"],
        "uncovered_hard_bins": gates["hfield_uncovered_hard_support_bin_count"]["value"],
    },
    "p2_negative_control": p2,
}
Path(output_path).write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
print(json.dumps(result, indent=2, sort_keys=True))
PY

ln -sfn "$OUT" "$ROOT/target/case9-regression-latest"
echo "case9_regression=$OUT/case9-regression.json"
