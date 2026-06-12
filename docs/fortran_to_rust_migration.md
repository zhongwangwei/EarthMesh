# Fortran-to-Rust Migration Plan

## Target

EarthMesh should reach a Rust-native v3/20XX core: Rust owns heavy mesh, geometry, refinement, mask, and adapter kernels; Python remains the orchestration and analysis layer where it is useful. The end state is no required Fortran build dependency for normal EarthMesh generation.

## Current scale

`src/*.F90` contains 13 files and about 43k lines. Roughly 28k lines are bundled BLAS/LAPACK (`blas.F90`, `lapack.F90`), so they should be replaced by maintained Rust/native math crates rather than hand-translated. The EarthMesh-specific Fortran surface is about 15k lines across configuration, I/O, area judging, containment, refinement target selection, grid preprocessing, mask postprocessing, icosahedron/spring logic, refinement, and `mkgrd` orchestration.

The machine-readable source inventory is `docs/fortran_to_rust_migration_manifest.json`. It is intentionally test-covered so new or removed Fortran files cannot silently drift from the migration plan.

## Migration phases

1. **Phase 0 — inventory and parity harness**
   - Track every `src/*.F90` file in the manifest.
   - Add fixture-based gates before behavior changes.
   - Preserve the existing Fortran executable as the oracle until Rust parity is proven.

2. **Phase 1 — external math and pure geometry**
   - Replace bundled BLAS/LAPACK with maintained providers.
   - Continue expanding `rust/earthmesh_geometry` for deterministic polygon and area kernels.

3. **Phase 2 — typed state and I/O boundaries**
   - Port `consts_coms.F90` into typed Rust configuration/state.
   - Wrap then port file preprocessing, NetCDF, text, and binary readers/writers.
   - Keep output format compatibility for CoLM2024, CoLM20XX, MPAS, FVCOM, and related adapters.

4. **Phase 3 — data/area/contain/refinement targets**
   - Port data preprocessing, area judging, containment, and `GetRef` target calculations.
   - Gate each with small structured/unstructured fixtures before touching the refinement loop.

5. **Phase 4 — grid/refine/mask kernels**
   - Port grid preprocessing, icosahedron/spring quality logic, refinement loop, and mask postprocessing.
   - Preserve explicit masks for every cell and maintain separate land/ocean/river/coast roles.

6. **Phase 5 — Rust CLI cutover**
   - Replace `mkgrd.x` orchestration with a Rust CLI and Python runtime bridge.
   - Keep v3 adapters shape-agnostic: triangle, quad, hex, river/channel cells, and coast refinement are cell roles/topology capabilities, not CoLM-specific assumptions.
   - Remove Fortran from the normal build only after end-to-end fixture parity passes.

## Adapter/service implications

- **CoLM2024 / CoLM20XX**: should consume role-aware mesh bundles and masks without being bound to triangle or hex internals.
- **MPAS**: requires stable polygon/dual-mesh ordering, neighbor connectivity, and mask postprocessing.
- **FVCOM**: requires triangle/unstructured compatibility and coastal/ocean boundary preservation.
- **Hydro/coast/coupling**: should stay as reusable capability layers: hydro provides river/channel/refinement roles, coast provides shoreline and land-ocean transition roles, coupling provides conservative overlap/containment and adapter export semantics.

## Completion definition

The migration is complete only when:

- Rust crates replace all EarthMesh-specific Fortran kernels or intentionally replace them through the v3 pipeline.
- BLAS/LAPACK are no longer vendored Fortran in the normal build.
- A Rust CLI/pipeline can run the same representative workflows as `mkgrd.x`.
- Smoke, Greater Bay Area, China/MERIT, and adapter fixture outputs pass documented parity tolerances.
- Python runtime uses Rust bindings for heavy kernels instead of Python fallbacks where performance matters.
- The Fortran build is optional or removed from the standard path.

