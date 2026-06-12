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

### 2026-06-12: `MOD_grid_preprocess.F90` `set_weightsOnEdge` ported

Extended `rust/earthmesh_mesh` with `set_weights_on_edge_fortran_indexed`, a tested Rust port of `MOD_grid_preprocess:set_weightsOnEdge`. The wrapper preserves Fortran-indexed edge/cell/vertex ids, computes `RivCell` from `kiteAreasOnVertex / areaCell`, uses the same two-sided edge stencil traversal, Kahan compensated cumulative sums, `dvEdge/dcEdge` scaling, inflow sign adjustment from `cellsOnEdge`, and zonal-flow reconstruction `error_segment`. Compact per-edge vectors replace the fixed `maxEdges2 x num_edge` Fortran storage.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetArea` production wrapper ported

Extended `rust/earthmesh_mesh` with `get_area_production_fortran_indexed`, a tested production-facing wrapper around the migrated `GetArea` unit workflow. It returns the `kiteAreasOnVertex`, `areaTriangle`, and `areaCell` arrays together with the reconstruction relative-error summary previously only printed by the Fortran routine. Real-mesh MPAS parity fixtures remain separate integration work.

### 2026-06-12: `MOD_grid_preprocess.F90` `GetEdge` production wrapper ported

Extended `rust/earthmesh_mesh` with `get_edge_production_fortran_indexed`, a tested production-facing wrapper that composes the migrated `GetEdge` connectivity workflow, `GetSort_verticesOnEdge`, optional `vp` midpoint generation, and `orderVertexArrays`. This provides a single Rust replacement path for sorted `cellsOnEdge`, `verticesOnEdge`, `edgesOnVertex`, `cellsOnVertex`, and edge midpoint coordinates; real MPAS mesh parity fixtures and adapter wiring remain separate integration gates.

### 2026-06-12: `MOD_grid_preprocess.F90` `Grid_Quality_Check_Global` calculation orchestration ported

Extended `rust/earthmesh_mesh` with `grid_quality_check_global_fortran_indexed`, a tested Rust wrapper for the calculation side of `Grid_Quality_Check_Global`. It counts 5/6/7-sided polygon classes, initializes all adjust flags and zero caches like the Fortran startup path, and composes the migrated `TriMeshQuality` plus 5/6/7-sided `PolyMeshQuality` wrappers. The NetCDF `quality_save_global` write remains an adapter/output-layer task.

### 2026-06-12: `MOD_grid_preprocess.F90` Springjustment topology helpers ported

Extended `rust/earthmesh_mesh` with `triangle_neighbors_from_cell_membership_fortran_indexed`, a tested Rust port of `set_ngrmm`, and `edges_on_edge_tri_fortran_indexed`, a tested Rust port of `set_edgesOnEdge_tri`. These cover the pure topology preparation used by `Springjustment_global` and regional spring workflows before the remaining spring-dynamics and NetCDF-backed orchestration layers.

### 2026-06-12: `MOD_grid_preprocess.F90` `set_distsOnEdge_global` calculation orchestration ported

Extended `rust/earthmesh_mesh` with `set_dists_on_edge_global_fortran_indexed`, a tested pure Rust orchestration wrapper for the calculation side of `set_distsOnEdge_global`. It initializes background `distsOnEdge`/optional `cellwidth`, preserves the active-iteration skip semantics through explicit step inputs, halves the selected scale each active iteration, builds transition layers with the migrated `dist_layers_make`, and composes the migrated edge/cellwidth layer update kernels. `refine_sjx_regional_make` flag generation and NetCDF save wiring remain outside this core calculation wrapper.

### 2026-06-12: `MOD_grid_preprocess.F90` spring edge adjustment formula ported

Extended `rust/earthmesh_mesh` with `spring_edge_adjustment_fortran`, a tested Rust port of the per-edge correction formula inside `spring_dynamics_global`. It computes the current edge vector length, `twocosphi3/twocosphi4` ratio clamp, target distance, fractional change, displacement vector, and squared fractional change. The full global/regional spring iteration loops and spherical renormalization orchestration remain pending.

### 2026-06-12: `MOD_grid_preprocess.F90` spring edge direction signs ported

Extended `rust/earthmesh_mesh` with `spring_edge_directions_fortran_indexed`, a tested Rust port of the `dirs(j, iw)` setup inside `spring_dynamics_global`. It preserves the Fortran sign rule: `+relax` when the cell is the second endpoint in `CellsOnEdge`, otherwise `-relax`, using compact `edgesOnCell` rows. The remaining spring work is the iterative accumulation loop, spherical renormalization, regional move-mask behavior, and high-level workflow wiring.

### 2026-06-12: `MOD_grid_preprocess.F90` spring cell displacement application ported

Extended `rust/earthmesh_mesh` with `spring_apply_cell_displacements_fortran_indexed`, a tested Rust port of the cell-side accumulation and spherical renormalization steps inside `spring_dynamics_global`. It applies per-edge displacement vectors with the migrated compact direction rows, then scales each updated cell coordinate back to the requested Earth radius. The remaining spring work is assembling the full multi-iteration global/regional dynamics loop and regional move-mask behavior.

### 2026-06-12: `MOD_grid_preprocess.F90` global spring single-iteration wrapper ported

Extended `rust/earthmesh_mesh` with `spring_global_iteration_fortran_indexed`, a tested Rust wrapper for one calculation iteration of `spring_dynamics_global`. It computes current edge distances, applies the migrated per-edge correction formula through `EdgesOnedge_tri`, builds the migrated direction signs, applies per-cell displacement accumulation, and renormalizes coordinates to the requested Earth radius. The remaining spring work is multi-iteration convergence orchestration plus regional move-mask behavior.

### 2026-06-12: `MOD_grid_preprocess.F90` global spring multi-iteration wrapper ported

Extended `rust/earthmesh_mesh` with `spring_dynamics_global_fortran_indexed`, a tested Rust wrapper for the multi-iteration core of `spring_dynamics_global`. It repeatedly applies the migrated single-iteration edge-distance, edge-correction, direction-sign, cell-displacement, and spherical-renormalization sequence while retaining the Fortran-style periodic `Max DS` diagnostic snapshots without storing every full-mesh iteration. Remaining spring work is the regional move-mask smoother plus `Springjustment_global`/`Springjustment_regional_step` high-level adapter wiring.

### 2026-06-12: `MOD_grid_preprocess.F90` regional spring move-mask smoother ported

Extended `rust/earthmesh_mesh` with `spring_dynamics_regional_fortran_indexed`, a tested Rust port of the core `spring_dynamics_regionalv2` move-mask smoother. It builds the calculation mask from movable cells plus their neighbor halo, updates only movable cells by previous-iteration neighbor averaging, renormalizes to the requested Earth radius, and records Fortran-style periodic `Max DS` diagnostics. Remaining spring work is now the high-level `Springjustment_global`/`Springjustment_regional_step` adapter wiring and NetCDF/file workflow integration.

### 2026-06-12: `MOD_grid_preprocess.F90` Springjustment_global pure core adapter ported

Extended `rust/earthmesh_mesh` with `springjustment_global_core_fortran_indexed`, a tested pure in-memory adapter for the calculation sequence inside `Springjustment_global`. It wires the migrated triangle-neighbor, GetEdge production, ConnectOnCell, EdgesOnedge_tri, global spring dynamics, cell lon/lat refresh, triangle centroid/circumcenter refresh, and final orderVertexArrays kernels without introducing NetCDF/file side effects. Remaining high-level work is distance-step/file adapter integration and `Springjustment_regional_step` orchestration.

### 2026-06-12: `MOD_grid_preprocess.F90` Springjustment_global distance-step wiring ported

Extended `springjustment_global_core_fortran_indexed` so the pure adapter now invokes the migrated `set_dists_on_edge_global_fortran_indexed` distance-layer workflow after GetEdge/connectivity construction and before global spring dynamics. The adapter now carries updated `dists_on_edge` and optional `cellwidth` outputs, matching the in-memory part of `Springjustment_global` while keeping NetCDF persistence outside the core boundary. Remaining Springjustment work is NetCDF/file adapter integration and `Springjustment_regional_step` high-level orchestration.

### 2026-06-12: `MOD_grid_preprocess.F90` Springjustment_regional_step pure core adapter ported

Extended `rust/earthmesh_mesh` with `springjustment_regional_core_fortran_indexed`, a tested pure in-memory adapter for the calculation sequence inside `Springjustment_regional_step`. It accepts the regional move mask explicitly, wires triangle-neighbor and GetEdge connectivity, rebuilds cellsOnCell/edgesOnCell, applies the migrated `spring_dynamics_regionalv2` smoother, refreshes cell lon/lat, and recomputes triangle centroid/circumcenter coordinates. Remaining Springjustment work is the move-mask derivation and NetCDF/file adapter boundary.

### 2026-06-12: `MOD_grid_preprocess.F90` set_dbxMove_regional_step mask derivation ported

Extended `rust/earthmesh_mesh` with `set_dbx_move_regional_step_fortran_indexed`, a tested pure Rust port of the move-mask derivation core in `set_dbxMove_regional_step`. It accepts explicit initial refined-triangle flags, expands the regional halo through mixed boundary cells, marks refined-triangle cells movable, freezes boundary cells, and applies the optional protected seed-cell neighborhood removal that corresponds to the original 12-vertex protection logic. Remaining Springjustment work is now adapter plumbing for NetCDF/file persistence and upstream refine-state sources.

### 2026-06-12: `MOD_grid_preprocess.F90` Springjustment_regional_step refinement-mask composition ported

Extended `rust/earthmesh_mesh` with `springjustment_regional_from_refinement_fortran_indexed`, a tested pure in-memory adapter that composes the migrated `set_dbxMove_regional_step` mask derivation with `springjustment_regional_core_fortran_indexed`. The new boundary accepts already-classified refined-triangle flags, derives the Fortran-style movable/boundary/protected masks, and then runs the regional spring/circumcenter refresh pipeline. Remaining Springjustment work is now NetCDF/file persistence plus the original upstream `refine_sjx_regional_make` source-classification adapter.

### 2026-06-12: `MOD_grid_preprocess.F90` refine_sjx_regional_make source-mask classifier ported

Extended `rust/earthmesh_mesh` with `refine_sjx_regional_make_fortran_indexed`, a tested pure Rust classifier for the non-file portion of `refine_sjx_regional_make`. The kernel accepts triangle-center lon/lat coordinates, source lon/lat vertex arrays, and an already-loaded mask_patch grid, mirrors the Fortran `Source_Find` boundary lookup plus `max(1, source - 1)` cell shift, and returns refined-triangle flags from the configured `num_mp_step(iter)` start index. Remaining Springjustment work is now NetCDF/file loading and persistence around the migrated kernels.

### 2026-06-12: `MOD_grid_preprocess.F90` Springjustment_regional_step source-mask composition ported

Extended `rust/earthmesh_mesh` with `springjustment_regional_from_source_mask_fortran_indexed`, a tested pure in-memory adapter for the no-`num_sjx_ref` branch of `Springjustment_regional_step`. It composes source-mask triangle classification, regional move-mask derivation, and the migrated regional spring/circumcenter refresh core while keeping NetCDF mask loading and output persistence outside the deterministic Rust kernel boundary. Remaining Springjustment work is the actual NetCDF adapter layer around these kernels.

### 2026-06-12: `MOD_grid_preprocess.F90` GetArea real MPAS fixture parity added

Added `rust/earthmesh_mesh/tests/mpas_real_fixture.rs`, a compact fixture extracted from `cases/ATMOS_hex_N64_refine2_global_LOM67_251027/result/MPASOUT_NXP0064_global.nc4`. The test validates the migrated `get_area_unit_fortran_indexed` kernel against real MPAS `areaCell` and `areaTriangle` values using x/y/z coordinates and compact reindexed connectivity, without adding a Rust NetCDF dependency. Remaining GetArea work is the NetCDF adapter boundary that loads/persists full production arrays.

### 2026-06-12: `MOD_grid_preprocess.F90` GetEdge real MPAS fixture parity added

Extended the compact MPAS fixture test with a real vertex-ring connectivity case from `MPASOUT_NXP0064_global.nc4`. The test validates `get_edge_connectivity_fortran_indexed` against MPAS `verticesOnEdge`, `edgesOnVertex`, and unordered `cellsOnEdge` topology for the sampled ring, documenting that cell-pair order is an orientation detail handled by later sorting rather than a topology mismatch. Remaining GetEdge work is full NetCDF adapter integration.

### 2026-06-12: `icosahedron.F90` initial grid counts and point coordinates ported

Extended `rust/earthmesh_mesh` with `icosahedron_counts_fortran`, `icosahedron_diamond_corners_fortran`, and `icosahedron_initial_grid_fortran`, covering the point-coordinate portion of `icosahedron(nxp0)` before `fill_diamond`, `tri_neighbors`, and `spring_dynamics1`. The Rust kernel preserves Fortran-indexed `nmd/nud/nwd` sizing, 12 `impent` pentagonal point indices, the 10 big-diamond corner coordinate formulas, the `pwrd=0.9` interpolation weights, and Earth-radius projection for active M points. Remaining icosahedron work is connectivity table construction, loop flags, neighbor derivation, and spring relaxation.

### 2026-06-12: `icosahedron.F90` fill_diamond connectivity seed ported

Extended `rust/earthmesh_mesh` with `icosahedron_fill_diamonds_fortran`, plus compact `IcosahedronUEdge` and `IcosahedronWFace` table structs for the fields written directly by `fill_diamond`. The new kernel preserves Fortran-indexed U/W allocation slots, southern/northern diamond neighbor index formulas, and the initial `im`, `iw`, `iu`, `mrlu`, `mrlw`, `mrlw_orig`, and `ngr` assignments before `tri_neighbors` completes reciprocal connectivity. Remaining icosahedron work is `tri_neighbors`, loop flags, spring relaxation, and post-relaxation NXP parity.

### 2026-06-12: `icosahedron.F90` mdloopf/udloopf/wdloopf loop flags ported

Extended `rust/earthmesh_mesh` with `apply_icosahedron_loop_flags_fortran`, a shared tested Rust port of the identical loop-flag semantics in `mdloopf`, `udloopf`, and `wdloopf`. The function preserves `mloops=7`, `init='f'` reset behavior, positive id enable, negative id disable, zero-id ignore, and Fortran 1-based loop ids. Remaining icosahedron work is reciprocal connectivity derivation, spring relaxation, single-precision wrapper decision, and post-relaxation NXP parity.

### 2026-06-12: `icosahedron.F90` single-precision de_ps/ps_de wrappers ported

Extended `rust/earthmesh_mesh` with explicit f32-compatible `CartesianPointF32`, `PlanePointF32`, `PoleBasisF32`, `project_to_polar_stereographic_f32`, and `unproject_from_polar_stereographic_f32` APIs for the Fortran `real` `de_ps`/`ps_de` branches. The existing f64 APIs remain the `*_r8` equivalents; the single-precision wrappers intentionally use f32 arithmetic and document the precision boundary separately from the r8 path. Remaining icosahedron work is reciprocal connectivity derivation, spring relaxation, and post-relaxation NXP parity.

### 2026-06-12: `icosahedron.F90` `tri_neighbors` W-face neighbor derivation ported

Extended `rust/earthmesh_mesh` with `derive_icosahedron_w_neighbors_fortran`, a tested Rust port of the W-face portions of `tri_neighbors`. It fills each active W face's polygon count, surrounding M points, three inner W neighbors, and six outer W neighbors while preserving the Fortran 1-based table convention and overwrite order. Remaining `tri_neighbors` work is U-edge reciprocal connectivity and M-point polygon assembly.

### 2026-06-12: `icosahedron.F90` `tri_neighbors` U-edge neighbor derivation ported

Extended `rust/earthmesh_mesh` with `derive_icosahedron_u_neighbors_fortran`, a tested Rust port of the U-edge portion of `tri_neighbors`. It fills each active U edge's refinement level, four adjacent U edges, four surrounding W faces, and eight second-ring U neighbors from the W-face connectivity tables while preserving the Fortran branch order. Remaining `tri_neighbors` work is M-point polygon assembly.

### 2026-06-12: `icosahedron.F90` `tri_neighbors` M-point polygon assembly ported

Extended `rust/earthmesh_mesh` with `derive_icosahedron_m_neighbors_fortran`, a tested Rust port of the final M-point polygon assembly loop in `tri_neighbors`. The kernel returns Fortran-indexed M-point neighbor tables, follows the U-edge ring walk, preserves wall-boundary termination behavior, and converts the original `npoly > 7` stop path into `None`. Remaining `tri_neighbors` work is an integrated wrapper over the migrated W/U/M phases plus fixture parity on full icosahedron connectivity.

### 2026-06-12: `icosahedron.F90` `tri_neighbors` integrated wrapper ported

Extended `rust/earthmesh_mesh` with `derive_icosahedron_tri_neighbors_fortran`, an integrated wrapper that runs the migrated W-face, U-edge, and M-point phases in the same high-level order as `tri_neighbors`. The wrapper mutates the Fortran-indexed U/W connectivity tables and returns the derived M-point neighbor table, making the migrated connectivity derivation usable after `icosahedron_fill_diamonds_fortran`. Remaining icosahedron work is spring relaxation and full NXP fixture parity after relaxation.

### 2026-06-12: `icosahedron.F90` `spring_dynamics1` setup tables ported

Extended `rust/earthmesh_mesh` with `icosahedron_spring_topology_fortran`, a tested Rust port of the pre-iteration setup in `spring_dynamics1`. It snapshots U-edge endpoint ids, first-ring U neighbors, M-point polygon edge ids, and the Fortran direction sign rule (`+relax` when the current M point is the second edge endpoint, otherwise `-relax`). Remaining `spring_dynamics1` work is the edge displacement calculation and multi-iteration coordinate relaxation loop.

### 2026-06-12: `icosahedron.F90` `spring_dynamics1` single-iteration update ported

Extended `rust/earthmesh_mesh` with `icosahedron_spring_iteration_fortran`, a tested Rust port of one main-loop iteration in `spring_dynamics1`. The kernel computes U-edge lengths from M-point coordinates, applies the opposite-angle `twocosphi3/twocosphi4` ratio clamp, uses the OLAM `dist00 / 1.2` target scaling, applies precomputed M-point direction signs, and renormalizes updated M points back to the requested radius. Remaining `spring_dynamics1` work is the multi-iteration wrapper and periodic Max-DS diagnostics.

### 2026-06-12: `icosahedron.F90` `spring_dynamics1` multi-iteration wrapper ported

Extended `rust/earthmesh_mesh` with `icosahedron_spring_dynamics1_fortran`, a tested Rust wrapper for the main `spring_dynamics1` iteration loop. It repeatedly applies the migrated single-iteration edge-displacement/coordinate-normalization kernel and records Fortran-style periodic Max-DS diagnostics for iteration 1 and `diagnostic_every` intervals. Remaining icosahedron work is full NXP fixture parity for the post-relaxation grid.

### 2026-06-12: `icosahedron.F90` integrated relaxed-grid wrapper ported

Extended `rust/earthmesh_mesh` with `icosahedron_relaxed_grid_fortran`, an integrated Rust entry point for the deterministic in-memory path of `icosahedron(nxp0)`. The wrapper composes initial M-point generation, `fill_diamond`, `tri_neighbors`, `spring_dynamics1` topology, Fortran `dist00 = beta * pi2_r8 * erad8 / (5 * nxp0)`, and the migrated spring loop. Remaining icosahedron work is fixture-based NXP parity for post-relaxation coordinates/connectivity against the Fortran output.

### 2026-06-12: `icosahedron.F90` NXP64 post-spring parity fixture added

Added an ignored long-running release fixture test for `icosahedron_relaxed_grid_fortran(64, 5000, beta=1.0, relax=0.035)`, comparing point counts and sampled post-spring coordinates against the existing Fortran-generated `gridfile_NXP0064_01_ori.nc4` `GLONW/GLATW` output. `voronoi()` moves `xemd/yemd/zemd` into final W-point coordinates, so this fixture validates the migrated initial-grid, `fill_diamond`, `tri_neighbors`, and `spring_dynamics1` path against the archived NXP0064 Fortran output; pole longitude is intentionally not asserted because it is coordinate-convention arbitrary at ±90° latitude.

### 2026-06-12: `mkgrd.F90` namelist parsing and `read_nl` validation ported

Extended `rust/earthmesh_core` with `EarthmeshConfig::from_mkgrd_namelist` and `EarthmeshConfig::file_dir`, covering the non-destructive compatibility layer for the Fortran `/mkgrd/ NL` namelist consumed by `mkgrd.F90:read_nl`. The Rust parser now accepts `NL%...` assignments case-insensitively, preserves Fortran string/logical/numeric values, derives `file_dir = trim(base_dir) // trim(expnme) // '/'`, and enforces the same `gridnum_perdegree` plus `mesh_type`/`output_format` constraints for CoLM, FVCOM, MPAS, MPAS-Simple, and LOC mesh modes. Filesystem setup/removal and later `mkrefine` namelist save/write compatibility remain orchestration-layer work.

