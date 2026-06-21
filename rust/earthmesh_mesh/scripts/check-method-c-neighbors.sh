#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob

readonly TARGET_FILES=(
  "rust/earthmesh_mesh/src/lib.rs"
)
readonly TEST_FILES=(
  rust/earthmesh_mesh/tests/*.rs
)

printf "\n[Method-C neighbor coverage]\n"
rg -n "derive_icosahedron_m_neighbors_fortran\\(" "rust/earthmesh_mesh/src/lib.rs"

printf "\n[Wrapper calls: _with_neighbors linkage]\n"
rg -n "fn olam_refinement_start_point\\(|fn olam_refinement_start_point_with_neighbors|fn olam_march_from_nearby_pentagon_to_region\\(|fn olam_march_from_nearby_pentagon_to_region_with_neighbors|fn olam_thirdm_neighbors_fortran\\(|fn olam_thirdm_neighbors_fortran_with_neighbors|fn opposite_ring_u_edge\\(|fn opposite_ring_u_edge_with_neighbors|fn mark_fill_rad3_faces\\(|fn mark_fill_rad3_faces_with_neighbors|fn close_olam_method_c_concavities\\(|fn close_olam_method_c_concavities_with_neighbors|fn selected_region_thirdm_seed_points\\(|fn selected_region_thirdm_seed_points_with_neighbors|fn spawn_nest_pass_method_c\\(|fn spawn_nest_pass_method_c" rust/earthmesh_mesh/src/lib.rs

printf "\n[Potential direct old-path calls]\n"
legacy_methods=(
  "olam_refinement_start_point"
  "olam_march_from_nearby_pentagon_to_region"
  "olam_thirdm_neighbors_fortran"
  "opposite_ring_u_edge"
  "mark_fill_rad3_faces"
  "selected_region_thirdm_seed_points"
)

violations=0
for file in "${TARGET_FILES[@]}" "${TEST_FILES[@]}"; do
  for method in "${legacy_methods[@]}"; do
    raw=$(rg -n -P "(self|[a-zA-Z0-9_]+)\\.${method}\\s*\\(" "$file" || true)
    filtered=$(printf "%s\n" "$raw" | rg -v "_with_neighbors|close_olam_selected_face_concavities|spawn_nest_pass_method_c\\(" || true)
    if [ -n "$filtered" ]; then
      printf "\n%s\n" "$file"
      printf "%s\n" "$filtered"
      violations=$((violations + 1))
    fi
  done
done

if [ "$violations" -ne 0 ]; then
  printf "\n[FAILED] found legacy direct calls above (should be none).\n"
  exit 1
fi
printf "[PASSED] no legacy direct calls found in Method-C paths.\n"

printf "\n[Done]\n"