## Current next milestone

The immediate milestone is not to delete Fortran. It is to lock a verified migration map and add parity fixtures module by module. The first gate is `tests/test_fortran_to_rust_migration_manifest.py`, which fails if any `src/*.F90` file is not explicitly tracked with a Rust target, phase, strategy, and parity gate.

## Progress notes

### 2026-06-12: `consts_coms` Rust core started

Added `rust/earthmesh_core` as the first Rust-native replacement surface for `src/consts_coms.F90`. The crate currently covers:

- `PIO180`, `PIU180`, and `PI2` formula parity.
- Degree/radian helper functions using the migrated constants.
- MPAS-compatible Earth radius initialization from `mkgrd.F90:init_consts`.
- Typed `EarthmeshConfig::default()` matching the Fortran `oname_vars` defaults.

Remaining `consts_coms.F90` work is intentionally explicit in the manifest: lon/lat mesh defaults, FVCOM mesh defaults, `mem_*` state modules, and namelist parser compatibility still need separate parity tests before this Fortran file can be considered fully replaced.

### 2026-06-12: lon/lat and FVCOM mesh defaults added

Extended `rust/earthmesh_core` with typed defaults for the co-located `lonlatmesh_coms` and `fvcommesh_coms` modules. These are now covered by Rust tests against the Fortran literal defaults. The remaining `consts_coms.F90` migration risk is no longer basic defaults; it is the larger `mem_*` mutable state layout and full `read_nl` namelist compatibility.

### 2026-06-12: first `mkgrd.F90` mesh formula ported

Added `rust/earthmesh_mesh` with a tested Rust port of `mkgrd.F90:grid_xyz2lonlat`. This establishes the first mesh-kernel crate depending on `earthmesh_core` rather than Fortran globals. The current Rust surface covers scalar and batch Earth-centered Cartesian to longitude/latitude conversion. Larger `mkgrd` orchestration remains unported.

### 2026-06-12: first `MOD_Area_judge.F90` geometry helpers ported

Extended `rust/earthmesh_geometry` with tested Rust ports of pure `MOD_Area_judge` helpers: `haversine`, `is_point_in_circle`, `cross_product`, and `is_point_in_convex_polygon`. These functions are used by area/mask classification and are a prerequisite for porting hydro/coast refinement decisions. The full `Area_judge` orchestration and NetCDF-backed mask reads remain in Fortran.

### 2026-06-12: `icosahedron.F90` polar stereographic helpers ported

Extended `rust/earthmesh_mesh` with tested Rust ports of `icosahedron.F90:de_ps_r8` and `ps_de_r8`. These helpers work on displacement vectors relative to the local pole/barycenter, matching how `mkgrd.F90:pcvt` and `MOD_grid_preprocess.F90` call the Fortran routines. Full icosahedron grid construction, connectivity, and spring dynamics remain unported.

### 2026-06-12: `MOD_Area_judge.F90` closed-curve helpers ported

Extended `rust/earthmesh_geometry` with Rust ports of closed-curve helper logic from `MOD_Area_judge`: scanline ray/segment intersection, strict segment intersection, component cross product, and dateline `CheckCrossing` longitude shifting. The Rust API maps the Fortran no-intersection longitude sentinel to `Option::None`, while tests preserve the same branch behavior. The full grid-row polygon fill and source-index boundary logic remain unported.

### 2026-06-12: `MOD_grid_preprocess.F90` lon/lat unit-vector conversion ported

Extended `rust/earthmesh_mesh` with a tested Rust port of `MOD_grid_preprocess:lonlat2xyz`. The function returns unit-sphere Cartesian coordinates, matching the Fortran routine before callers multiply by `erad8`. Round-trip tests use the already migrated Rust `xyz_to_lonlat_degrees` path. Most grid preprocessing remains unported.

### 2026-06-12: `MOD_grid_preprocess.F90` spherical centroid helper ported

