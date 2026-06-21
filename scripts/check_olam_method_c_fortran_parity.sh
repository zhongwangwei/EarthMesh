#!/usr/bin/env bash
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

cd "$ROOT"

echo "== reduced Fortran Method-C golden checks =="
scripts/olam_reduced_fortran_probe.sh --check all

echo "== Rust Method-C accepted reduced-Fortran comparisons =="
cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_matches_reduced_fortran -- --nocapture

echo "== Rust Method-C rejected reduced-Fortran comparisons =="
cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib rejects_reduced_fortran -- --nocapture
