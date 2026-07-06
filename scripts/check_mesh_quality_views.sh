#!/usr/bin/env bash
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

CARGO=${CARGO:-cargo}
"$CARGO" test --manifest-path rust/earthmesh_cli/Cargo.toml --test mesh_quality_views -- --nocapture
