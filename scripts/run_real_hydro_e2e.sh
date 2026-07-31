#!/bin/sh
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo"
CARGO=${CARGO:-cargo}

: "${EARTHMESH_REAL_MERIT_ROOT:=/Volumes/Data01/MERIT_Hydro}"
: "${EARTHMESH_REAL_CAMA_ROOT:=/Volumes/Data01/CaMa-Map/glb_15min}"
: "${EARTHMESH_REAL_GRIDFILE:=$repo/test/earthmesh-project-run-43185-1784688745021471000-3/project.yaml.earthmesh-run-14828-1784777092876404000-0/earthmesh-project/gridfile/gridfile_NXP0042_01_hex.nc4}"
: "${EARTHMESH_REAL_SOURCE_NAMELIST:=$repo/test/earthmesh-project-run-43185-1784688745021471000-3/project.yaml.earthmesh-run-14828-1784777092876404000-0/project.nml}"
: "${EARTHMESH_REAL_LANDTYPE:=$repo/input/landtype_igbp_update.nc}"
: "${EARTHMESH_REAL_CELL_KIND:=hex}"
: "${EARTHMESH_REAL_BBOX_W:=113.25}"
: "${EARTHMESH_REAL_BBOX_S:=22.0}"
: "${EARTHMESH_REAL_BBOX_E:=113.5}"
: "${EARTHMESH_REAL_BBOX_N:=22.25}"
: "${EARTHMESH_REAL_EXPECT_CELL_COUNT:=1}"
: "${EARTHMESH_REAL_MERIT_STRIDE:=1}"
: "${EARTHMESH_REAL_KEEP_PRODUCTION_NITER:=0}"
: "${EARTHMESH_REAL_MAX_PASSES:=2}"

for path in "$EARTHMESH_REAL_MERIT_ROOT" "$EARTHMESH_REAL_CAMA_ROOT"; do
    if [ ! -d "$path" ]; then
        echo "required external data directory not found: $path" >&2
        exit 2
    fi
done
if [ ! -f "$EARTHMESH_REAL_GRIDFILE" ]; then
    echo "required production gridfile not found: $EARTHMESH_REAL_GRIDFILE" >&2
    exit 2
fi
if [ ! -f "$EARTHMESH_REAL_SOURCE_NAMELIST" ]; then
    echo "required production source namelist not found: $EARTHMESH_REAL_SOURCE_NAMELIST" >&2
    exit 2
fi
if [ ! -f "$EARTHMESH_REAL_LANDTYPE" ]; then
    echo "required production landtype file not found: $EARTHMESH_REAL_LANDTYPE" >&2
    exit 2
fi

export EARTHMESH_REAL_MERIT_ROOT EARTHMESH_REAL_CAMA_ROOT EARTHMESH_REAL_GRIDFILE
export EARTHMESH_REAL_SOURCE_NAMELIST
export EARTHMESH_REAL_LANDTYPE
export EARTHMESH_REAL_CELL_KIND EARTHMESH_REAL_BBOX_W EARTHMESH_REAL_BBOX_S
export EARTHMESH_REAL_BBOX_E EARTHMESH_REAL_BBOX_N EARTHMESH_REAL_MERIT_STRIDE
export EARTHMESH_REAL_EXPECT_CELL_COUNT
export EARTHMESH_REAL_KEEP_PRODUCTION_NITER
export EARTHMESH_REAL_MAX_PASSES

"$CARGO" test --release -p earthmesh_cli --lib real_merit_tiles_drive_coast_distance_across_the_antimeridian -- --ignored --nocapture
exec "$CARGO" test --release -p earthmesh_cli --test project_hydro_real_e2e -- --ignored --nocapture