### 2026-06-12: `refine_vars` defaults and core `mkrefine` validation ported

Extended `rust/earthmesh_core` with `RefineConfig`, covering the operational defaults from `consts_coms.F90:refine_vars` and the first non-I/O compatibility layer for the Fortran `/mkrefine/ RL` namelist. The Rust parser now covers core refinement controls (`Istransition`, spring type switches, transition arrays, regional protection layers, iteration counts, specified/calculated refinement mode switches, and mask prefixes) and reproduces the `read_nl` derived behavior for `refine_setting`, disabled-transition spring-type forcing, spring-type exclusivity, `vertex_pretect_layers`, and the `atmosmesh`/`refine_cal` rejection. Full threshold-variable matrix parsing, mask-count validation after `Mask_make`, and `namelist.save` copy behavior remain orchestration-layer work.

### 2026-06-12: `mkrefine` threshold switch/value matrix ported

Extended `RefineConfig::from_mkrefine_namelist` with the land, ocean, atmosphere, and LOC threshold-switch matrix used by `mkgrd.F90:read_nl`. The Rust parser now maps the `refine_lai/slope/k_*`, `refine_sst/ssh/eke/sea_slope`, and `refine_typhoon` switches plus their `th_*` threshold values into typed arrays, including two-layer land thresholds. It also reproduces the Fortran calculate/mixed-mode requirement that each mesh type enables at least one relevant threshold criterion, and rejects enabled threshold switches whose required `th_*` value remains at the `999.` sentinel. Remaining `read_nl` migration work is now filesystem setup/removal, `Mask_make`-derived mask-count validation, and preserving the original input as `result/namelist.save`.

