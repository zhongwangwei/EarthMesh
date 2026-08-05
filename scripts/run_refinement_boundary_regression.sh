#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}
OUT=${EARTHMESH_BOUNDARY_OUTPUT:-"$ROOT/target/refinement-boundary-regression-$(date +%s)"}

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

command -v ncdump >/dev/null 2>&1 || {
    echo "ncdump is required for polar/dateline location checks" >&2
    exit 2
}
mkdir -p "$OUT"
OUT=$(CDPATH= cd -- "$OUT" && pwd -P)

cat >"$OUT/polar.yaml" <<'EOF'
schema_version: 3.0.0
metadata:
  name: global-polar-circle
  authors: []
  description: Global north-pole Method-C regression
domain: Global
target:
  kind: Atmosphere
  cell: Hex
  intent: AtmosphereMpas
  resolution: !Nxp 81
  model_format: Mpas
data_layers: []
refinement:
  enabled: true
  max_passes: 2
  specified_circle:
    lon: 30.0
    lat: 89.0
    radius_km: 600.0
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

cat >"$OUT/dateline.yaml" <<'EOF'
schema_version: 3.0.0
metadata:
  name: global-dateline-bbox
  authors: []
  description: Global antimeridian Method-C regression
domain: Global
target:
  kind: Atmosphere
  cell: Hex
  intent: AtmosphereMpas
  resolution: !Nxp 81
  model_format: Mpas
data_layers: []
refinement:
  enabled: true
  max_passes: 2
  specified_bbox:
    w: 170.0
    e: -170.0
    s: -20.0
    n: 20.0
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

if [[ -n "${EARTHMESH_BOUNDARY_CLI:-}" ]]; then
    CLI=$EARTHMESH_BOUNDARY_CLI
else
    (
        cd "$ROOT"
        "$CARGO" build --release --bin earthmesh_cli "${feature_args[@]}"
    )
    CLI="$ROOT/target/release/earthmesh_cli"
fi
[[ -x "$CLI" ]] || {
    echo "boundary regression CLI is not executable: $CLI" >&2
    exit 2
}

for case_name in polar dateline; do
    "$CLI" --project "$OUT/$case_name.yaml" --max-tris 1000000 --quiet \
        >"$OUT/$case_name.log" 2>&1
done

polar_root=$(find "$OUT" -maxdepth 1 -type d -name 'polar.yaml.earthmesh-run-*' -print -quit)
dateline_root=$(find "$OUT" -maxdepth 1 -type d -name 'dateline.yaml.earthmesh-run-*' -print -quit)
polar_result="$polar_root/global-polar-circle/result"
dateline_result="$dateline_root/global-dateline-bbox/result"
polar_grid="$polar_result/gridfile_NXP0081_hex.nc4"
dateline_grid="$dateline_result/gridfile_NXP0081_hex.nc4"
# The project run writes a quality report only under the AutoRefine policy;
# these cases use Warn so the mesh is judged as produced, not repaired. Measure
# it here instead, from the namelist the run actually saved.
for case_dir in "$polar_result" "$dateline_result"; do
    grid=$(ls "$case_dir"/gridfile_*.nc4 2>/dev/null | head -1)
    "$CLI" --mesh-quality "$grid" "$case_dir" "$case_dir/namelist.save" --kind hex \
        >"$case_dir/quality.log" 2>&1 || {
        echo "quality measurement failed for $case_dir; see $case_dir/quality.log" >&2
        exit 1
    }
done

polar_quality="$polar_result/quality_summary.json"
dateline_quality="$dateline_result/quality_summary.json"
for required in "$polar_grid" "$dateline_grid" "$polar_quality" "$dateline_quality"; do
    [[ -f "$required" ]] || {
        echo "boundary regression output is missing: $required" >&2
        exit 1
    }
done

polar_sha=$(sha256 "$polar_grid")
dateline_sha=$(sha256 "$dateline_grid")

# Determinism is checked against this build, not against a hash pinned from
# another one: a pinned hash makes every intentional change look like a
# regression, and says nothing about whether the run repeats. Re-run the same
# project and require the bytes to match.
mkdir -p "$OUT/repeat"
for case_name in polar dateline; do
    cp "$OUT/$case_name.yaml" "$OUT/repeat/"
    "$CLI" --project "$OUT/repeat/$case_name.yaml" --max-tris 1000000 --quiet \
        >"$OUT/repeat/$case_name.log" 2>&1
done
repeat_polar_root=$(find "$OUT/repeat" -maxdepth 1 -type d -name 'polar.yaml.earthmesh-run-*' -print -quit)
repeat_dateline_root=$(find "$OUT/repeat" -maxdepth 1 -type d -name 'dateline.yaml.earthmesh-run-*' -print -quit)
repeat_polar_sha=$(sha256 "$repeat_polar_root/global-polar-circle/result/gridfile_NXP0081_hex.nc4")
repeat_dateline_sha=$(sha256 "$repeat_dateline_root/global-dateline-bbox/result/gridfile_NXP0081_hex.nc4")
[[ "$polar_sha" == "$repeat_polar_sha" ]] || {
    echo "polar run is not deterministic: $polar_sha vs $repeat_polar_sha" >&2
    exit 1
}
[[ "$dateline_sha" == "$repeat_dateline_sha" ]] || {
    echo "dateline run is not deterministic: $dateline_sha vs $repeat_dateline_sha" >&2
    exit 1
}