Extended `rust/earthmesh_mesh` with `spherical_centroid_degrees`, a tested Rust port of `MOD_grid_preprocess:centroid_spherical_single`. It preserves the Fortran method: lon/lat vertices are converted to unit Cartesian vectors, averaged component-wise, then converted back to lon/lat. The grid-wide `centroid_spherical_calculation` wrapper is now covered by `centroid_spherical_mesh_fortran_indexed`, preserving the Fortran triangle-id loop from `2..sjx_points` and zero-initialized unwritten slots.

### 2026-06-12: `MOD_grid_preprocess.F90` longitude normalization and arc length ported

Extended `rust/earthmesh_mesh` with tested ports of `CheckLon` and `arc_length`. `CheckLon` preserves the Fortran single-step +/-360 behavior rather than full modulo wrapping. `arc_length_unit_sphere` preserves the MPAS-compatible mixed precision behavior in the Fortran implementation by squaring the half-angle sine terms as `f32` before converting back to `f64`.

### 2026-06-12: `MOD_grid_preprocess.F90` polygon length/angle helper ported

Extended `rust/earthmesh_mesh` with `polygon_length_angle_metrics`, a tested Rust port of `Get_Length_Angle`. It preserves the Fortran cyclic `(previous, current, next)` triplet, spherical half-angle formula, and Earth-radius scaling of `length(1)`. The helper now gives `TriMeshQuality` and `PolyMeshQuality` a Rust-native numerical kernel to build on.

### 2026-06-12: `MOD_grid_preprocess.F90` triangle quality aggregation core ported

Extended `rust/earthmesh_mesh` with `triangle_mesh_quality`, a tested Rust port of the aggregation core in `TriMeshQuality`. It reuses the Rust `polygon_length_angle_metrics` kernel, preserves the Fortran 45/75 degree triangle thresholds, average min/max accumulation, and RMS angle deviation from 60 degrees. The Fortran-style cached array/update wrapper around `adjust_sjx_flag` remains unported.

### 2026-06-12: `MOD_grid_preprocess.F90` polygon quality aggregation core ported

Extended `rust/earthmesh_mesh` with `polygon_mesh_quality`, a tested Rust port of the aggregation core in `PolyMeshQuality`. It preserves the Fortran regular-angle formula `(num_edges - 2) * 180 / num_edges`, 0.9/1.1 threshold bands, RMS angle deviation, and average min/max accumulation. The Fortran-style filtering by `n_ngrwm_f` and cached update arrays remain separate wrapper work.

### 2026-06-12: `MOD_grid_preprocess.F90` robust spherical area helper ported

Extended `rust/earthmesh_mesh` with `robust_spherical_area_unit`, a tested Rust port of `robust_spherical_area`. It preserves the Fortran dateline-aware longitude delta adjustment and signed unit-sphere area result. Physical area scaling by radius squared remains caller responsibility, matching how this helper should feed future `GetArea` migration.

### 2026-06-12: `MOD_grid_preprocess.F90` spherical triangle area helper ported

Extended `rust/earthmesh_mesh` with `spherical_triangle_area_unit`, a tested Rust port of `triangle_signed_area_sphere`. The helper preserves the Fortran l'Huilier spherical excess formula and deliberately reuses the mixed-precision `arc_length` port, so it can become the area primitive for the future `GetArea` kite, triangle, and cell-area workflow. The full `GetArea` connectivity loop and MPAS kite-area reconstruction remain unported.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetArea` primitives split out