### 2026-06-12: `read_nl` workspace side effects split into a Rust plan

Extended `EarthmeshConfig` with a non-destructive `read_nl_workspace_plan`, plus typed `MkgrdWorkspacePlan` and `MaskOperation` records. This captures the filesystem and mask-preprocess side effects implied by `mkgrd.F90:read_nl`: file-dir reset intent, `contain/gridfile/patchtype/result/tmpfile/threshold` directory creation, `result/namelist.save` destination, domain/patch/refine `Mask_make` calls, and the `mask_restart` short-circuit that avoids directory rebuild while still allowing patch masks. The Rust core still does not execute shell commands or copy files; that remains for the future CLI/Python orchestration adapter.

### 2026-06-12: `mem_grid` and `mem_ijtabs` allocation defaults ported

Extended `rust/earthmesh_core` with Rust-owned `GridMemory` and `IjTabs` state. `GridMemory` now mirrors `mem_grid` zero-filled coordinate and lon/lat allocation routines, while `IjTabs` ports the `mem_ijtabs` loop constants and default M/V/W record initialization (`loop(:)=.false.`, neighbor ids initialized to `1`, rank defaults, and W-direction defaults). Remaining `mem_*` work is focused on `mem_delaunay` copy/original arrays and wiring these typed state containers into production mesh orchestration.

