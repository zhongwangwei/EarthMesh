#!/usr/bin/env bash
# A regional Hex mesh whose domain has an interior hole, measured end to end.
#
# The case exists for the topology: a domain with a hole is a disk with one
# puncture, so the mesh must come out with Euler characteristic 0 and exactly
# two boundary rims -- the outer rim and the one around the hole. A run that
# quietly filled the hole in would still be a valid mesh and would still pass
# every per-cell check; only the boundary count says otherwise.
#
# The domain and the refinement curve are generated here rather than committed
# as binary fixtures: the repository carries no shapefiles, and a script that
# depends on one that is not there fails with nothing to act on.
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

# Minimal ESRI Polygon shapefiles. Only the geometry is read (see
# `read_shapefile_polygon_rings`), so no .shx/.dbf/.prj is written; absent a
# .prj the coordinates are taken as WGS84, which is what these are.
python3 - "$OUT" <<'PY'
import struct
import sys
from pathlib import Path

out = Path(sys.argv[1])


def polygon_shapefile(parts):
    total = sum(len(part) for part in parts)
    content_len = 44 + len(parts) * 4 + total * 16
    lons = [lon for part in parts for lon, _ in part]
    lats = [lat for part in parts for _, lat in part]

    header = bytearray(100)
    header[0:4] = struct.pack(">i", 9994)
    header[24:28] = struct.pack(">i", (100 + 8 + content_len) // 2)
    header[28:32] = struct.pack("<i", 1000)
    header[32:36] = struct.pack("<i", 5)
    header[36:68] = struct.pack("<4d", min(lons), min(lats), max(lons), max(lats))

    body = bytearray()
    body += struct.pack(">ii", 1, content_len // 2)
    body += struct.pack("<i", 5)
    body += struct.pack("<4d", min(lons), min(lats), max(lons), max(lats))
    body += struct.pack("<ii", len(parts), total)
    start = 0
    for part in parts:
        body += struct.pack("<i", start)
        start += len(part)
    for part in parts:
        for lon, lat in part:
            body += struct.pack("<2d", lon, lat)
    return bytes(header + body)


def ring(w, e, s, n):
    return [(w, s), (e, s), (e, n), (w, n), (w, s)]


# A 16-degree basin with a 5-degree lake in the middle. ESRI holes wind the
# other way round from their shell; the reader classifies by containment, but
# writing it correctly keeps the file honest for any other consumer.
shell = ring(104.0, 120.0, 16.0, 32.0)
lake = list(reversed(ring(110.0, 115.0, 22.0, 27.0)))
(out / "watershed_with_hole.shp").write_bytes(polygon_shapefile([shell, lake]))

# The refinement curve: a band inside the basin, clear of the lake.
(out / "basin_refinement.shp").write_bytes(
    polygon_shapefile([ring(105.5, 118.5, 17.5, 20.5)])
)
PY

cat >"$OUT/project.yaml" <<EOF
schema_version: 3.0.0
metadata:
  name: regional-basin-hole-hex
  authors: []
  description: Regional Hex watershed with one interior hole
domain: !Regional
  shape: !Shapefile
    path: $OUT/watershed_with_hole.shp
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
    path: $OUT/basin_refinement.shp
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

"$CLI" --project "$OUT/project.yaml" --quiet >"$OUT/run.log" 2>&1 || {
    echo "basin-hole run failed; see $OUT/run.log" >&2
    tail -20 "$OUT/run.log" >&2
    exit 1
}
run_root=$(find "$OUT" -maxdepth 1 -type d -name 'project.yaml.earthmesh-run-*' -print -quit)
result="$run_root/regional-basin-hole-hex/result"
grid="$result/gridfile_NXP0081_hex.nc4"
[[ -f "$grid" ]] || {
    echo "basin-hole regression output is missing: $grid" >&2
    exit 1
}

# The project run writes a quality report only under the AutoRefine policy; this
# case uses Warn so the mesh is judged as produced. Measure it here.
"$CLI" --mesh-quality "$grid" "$result" "$result/namelist.save" --kind hex \
    >"$result/quality.log" 2>&1 || {
    echo "quality measurement failed; see $result/quality.log" >&2
    exit 1
}
quality="$result/quality_summary.json"
[[ -f "$quality" ]] || {
    echo "basin-hole quality report is missing: $quality" >&2
    exit 1
}

# Determinism against this build rather than a hash pinned from another one:
# a pinned hash turns every intentional change into a false regression and says
# nothing about whether the run repeats.
grid_sha=$(sha256 "$grid")
mkdir -p "$OUT/repeat"
cp "$OUT/project.yaml" "$OUT/repeat/"
"$CLI" --project "$OUT/repeat/project.yaml" --quiet >"$OUT/repeat/run.log" 2>&1
repeat_root=$(find "$OUT/repeat" -maxdepth 1 -type d -name 'project.yaml.earthmesh-run-*' -print -quit)
repeat_sha=$(sha256 "$repeat_root/regional-basin-hole-hex/result/gridfile_NXP0081_hex.nc4")
[[ "$grid_sha" == "$repeat_sha" ]] || {
    echo "basin-hole run is not deterministic: $grid_sha vs $repeat_sha" >&2
    exit 1
}

python3 - "$quality" "$OUT/basin-hole-regression.json" \
    "$(sha256 "$CLI")" "$grid_sha" <<'PY'
import json
import sys
from pathlib import Path

quality_path, output_path, cli_sha, grid_sha = sys.argv[1:]
quality = json.loads(Path(quality_path).read_text())
geometry = quality["geometry"]
topology = quality["topology"]
hfield = quality["hfield"]

assert quality["cell_view"] == "hex"
assert quality["verdict"] == "pass", quality["verdict"]
# Asserted as a shape, not a pinned count: the cell count moves with any
# intentional change to the engine and would turn every one into a false
# regression, while the topology below is what this case is about.
assert geometry["cell_count"] > 0
for field in (
    "zero_area_cell_count",
    "negative_area_cell_count",
    "non_finite_cell_count",
    "self_intersection_count",
    "invalid_polygon_count",
):
    assert geometry[field] == 0, (field, geometry[field])
# The hole, stated three ways. Each would survive the loss of the other two.
assert topology["euler_characteristic"] == 0, topology["euler_characteristic"]
# Measured standalone, so the project's declared topology is not in scope and
# the expectation is absent. What must never happen is a *wrong* expectation
# quietly passing the gate.
assert topology["expected_euler_characteristic"] in (0, None), topology[
    "expected_euler_characteristic"
]
assert topology["connected_component_count"] == 1
assert topology["boundary_loop_count"] == 2, topology["boundary_loop_count"]
for field in (
    "non_manifold_vertex_fan_count",
    "invalid_vertex_index_count",
    "invalid_cell_index_count",
    "duplicate_edge_count",
    "dangling_edge_count",
    "misoriented_shared_edge_count",
    "neighbor_reciprocity_failure_count",
    "orphan_cell_count",
    "boundary_vertex_degree_violation_count",
):
    assert topology[field] == 0, (field, topology[field])
assert hfield["target_above_actual_count"] == 0
refined = sum(
    row["count"] for row in hfield["actual_refine_level_distribution"] if row["level"] > 0
)
assert refined > 0, "the refinement curve must have refined something"

result = {
    "kind": "earthmesh_basin_hole_regression",
    "build_profile": "release",
    "executable_sha256": cli_sha,
    "gridfile_sha256": grid_sha,
    "cells": geometry["cell_count"],
    "refined_cells": refined,
    "boundary_loops": topology["boundary_loop_count"],
    "euler_characteristic": topology["euler_characteristic"],
}
Path(output_path).write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
print(json.dumps(result, indent=2, sort_keys=True))
PY

ln -sfn "$OUT" "$ROOT/target/basin-hole-regression-latest"
echo "basin_hole_regression=$OUT/basin-hole-regression.json"