python3 - "$polar_quality" "$polar_grid" "$dateline_quality" "$dateline_grid" \
    "$OUT/boundary-regression.json" "$(sha256 "$CLI")" "$polar_sha" "$dateline_sha" <<'PY'
import json
import re
import subprocess
import sys
from pathlib import Path

polar_quality, polar_grid, dateline_quality, dateline_grid, output, cli_sha, polar_sha, dateline_sha = sys.argv[1:]

def load_quality(path, cells):
    report = json.loads(Path(path).read_text())
    gates = {gate["metric"]: gate for gate in report["gates"]}
    geometry = report["geometry"]
    topology = report["topology"]
    hfield = report["hfield"]
    assert report["cell_view"] == "hex"
    assert report["verdict"] in {"pass", "warn"}
    assert geometry["cell_count"] == cells
    assert geometry["self_intersection_count"] == 0
    assert geometry["invalid_polygon_count"] == 0
    assert topology["euler_characteristic"] == 2
    # `expected_euler_characteristic` is opt-in here (see the quality crate's
    # `expected_euler_characteristic_is_opt_in_and_enforced`), so it is null
    # unless a caller states what it expects. The measured value above is what
    # this script is checking.
    assert topology["expected_euler_characteristic"] in (None, 2)
    assert topology["connected_component_count"] == 1
    assert topology["boundary_edge_count"] == 0
    assert topology["boundary_loop_count"] == 0
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
    # The boundary-degree gate replaces the reference build's
    # `hfield_uncovered_hard_support_bin_count`, which is not wired here: it
    # needs the pre-gradient-limit field, which this build does not keep. See
    # the technical guide on why that was reverted rather than approximated.
    assert gates["boundary_vertex_degree_violation_count"]["value"] == 0
    return report

def grid_rows(path):
    text = subprocess.check_output(
        ["ncdump", "-v", "GLONW,GLATW,earthmesh_w_refine_level", path],
        text=True,
    )
    arrays = {}
    for name, cast in (("GLONW", float), ("GLATW", float), ("earthmesh_w_refine_level", int)):
        match = re.search(rf"\n {name} = (.*?);", text, re.S)
        assert match, name
        values = re.findall(
            r"(?<![A-Za-z_])[-+]?(?:\d+\.?\d*|\.\d+)(?:[Ee][-+]?\d+)?",
            match.group(1),
        )
        arrays[name] = [cast(float(value)) if cast is int else cast(value) for value in values]
    return list(zip(arrays["GLONW"][1:], arrays["GLATW"][1:], arrays["earthmesh_w_refine_level"][1:]))

# Cell counts for this build. They are not a contract -- a deliberate change to
# refinement moves them -- but pinning them turns a silent change in mesh size
# into a visible one, which is the whole point of a regression script.
polar = load_quality(polar_quality, 71033)
dateline = load_quality(dateline_quality, 92822)
polar_rows = grid_rows(polar_grid)
dateline_rows = grid_rows(dateline_grid)
polar_refined = [row for row in polar_rows if row[2] >= 2]
dateline_refined = [row for row in dateline_rows if row[2] >= 2]

assert min(row[1] for row in polar_rows) < -89.99
assert max(row[1] for row in polar_rows) > 89.99
# Counts for this build. The assertions that matter are the ones below them --
# refinement has to land at the pole and across the dateline, not merely produce
# some number of cells -- but a silent change in how much gets refined is worth
# seeing, so the counts are pinned too.
assert len(polar_refined) == 4735
# The 600 km circle sits at 89N; transition rows widen the refined band beyond
# it, so the bound is on where refinement must *not* reach rather than on the
# circle itself. Anything this far north is the polar cap and nothing else.
assert min(row[1] for row in polar_refined) > 80.0
assert max(row[1] for row in polar_refined) > 89.99
assert len(dateline_refined) == 26371
assert sum(row[0] > 170.0 for row in dateline_refined) > 10000
assert sum(row[0] < -170.0 for row in dateline_refined) > 10000
assert min(row[1] for row in dateline_refined) > -23.5
assert max(row[1] for row in dateline_refined) < 23.5

result = {
    "kind": "earthmesh_refinement_boundary_regression",
    "build_profile": "release",
    "executable_sha256": cli_sha,
    "polar": {
        "gridfile_sha256": polar_sha,
        "cells": polar["geometry"]["cell_count"],
        "refined_cells": len(polar_refined),
        "refined_latitude_range": [
            min(row[1] for row in polar_refined),
            max(row[1] for row in polar_refined),
        ],
    },
    "dateline": {
        "gridfile_sha256": dateline_sha,
        "cells": dateline["geometry"]["cell_count"],
        "refined_cells": len(dateline_refined),
        "east_seam_refined_cells": sum(row[0] > 170.0 for row in dateline_refined),
        "west_seam_refined_cells": sum(row[0] < -170.0 for row in dateline_refined),
    },
}
Path(output).write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
print(json.dumps(result, indent=2, sort_keys=True))
PY

ln -sfn "$OUT" "$ROOT/target/refinement-boundary-regression-latest"
echo "boundary_regression=$OUT/boundary-regression.json"