### 2026-06-12: `mem_delaunay` typed state and allocation defaults ported

Extended `rust/earthmesh_core` with `DelaunayMemory` and typed Delaunay records (`ItabMd`, `ItabUd`, `ItabWd`, `NestUd`, `NestWd`). The Rust state now mirrors `mem_delaunay` defaults for loop flags, neighbor arrays, refinement metadata, copy/original buffers, and `alloc_itabsd` zero-filled `xemd/yemd/zemd` allocation. This closes the typed-state replacement surface inside `consts_coms.F90`; remaining work for this file is production wiring from the legacy module globals into Rust-owned orchestration state.

### 2026-06-12: `read_nl` workspace filesystem adapter started

Added `rust/earthmesh_cli` with `apply_read_nl_workspace_plan`, the first Rust orchestration adapter for `mkgrd.F90:read_nl` side effects. It executes the safe filesystem subset from `MkgrdWorkspacePlan`: optional case-directory removal, `*_filelist.txt` cleanup in the working directory, required directory creation, and copying the input namelist to `result/namelist.save`. Restart plans preserve the Fortran short-circuit by avoiding case-directory deletion while still saving the namelist. `Mask_make` file discovery and mask-count validation remain separate adapter work because they depend on bbox/lambert/circle/close mask formats and NetCDF outputs.

### 2026-06-12: `Mask_make` prefix discovery ported without shell execution

Extended `rust/earthmesh_cli` with `discover_mask_sources`, the first Rust adapter slice for `mkgrd.F90:Mask_make`. It preserves the Fortran path split and `mask_fprefix*` source-listing behavior without invoking `ls` or writing temporary `*_filelist.txt` files, returning a typed `MaskSourceDiscovery` with parent directory, file prefix, and sorted matching files. Mask-type parsing (`bbox`, `lambert`, `circle`, `close`), NetCDF output generation, and mask-count updates remain separate parity-gated adapter work.

### 2026-06-12: `bbox_mask_make` text input parsing and numbering ported

Extended `rust/earthmesh_cli` with the text `.nml` branch of `mkgrd.F90:bbox_mask_make`. The new `parse_bbox_mask_nml` preserves the Fortran free-format `bbox_num`, `bbox_refine`, and bbox point parsing, validates West/East and North/South orientation, and returns no output plan when `refine_degree > max_iter_spc` just like the Fortran early return. Added `MaskCountState::next_bbox_output` to reproduce `mask_domain_ndm`, `mask_refine_ndm(refine_degree)`, `mask_patch_ndm(refine_degree)`, and the `tmpfile/{mask_select}_bbox_{refine}_{NN}.nc4` output naming. NetCDF bbox copy/write remains pending.

### 2026-06-12: `bbox_mask_make` NetCDF copy branch adapter ported

Extended `rust/earthmesh_cli` with `copy_bbox_mask_netcdf_with_refine`, covering the `.nc/.nc4` branch side effects after `bbox_refine` has been read: extension validation, early no-op when `refine_degree > max_iter_spc`, Fortran-compatible counter advancement through `MaskCountState`, creation of the `tmpfile` parent, and byte-for-byte copy into `tmpfile/{mask_select}_bbox_{refine}_{NN}.nc4`. Reading `bbox_refine` from NetCDF metadata and writing bbox NetCDF files from text `.nml` points remain pending so that the NetCDF dependency boundary can be decided separately.

### 2026-06-12: `bbox_mask_make` NetCDF reader/writer ported

Added the Rust `netcdf` crate to `rust/earthmesh_cli` and implemented `read_bbox_refine_netcdf` plus `write_bbox_mask_netcdf`. The reader extracts the scalar `bbox_refine` used by the `.nc/.nc4` branch, rejects negative values, and feeds the existing copy/count adapter. The writer creates `bbox_num`, `four`, `bbox_points`, and `bbox_refine`, giving text `.nml` bbox inputs a reusable NetCDF output that can be consumed by the copied-file branch. A small `build.rs` adds the `nc-config --libs` library directory as an rpath for macOS test/runtime parity with the local NetCDF C install. Remaining `Mask_make` work is lambert/circle/close generation, integrating the per-file dispatcher, and validating aggregate mask counts.

### 2026-06-12: `circle_mask_make` and `close_mask_make` adapters ported

Extended `rust/earthmesh_cli` with typed `CircleMask`, `CloseMask`, and shared `LonLatPoint` records. The Rust adapter now parses text `.nml` circle/close inputs, preserves the `refine_degree > max_iter_spc` no-op behavior, writes and reads `circle_refine`/`close_refine` NetCDF metadata, writes `circle_points`/`circle_radius` and `close_points`, and copies `.nc/.nc4` inputs into the Fortran tmpfile naming scheme. `MaskCountState` now also preserves the count-width difference: bbox/circle use two-digit counters while close uses three-digit counters. Remaining `Mask_make` work is lambert conversion, per-file dispatcher integration over discovered sources, and final mask-count validation.

### 2026-06-12: `lamb_mask_make` mode4 conversion ported

Extended `rust/earthmesh_cli` with `LambertVertices`, `Mode4Mesh`, `read_lambert_vertices_netcdf`, `lambert_vertices_to_mode4_mesh`, `write_mode4_mesh_netcdf`, and `convert_lambert_mask_netcdf`. The Rust path reads `xi_vert`/`eta_vert`, `lon_vert`, and `lat_vert`, applies the Fortran longitude wrap for values above 180°, preserves the sentinel first bound/mode rows, derives `ngr_bound` and `n_ngr` with the same one-based offset convention, and writes the `Mode4_Mesh_Save`-style NetCDF output under `tmpfile/{mask_select}_lambert_0_NN.nc4`. Remaining `Mask_make` work is now per-file dispatcher integration over discovered sources plus final mask-count validation.

### 2026-06-12: `Mask_make` dispatcher and refine-count validation ported

Extended `rust/earthmesh_cli` with `apply_mask_operation` and `MaskOperationReport`, wiring the Rust `MaskOperation` plan into bbox, lambert, circle, and close source handlers over `discover_mask_sources`. Text `.nml` inputs now parse and write NetCDF outputs through the shared `MaskCountState`, while `.nc/.nc4` inputs read their refine metadata and copy or convert with Fortran-compatible skip/count behavior. Added `validate_mask_refine_reaches_max_iter_spc` to preserve the specified-refinement `read_nl` guard that requires at least one `mask_refine` source at `max_iter_spc`. Remaining `mkgrd.F90` work moves beyond Mask_make into mode4mesh/gridinit/voronoi/pcvt/gridfile_write orchestration and final Rust CLI replacement.

