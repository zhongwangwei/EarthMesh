#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
OUT=${EARTHMESH_BASIN_HOLE_OUTPUT:-"$ROOT/target/basin-hole-regression-$(date +%s)"}
CLI=${EARTHMESH_BASIN_HOLE_CLI:-}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

mkdir -p "$OUT"
OUT=$(CDPATH= cd -- "$OUT" && pwd -P)

cat >"$OUT/project.yaml" <<EOF
schema_version: 3.0.0
metadata:
  name: regional-basin-hole-hex
  authors: []
  description: Regional Hex watershed with one interior hole
domain: !Regional
  shape: !Shapefile
    path: $ROOT/test/fixtures/watershed_with_hole.shp
  sea_ratio: null
target:
  kind: Earth
  cell: Hex
  intent: Custom
  resolution: !Nxp 81
  model_format: CoLM
data_layers: []
refinement:
  enabled: true
  threshold_enabled: false
  max_passes: 2
  threshold_criteria: []
  specified_circle: null
  specified_bbox: null
  specified_close:
    path: $ROOT/test/fixtures/basin_refinement.shp
    boundary:
      mode: polyline
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
  niter: 5000
  niter_refine: 5000
  max_iter_spc: 2
hydro_coast: null
coupling: null
EOF

if [[ -z "$CLI" ]]; then
    (
        cd "$ROOT"
        cargo build --release --bin earthmesh_cli --features static-netcdf
    )
    CLI="$ROOT/target/release/earthmesh_cli"
fi

"$CLI" --project "$OUT/project.yaml" --quiet >"$OUT/run.log" 2>&1
run_root=$(find "$OUT" -maxdepth 1 -type d -name 'project.yaml.earthmesh-run-*' -print -quit)
result="$run_root/regional-basin-hole-hex/result"
grid="$result/gridfile_NXP0081_hex.nc4"
quality="$result/quality_summary.json"
demand="$grid.source-demand.json"
for required in "$grid" "$quality" "$demand"; do
    [[ -f "$required" ]] || {
        echo "basin-hole regression output is missing: $required" >&2
        exit 1
    }
done

grid_sha=$(sha256 "$grid")
[[ "$grid_sha" == "44c67c05c3463ab186ec665c9395d626954b956b157e83fa76b11bb467bbd807" ]] || {
    echo "unexpected basin-hole grid hash: $grid_sha" >&2
    exit 1
}

python3 - "$quality" "$demand" "$OUT/basin-hole-regression.json" \
    "$(sha256 "$CLI")" "$grid_sha" <<'PY'
import json
import sys
from pathlib import Path

quality_path, demand_path, output_path, cli_sha, grid_sha = sys.argv[1:]
quality = json.loads(Path(quality_path).read_text())
demand = json.loads(Path(demand_path).read_text())
geometry = quality["geometry"]
topology = quality["topology"]
hfield = quality["hfield"]
gates = {gate["metric"]: gate for gate in quality["gates"]}

assert quality["cell_view"] == "hex"
assert quality["verdict"] == "pass"
assert geometry["cell_count"] == 211
for field in (
    "zero_area_cell_count",
    "negative_area_cell_count",
    "non_finite_cell_count",
    "self_intersection_count",
    "invalid_polygon_count",
):
    assert geometry[field] == 0, (field, geometry[field])
assert topology["euler_characteristic"] == 0
assert topology["expected_euler_characteristic"] == 0
assert topology["connected_component_count"] == 1
assert topology["boundary_loop_count"] == 2
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
assert hfield["max_level"] == 2
assert hfield["target_above_actual_count"] == 0
assert max(row["level"] for row in hfield["actual_refine_level_distribution"]) == 2
assert gates["hfield_uncovered_hard_support_bin_count"]["value"] == 0
assert max(demand["hard_levels"]) == 2
assert sum(level > 0 for level in demand["hard_levels"]) == 24
assert len(demand["hard_layers"]) == 1
layer = demand["hard_layers"][0]
assert layer["kind"] == "specified"
assert max(layer["levels"]) == 2
assert sum(level > 0 for level in layer["levels"]) == 24

result = {
    "kind": "earthmesh_basin_hole_regression",
    "build_profile": "release",
    "executable_sha256": cli_sha,
    "gridfile_sha256": grid_sha,
    "cells": geometry["cell_count"],
    "refined_cells": sum(
        row["count"]
        for row in hfield["actual_refine_level_distribution"]
        if row["level"] > 0
    ),
    "boundary_loops": topology["boundary_loop_count"],
    "hard_support_bins": sum(level > 0 for level in demand["hard_levels"]),
}
Path(output_path).write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
print(json.dumps(result, indent=2, sort_keys=True))
PY

ln -sfn "$OUT" "$ROOT/target/basin-hole-regression-latest"
echo "basin_hole_regression=$OUT/basin-hole-regression.json"