Extended `rust/earthmesh_mesh` with two tested Rust primitives from `GetArea`: `spherical_kite_area_unit` for the MPAS two-triangle kite area and `spherical_cell_area_from_vertices_unit` for the cell fan triangulation over `verticesOnCell`. These reuse the migrated spherical triangle area helper. Remaining `GetArea` work is now the connectivity-driven assignment of `kiteAreasOnVertex`, reconstruction of `areaTriangle`, and fixture parity against MPAS-style mesh arrays.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetArea` connectivity helpers ported

Extended `rust/earthmesh_mesh` with `shared_cell_for_edge_pair` and `vertex_cell_position`, matching the pure lookup logic inside `GetArea`. The Rust helpers preserve the Fortran behavior of checking all four `cellsOnEdge` combinations, ignoring zero as the no-cell sentinel, and scanning the three `cellsOnVertex` slots in order. Remaining `GetArea` work is the full array workflow that loops over vertices/edges and writes `kiteAreasOnVertex`, `areaTriangle`, and `areaCell`.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetArea` unit workflow ported

Added `get_area_unit_fortran_indexed`, the first Rust array-level port of `GetArea`. It keeps the Fortran ID convention with indices `0`/`1` unused or skipped, computes `kiteAreasOnVertex` from consecutive edge pairs, reconstructs `areaTriangle` as the kite sum, and fan-triangulates `areaCell`. The current test uses a compact Fortran-indexed synthetic mesh; remaining work is a production wrapper against real MPAS/EarthMesh arrays plus parity fixtures for reconstruction error reporting.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetArea` reconstruction error summary ported

Added `area_triangle_reconstruction_error_fortran_indexed`, matching the `GetArea` diagnostic that recomputes triangle area from `cellsOnVertex` cell centers and reports max/average relative reconstruction error. This closes another pure numeric part of `GetArea`; remaining work is production fixture parity and integration with real mesh arrays.

### 2026-06-12: `MOD_grid_preprocess.F90` `IsNgrmm` and `GetEdge` mapping helpers ported

Added `is_ngrmm`, a Rust port of the Fortran neighbor classifier that returns which triangle vertex is opposite the shared edge, and `cells_on_edge_from_neighbor_cells`, the `GetEdge` mapping that derives sorted `cellsOnEdge` pairs from neighboring triangle cell ids. The remaining `GetEdge` migration is the stateful edge-id reuse loop, optional spherical midpoint calculation, and edge/vertex sorting routines.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetSort_verticesOnEdge` swap predicate ported

Added `should_swap_vertices_on_edge`, a Rust port of the 2-D cross-product rule used by `GetSort_verticesOnEdge` to decide whether `verticesOnEdge(1:2, i)` should be swapped. The helper preserves the Fortran single-step dateline adjustment for longitude differences. Remaining edge work is the full edge-id workflow, optional midpoint generation, and broader ordering routines.

### 2026-06-12: `MOD_grid_preprocess.F90` `normalizeRotation` helper ported

Added `normalize_vertex_rotation`, a Rust port of the per-vertex rotation rule from `normalizeRotation`. It moves the minimum positive `cellsOnVertex` id to the first slot and rotates `edgesOnVertex` in lockstep, preserving the Fortran behavior for zero sentinels. Remaining ordering work is the full `orderVertexArrays` 3-D sorting logic and integration into the edge workflow.

### 2026-06-12: `MOD_grid_preprocess.F90` `orderVertexArrays` CCW selector ported

Added `next_ccw_edge_candidate_slot`, a Rust port of the inner `orderVertexArrays` rule that picks the next counter-clockwise edge candidate with the smallest positive angle around the vertex normal. This isolates the 3-D cross-product/dot-product geometry before porting the full stateful edge-slot swapping and `cellsOnVertex` rebuild workflow.

### 2026-06-12: `MOD_grid_preprocess.F90` per-vertex `orderVertexArrays` workflow ported

Added `order_vertex_arrays_for_vertex`, a Rust port of the per-vertex `orderVertexArrays` mutation/rebuild workflow. It uses the migrated CCW candidate selector to swap `edgesOnVertex` into order, then rebuilds `cellsOnVertex` from `verticesOnEdge` and `cellsOnEdge` using the same side-of-edge rule as Fortran. Remaining work is an array-level wrapper over all vertices and real mesh fixture parity.