### 2026-06-12: `mode4mesh_make` Lambert NetCDF branch ported

Extended `rust/earthmesh_cli` with `Mode4MeshMakeReport` and `mode4mesh_make_netcdf`, covering the active Lambert `.nc/.nc4` branch of `mkgrd.F90:mode4mesh_make`. The adapter reuses the migrated Lambert vertex reader and mode4 mesh conversion, writes `Mode4_Mesh_Save`-compatible NetCDF outputs to a requested gridfile path, reports bound/mode point counts, and preserves the legacy unsupported paths as explicit `InvalidInput` errors for Lambert `.nml`, lonlat NetCDF, cubical, and unknown grid selections. Remaining `mkgrd.F90` work is the gridinit/voronoi/pcvt/gridfile_write pipeline and final Rust CLI orchestration.

### 2026-06-12: workspace and mask operation orchestration wired

Extended `rust/earthmesh_cli` with `WorkspaceMaskApplyReport` and `apply_workspace_and_mask_operations`, which applies the Rust `read_nl` workspace plan, executes every planned `Mask_make` operation in order, returns final `MaskCountState`, and optionally enforces the specified-refinement `max_iter_spc` mask-count guard. This closes the Rust adapter wiring between namelist-derived workspace setup and the migrated bbox/lambert/circle/close mask machinery; remaining work is focused on the gridinit/voronoi/pcvt/gridfile_write pipeline and a final Rust CLI replacement for `mkgrd.x`.

### 2026-06-12: `gridfile_write` unstructured NetCDF writer ported

Extended `rust/earthmesh_cli` with `UnstructuredMesh`, `gridfile_mesh_from_state`, `write_unstructured_mesh_netcdf`, `gridfile_output_path`, and `write_gridfile_from_state`, covering the compact `MOD_file_preprocess.F90:Unstructured_Mesh_Save` schema used by `mkgrd.F90:gridfile_write`. The Rust adapter derives `GLONM/GLATM/GLONW/GLATW`, `itab_m%iw`, `itab_w%im`, and `n_ngrwm` from `GridMemory` plus `IjTabs`, preserves the Fortran `n_ngrwm(1)=1` and pentagon/hexagon rule, and writes `gridfile/gridfile_NXP####_##_<mode_grid>.nc4`. Remaining `mkgrd.F90` work is now the `gridinit` generation path itself, especially Delaunay-to-Voronoi conversion and `pcvt` circumcenter adjustment before final Rust CLI orchestration.

### 2026-06-12: `grid_xyz2lonlat` state adapter ported

Extended `rust/earthmesh_mesh` with `grid_xyz2lonlat_state`, a state-level adapter for `mkgrd.F90:grid_xyz2lonlat`. It validates placeholder-inclusive M/W Cartesian arrays, allocates `GLONM/GLATM/GLONW/GLATW` through `GridMemory::allocate_grid_lonlatmw`, and fills lon/lat values using the already migrated scalar `xyz_to_lonlat_degrees` formula. This gives the remaining `gridinit` pipeline a tested Rust step between `pcvt` Cartesian coordinates and the Rust `gridfile_write` adapter.

### 2026-06-12: Fortran-indexed gridfile boundary locked

Extended `rust/earthmesh_cli` with `gridfile_mesh_from_fortran_indexed_state` and `write_gridfile_from_fortran_indexed_state`, a tested boundary for kernels that keep direct Fortran one-based arrays (`slot 0` unused, valid records in `1..=nma` and `1..=nwa`). This prevents the remaining `gridinit/voronoi/pcvt` migration from forcing an unsafe compact-index conversion before writing `Unstructured_Mesh_Save` gridfiles, while preserving legacy connectivity IDs and `n_ngrwm` rules.

### 2026-06-12: `voronoi` one-based state conversion ported

Extended `rust/earthmesh_mesh` with `VoronoiGridState`, `voronoi_grid_from_icosahedron_relaxed`, and `grid_xyz2lonlat_fortran_indexed_state`. The new path ports the global `mkgrd.F90:voronoi` count swap from Delaunay M/U/W to grid M/U/V/W, moves relaxed icosahedron M coordinates into one-based W arrays, initializes one-based M coordinates as normalized barycenters of Delaunay W faces, and carries M/W connectivity into `IjTabs` for downstream `pcvt` and the one-based gridfile writer. Remaining `gridinit` work is the `pcvt` circumcenter adjustment plus final orchestration around the already migrated lon/lat fill and gridfile write.

### 2026-06-12: `pcvt` and in-memory `gridinit` orchestration ported

Extended `rust/earthmesh_mesh` with `pcvt_adjust_voronoi_grid_state` and `gridinit_voronoi_state_fortran`. The new PCVT adapter ports the global `mkgrd.F90:pcvt` loop over one-based M points, using each triangle's three W vertices to replace barycentric M coordinates with spherical circumcenters while preserving placeholder slots. The gridinit wrapper composes the migrated `icosahedron_relaxed_grid_fortran -> voronoi_grid_from_icosahedron_relaxed -> pcvt_adjust_voronoi_grid_state -> grid_xyz2lonlat_fortran_indexed_state` sequence and returns a one-based `VoronoiGridState` ready for the existing Rust gridfile writer boundary. Remaining `mkgrd.F90` work is final Rust CLI orchestration/replacement for `mkgrd.x` and larger fixture parity across release NXP settings.

### 2026-06-12: initial `mkgrd.x` gridinit CLI path ported

Extended `rust/earthmesh_cli` with `run_mkgrd_gridinit_global_namelist` and a minimal `earthmesh_cli` binary. The new path parses a legacy mkgrd namelist, applies the migrated `read_nl` workspace and mask-operation plan, runs the Rust global gridinit sequence, and writes the Fortran-compatible initial gridfile `gridfile/gridfile_NXP####_01_<mode_grid>.nc4`. The binary initially covered the no-existing-`mode_file` `hex`/`tri` branch and intentionally rejected `mask_restart` plus existing mode-file ingestion until those branches were migrated behind tests. Existing `mode_file` ingestion is now covered separately for EarthMesh, MPAS, FVCOM, and IAP-Ocean; remaining work is restart/remask handling and the refine/postprocess loop around the initial gridfile.

### 2026-06-12: full NXP64 Rust gridinit gridfile parity fixture added

Added an ignored long-running `rust/earthmesh_cli` fixture for the fully migrated initial gridinit path at `NXP=64, niter=5000, beta=1.0, relax=0.035`. The fixture runs the Rust namelist-to-gridfile path, writes `gridfile_NXP0064_01_hex.nc4`, and compares dimensions, sampled `GLONM/GLATM/GLONW/GLATW`, `itab_m%iw`, and `n_ngrwm` against the archived Fortran `cases/ATMOS_hex_N64_refine2_global_LOM67_251027/gridfile/gridfile_NXP0064_01_hex.nc4`. This closes release-scale parity for the no-existing-`mode_file` initial gridinit branch; after the existing `mode_file` converter ports, remaining `mkgrd.x` migration work is restart/remask and refine/postprocess orchestration after the initial gridfile.

### 2026-06-12: existing EarthMesh `mode_file` ingestion branch ported

Extended `rust/earthmesh_cli` with `copy_existing_earthmesh_mode_file` and wired `run_mkgrd_gridinit_global_namelist` to mirror the Fortran branch where `mode_file` exists and `mode_file_description='EarthMesh'`. The Rust path now applies the workspace/mask plan, validates the source gridfile dimensions, copies the existing EarthMesh gridfile into `gridfile/gridfile_NXP####_01_<mode_grid>.nc4`, and reports copied dimensions without regenerating the grid. MPAS, FVCOM, and IAP-Ocean mode-file converters were migrated in subsequent Rust slices.

