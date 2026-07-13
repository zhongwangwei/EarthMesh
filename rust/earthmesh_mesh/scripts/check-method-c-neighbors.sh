#!/usr/bin/env bash
set -euo pipefail

readonly SRC="rust/earthmesh_mesh/src"

printf "\n[Method-C neighbor coverage]\n"
rg -n "pub fn derive_icosahedron_m_neighbors_canonical\\(" \
  "$SRC/icosahedron_m_neighbors/mod.rs"
rg -n "fn (method_c_refinement_start_point_with_neighbors|method_c_march_from_nearby_pentagon_to_region_with_neighbors|method_c_thirdm_neighbors_canonical_with_neighbors|opposite_ring_u_edge_with_neighbors|mark_fill_rad3_faces_with_neighbors|selected_region_thirdm_seed_points_with_neighbors)" \
  "$SRC"

printf "\n[Potential bypass calls]\n"
if rg -n -P "\\.(method_c_refinement_start_point|method_c_march_from_nearby_pentagon_to_region|method_c_thirdm_neighbors_canonical|opposite_ring_u_edge|mark_fill_rad3_faces|selected_region_thirdm_seed_points)\\s*\\(" \
  "$SRC" rust/earthmesh_mesh/tests; then
  printf "\n[FAILED] found a call that bypasses explicit neighbor connectivity.\n"
  exit 1
fi

printf "[PASSED] Method-C selection paths use explicit neighbor connectivity.\n"
