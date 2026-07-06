#!/usr/bin/env bash
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

GRIDFILE=${GRIDFILE:-cases/quickstart_n16/gridfile/gridfile_NXP0016_01_hex.nc4}
CARGO=${CARGO:-cargo}
if [ ! -f "$GRIDFILE" ]; then
  echo "gridfile fixture not found: $GRIDFILE" >&2
  exit 2
fi

tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/earthmesh-quality-views.XXXXXX")
CLEAN_WORKDIR=${CLEAN_WORKDIR:-1}
trap 'if [ "$CLEAN_WORKDIR" = "1" ]; then rm -rf "$tmpdir"; else echo "kept $tmpdir"; fi' EXIT

for kind in tri hex; do
  mkdir -p "$tmpdir/$kind"
  "$CARGO" run --quiet --manifest-path rust/earthmesh_cli/Cargo.toml -- \
    --mesh-quality "$GRIDFILE" "$tmpdir/$kind" --kind "$kind" > "$tmpdir/$kind.stdout"
done

python3 - "$tmpdir" <<'PY'
import json
import pathlib
import sys


def fail(message):
    raise SystemExit(message)


base = pathlib.Path(sys.argv[1])
for kind in ("tri", "hex"):
    out_dir = base / kind
    for name in (
        "quality_summary.json",
        "quality_summary.csv",
        "worst_cells.geojson",
        "quality_report.md",
    ):
        if not (out_dir / name).is_file():
            fail(f"{kind}: missing {name}")
    data = json.loads((out_dir / "quality_summary.json").read_text())
    if data.get("cell_view") != kind:
        fail(f"{kind}: cell_view={data.get('cell_view')!r}")
    topo = data["topology"]
    stdout = (base / f"{kind}.stdout").read_text()
    if f"mesh_quality_kind={kind}" not in stdout:
        fail(f"{kind}: missing mesh_quality_kind stdout")
    if "mesh_quality_cell_sides=" not in stdout:
        fail(f"{kind}: missing mesh_quality_cell_sides stdout")
    report_md = (out_dir / "quality_report.md").read_text()
    if f"- cell view: `{kind}`" not in report_md:
        fail(f"{kind}: missing report cell view")
    csv = (out_dir / "quality_summary.csv").read_text()
    if f"summary,cell_view,,{kind}" not in csv:
        fail(f"{kind}: missing CSV cell_view row")
    side_total = sum(topo.get(k, 0) for k in (
        "triangle_cell_count",
        "quadrilateral_cell_count",
        "pentagon_cell_count",
        "hexagon_cell_count",
        "heptagon_cell_count",
        "other_polygon_cell_count",
    ))
    cell_count = data["geometry"]["cell_count"]
    if side_total != cell_count:
        fail(f"{kind}: side counts {side_total} != cells {cell_count}")
    print(kind, data["verdict"], cell_count, "cells")
PY