### 2026-06-12: existing MPAS `mode_file` converter ported

Extended `rust/earthmesh_cli` with `convert_mpas_mode_file_to_earthmesh` and wired the existing-`mode_file`, `mode_file_description='MPAS'` branch in `run_mkgrd_gridinit_global_namelist`. The converter ports `MOD_file_preprocess.F90:MPAS_Mesh_Read`: it reads MPAS `nVertices/nCells/maxEdges`, radian `lonVertex/latVertex/lonCell/latCell`, `cellsOnVertex`, `verticesOnCell`, and `nEdgesOnCell`; inserts EarthMesh placeholder records; shifts connectivity by one; normalizes longitudes into `[-180, 180]`; and writes the standard `Unstructured_Mesh_Save` gridfile schema. FVCOM and IAP-Ocean converters were migrated in subsequent Rust slices.

### 2026-06-12: existing FVCOM `mode_file` converter ported

Extended `rust/earthmesh_cli` with `convert_fvcom_mode_file_to_earthmesh` and wired the existing-`mode_file`, `mode_file_description='FVCOM'` branch in `run_mkgrd_gridinit_global_namelist`. The converter ports `MOD_file_preprocess.F90:FVCOM_Mesh_Read`: it reads FVCOM `maxelem/node/nele`, element centers `lonc/latc`, node coordinates `lon/lat`, element-to-node `nv`, node-to-element `nbve`, and node neighbor counts `ntve`; retains the legacy placeholder record semantics; shifts connectivity by one; normalizes longitudes into `[-180, 180]`; and writes the standard `Unstructured_Mesh_Save` gridfile schema. IAP-Ocean conversion was migrated in the subsequent Rust slice.

### 2026-06-12: existing IAP-Ocean `mode_file` converter ported

Extended `rust/earthmesh_cli` with `convert_iap_ocean_mode_file_to_earthmesh` and wired the existing-`mode_file`, `mode_file_description='IAP-Ocean'` branch in `run_mkgrd_gridinit_global_namelist`. The converter ports the `mkgrd.F90` branch that calls `MOD_grid_preprocess.F90:IAP_Mesh_make`: it reads IAP `sjx_points/lbx_points`, radian `GLONW/GLATW`, and `itab_m%im/itab_m%iw`; inserts legacy placeholder records; rebuilds M-point spherical circumcenters from W-point triangles using the migrated Rust mesh geometry kernels; derives W-to-M adjacency with the `GetSortNew` adjacency-walk/orientation semantics; and writes the standard `Unstructured_Mesh_Save` gridfile schema. Existing `mode_file` ingestion now covers EarthMesh, MPAS, FVCOM, and IAP-Ocean in Rust.

### 2026-06-12: top-level `mask_restart` remask decision ported

Added `rust/earthmesh_cli::plan_mkgrd_mask_restart_namelist` with a focused fixture for the `mkgrd.F90` branch where `mask_restart=.true.`, `mesh_type='oceanmesh'`, and `mask_patch_on=.false.`. The Rust plan now preserves the Fortran state transition around remasking (`refine=.false.`, `step=max_iter+1`) and selects an explicit `RunMaskPostproc` action without deleting/recreating the case workspace or re-running grid initialization. The heavy `MOD_mask_postproc.F90:mask_postproc` execution remains a separate migration surface; this slice only ports the top-level branch decision and report contract.

### 2026-06-12: mask_postproc vertex reindex helpers ported

Extended `rust/earthmesh_mesh` with tested ports of `MOD_mask_postproc.F90:extract_unique_vertices` and `sort_and_reindex`. The Rust helpers preserve the legacy one-based placeholder vertex `1`, scan center-neighbor rows from id `2`, retain first-seen unique-vertex ordering before sorting, and build the old-vertex-id to compact new-id mapping used before final Earth/ocean mask-postprocess output writes. Full `mask_postproc` execution, NetCDF I/O, boundary renewal, and data-finalization orchestration remain pending.

### 2026-06-12: mask_postproc `Data_Renew` helper ported

Extended `rust/earthmesh_mesh` with `renew_mask_postproc_data_fortran_indexed`, a tested pure-data port of `MOD_mask_postproc.F90:Data_Renew`. The helper compacts active centers according to `IsInDmArea_ustr`, rebuilds center-to-vertex and vertex-to-center counts/tables for `tri` and `hex` width conventions, preserves the one-based placeholder row, computes `ustr_points_next`/`ustr_bounds_next`, and intentionally writes original source center ids into vertex adjacency as the Fortran comment requires. `Data_Finial`, boundary repair, and full mask_postproc NetCDF orchestration remain pending.

### 2026-06-12: mask_postproc `Data_Finial` helper ported

Extended `rust/earthmesh_mesh` with `finalize_mask_postproc_data_fortran_indexed`, a tested pure-data port of `MOD_mask_postproc.F90:Data_Finial`. The helper compacts active center coordinates and center-neighbor rows, rebuilds intermediate vertex adjacency using compact center ids as the Fortran `k` branch requires, then compacts surviving vertex coordinates and vertex-neighbor rows while preserving the one-based placeholder convention. Remaining `mask_postproc` work is boundary/isolated-ocean/narrow-waterway repair plus Earth/Lnd/Ocn/Atmos NetCDF orchestration.

### 2026-06-12: mask_postproc domain-renew helpers ported

Extended `rust/earthmesh_mesh` with `renew_mask_postproc_domain_triangles_fortran_indexed` and `renew_mask_postproc_opposite_domain_triangles_fortran_indexed`, tested ports of `MOD_mask_postproc.F90:IsInDmArea_ustr_Renew` and `IsInDmArea_ustr_Renew_v2`. The helpers preserve the one-based placeholder arrays, active/inactive `IsInDmArea_ustr` integer semantics, the solid-boundary deletion rule (`n_ustr_ngr == 6`), the one-missing-triangle refill count update, and the v2 opposite-slot (`j`/`j+3`) two-triangle refill. Remaining `mask_postproc` work is isolated-ocean/narrow-waterway repair plus Earth/Lnd/Ocn/Atmos NetCDF orchestration.

### 2026-06-12: mask_postproc narrow-waterway helper ported

Extended `rust/earthmesh_mesh` with `widen_narrow_waterway_fortran_indexed`, a tested pure-data port of `MOD_mask_postproc.F90:narrow_waterway_widen`. The helper rebuilds the temporary boundary vertex-to-vertex graph from compact ocean center rows, preserves the four-connection narrow-waterway signature and duplicate-neighbor detection, and activates all original centers adjacent to the duplicated boundary neighbor. Remaining `mask_postproc` work is the isolated-ocean removal helper plus Earth/Lnd/Ocn/Atmos NetCDF orchestration.

### 2026-06-12: mask_postproc boundary closed-curve helper ported

Extended `rust/earthmesh_mesh` with `boundary_closed_curves_fortran_indexed`, a tested pure-data port of `MOD_mask_postproc.F90:bdy_connection_closed_curve`. The helper preserves the Fortran boundary walk order, placeholder curve slot, closed-curve length records, longest-curve id, and the legacy `num_bdy_long(1:2)+1` allocation convention. Remaining `mask_postproc` work is the boundary graph builder, isolated-ocean removal helper, and Earth/Lnd/Ocn/Atmos NetCDF orchestration.

### 2026-06-12: mask_postproc boundary graph helper ported