### 2026-06-12: `MOD_grid_preprocess.F90` array-level `orderVertexArrays` wrapper ported

Added `order_vertex_arrays_fortran_indexed`, a Rust array-level wrapper over the per-vertex ordering workflow. It preserves the Fortran indexing convention by skipping indices `0` and `1`, returns ordered `edgesOnVertex`, and rebuilds `cellsOnVertex` for every migrated vertex. Remaining work is real mesh fixture parity and integration with the broader `GetEdge` workflow.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetSort_verticesOnEdge` wrapper ported

Added `order_vertices_on_edge_fortran_indexed`, a Rust port of the array-level `GetSort_verticesOnEdge` loop. It preserves Fortran edge ids by starting at index `2`, uses the migrated cross-product predicate to decide swaps, and returns a sorted `verticesOnEdge` copy. Remaining edge work is full `GetEdge` edge-id construction, optional midpoint generation, and real mesh parity fixtures for the combined edge-ordering path.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetEdge` core connectivity workflow ported

Added `get_edge_connectivity_fortran_indexed`, a Rust port of the core `GetEdge` loop that creates/reuses edge ids, writes `edgesOnVertex`, `verticesOnEdge`, and derives sorted `cellsOnEdge` pairs from neighboring triangle cell ids. The optional spherical midpoint output and real mesh combined parity fixtures remain separate work.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetEdge` optional midpoint output ported

Added `edge_midpoints_from_cells_fortran_indexed`, a Rust port of the optional `vp` branch in `GetEdge`. It computes each edge point from the spherical centroid of the two neighboring polygon cell centers, preserving Fortran edge ids by starting at index `2`. Remaining `GetEdge` work is combined real-mesh fixture parity and integration into a production Rust replacement path.

### 2026-06-12: `MOD_grid_preprocess.F90` centroid batch wrapper ported

Extended `rust/earthmesh_mesh` with `centroid_spherical_mesh_fortran_indexed`, a tested Rust wrapper for `MOD_grid_preprocess:centroid_spherical_calculation`. It resolves three cell-center lon/lat references per triangle, starts at Fortran triangle id `2`, leaves slots `0` and `1` initialized to `(0, 0)`, and rejects out-of-range connectivity before a future Python/Rust runtime boundary can silently propagate bad mesh topology.

### 2026-06-12: `MOD_grid_preprocess.F90` spherical circumcenter workflow ported

Extended `rust/earthmesh_mesh` with `spherical_circumcenter_from_barycenter` and `circumcenter_spherical_mesh_fortran_indexed`, tested Rust ports of `MOD_grid_preprocess:circumcenter_spherical_calculation`. The implementation uses the migrated polar stereographic helpers, preserves the Fortran algebraic 2-D circumcenter solve, updates only triangle ids from `2` onward, preserves unvisited inout slots, validates vertex connectivity, and renormalizes results to the MPAS Earth radius.

### 2026-06-12: `MOD_grid_preprocess.F90` fractional index and distance-layer helpers ported

Extended `rust/earthmesh_mesh` with `find_frac_index_fortran` and `distance_layers`, tested Rust ports of `find_frac_index` and `dist_layers_make`. The fractional-index helper preserves the Fortran 1-based interval index and clamped fraction for ascending longitude and descending latitude grids, while returning `None` for out-of-bounds or zero-width cells. The layer helper covers the four `set_dis_type` formulas (`linear`, `nonlinear1`, `nonlinear2`, `nonlinear3`) as explicit Rust enum variants. Mesh-wide `distsOnEdge_layers_make` and `cellwidth_layers_make` application logic remains unported.

### 2026-06-12: `MOD_grid_preprocess.F90` `TriMeshQuality` cache wrapper ported

Extended `rust/earthmesh_mesh` with `triangle_mesh_quality_fortran_indexed`, a cache-aware Rust wrapper for `MOD_grid_preprocess:TriMeshQuality`. It preserves Fortran-indexed triangle ids from `2` onward, recomputes only adjusted triangles, reuses cached length/angle arrays for unchanged triangles, updates angle threshold flags, and reproduces the extreme/average/stddev aggregation over active triangles. `PolyMeshQuality` cache/update behavior remains separate because it filters cells by polygon edge count.

### 2026-06-12: `MOD_grid_preprocess.F90` `PolyMeshQuality` compact-cache wrapper ported

Extended `rust/earthmesh_mesh` with `polygon_mesh_quality_fortran_indexed`, a cache-aware Rust wrapper for `MOD_grid_preprocess:PolyMeshQuality`. It preserves the Fortran loop over cell ids from `2`, filters by `n_ngrwm == num_edges`, uses compact `j`-indexed length/angle caches for only matching polygons, recomputes adjusted cells, reuses unchanged caches, and reproduces regular-angle threshold flags plus aggregate min/max/stddev metrics.

### 2026-06-12: `MOD_grid_preprocess.F90` layer application wrappers ported

Extended `rust/earthmesh_mesh` with `dists_on_edge_layers_fortran_indexed` and `cellwidth_layers_fortran_indexed`, tested Rust ports of the core update rules in `distsOnEdge_layers_make` and `cellwidth_layers_make`. They preserve the Fortran-indexed triangle/cell/edge ids, inner refined-region halving, optional inward `num_rc` stripping, boundary-cell detection from `ngrmw`/`ngrwm`, first halo expansion, and per-layer transition values. The broader `set_distsOnEdge_global` orchestration remains part of the Springjustment/global workflow work.

### 2026-06-12: `MOD_grid_preprocess.F90` cell ordering/connectivity helpers ported

Extended `rust/earthmesh_mesh` with `order_vertices_on_cell_fortran_indexed`, `standardize_vertices_on_cell_rotation_fortran_indexed`, and `connect_on_cell_fortran_indexed`, tested Rust ports of `orderVerticesOnCell`, `standardizeVerticesOnCellRotation`, and `Get_ConnectOnCell`. These preserve Fortran-indexed cell ids, MPAS-style CCW vertex selection by positive `cross(vec1, vec2) · normal`, minimum-vertex rotation standardization, and reconstruction of `edgesOnCell`/`cellsOnCell` from ordered `verticesOnCell`. Remaining MPAS postprocess geometry includes `edgeIDSort`, `Get_Edge_DIS_Angle`, and `set_weightsOnEdge`.

### 2026-06-12: `MOD_grid_preprocess.F90` edge distance/angle metrics ported

Extended `rust/earthmesh_mesh` with `plane_angle_signed` and `edge_distance_angle_fortran_indexed`, tested Rust ports of `planeAngle` and `Get_Edge_DIS_Angle`. The wrapper preserves Fortran-indexed edge ids from `2`, computes `dvEdge` from edge vertices, `dcEdge` from adjacent cell centers, applies the latitude-difference angle formula, signs the angle with the MPAS plane-angle normal rule, and wraps into `[-pi, pi]`. Remaining MPAS postprocess work is now concentrated in `edgeIDSort` and `set_weightsOnEdge`.

### 2026-06-12: `MOD_grid_preprocess.F90` `edgeIDSort` ported

Extended `rust/earthmesh_mesh` with `edge_id_sort_fortran_indexed`, a tested Rust port of `MOD_grid_preprocess:edgeIDSort`. It reorders current `cellsOnEdge`, `verticesOnEdge`, and edge point coordinates to match a reference `cellsOnEdge` ordering, then rebuilds `edgesOnVertex` from the sorted vertex-edge pairs while preserving Fortran-indexed edge ids from `2`. Remaining MPAS postprocess work is concentrated in `set_weightsOnEdge`.
