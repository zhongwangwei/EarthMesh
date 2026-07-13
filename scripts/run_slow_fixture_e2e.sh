#!/usr/bin/env bash
set -euo pipefail

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CARGO=${CARGO:-cargo}

feature_args=()
if [[ -n "${CLI_FEATURES:-}" ]]; then
    # CLI_FEATURES is supplied by the Makefile as ordinary cargo arguments.
    read -r -a feature_args <<< "$CLI_FEATURES"
fi

absolute_file() {
    local path=$1
    local directory
    directory=$(CDPATH= cd -- "$(dirname -- "$path")" && pwd -P)
    printf '%s/%s\n' "$directory" "$(basename -- "$path")"
}

landtype=${EARTHMESH_LANDTYPE:-"$ROOT/input/landtype_igbp_update.nc"}
if [[ ! -f "$landtype" ]]; then
    echo "required slow-test landtype file not found: $landtype" >&2
    echo "set EARTHMESH_LANDTYPE to a real global land-type NetCDF" >&2
    exit 2
fi
EARTHMESH_LANDTYPE=$(absolute_file "$landtype")

owned_fixture_root=0
fixture_root=${EARTHMESH_SLOW_FIXTURE_ROOT:-}
if [[ -z "${EARTHMESH_SLOW_GRIDFILE:-}" ]]; then
    if [[ -z "$fixture_root" ]]; then
        temp_base=${TMPDIR:-/tmp}
        fixture_root=$(mktemp -d "${temp_base%/}/earthmesh-slow-fixtures.XXXXXX")
        owned_fixture_root=1
    else
        mkdir -p "$fixture_root"
        fixture_root=$(CDPATH= cd -- "$fixture_root" && pwd -P)
    fi
fi

cleanup() {
    if [[ $owned_fixture_root -eq 1 ]]; then
        rm -rf "$fixture_root"
    fi
}
trap cleanup EXIT INT TERM

if [[ -n "${EARTHMESH_SLOW_GRIDFILE:-}" ]]; then
    if [[ ! -f "$EARTHMESH_SLOW_GRIDFILE" ]]; then
        echo "EARTHMESH_SLOW_GRIDFILE is missing: $EARTHMESH_SLOW_GRIDFILE" >&2
        exit 2
    fi
    EARTHMESH_SLOW_GRIDFILE=$(absolute_file "$EARTHMESH_SLOW_GRIDFILE")
else
    quickstart_namelist="$fixture_root/quickstart_n16.nml"
    sed "s|NL%base_dir = './cases/'|NL%base_dir = './'|" \
        "$ROOT/examples/00_quickstart_n16.nml" > "$quickstart_namelist"
    (
        cd "$fixture_root"
        "$CARGO" run ${feature_args[@]+"${feature_args[@]}"} --quiet \
            --manifest-path "$ROOT/rust/earthmesh_cli/Cargo.toml" \
            -- "$quickstart_namelist" --quiet
    )
    EARTHMESH_SLOW_GRIDFILE="$fixture_root/quickstart_n16/gridfile/gridfile_NXP0016_01_hex.nc4"
    if [[ ! -f "$EARTHMESH_SLOW_GRIDFILE" ]]; then
        echo "slow-test NXP16 generation did not produce $EARTHMESH_SLOW_GRIDFILE" >&2
        exit 1
    fi
fi

export EARTHMESH_LANDTYPE EARTHMESH_SLOW_GRIDFILE
echo "slow_fixture_gridfile=$EARTHMESH_SLOW_GRIDFILE"
echo "slow_fixture_landtype=$EARTHMESH_LANDTYPE"

cd "$ROOT"
"$CARGO" test ${feature_args[@]+"${feature_args[@]}"} \
    --manifest-path rust/earthmesh_cli/Cargo.toml \
    --test colm_coupling_csv_from_mesh -- \
    --ignored --test-threads=1
"$CARGO" test ${feature_args[@]+"${feature_args[@]}"} \
    --manifest-path rust/earthmesh_cli/Cargo.toml \
    --test hydro_workflow \
    full_chain_with_mesh_landtype_coupling_quality -- --ignored
"$CARGO" test ${feature_args[@]+"${feature_args[@]}"} \
    --manifest-path rust/earthmesh_cli/Cargo.toml \
    --test refine_end_to_end_topology \
    specified_bbox_refine_produces_consistent_closed_mpas -- --ignored