Extended `rust/earthmesh_mesh` with `boundary_connection_fortran_indexed`, a tested pure-data port of the graph-building portion of `MOD_mask_postproc.F90:bdy_connection`. The helper scans compact center rows for boundary vertex pairs, preserves the Fortran first-boundary-edge-per-center rule, validates that each retained boundary vertex has exactly two true connections, returns the one-based `bdy_order`/`bdy_ngr` equivalents, and composes with the migrated closed-curve helper. Remaining `mask_postproc` work is isolated-ocean removal plus Earth/Lnd/Ocn/Atmos NetCDF orchestration.

### 2026-06-12: mask_postproc isolated-ocean removal helper ported

Extended `rust/earthmesh_mesh` with `remove_isolated_ocean_fortran_indexed`, a tested pure-data port of `MOD_mask_postproc.F90:Isolated_Ocean_Renew` after boundary graph construction. The helper preserves longest-boundary retention, `bdy_long_order` placeholder layout, isolated-curve classification via `sum(2*n_new-n_original) < 0`, boundary vertex count zeroing, center mask removal, and the inward peeling loop for fully surrounded next-layer vertices. Remaining `mask_postproc` work is `bdy_calculation`/boundary-output wiring plus Earth/Lnd/Ocn/Atmos NetCDF orchestration.

### 2026-06-12: mask_postproc boundary classification helper ported

Extended `rust/earthmesh_mesh` with `classify_boundary_orders_fortran_indexed`, a tested pure-data port of the classification and ordering portion of `MOD_mask_postproc.F90:bdy_calculation`. The helper maps longest-boundary vertices through `vertex_mapping`, splits OBC/IBC points from the active mask, converts singleton OBC points to IBC, and preserves the legacy boundary-order rotation rule. Remaining `mask_postproc` work is now the adapter/orchestration layer: Earth/Lnd/Ocn/Atmos NetCDF reads/writes plus `obc.nc4`/`obcv2.nc4` writer wiring around the migrated helpers.

### 2026-06-12: mask_postproc OBC NetCDF writers ported

Extended `rust/earthmesh_cli` with tested adapter-layer writers for `MOD_mask_postproc.F90:bdy_calculation` and `bdy_connection` outputs. The new path helpers preserve the legacy `result/obc.nc4`, `result/obc_patch.nc4`, `result/obcv2.nc4`, and `result/obcv2_patch.nc4` filenames, while `write_obc_boundary_netcdf` writes `bdy_num`, `bdy_order`, `obc_order`, and `ibc_order`, and `write_obcv2_boundary_netcdf` writes `num1`, `num2`, `close_curve`, and `n_close_curve` from the migrated pure boundary helpers. Remaining `mask_postproc` work is full Earth/Lnd/Ocn/Atmos orchestration and NetCDF read/write composition around the migrated helpers.

### 2026-06-12: mask_postproc domain I/O plan ported

Added a tested side-effect-free `MaskPostprocDomainIoPlan` in `rust/earthmesh_cli` for the Earth/Lnd/Ocn branches of `MOD_mask_postproc.F90:mask_postproc`. The plan preserves legacy `gridfile_NXP####_<mode>.nc4` inputs, `contain_<mesh>_domain_NXP####_<mode>.nc4` inputs, clipped `gridfile_NXP####_<mode>_<mesh>[_patch].nc4` outputs, Earth/Land `patchtype_NXP####_<mode>.nc4` outputs, and Ocean-tri `obc`/`obcv2` patch-aware boundary outputs. Remaining work is composing real NetCDF reads/writes and the full branch execution around these plans and migrated helpers.

### 2026-06-12: Unstructured_Mesh_Read adapter ported

Added `read_unstructured_mesh_netcdf` in `rust/earthmesh_cli` as the Rust reader counterpart to the existing `write_unstructured_mesh_netcdf` adapter for `MOD_file_preprocess.F90:Unstructured_Mesh_Read`/`Unstructured_Mesh_Save` gridfiles. The reader validates `sjx_points`, `lbx_points`, `dimb=3`, `dimc`, coordinate variables, `itab_m%iw`, `itab_w%im`, and `n_ngrwm`, then restores the typed `UnstructuredMesh` payload used by grid initialization and future `mask_postproc` orchestration. It trims writer-added trailing zero padding from fixed-width `itab_w%im` rows so the Rust layer keeps its ragged connectivity semantics.

### 2026-06-12: Contain_Read/Contain_Save adapters ported

Added typed `ContainMesh` plus `read_contain_netcdf` and `write_contain_netcdf` in `rust/earthmesh_cli` for the `MOD_file_preprocess.F90:Contain_Read`/`Contain_Save` schema used by `mask_postproc`. The adapter validates `num_ustr`, `num_ii`, `dim_a`, `dim_b`, `ustr_id`, `ustr_ii`, and `IsInArea_ustr`, rejects ragged rows or mask-length mismatches before writing, and round-trips representative land/ocean/earth-style integer payloads through NetCDF.

### 2026-06-12: PatchID_Save adapter ported

Added typed `PatchIdMesh` and `write_patchid_netcdf` in `rust/earthmesh_cli` for the `MOD_mask_postproc.F90:PatchID_Save` schema used by the Earth/Land mask-postprocess branches. The writer preserves `nlon`, `nlat`, `elmindex`, `lon_w`, `lon_e`, `lat_n`, `lat_s`, `longitude`, and `latitude`, and validates matrix/vector dimension consistency before creating the patchtype NetCDF file.

### 2026-06-12: mask_postproc_Ocn contain mask adjustment ported

Added `apply_ocean_mask_sea_ratio_fortran_indexed` in `rust/earthmesh_cli`, a tested pure-data port of the `mask_postproc_Ocn` loop that copies `IsInDmArea_ustr` from `Contain_Read` and then demotes post-`num_vertex` records to `-1` when `ustr_id(i,1) / real(ustr_id(i,3)) < mask_sea_ratio`. The helper preserves Fortran one-based row semantics, validates the ocean-specific third `ustr_id` column, and reports invalid denominators before the full Ocean branch orchestration is wired.

### 2026-06-12: mask_postproc_Earth patchtypes_make ported

Added `build_earth_patchtypes_fortran_indexed` in `rust/earthmesh_cli`, a tested pure-data port of the `mask_postproc_Earth` `patchtypes_make` loop. The helper preserves Fortran one-based `ustr_id`/`ustr_ii` row references, land-pixel ratio classification with the strict `> mask_sea_ratio` rule, `seaorland_ustr` values (`0` missing, `-1` sea, `1` land), land/sea counters, and longitude/latitude index remapping into the patchtype grid before the full Earth branch orchestration is wired.

### 2026-06-12: mask_postproc tri/hex layout setup ported

Added `mask_postproc_layout_from_unstructured_mesh` and `MaskPostprocLayout` in `rust/earthmesh_cli`, covering the repeated `mode_grid == 'tri'/'hex'` setup in `mask_postproc_Earth`, `mask_postproc_Lnd`, and `mask_postproc_Ocn`. The helper preserves the Fortran orientation rule: tri uses M points as centers and W points as vertices, while hex swaps W points to centers and M points to vertices, carrying the matching center/vertex connectivity and neighbor-count arrays into the Rust orchestration shape.

### 2026-06-12: `mask_postproc_Lnd` patchtype fallback ported

Added `build_land_patchtypes_fortran_indexed`, a tested Rust port of the land-only `patchtypes_make` loop. It preserves the Fortran rule that any non-zero `IsInDmArea_ustr` cell is active, maps active contain pixels to `patchtypes_select`, clears covered `seaorland` land pixels, and fills ignored land pixels from the previous latitude row while rejecting cases that would leave a land cell without a patch id. Full `mask_postproc_Lnd` mesh clipping, boundary renewal, and NetCDF orchestration remain unported.

### 2026-06-12: final mask_postproc vertex reindex loop ported

