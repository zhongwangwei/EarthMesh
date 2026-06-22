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

echo "== full M/U/W table dumps: reduced Fortran vs Rust =="
tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/olam-method-c-parity.XXXXXX")
trap 'rm -rf "$tmpdir"' EXIT
accepted_cases="nxp6_circle nxp7_circle nxp6_corridor nxp7_corridor nxp6_variable_corridor nxp6_three_point_corridor nxp6_two_circle nxp7_two_circle nxp6_two_corridor nxp7_two_corridor"
for case_name in $accepted_cases; do
  scripts/olam_reduced_fortran_probe.sh --dump-tables "$case_name" \
    | awk '/^counts nmd=|^[MUW] / { print }' > "$tmpdir/fortran-$case_name.tables"
  cargo run --quiet --manifest-path rust/earthmesh_mesh/Cargo.toml --example olam_method_c_probe -- tables "$case_name" \
    > "$tmpdir/rust-$case_name.tables"
  diff -u "$tmpdir/fortran-$case_name.tables" "$tmpdir/rust-$case_name.tables" \
    > "$tmpdir/diff-$case_name.tables"
  echo "table diff ok $case_name"
done

echo "== real spring_dynamics global numeric probe: Fortran vs Rust =="
scripts/olam_reduced_fortran_probe.sh --spring nxp6_circle \
  | awk '/^spring / { print }' > "$tmpdir/fortran-nxp6.spring"
cargo run --quiet --manifest-path rust/earthmesh_mesh/Cargo.toml --example olam_method_c_probe -- spring nxp6_circle \
  > "$tmpdir/rust-nxp6.spring"
awk '
  /^spring M/ {
    split($4, x, "="); split($5, y, "="); split($6, z, "=");
    printf "spring M %s x=%.0f y=%.0f z=%.0f\n", $3, x[2], y[2], z[2];
    next
  }
  { print }
' "$tmpdir/fortran-nxp6.spring" > "$tmpdir/fortran-nxp6.spring.rounded"
awk '
  /^spring M/ {
    split($4, x, "="); split($5, y, "="); split($6, z, "=");
    printf "spring M %s x=%.0f y=%.0f z=%.0f\n", $3, x[2], y[2], z[2];
    next
  }
  { print }
' "$tmpdir/rust-nxp6.spring" > "$tmpdir/rust-nxp6.spring.rounded"
diff -u "$tmpdir/fortran-nxp6.spring.rounded" "$tmpdir/rust-nxp6.spring.rounded" > "$tmpdir/diff-nxp6.spring"
echo "spring rounded-meter diff ok nxp6_circle"