Added `reindex_final_center_vertices_fortran_indexed`, a tested Rust port of the post-`Data_Finial` loop that rewrites final center-neighbor vertex ids through `vertex_mapping`. This closes the pure renumbering step after `extract_unique_vertices` and `sort_and_reindex`; full Earth/Lnd/Ocn orchestration still needs to compose the migrated helpers around real NetCDF inputs and outputs.

### 2026-06-12: final mask_postproc gridfile adapter ported

Added `unstructured_mesh_from_mask_postproc_final`, a tested CLI adapter for the final `Unstructured_Mesh_Save` call in `mask_postproc_*`. The helper writes `tri` final centers/vertices directly and applies the Fortran `hex` argument swap so the legacy gridfile still stores triangles in `m*` arrays and polygons in `w*` arrays. Full Earth/Lnd/Ocn orchestration still needs to call this after the migrated mesh-renewal helpers and before NetCDF persistence.

### 2026-06-12: mask_postproc finalization pipeline adapter ported

Added `finalize_mask_postproc_layout_to_unstructured_mesh`, a tested composition layer that turns a prepared `MaskPostprocLayout` plus `IsInDmArea_ustr` into the final legacy `UnstructuredMesh` payload. It composes the migrated `Data_Finial`, unique-vertex extraction, sorted vertex remapping, final center-neighbor reindexing, and tri/hex gridfile adapter. Domain-specific mask edits, boundary renewal, patchtype writing, and NetCDF orchestration still remain outside this helper.

### 2026-06-12: mask_postproc final gridfile writer orchestration ported

Added `write_mask_postproc_final_gridfile`, a tested orchestration helper that uses `MaskPostprocDomainIoPlan.result_gridfile`, the finalization pipeline, and the legacy unstructured-grid NetCDF writer to persist a postprocessed domain gridfile. This is the first Rust CLI layer that writes the final `mask_postproc_*` result path from a prepared layout and completed `IsInDmArea_ustr`; the remaining work is to construct that prepared layout and domain mask directly from the full Earth/Lnd/Ocn/Atmos workflows.

### 2026-06-12: mask_postproc domain input reader orchestration ported

Added `read_mask_postproc_domain_inputs`, a tested Rust CLI helper that loads the source unstructured gridfile and contain-domain NetCDF selected by `MaskPostprocDomainIoPlan`, converts the gridfile into `MaskPostprocLayout`, and carries forward the initial `IsInDmArea_ustr` mask. This covers the common read side of the Earth/Lnd/Ocn domain branches; the remaining orchestration work is applying branch-specific domain edits, patchtype generation, ocean boundary renewal, and final writer calls end-to-end.

### 2026-06-12: PatchID selected-domain coordinate builder ported

Added `patchid_mesh_from_selected_domain`, a tested Rust port of the coordinate-array construction inside `PatchID_Save`. It builds `lon_w/lon_e/lat_n/lat_s/longitude/latitude` from the selected-domain `minlon_DmArea`, `maxlat_DmArea`, `lon_vertex`, `lat_vertex`, `lon_i`, and `lat_i` lookup arrays, so Earth/Lnd patchtype generation can feed the existing NetCDF writer without duplicating Fortran indexing rules.

### 2026-06-12: Earthmesh info writer and payload builder ported

Added `EarthmeshInfo`, `write_earthmesh_info_netcdf`, and `build_earthmesh_info_fortran_indexed` in `rust/earthmesh_cli` for the Earth branch `earthmesh_info.nc4` output. The writer preserves the `LOCmesh_info_save` schema (`num_step`, `num_ustr`, `num_step_f`, `refine_degree_f`, `seaorland_ustr_f`), while the builder ports the final `mask_postproc_Earth` tri/hex refinement and land/ocean-role compaction loops from `IsInDmArea_ustr` and `seaorland_ustr`. Remaining `mask_postproc` work is full branch orchestration around real Earth/Lnd/Ocn/Atmos inputs, mask renewal, boundary outputs, and final file sequencing.

### 2026-06-12: mask_postproc patchtype writer orchestration ported

Added `write_mask_postproc_patchtype_netcdf`, a tested Rust CLI composition helper that connects the selected-domain patchtype grid to the `MaskPostprocDomainIoPlan.patchtype_output` path. The helper reuses `patchid_mesh_from_selected_domain` and `write_patchid_netcdf`, preserving the Earth/Lnd `patchtype/patchtype_NXP####_<mode>.nc4` filename while rejecting Ocean plans that intentionally have no patchtype output. Full Earth/Lnd branch execution still needs to sequence the patchtype writer with contain reads, branch mask edits, final gridfile clipping, and optional Earth info output.

### 2026-06-12: mask_postproc Earth info writer orchestration ported

Added `write_mask_postproc_earth_info_netcdf`, a tested Earth-only composition helper that binds `build_earthmesh_info_fortran_indexed` and `write_earthmesh_info_netcdf` to the legacy `result/earthmesh_info.nc4` path selected from `MaskPostprocDomainIoPlan.file_dir`. It rejects Land/Ocean plans so `earthmesh_info` remains an Earth branch side effect. Remaining work is full Earth branch sequencing from real contain/gridfile reads through patchtype output, final gridfile clipping, and this info writer.

### 2026-06-12: MPAS simple mesh NetCDF writer ported

Added `MpasSimpleMesh` and `write_mpas_simple_mesh_netcdf` in `rust/earthmesh_cli`, a tested Rust writer for `MOD_file_preprocess.F90:MPAS_Mesh_Simple_Save`. The adapter preserves the simple MPAS dimensions (`nCells`, `nVertices`, `vertexDegree`), writes cell/vertex Cartesian coordinates, `cellsOnVertex`, `meshDensity`, and global `on_a_sphere`/`sphere_radius` attributes while applying the Fortran placeholder slicing convention (`2:num_dbx`, `2:num_sjx`). Remaining atmosmesh work is to build this payload from `MPAS_Mesh_Cal_Simple` inputs and then port the full MPAS mesh writer/graph-info path.

### 2026-06-12: MPAS simple mesh calculation path ported

Added `read_cellwidth_netcdf`, `build_mpas_simple_mesh_from_unstructured_fortran_indexed`, and `write_mpas_simple_mesh_from_netcdf_inputs` in `rust/earthmesh_cli`, covering the file-level `MOD_mask_postproc.F90:MPAS_Mesh_Cal_Simple` path for MPAS-Simple output. The Rust path reads `Unstructured_Mesh_Read`-compatible gridfiles plus `cellwidth_NXP####_global.nc4`, keeps the legacy first placeholder row, converts lon/lat to unit-sphere Cartesian coordinates like `lonlat2xyz` in this Fortran subroutine, converts `ngrmw` to zero-based `cellsOnVertex`, computes `meshDensity = (min(cellwidth) / cellwidth) ** 4`, and writes the existing MPAS-Simple NetCDF schema. Remaining atmosmesh work is the full MPAS mesh writer/graph-info path and top-level `mask_postproc_Atmos` dispatch integration.

### 2026-06-12: Atmos MPAS-Simple mask-postprocess dispatch ported

Added `write_mask_postproc_atmos_mpas_simple_netcdf` in `rust/earthmesh_cli`, a tested Rust entry point for the `mask_postproc_Atmos` branch when `mesh_type='atmosmesh'` and `output_format='MPAS-Simple'`. The helper preserves the legacy `result/gridfile_NXP####_<mode_grid>.nc4`, `result/cellwidth_NXP####_global.nc4`, and `result/MPASOUT_NXP####_global_Simple.nc4` paths and delegates to the migrated MPAS-Simple file pipeline. Remaining atmosphere work is the full MPAS (non-simple) `MPAS_Mesh_Cal` writer/graph-info path; remaining domain work is Earth/Lnd/Ocn top-level execution composition.
