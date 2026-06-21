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

### 2026-06-12: MPAS graph.info writer ported

Added `MpasGraphInfoWriteReport` and `write_mpas_graph_info` in `rust/earthmesh_cli`, a tested Rust port of `MOD_file_preprocess.F90:MPAS_info_Save`. The writer keeps the legacy placeholder row, counts only interior edges with both adjacent cells present, emits Fortran-style width-10 integer rows, and reports cells whose positive neighbor count is lower than `nEdgesOnCell`. This removes one more dependency from the full MPAS atmosphere output path; the large MPAS NetCDF writer and remaining geometry/file orchestration are still separate migration gates.

### 2026-06-12: Full MPAS mesh NetCDF writer ported

Added `MpasMesh`, `MpasMeshWriteReport`, and `write_mpas_mesh_netcdf` in `rust/earthmesh_cli`, a tested Rust writer for `MOD_file_preprocess.F90:MPAS_Mesh_Save`. The adapter preserves the full MPAS dimensions (`nCells`, `nVertices`, `nEdges`, `maxEdges`, `maxEdges2`, `TWO`, `vertexDegree`), generated `indexTo*ID` variables, placeholder-row slicing (`2:num_dbx`, `2:num_sjx`, `2:num_edge`), representative 1D/2D/scalar variables, and the legacy global attributes including `mesh_spec`, `sphere_radius`, and `source`. Remaining full-atmosphere work is now the `MPAS_Mesh_Cal` file-level orchestration that builds this payload from the migrated geometry kernels and pairs it with the graph.info writer.

### 2026-06-12: MPAS edge-reference reader ported

Added `MpasEdgeReference` and `read_mpas_edge_reference_netcdf` in `rust/earthmesh_cli`, a tested Rust port of `MOD_file_preprocess.F90:data_read`. The reader loads full MPAS `cellsOnEdge`, `lonEdge`, and `latEdge`, restores the legacy placeholder row, applies the Fortran `cellsOnEdge_reference = cellsOnEdge_reference + 1` ID shift, converts edge coordinates from radians to degrees, and preserves the single-step longitude normalization used by the Fortran branch. This removes another file I/O dependency from `MPAS_Mesh_Cal`; remaining work is composing the full payload from geometry outputs and writing `distsOnEdge`/spring-adjustment persistence adapters.

### 2026-06-12: distsOnEdge NetCDF writer ported

Added `DistsOnEdgeMesh`, `DistsOnEdgeWriteReport`, and `write_dists_on_edge_netcdf` in `rust/earthmesh_cli`, a tested Rust port of `MOD_file_preprocess.F90:distsOnEdge_save`. The writer preserves the legacy `num_edge` dimension and variables `lonv`, `latv`, and `distsOnEdge`, with validation that edge coordinates and distance arrays have matching lengths. This closes another Springjustment/global persistence dependency; remaining grid-preprocess adapter work is composing these file writers/readers around the migrated GetEdge/GetArea/spring kernels.

### 2026-06-12: cellwidth NetCDF writer ported

Added `CellwidthMesh`, `CellwidthWriteReport`, and `write_cellwidth_netcdf` in `rust/earthmesh_cli`, a tested Rust port of `MOD_file_preprocess.F90:cellwidth_save`. The writer preserves the legacy `num_dbx` dimension and variables `lonw`, `latw`, and `cellwidth`, validates coordinate/value length consistency, and round-trips with the existing `read_cellwidth_netcdf` adapter. This completes the read/write pair used by MPAS-Simple and the Springjustment/global file pipeline.

### 2026-06-12: quality_save_global NetCDF writer ported

Added `QualityClassMetrics`, `GlobalQualityMesh`, and `write_quality_global_netcdf` in `rust/earthmesh_cli`, a tested Rust port of `MOD_file_preprocess.F90:quality_save_global`. The writer preserves the legacy dimensions (`num_sjx`, `num_wbx`, `num_lbx`, `two`, `thr`, `fiv`, `six`) plus optional `num_qbx`/`sev`, writes the `length_*`, `angle_*`, `Extr_*`, `Eavg_*`, `Savg_*`, `less_*`, and `more_*` variables for each class, and rejects row-width/count mismatches before creating output. This closes the file-writer dependency for `Grid_Quality_Check_Global`; remaining grid-preprocess adapter work is composing quality calculation, Springjustment persistence, and GetArea/GetEdge NetCDF boundaries end-to-end.

### 2026-06-12: Grid_Quality_Check_Global quality adapter composition ported

Added `global_quality_mesh_from_grid_quality` and `write_grid_quality_global_netcdf` in `rust/earthmesh_cli`, wiring the migrated pure `earthmesh_mesh::GridQualityGlobalOutput` into the `quality_save_global` NetCDF writer. The adapter preserves Fortran placeholder triangle rows, compact polygon quality rows, converts boolean less/more angle flags to legacy integer arrays, and omits heptagon/qbx output when no heptagon quality group exists. Remaining `MOD_grid_preprocess.F90` adapter work is now Springjustment persistence plus GetArea/GetEdge NetCDF boundaries.

### 2026-06-12: Springjustment_global persistence adapter ported

Added `write_springjustment_global_persistence` in `rust/earthmesh_cli`, wiring the pure `earthmesh_mesh::SpringjustmentGlobalCoreOutput` to the migrated `distsOnEdge_save` and `cellwidth_save` NetCDF writers. The adapter preserves legacy `result/distsOnEdge_NXP####_##_global.nc4` and optional `result/cellwidth_NXP####_global.nc4` paths, keeps `distsOnEdge` edge coordinates from the pure kernel, and requires pre-spring cell coordinates for the MPAS cellwidth side effect to match the Fortran `wp` argument. Remaining Springjustment work is full NetCDF loading/orchestration for global runs and regional-step persistence wiring.

### 2026-06-12: GetEdge gridfile adapter ported

Added `get_edge_from_unstructured_gridfile` and `get_edge_from_unstructured_mesh` in `rust/earthmesh_cli`, wiring the migrated `Unstructured_Mesh_Read` gridfile adapter into the pure `earthmesh_mesh::get_edge_production_fortran_indexed` workflow. The adapter validates legacy Fortran-indexed `itab_m%iw`, `itab_w%im`, and `n_ngrwm` ids, reconstructs triangle-neighbor membership through the migrated `set_ngrmm` equivalent, and returns the production `cellsOnEdge`, `verticesOnEdge`, `edgesOnVertex`, `cellsOnVertex`, and edge midpoint payload. Remaining `MOD_grid_preprocess.F90` adapter work is `GetArea` NetCDF integration plus Springjustment global loading/regional persistence.

### 2026-06-12: GetArea gridfile adapter ported

Added `get_area_from_unstructured_gridfile` and `get_area_from_unstructured_mesh` in `rust/earthmesh_cli`, composing `Unstructured_Mesh_Read`, the migrated `GetEdge` adapter, and `earthmesh_mesh::get_area_production_fortran_indexed`. The adapter validates gridfile connectivity, converts triangle/cell/edge lon-lat rows to unit-sphere Cartesian points, preserves original gridfile `ngrmw`/`cellsOnVertex` ordering for area reconstruction, and returns `kiteAreasOnVertex`, `areaTriangle`, `areaCell`, plus the reconstruction diagnostic. Remaining `MOD_grid_preprocess.F90` adapter work is now Springjustment global NetCDF loading and regional-step persistence/orchestration.

### 2026-06-12: Springjustment_global gridfile adapter ported

Added `run_springjustment_global_from_unstructured_gridfile` and `run_springjustment_global_from_unstructured_mesh` in `rust/earthmesh_cli`, wiring legacy `Unstructured_Mesh_Read` gridfiles into the migrated pure `springjustment_global_core_fortran_indexed` workflow. The adapter validates Fortran-indexed gridfile connectivity, preserves original connectivity while returning updated triangle/cell lon-lat payloads for a later `Unstructured_Mesh_Save`, and reuses the legacy `distsOnEdge_NXP####_##_global.nc4` plus optional `cellwidth_NXP####_global.nc4` persistence adapter. Remaining `MOD_grid_preprocess.F90` Springjustment work is regional-step persistence/orchestration around the already migrated regional kernels.

### 2026-06-12: Springjustment_regional_step gridfile adapter ported

Added `run_springjustment_regional_from_unstructured_gridfile`, `run_springjustment_regional_from_unstructured_mesh`, and `write_springjustment_regional_gridfile` in `rust/earthmesh_cli`. The adapter wires legacy `Unstructured_Mesh_Read` payloads and an already-derived regional move mask into the migrated pure `springjustment_regional_core_fortran_indexed` workflow, returns updated triangle/cell lon-lat coordinates with original connectivity preserved, and exposes a tested `Unstructured_Mesh_Save` persistence hook for the caller-owned final gridfile path. This closes the tracked `MOD_grid_preprocess.F90` Rust migration surfaces in the manifest; remaining project work is now in const state wiring, mkgrd orchestration, Area_judge, and mask_postproc composition.

### 2026-06-12: Full MPAS mesh calculation pipeline ported

Added `build_mpas_mesh_from_unstructured_fortran_indexed` and `write_mpas_mesh_from_netcdf_inputs` in `rust/earthmesh_cli`, covering the file-level `MOD_mask_postproc.F90:MPAS_Mesh_Cal` path before the final top-level atmosphere dispatch is wired. The builder composes migrated GetEdge, ordered verticesOnCell, GetArea, edge distance/angle, set_weightsOnEdge, MPAS zero-based connectivity conversion, cellwidth-derived meshDensity, nominalMinDc, the full MPAS NetCDF writer, and graph.info writer from legacy `Unstructured_Mesh_Read` plus `cellwidth_read` inputs. Remaining `MOD_mask_postproc.F90` manifest work is now Earth/Lnd/Ocn top-level execution composition around the already migrated domain helpers.

### 2026-06-13: mask_restart Area_judge final postprocess library runner

Promoted the previously CLI-local mask_restart ContinueMkgrd final handoff into `rust/earthmesh_cli` as `run_mkgrd_mask_restart_area_judge_postproc_namelist`. The runner composes restarted `Area_judge`, final `Get_Contain(0)`, and the land/ocean/earth `mask_postproc` branches from one typed options bundle, and the binary now delegates to that library surface while preserving its output lines. This narrows the remaining `mkgrd.F90` restart work to true refine-enabled restart loop handoff, rather than duplicated final postprocess wiring.

### 2026-06-13: Area_judge restart refine-loop handoff

Added `run_mkgrd_refine_loop_namelist_with_area_judge_restart_grids_and_migrated_executor`, a library-level handoff from saved `Area_judge` restart grids into the migrated refine-loop executor stack. The runner prepares the namelist/workspace, restores the domain and sea-land state from `IsInDmArea_grid`, optionally rebuilds the iter-zero calculated refine selected grid, seeds the first refine-loop gridfile, derives source-branch options from the restored restart state, and then executes the migrated source/refine/final handoff pipeline. The remaining `mkgrd.F90` restart work is now focused on broad binary/top-level exposure and any additional initial-gridfile restart variants.

### 2026-06-13: Area_judge restart refine-loop CLI exposure

Exposed the restart-grid handoff through the `earthmesh_cli` binary as `--run-mask-restart-area-judge-refine` with `--restart-refine-source-state` and `--restart-refine-initial-gridfile`. The CLI now reads a compact source-state file for source-grid geometry/landtype metadata, derives the standard `result/IsInDmArea_grid.nc4` restart input from the namelist file_dir, restores the restart domain/sea-land grid, seeds the first refine-loop gridfile, and runs the migrated source/refine executor stack. The library runner also now backfills refine `Mask_make` operations for `NL%mask_restart=.true.` namelists, because the legacy restart read_nl plan intentionally skips normal refine-mask preparation. Remaining restart-refine work is production data_preprocess/landtype-file wiring beyond compact source-state inputs and any additional post-initial-gridfile variants.

### 2026-06-13: Area_judge restart refine-loop landtype source CLI

Added `--run-mask-restart-area-judge-refine-landtype-source`, a production-facing variant of the restart-refine handoff that reads `NL%landtype_file` through the migrated Rust data_preprocess reader instead of requiring a compact source-state text file. The binary derives source axes, `landtypes_global`, `maxlc`, and mode-grid `num_vertex`, reuses the saved `result/IsInDmArea_grid.nc4` restart payload for domain/sea-land state, and then enters the same migrated Area_judge-refine/Get_Contain/GetRef/refine_loop stack. This leaves compact source-state as a debugging/fixture path while giving restart-refine production cases a direct landtype-file route.

### 2026-06-13: Area_judge restart refine-loop final postprocess handoff

Extended the Area_judge restart-refine runner so the migrated refine-loop handoff can optionally compose the final `Get_Contain(0)` domain contain file and land/ocean `mask_postproc` output after the final refine step. The `earthmesh_cli --run-mask-restart-area-judge-refine-landtype-source` path now uses `--mask-postproc-num-vertex` to trigger this final handoff while still deriving source geometry and landtype metadata directly from `NL%landtype_file`. For landmesh postprocess, the selected sea/land mask and selected-domain bounds are read from the restart `IsInDmArea_grid.nc4` payload so patchtype construction is tied to the restored Area_judge window rather than the full global source raster. The PatchID selected-domain coordinate builder was also corrected to advance latitude source indices southward from `maxlat_DmArea`, matching the existing `patchtype_indices` convention and preventing multi-row selected domains from underflowing latitude coordinates.

### 2026-06-13: Native hydro close-mask ring simplification

Extended the Rust-native hydro/coast close-mask NML exporter with `--simplify-tolerance-deg`, moving the simplification control from recipe metadata into the actual `.nml` generation path. The exporter now validates a finite non-negative tolerance, simplifies closed GeoJSON exterior rings with a tolerance-based line-distance pass, preserves at least three vertices, and writes the simplified coordinates directly to `close_num`/`close_refine` mask files. Existing cumulative refine, class caps, per-degree caps, and ring-separation behavior remain unchanged. Remaining close-mask geometry work is true buffer/offset generation and any topology-aware dissolve/union behavior; no new geometry dependency was introduced for this simplification slice.

### 2026-06-13: Native hydro close-mask envelope buffer export

Extended the Rust-native hydro/coast close-mask NML exporter with `--buffer-deg-by-refine-degree`, so the same per-refine-degree buffer controls that were already written into recipe JSON now affect generated `.nml` masks directly. The current Rust implementation uses a conservative lon/lat bounding-envelope expansion per ring and per refine degree, which avoids under-covering hydro/coast refinement corridors without adding a new topology dependency. This works with cumulative refine masks, class caps, ring-separation filtering, and the native simplification pass. Remaining close-mask geometry work is higher-fidelity polygon offset plus topology-aware dissolve/union for overlapping corridor buffers.

### 2026-06-13: top-level mask_restart dispatcher avoids gridinit fallthrough

Added an option-free `run_mkgrd_top_level_namelist` dispatcher in `rust/earthmesh_cli` so `mkgrd.x` mask-restart namelists are classified before the normal gridinit branch. The dispatcher executes the already migrated `mask_patch_on` restart preprocessing branch directly and returns a typed restart plan for option-dependent restart branches. The default `earthmesh_cli <mkgrd.nml>` entry now uses this dispatcher, so `mask_restart=.true.`/`mask_patch_on=.true.` no longer falls through to the old `mask_restart mkgrd branch is not yet migrated to Rust` gridinit error.

### 2026-06-13: Native hydro close-mask envelope dissolve

Extended the Rust-native hydro/coast close-mask exporter with `--dissolve-overlapping-envelopes` and a matching `dissolve_overlapping_envelopes` composite-recipe component option. When enabled, the exporter merges overlapping or touching close-mask envelopes that share the same class, refine degree, and target refine degree before writing `.nml` files. This covers the current CaMa/MERIT envelope-buffer workflow and reduces duplicate hydro/coast refinement masks without changing default output behavior. Remaining close-mask geometry work is still true arbitrary-polygon offset, dissolve, and union beyond axis-aligned envelope merging.

### 2026-06-13: default mkgrd restart-refine handoff dispatch

The default `earthmesh_cli <mkgrd.nml>` path now recognizes restart-refine handoff inputs without requiring the explicit debug flags. Supplying `--restart-refine-initial-gridfile` plus either `--restart-refine-source-state` or an `NL%landtype_file`-backed namelist enters the migrated Area_judge restart/refine stack directly, including the production landtype-file source route and optional final land/ocean postprocess controls. This moves the Rust CLI closer to the real `mkgrd.x` top-level restart flow instead of stopping at a `ContinueMkgrd` plan for refine-enabled restarts.

### 2026-06-13: runtime state carries refine final handoff step

`EarthmeshRuntimeState` now exposes a typed `with_step` update and the migrated refine-loop namelist prepare path stores the planned final `mkgrd` handoff step in `runtime_state.step`. This removes another implicit `consts_coms`/module-global style handoff from the refine orchestration: downstream Rust callers can read the current loop step from the explicit runtime state while `num_mp_step`/`num_wp_step` remain reserved for real mesh point counts instead of being overloaded as step sentinels.


### 2026-06-13: runtime state records initial mesh counts

Added `EarthmeshRuntimeState::record_mesh_counts_for_step` for Fortran-style 1-based `mkgrd` step counters and wired the initial gridinit plus existing/converted `mode_file` ingestion paths to store real `num_mp_step`/`num_wp_step` counts. This replaces another `consts_coms` module-global handoff with explicit Rust state: generated NXP grids and compact mode-file conversions now carry the triangle/cell counts needed by later `Get_Contain`/`refine_loop` style adapters instead of leaving the count arrays at their legacy placeholder defaults.

### 2026-06-13: Get_Contain reports explicit runtime counters

Extended the file-backed `Get_Contain` refine adapter report with `GetContainRuntimeCounts`, carrying the current input grid triangle/cell counts plus the caller-provided previous `num_vertex` boundary. This makes the `MOD_GetContain.F90` handoff that previously mutated `consts_coms:num_mp_step`, `num_wp_step`, and `num_vertex` visible to Rust orchestration and later `refine_loop` adapters, without reintroducing module-global mutable state.

### 2026-06-13: source-branch executor preserves Get_Contain counters

The migrated `MkgrdRefineSourceBranchExecutor` now records calculated/specified source-branch reports when called through the generic `MkgrdRefineLoopExecutor` trait path. This prevents the Rust orchestration layer from discarding `GetContainRuntimeCounts` during normal `Area_judge_refine -> Get_Contain -> GetRef` dispatch, keeping the former `consts_coms` `num_mp_step`/`num_wp_step` handoff inspectable after source-branch execution.

### 2026-06-13: migrated refine executor exposes source reports

Added a `source_branch_reports()` accessor on the standard `MkgrdMigratedRefineLoopExecutor`. Callers using the normal migrated source/working-state executor stack can now inspect recorded calculated/specified branch reports, including `GetContainRuntimeCounts`, without unpacking the generic composite executor. This keeps the replacement for `consts_coms` mesh-count handoffs available at the Rust orchestration boundary.

### 2026-06-13: refine-loop execution returns source reports

Extended `MkgrdRefineLoopExecutionReport` with `source_branch_reports` and added a default `MkgrdRefineLoopExecutor::source_branch_reports()` accessor. Generic refine-loop execution now returns any recorded calculated/specified branch reports from the executor, so the explicit `GetContainRuntimeCounts` handoff survives not only the source executor but also the top-level `mkgrd` refine-loop report boundary.

### 2026-06-13: namelist/top-level reports expose source reports

Added `source_branch_reports()` forwarding on `MkgrdRefineLoopNamelistRunReport` and `MkgrdTopLevelNamelistRunReport`. Source-branch reports, including migrated `GetContainRuntimeCounts`, now survive the standard executor, refine-loop execution report, namelist runner, and top-level `mkgrd.x` runner API boundaries without callers unpacking nested execution structs. This further replaces implicit `consts_coms` mesh-count handoffs with explicit Rust report surfaces.

### 2026-06-13: default restart-refine dispatch validates source inputs

Hardened the default `earthmesh_cli <mkgrd.nml> --restart-refine-initial-gridfile ...` dispatch so it only infers the landtype-file restart-refine path when the namelist explicitly provides `NL%landtype_file`. If neither `--restart-refine-source-state` nor an explicit landtype file is present, the binary now stops with a clear source-input error before attempting to open a default or unrelated NetCDF path. This makes the top-level restart-refine handoff safer for production `mkgrd.x` replacement workflows.

### 2026-06-13: hydro close-mask ring-aware offset buffering

Replaced the close-mask buffer path's envelope-only expansion with a ring-aware planar offset for valid GeoJSON exterior rings. The Rust exporter now shifts each polygon edge outward and intersects adjacent shifted edges, so non-axis-aligned hydro/coast corridors keep their original ring shape when buffered instead of being forced into a coarse bounding rectangle. Axis-aligned rectangles still produce the previous envelope-equivalent output, and the older envelope fallback remains for degenerate rings. Remaining close-mask geometry work is topology-aware arbitrary-polygon dissolve/union beyond the current same-degree envelope dissolve.

### 2026-06-13: hydro close-mask rectilinear rectangle union

Extended close-mask dissolve beyond pure bounding-envelope merging for a common hydro/coast case: touching or overlapping axis-aligned rectangle rings now trace the rectilinear union boundary before writing `.nml` files. This preserves L-shaped corridor unions rather than over-covering them with a coarse bbox, while retaining the previous envelope fallback for non-rectangular or degenerate cases. Remaining close-mask geometry work is full arbitrary-polygon topology-aware dissolve/union.

### 2026-06-13: hydro close-mask chained rectilinear union

Extended close-mask dissolve so multi-segment axis-aligned rectangle corridors do not collapse back to a coarse bounding box after the first pairwise merge. Previously a two-rectangle L-shape could be preserved, but adding a third touching rectangle caused the merged L-shaped mask to lose rectangle identity and fall back to bbox merging. The Rust exporter now decomposes rectilinear merged masks into covered cells, unions those cells with the next rectangle/ring, and retraces the exterior boundary. This keeps chained river/coast corridor masks tighter while leaving full arbitrary-polygon topology-aware dissolve/union as the remaining geometry gap.

### 2026-06-13: hydro close-mask shared-edge polygon union

Extended close-mask dissolve beyond rectilinear corridors for a first non-rectangular topology case: two same-class polygons that share a complete boundary edge are now merged by cancelling the shared reversed edge and tracing the remaining exterior boundary. This prevents triangular or skewed hydro/coast refinement corridors from being over-covered by a bounding box when their polygons tile along a full edge. The remaining geometry gap is still full arbitrary-polygon topology-aware dissolve/union for partial overlaps, crossing edges, holes, and more complex multi-polygon arrangements.

### 2026-06-13: hydro close-mask partial shared-edge polygon union

Extended the native close-mask dissolve path from exact shared-edge cancellation to partial shared-edge polygon union. The Rust exporter now splits each polygon edge at vertices from the adjacent polygon before cancelling reversed boundary segments, so a triangular/skewed hydro corridor that only shares part of an edge with the next polygon can be merged and traced without falling back to a bounding envelope. This is another step toward arbitrary-polygon dissolve while the remaining gap still includes crossing-edge intersections, true overlap topology, holes, and complex multi-polygon union.

### 2026-06-13: hydro close-mask contained polygon union

Extended native close-mask dissolve for containment topology: when one same-class polygon is fully inside another polygon, the Rust exporter now keeps the outer ring as the union boundary instead of falling back to the combined bounding envelope. This covers contained hydro/coast refinement islands or duplicate nested features for non-rectangular rings. Remaining arbitrary-polygon dissolve work still includes crossing-edge intersections, true partial overlaps, holes, and complex multi-polygon topology.

### 2026-06-13: hydro close-mask crossing-edge polygon union

Extended native close-mask dissolve for simple overlapping non-rectangular polygons whose boundaries cross. The Rust exporter now splits polygon edges at true segment intersections, removes split edge segments whose midpoints lie strictly inside the adjacent polygon, cancels reversed shared edges, and traces the remaining exterior boundary. This avoids bounding-box fallback for a common hydro/coast overlap topology while keeping remaining arbitrary-polygon work focused on holes, multi-component unions, and more complex self/overlap arrangements.

### 2026-06-13: source-branch Get_Contain counts update runtime state

Extended `MkgrdRefineSourceBranchExecutor` with an optional `EarthmeshRuntimeState` owner so the generic source-branch execution path no longer only preserves `GetContainRuntimeCounts` in reports. After calculated or specified source dispatch, the executor now writes the current `Get_Contain` mesh counts into `EarthmeshRuntimeState::num_mp_step` and `num_wp_step` for the active 1-based mkgrd step. This moves another former `consts_coms` mutable-global handoff into explicit Rust-owned state while keeping report-only callers unchanged.

### 2026-06-13: refine-loop execution returns runtime state snapshots

Added a default `MkgrdRefineLoopExecutor::runtime_state()` hook and carried its cloned value into `MkgrdRefineLoopExecutionReport`. Composite and standard migrated executors forward the source executor runtime state, so the `Get_Contain` mesh-count writeback performed during source dispatch now survives the generic refine-loop execution boundary. This keeps former `consts_coms` step/count state visible through the normal Rust orchestration API rather than only through concrete executor internals.

### 2026-06-13: namelist and top-level reports expose runtime state

Added `runtime_state()` accessors to `MkgrdRefineLoopNamelistRunReport` and `MkgrdTopLevelNamelistRunReport`. The refine report returns the execution runtime-state snapshot when the executor provides one, falling back to the prepared state; the top-level report forwards the refine runtime state when present and otherwise exposes gridinit state. This keeps the Rust-owned replacement for former `consts_coms` step/count state available through the same namelist/top-level API boundary that already forwards source-branch reports.

### 2026-06-13: standard migrated executor seeds runtime state

Added a seeded standard migrated refine-loop executor builder and wired the namelist/top-level migrated-stack runners to pass `prepare.runtime_state` into `MkgrdRefineSourceBranchExecutor`. The production migrated stack now updates `EarthmeshRuntimeState::num_mp_step` and `num_wp_step` from source-branch `Get_Contain` counts, instead of only supporting that writeback for manually constructed source executors. This closes another `consts_coms` handoff gap in the normal Rust replacement path.

### 2026-06-13: default restart-refine infers existing case gridfile

Extended the default `earthmesh_cli <mkgrd.nml>` restart-refine handoff so a source-state restart can use the standard existing case gridfile path `gridfile/gridfile_NXP####_01_<mode>.nc4` when `--restart-refine-initial-gridfile` is omitted. The Area_judge restart-refine runner also skips the initial-gridfile copy when the inferred source already equals the first refine-loop input path, avoiding NetCDF self-copy corruption. This moves another mask_restart/refine continuation case closer to Fortran `ContinueMkgrd` behavior while preserving the explicit initial-gridfile override.

### 2026-06-13: default landtype restart-refine infers existing case gridfile

Extended the default `earthmesh_cli <mkgrd.nml>` restart-refine dispatch for `NL%landtype_file` sources so it can reuse the standard existing case gridfile path `gridfile/gridfile_NXP####_01_<mode>.nc4` when `--restart-refine-initial-gridfile` is omitted. The auto-handoff is intentionally conservative: it only activates for `mask_restart + refine + NL%landtype_file` when the standard gridfile already exists, so ordinary restart continuations without a prepared refine grid keep their prior path.

### 2026-06-13: default mask_restart ocean postprocess executes

Extended the default `earthmesh_cli <mkgrd.nml>` mask-restart dispatcher so the `oceanmesh + mask_patch_on=.false.` `RunMaskPostproc` branch now runs the migrated ocean `mask_postproc` path instead of only printing a typed plan. When `--mask-postproc-num-vertex` is omitted, the CLI infers the legacy `num_vertex` boundary from the restart contain file's `ustr_ii` rows, then writes the final ocean gridfile and tri OBC outputs through the existing Rust runner. This moves another production `mkgrd.F90` restart branch from advisory planning into executable Rust behavior.

### 2026-06-13: explicit mask_restart ocean infers num_vertex

Relaxed the explicit `earthmesh_cli <mkgrd.nml> --run-mask-restart-ocean` path so `--mask-postproc-num-vertex` is now optional. When omitted, the CLI uses the same Rust inference as the default mask-restart ocean branch: it reads the restart contain file and uses the `ustr_ii` row count as the legacy `num_vertex` boundary before executing the migrated ocean `mask_postproc` runner. This removes another debug-only manual argument from the production restart path while keeping the explicit override available for unusual fixtures.

### 2026-06-13: top-level dispatcher owns mask_restart ocean postprocess

Moved the default mask-restart ocean execution from a CLI-only fallback into `rust/earthmesh_cli::run_mkgrd_top_level_namelist`. The library dispatcher now returns a `MaskRestartOcean` report and writes the migrated ocean `mask_postproc` outputs directly for `oceanmesh + mask_patch_on=.false.`, inferring `num_vertex` from the contain file. The CLI now only formats the returned report, so the Rust API boundary itself owns this `mkgrd.F90` top-level restart branch.

### 2026-06-13: mask_restart ocean num_vertex inference is reusable

Promoted the restart-ocean `num_vertex` reconstruction into `rust/earthmesh_cli::infer_mask_restart_ocean_num_vertex_from_config`. The helper reads the persisted restart contain file and derives the legacy boundary from `ustr_ii`, matching the state that Fortran previously kept in modules. Both the library top-level dispatcher and the explicit CLI ocean restart path now use the same Rust API instead of duplicating inference logic in `main.rs`.

### 2026-06-13: restart-refine initial gridfile inference is reusable

Promoted the standard restart-refine initial gridfile inference into `rust/earthmesh_cli` library helpers: `restart_refine_initial_gridfile_path_from_config`, `infer_restart_refine_initial_gridfile_from_config`, and `maybe_infer_restart_refine_initial_gridfile_from_config`. The CLI default restart-refine dispatcher now reuses these APIs instead of carrying local path-building logic in `main.rs`, keeping another piece of former `mkgrd.F90` restart/refine state reconstruction at the Rust library boundary.

### 2026-06-13: default restart-refine handoff classification is reusable

Moved the default `mkgrd.x` restart-refine source selection into `rust/earthmesh_cli::infer_default_restart_refine_handoff_from_config`. The reusable API now owns the source-state vs `NL%landtype_file` decision, standard initial-gridfile inference, conservative implicit landtype auto-handoff, and the missing-source validation that was previously embedded only in the CLI front-end. The binary now only maps the typed library decision to the existing execution branch flags, reducing CLI-only restart/refine orchestration logic.

### 2026-06-13: restart land selected-domain reconstruction is reusable

Promoted the restart-refine land postprocess selected-domain reconstruction into `rust/earthmesh_cli::selected_land_domain_from_area_judge_grid_payload`. The library API now validates the saved Area_judge restart payload, requires `seaorland_select`, carries the selected source bounds, and returns the exact sea/land matrix needed by land `mask_postproc`. The CLI restart-refine final land handoff now reuses this API instead of keeping the state reconstruction only in `main.rs`.

### 2026-06-13: restart/refine source axes are reusable

Promoted the CLI-local global source-axis reconstruction into `rust/earthmesh_cli::build_global_source_axes_fortran_indexed` and `GlobalSourceAxes`. Restart/refine handoffs that still need source-state geometry now obtain one-based `lon_vertex`, `lat_vertex`, `lon_i`, and `lat_i` from the library, and the CLI no longer owns a separate `SourceAxes` builder. The landtype data_preprocess reader also reuses the same axis builder, keeping source geometry reconstruction consistent across data_preprocess, source-state, and restart-refine paths.

### 2026-06-13: compact source-state parsing is reusable

Promoted the source-state text parser used by migrated `mkgrd` source-state and restart-refine handoffs into `rust/earthmesh_cli::parse_mkgrd_compact_source_state` / `read_mkgrd_compact_source_state`. The typed `MkgrdCompactSourceState` now owns source dimensions, first-triangle and `num_vertex` handoffs, optional calculated-refine bounds, final contain/postprocess requests, and the `is_in_domain`/`seaorland`/`landtypes_global` matrices. The CLI now reads this library state directly instead of carrying a private parser and private final-postprocess enum in `main.rs`.

### 2026-06-13: compact source-state selected matrix extraction is reusable

Promoted the compact source-state selected-matrix extraction used by land final `mask_postproc` into `rust/earthmesh_cli::compact_source_state_selected_matrix_fortran_order`. The library now owns the one-based matrix shape check and the Fortran postprocess ordering that writes longitude rows with latitude reversed from `nlats_source` down to `1`. The CLI source-state branch now reuses this helper instead of keeping a private matrix-ordering function.

### 2026-06-13: compact source-state final postprocess request is reusable

Promoted compact source-state final postprocess request construction into `rust/earthmesh_cli::compact_source_state_final_postproc_request`. The library now validates the required `final_domain_contain` handoff for land/ocean postprocess requests, owns the land selected-domain matrix normalization plus selected-domain dimensions, and returns a typed request consumed by the CLI. This removes another source-state final `mask_postproc` orchestration detail from `main.rs` while preserving the existing axes borrowing and output writer composition.

### 2026-06-13: compact source-state final contain request is reusable

Promoted compact source-state final contain payload/options construction into `rust/earthmesh_cli::compact_source_state_final_domain_area_payload_fortran_indexed` and `compact_source_state_final_contain_options`. The library now owns the full-source Area_judge selection window and typed `MkgrdFinalDomainContainOptions` borrowing for source-state final `Get_Contain(0)`, while the CLI only supplies the output path and forwards the resulting options. This keeps source-state final-domain contain orchestration aligned with the reusable source axes and final postprocess request helpers.

### 2026-06-13: data-preprocess source-state land-domain selection is reusable

Promoted the `--run-refine-landtype-source` final land `mask_postproc` selected-domain reconstruction into `rust/earthmesh_cli::selected_land_domain_from_full_source_seaorland_fortran_order`. The library now owns the full-source `seaorland` shape check, minimal nonzero land bounding window, empty-land fallback, and selected matrix extraction used by data_preprocess-derived source-state handoffs. This removes another private `main.rs` state-reconstruction helper and aligns landtype-source final postprocess with the reusable Area_judge restart selected-domain API.

### 2026-06-14: data-preprocess source-state final contain request is reusable

Promoted the `--run-refine-landtype-source` final-domain contain payload/options construction into `rust/earthmesh_cli::data_preprocess_source_state_final_domain_area_payload_fortran_indexed` and `data_preprocess_source_state_final_contain_options`. The library now owns the full-source Area_judge selection window plus mesh-type-to-`GetContainMeshKind` option mapping for data_preprocess-derived source-state handoffs, preserving the previous no-contain behavior for unsupported mesh types. The CLI now only writes the generated payload and forwards the typed options into the migrated final handoff.

### 2026-06-14: data-preprocess source-state final postprocess request is reusable

Promoted the `--run-refine-landtype-source` final land/ocean `mask_postproc` request construction into `rust/earthmesh_cli::data_preprocess_source_state_final_postproc_request`. The library now owns the landmesh selected-domain reconstruction, selected-domain integer bounds, ocean `num_vertex` handoff, and unsupported-mesh no-postprocess behavior for data_preprocess-derived source-state handoffs. The CLI now only maps the typed request to existing borrowed runner options, removing another `mesh_type` postprocess branch from `main.rs`.

### 2026-06-14: landtype source mode and sea-land conversion are reusable

Promoted the migrated landtype-source `NL%mode_grid` to `num_vertex` inference and landtype-to-`seaorland` conversion into `rust/earthmesh_cli::mkgrd_mode_grid_num_vertex` and `seaorland_from_landtypes_global_fortran_indexed`. Restart-refine and direct landtype-source handoffs now share the same Rust API for tri/hex boundary inference and one-based land/ocean mask construction instead of keeping these Fortran-state reconstruction details as private CLI helpers.

### 2026-06-14: restart-refine final postprocess request is reusable

Promoted the Area_judge restart-refine final land/ocean `mask_postproc` request construction into `rust/earthmesh_cli::restart_refine_final_postproc_request`. The library now owns the no-request gating, land selected-domain bounds/state copy, ocean `mask_sea_ratio` and `num_vertex` handoff, and unsupported-mesh error. The source-state and landtype-source CLI branches now only map the typed request to their axes-specific borrowed runner options, removing duplicated postprocess `mesh_type` branching from `main.rs`.

### 2026-06-14: restart-refine final contain options are reusable

Promoted the Area_judge restart-refine final `Get_Contain(0)` options construction into `rust/earthmesh_cli::restart_refine_final_contain_options`. The library now owns requested/no-request gating, restart mesh-type mapping for land/ocean/atmos/LOC, borrowed sea-land/source-axis option assembly, and the unsupported-mesh error used by restart-refine handoffs. Both source-state and landtype-source CLI restart-refine branches now reuse this API before their axes-specific final postprocess mapping.

### 2026-06-14: compact source-state restart axes are reusable

Added `MkgrdCompactSourceState::build_global_source_axes` so compact source-state refine and restart-refine handoffs reconstruct their one-based source geometry through the Rust library API instead of manually passing dimensions through the CLI front-end. The source-state direct-refine and restart-refine branches now build axes from the typed compact state and derive `MkgrdRefinePrepareSourceGridOptions` through `GlobalSourceAxes::refine_prepare_source_grid`, keeping another `mkgrd.F90` source-state recovery detail out of `main.rs`.

### 2026-06-14: landtype preprocess source-grid options are reusable

Added `LandtypeDataPreprocessReport::refine_prepare_source_grid` and carried `gridnum_perdegree` in the data_preprocess report so landtype-file refine/restart handoffs can borrow source axes through a typed Rust library API. The restart-refine landtype-source CLI branch now uses this helper instead of manually assembling `MkgrdRefinePrepareSourceGridOptions`, moving another `MOD_data_preprocess.F90` / `mkgrd.F90` source-state handoff detail out of `main.rs`.

### 2026-06-14: restart Area_judge options are reusable

Added `GlobalSourceAxes::restart_area_judge_options` so the mask-restart Area_judge continuation can borrow source axes through a typed Rust library API. The explicit `--run-mask-restart-area-judge` CLI branch now passes the library-built options into `run_mkgrd_mask_restart_area_judge_namelist` instead of manually expanding `lon_vertex`, `lat_vertex`, `lon_i`, `lat_i`, and source dimensions in `main.rs`.

### 2026-06-14: data-preprocess final postprocess runner options are reusable

Promoted the data_preprocess-derived final postprocess request-to-runner-options mapping into `rust/earthmesh_cli::data_preprocess_source_state_final_postproc_options`. The direct `--run-refine-landtype-source` CLI branch now asks the library to convert typed land/ocean final postprocess requests into borrowed `MkgrdFinalDomainPostprocOptions`, so `main.rs` no longer needs to unpack selected land-domain bounds or ocean `num_vertex` handoffs for this migrated `mkgrd.F90` final-domain path.

### 2026-06-14: data-preprocess final contain write is reusable

Promoted the direct `--run-refine-landtype-source` final-domain contain write into `rust/earthmesh_cli::write_data_preprocess_source_state_final_domain_contain_options`. The library now owns the Area_judge payload generation, NetCDF write, and `Get_Contain(0)` contain-option assembly for data_preprocess-derived source-state handoffs; the CLI only supplies the output path and forwards typed options into the migrated final-domain execution.

### 2026-06-14: data-preprocess final-domain handoff runner is reusable

Promoted the direct `--run-refine-landtype-source` final-domain execution composition into `rust/earthmesh_cli::run_mkgrd_refine_loop_execution_with_data_preprocess_final_domain_handoff`. The library now composes data_preprocess Area_judge payload writing, final `Get_Contain(0)` options, final land/ocean postprocess request mapping, and the migrated refine-loop final handoff in one API; the CLI branch no longer assembles contain/postprocess options by hand.

### 2026-06-14: data-preprocess calculated-refine source is reusable

Promoted the direct `--run-refine-landtype-source` iter-zero calculated-refine Area_judge construction into `rust/earthmesh_cli::data_preprocess_source_state_calculated_refine_from_prepare`. The library now reads the prepared mkgrd/read_nl runtime refine controls, validates `mask_refine_ndm(0)`, and expands the data_preprocess source-state axes/domain into the calculated refine report; the CLI no longer parses `RefineConfig` or hand-expands `build_area_judge_calculated_refine_fortran_indexed` arguments for this path.

### 2026-06-14: data-preprocess source-branch options callback is reusable

Promoted the direct `--run-refine-landtype-source` source-branch option assembly into `rust/earthmesh_cli::with_data_preprocess_source_state_refine_source_branch_options_from_prepare`. The library now owns the calculated-refine report lifetime, calculated/specifed source-branch option selection, and data_preprocess source-state borrowing for `Area_judge_refine/Get_Contain/GetRef`; the CLI only supplies a closure that runs the migrated executor/final handoff.

### 2026-06-14: data-preprocess refine execution runner is reusable

Promoted the direct `--run-refine-landtype-source` refine execution composition into `rust/earthmesh_cli::run_mkgrd_refine_loop_execution_with_data_preprocess_source_state`. The library now seeds the first refine-loop gridfile from the initial mesh, constructs data_preprocess source-branch executors, and runs the migrated refine/final-domain handoff through one reusable API; the CLI no longer writes the first refine input or builds the migrated executor directly for this path.

### 2026-06-14: data-preprocess source-state config expansion is reusable

Promoted the direct `--run-refine-landtype-source` mkgrd config expansion into `rust/earthmesh_cli::build_mkgrd_data_preprocess_source_state_from_config_fortran_indexed`. The library now owns `NL%landtype_file`, `NL%gridnum_perdegree`/override selection, domain flags, `NL%mode_grid` to `num_vertex`, and source first-triangle wiring before building the typed data_preprocess source state; the CLI no longer manually expands those namelist fields for this path.

### 2026-06-14: data-preprocess landtype-source namelist runner is reusable

Promoted the direct `--run-refine-landtype-source` orchestration into `rust/earthmesh_cli::run_mkgrd_refine_landtype_source_namelist`. The library now composes parsed mkgrd config, data_preprocess source-state construction, gridinit, refine prepare, first-grid seeding, migrated source execution, and final-domain handoff into one report; the CLI branch now only invokes the runner and formats its report fields.

### 2026-06-14: compact source-state namelist runner is reusable

Promoted the direct `--run-refine-source-state` compact source-state orchestration into `rust/earthmesh_cli::run_mkgrd_refine_compact_source_state_namelist`. The library now owns compact source-state parsing, global source-axis reconstruction, calculated-refine metadata wiring, final-domain contain/postprocess option construction, final-domain Area_judge payload writing, and migrated top-level refine execution; the CLI branch now only invokes the runner and formats its report fields.

### 2026-06-14: compact source-state restart-refine options are reusable

Promoted compact source-state restart-refine option construction into `rust/earthmesh_cli::read_mkgrd_compact_restart_refine_source_state` and `MkgrdCompactRestartRefineSourceState::area_judge_restart_refine_loop_options`. The library now owns source-state parsing, global source-axis reconstruction, source-grid derivation, and `MkgrdAreaJudgeRestartRefineLoopOptions` assembly for restart Area_judge refine handoffs; the CLI no longer manually expands those fields before invoking the migrated restart-refine runner.

### 2026-06-14: compact source-state restart-refine runner is reusable

Promoted the direct `--run-mask-restart-area-judge-refine` compact source-state orchestration into `rust/earthmesh_cli::run_mkgrd_restart_refine_compact_source_state_namelist`. The library now owns mkgrd config parsing, restart Area_judge grid discovery, compact source-state/axis reconstruction, final `Get_Contain(0)` option construction, land/ocean final postprocess option mapping, and standard migrated restart-refine execution; the CLI branch now only validates required paths, invokes the runner, and formats the report.

### 2026-06-14: landtype-source restart-refine runner is reusable

Promoted the direct `--run-mask-restart-area-judge-refine-landtype-source` orchestration into `rust/earthmesh_cli::run_mkgrd_restart_refine_landtype_source_namelist`. The library now owns mkgrd config parsing, `NL%landtype_file` data_preprocess reading, source-grid derivation, restart Area_judge grid discovery, final `Get_Contain(0)` option construction, land/ocean final postprocess option mapping, and standard migrated restart-refine execution; the CLI branch now only validates the initial gridfile, invokes the runner, and formats the report.

### 2026-06-14: global-source mask-restart Area_judge runner is reusable

Promoted the direct `--run-mask-restart-area-judge` source-axis expansion into `rust/earthmesh_cli::run_mkgrd_mask_restart_area_judge_global_source_namelist`. The library now owns global source-axis reconstruction from compact dimensions and optional final `Get_Contain(0)`/`mask_postproc` continuation for this restart path; the CLI branch now only validates scalar inputs, invokes the runner, and formats the report.

### 2026-06-14: global-source refine passthrough runner was a temporary smoke path

Promoted the direct `--run-refine-passthrough` source-axis expansion and smoke executor into the library as an intermediate migration step. This path has since been retired as a production surface: ordinary specified-refine dispatch now routes through the OLAM direct implementation described below, and the file-copying passthrough executor has been removed to avoid confusing smoke behavior with real refinement.

### 2026-06-18: refine public dispatch routes to OLAM direct

Replaced the reusable passthrough/synthetic-source refine entry points with OLAM direct dispatch. `run_mkgrd_refine_passthrough_global_source_namelist` and `run_mkgrd_atmos_specified_refine_global_source_namelist` now preserve their compatibility signatures but return `MkgrdOlamSpecifiedRefineRunReport` from `run_mkgrd_olam_specified_refine_global_source_namelist`; the legacy source-grid dimension arguments are no longer used to drive a refine-loop executor. The plain `run_mkgrd_top_level_namelist` dispatcher now also routes non-restart `NL%refine=.true.` namelists for supported mesh types directly to the OLAM Delaunay/Voronoi refine branch instead of falling back to gridinit-only output.

The obsolete injected-executor passthrough wrappers and their tests were removed after this routing change. Remaining `run_mkgrd_refine_loop_*migrated_executor*` APIs are restart/data-preprocess compatibility surfaces for the older Area_judge/GetRef/refine-loop handoff and should not be used for new ordinary specified-region refinement work.

### 2026-06-14: default restart-refine dispatch runner is reusable

Promoted the no-explicit-mode restart-refine handoff dispatch into `rust/earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff`. The library now owns reading/parsing the mkgrd namelist, applying default source-state or landtype restart-refine handoff classification, invoking the corresponding restart-refine runner, or falling back to normal top-level dispatch; the CLI no longer mutates execution-mode flags for this default path.

### 2026-06-14: configured global-source Area_judge continuation is reusable

Added `rust/earthmesh_cli::run_mkgrd_mask_restart_area_judge_configured_global_source_namelist` for the mask-restart ContinueMkgrd Area_judge path. The runner now reconstructs regular global source dimensions from `NL%gridnum_perdegree` (`360*gpd` by `180*gpd`) and delegates to the reusable global-source Area_judge continuation; the CLI accepts `--run-mask-restart-area-judge` without manual `--source-*` dimensions unless the caller wants to override all three values explicitly.

### 2026-06-14: explicit restart-refine initial gridfile inference is wired

Extended the explicit `--run-mask-restart-area-judge-refine` and `--run-mask-restart-area-judge-refine-landtype-source` CLI branches to reuse `rust/earthmesh_cli::infer_restart_refine_initial_gridfile_from_config` when `--restart-refine-initial-gridfile` is omitted. Both source-state and landtype restart-refine paths can now recover the standard `<case>/gridfile/gridfile_NXP####_01_<mode_grid>.nc4` handoff path from the mkgrd namelist instead of requiring users to restate Fortran module state on the command line.

### 2026-06-14: default non-ocean restart Area_judge dispatch executes

Extended the option-free `rust/earthmesh_cli::run_mkgrd_top_level_namelist` dispatcher so `mask_restart=.true.`, `mask_patch_on=.false.`, non-ocean `ContinueMkgrd` cases now run the configured global-source restarted `Area_judge` continuation instead of returning only `MaskRestartPlan`. This keeps the normal CLI entry point moving through the same Rust Area_judge restart path already used by the explicit `--run-mask-restart-area-judge` mode while still leaving final non-ocean postprocess gated on a proven `num_vertex` handoff.

### 2026-06-14: default non-ocean restart final postprocess can run with supplied boundary

Extended `rust/earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff` so the no-explicit-mode restart path forwards a supplied `mask_postproc_num_vertex` into the configured global-source `Area_judge` continuation for non-ocean `ContinueMkgrd` cases. The default CLI/library entry can now continue through restarted `Area_judge`, final `Get_Contain(0)`, and land `mask_postproc` when the caller supplies the persisted legacy `num_vertex` boundary, instead of stopping at an Area_judge-only continuation.

### 2026-06-14: default land restart final postprocess infers persisted boundary

Added `maybe_infer_mask_restart_non_ocean_num_vertex_from_config` so top-level non-ocean `mask_restart ContinueMkgrd` can recover the legacy `num_vertex` boundary from the existing final contain file's `ustr_ii` row count before rerunning final `Get_Contain(0)`. The default dispatcher now uses this persisted-boundary recovery for landmesh restart cases and continues through final land `mask_postproc` without requiring a manual `--mask-postproc-num-vertex`; if the contain file is absent, it safely preserves the existing Area_judge-only continuation instead of guessing from `mode_grid`.

### 2026-06-14: default land restart final postprocess is reported by CLI

Updated the default `earthmesh_cli <mkgrd.nml>` report formatting for `MaskRestartAreaJudge` so an inferred non-ocean final postprocess handoff prints the generated final contain path and mesh-specific postprocess outputs, matching the explicit restart Area_judge mode. This makes the Rust-owned landmesh `mask_restart ContinueMkgrd` final `Get_Contain(0)`/`mask_postproc` continuation visible to users instead of reporting only the restarted Area_judge grid.

### 2026-06-14: restart-refine final postprocess infers persisted boundary

Extended the migrated restart-refine compact source-state and landtype-source runners so non-ocean final `Get_Contain(0)`/`mask_postproc` handoffs can recover the legacy `num_vertex` boundary from the existing final contain file when `mask_postproc_num_vertex` is not supplied. The runners still prefer an explicit boundary and still skip final postprocess when no persisted contain exists, avoiding unsafe guesses from `mode_grid` while allowing Rust-owned restart-refine handoffs to resume from Fortran-compatible persisted state.

### 2026-06-14: explicit Area_judge restart final postprocess infers persisted boundary

Extended `run_mkgrd_mask_restart_area_judge_configured_global_source_namelist` so the explicit `--run-mask-restart-area-judge` path matches the default dispatcher for non-ocean `ContinueMkgrd` restarts. When `--mask-postproc-num-vertex` is omitted, the runner now recovers the final postprocess boundary from the existing final contain file and continues through final `Get_Contain(0)` plus land `mask_postproc`; if that persisted contain is absent, the explicit path still remains an Area_judge-only continuation instead of guessing.

### 2026-06-14: explicit Area_judge source override infers persisted boundary

Moved non-ocean persisted-boundary recovery into `run_mkgrd_mask_restart_area_judge_global_source_namelist` so both configured global-source and explicit `--source-gridnum-perdegree/--source-nlons/--source-nlats` restart `Area_judge` continuations can resume final `Get_Contain(0)` plus land `mask_postproc` without a manual `--mask-postproc-num-vertex`. Existing behavior is preserved when the contain file is absent: the path remains an Area_judge-only continuation instead of guessing the legacy boundary.

### 2026-06-14: ocean Area_judge restart final postprocess infers persisted boundary

Extended the explicit global-source `mask_restart Area_judge` continuation so oceanmesh final `Get_Contain(0)` plus `mask_postproc_ocean` can recover its legacy `num_vertex` boundary from the existing final contain file when `--mask-postproc-num-vertex` is omitted. The global-source runner now uses the ocean-specific persisted-boundary helper for ocean cases and the non-ocean helper for land/earth cases, reducing another manual Fortran module-state argument from the Rust CLI handoff.

### 2026-06-14: ocean Area_judge restart preserves area-only fallback

Added an optional ocean persisted-boundary helper for explicit `mask_restart Area_judge` continuations. Oceanmesh Area_judge handoffs now infer `num_vertex` and run final ocean postprocess only when the persisted final contain file exists; if that file is absent, the Rust runner preserves the previous Area_judge-only continuation rather than failing while trying to recover a boundary that has not been written yet. The strict ocean inference helper remains available for entry points where final postprocess is mandatory.

### 2026-06-14: default restart-refine dispatcher forwards runtime state

Added `source_branch_reports()` and `runtime_state()` accessors to `MkgrdTopLevelDefaultRestartRefineRunReport`, plus dispatch-level runtime-state forwarding for the gridinit branch. The default restart-refine top-level API now exposes source-branch `Get_Contain` reports and the latest `EarthmeshRuntimeState` for compact source-state and landtype-source handoffs, so mesh-count writeback no longer disappears behind the default restart-refine dispatcher enum.

### 2026-06-14: direct restart-refine reports forward runtime state

Added `source_branch_reports()` and `runtime_state()` accessors to the direct compact source-state and landtype-source restart-refine report types. Library callers can now inspect migrated `Get_Contain` source-branch reports and the latest `EarthmeshRuntimeState` without reaching through nested execution fields; the default restart-refine top-level report now delegates to those report-level accessors.

### 2026-06-14: direct refine reports forward runtime state

Added `source_branch_reports()` and `runtime_state()` accessors to the direct compact source-state and landtype-source refine report types. Non-restart library callers now get the same flat API surface as the top-level namelist and restart-refine reports, preserving migrated `Get_Contain` source-branch reports and latest `EarthmeshRuntimeState` across direct reusable runner boundaries.

### 2026-06-14: default mask-restart reports forward final contain counters

Added `final_domain_contain_runtime_counts()` to the top-level default restart-refine and dispatch report APIs. Default `mask_restart ContinueMkgrd` Area_judge continuations now expose the final-domain `Get_Contain(0)` runtime counters, including the recovered `previous_num_vertex`, without forcing callers to traverse nested postprocess fields.

### 2026-06-14: direct Area_judge reports forward final contain counters

Added `final_domain_contain_runtime_counts()` to the direct mask-restart Area_judge postprocess and global-source report APIs. Explicit `ContinueMkgrd` Area_judge runs now expose final-domain `Get_Contain(0)` runtime counters, and the top-level dispatch accessor delegates to the direct report instead of reaching through nested postprocess fields.

### 2026-06-14: restart-refine ocean final postprocess infers persisted boundary

Extended the compact source-state restart-refine runner so oceanmesh final `Get_Contain(0)`/`mask_postproc_ocean` handoffs can recover `num_vertex` from the persisted final contain file when `mask_postproc_num_vertex` is omitted. The same optional ocean inference is wired through the landtype-source restart-refine runner path, preserving the no-contain fallback while covering another `mkgrd.x` restart/final-postprocess combination.

### 2026-06-14: direct ocean mask-restart infers persisted boundary

Added `run_mkgrd_mask_restart_ocean_inferred_namelist`, an option-free library wrapper for the direct `oceanmesh` `mask_restart` postprocess branch. The runner recovers the legacy `num_vertex` boundary from the persisted final contain file and then reuses the typed ocean postprocess executor, so reusable Rust callers no longer need to manually restate the Fortran module-state boundary for this mandatory ocean restart path.

### 2026-06-14: hydro close-mask point-touch dissolve guard

Tightened native hydro/coast close-mask dissolve so same-class polygons that only touch at a single vertex are no longer collapsed into one coarse bounding box. The dissolve gate now allows the old bbox fallback only when bounding boxes overlap with positive area; zero-area contact must be proven by a real rectilinear, containment, or shared-edge polygon union. This preserves separate close-mask rings for point-only hydro/coast contacts and avoids over-refining the square envelope between diagonally adjacent corridors.

### 2026-06-14: hydro close-mask rectangular holes split into rings

Extended the native hydro/coast GeoJSON close-mask exporter beyond exterior-only polygons for a first hole topology: an axis-aligned rectangular Polygon exterior with one axis-aligned rectangular interior ring is decomposed into multiple rectangular close-mask rings around the hole. This avoids refining the hole interior as part of a single outer-envelope mask while preserving the legacy close-mask format's single-ring constraint. Non-rectangular or multi-hole polygons still fall back to the previous exterior-ring behavior until more general polygon-with-holes decomposition is migrated.

### 2026-06-14: hydro close-mask multiple rectangular holes split into grid rings

Extended the native hydro/coast GeoJSON close-mask exporter from one rectangular interior ring to multiple axis-aligned rectangular holes. For this supported topology, Rust now partitions the exterior rectangle by all hole edges and emits close-mask rectangles only for grid cells whose centers are inside the exterior and outside every hole, avoiding the previous exterior-only fallback that refined hole interiors. Non-rectangular holes and more complex multi-component polygon topology remain explicit follow-up gaps.

### 2026-06-14: hydro close-mask rectilinear holes split into grid rings

Generalized the rectangular-hole close-mask path for axis-aligned rectangular exteriors with rectilinear interior rings. Rust now partitions the exterior by every hole vertex x/y coordinate and emits only cell rectangles whose centers are outside all hole rings, so non-rectangular orthogonal holes such as L-shaped exclusions no longer collapse to one exterior mask that refines the hole interior. Non-rectilinear holes and more complex multi-component topology remain follow-up work.

### 2026-06-14: hydro close-mask upward triangular holes split into rings

Added a bounded non-rectilinear hole topology to the native hydro/coast close-mask exporter: an axis-aligned rectangular exterior with one fully contained triangular interior ring whose base is horizontal and whose apex points upward. Rust now decomposes that case into bottom, top, left, and right close-mask rings that trace the triangular hole boundary, avoiding the previous exterior-only fallback that refined the triangular hole interior. Other non-rectilinear holes and more complex polygon topology remain explicit follow-up gaps.

### 2026-06-14: hydro close-mask horizontal-base triangular holes support both orientations

Generalized the bounded triangular-hole close-mask decomposition so an axis-aligned rectangular exterior with one fully contained triangular interior ring can be handled when the horizontal edge is either the lower base or the upper edge. Rust now mirrors the four-ring split for upward and downward triangular holes, tracing the apex in either orientation and avoiding the exterior-only fallback that would refine the triangular hole interior. Arbitrary slanted-edge/non-horizontal-base holes remain follow-up work.

### 2026-06-14: hydro close-mask vertical-base triangular holes split into rings

Extended the bounded triangular-hole close-mask decomposition from horizontal-edge triangles to vertical-edge triangles. For an axis-aligned rectangular exterior with one fully contained triangular interior ring whose base edge is vertical and whose apex points left or right, Rust now emits left/right rectangle masks plus top/bottom polygon masks that trace the triangular hole boundary. This avoids the exterior-only fallback for another common non-rectilinear hole orientation while still leaving arbitrary non-axis-aligned holes and complex topology as explicit follow-up work.

### 2026-06-14: hydro close-mask slanted triangular holes split into slab rings

Generalized the bounded single-triangle close-mask hole support beyond horizontal or vertical triangle edges. For an axis-aligned rectangular exterior with one fully contained triangular interior ring, Rust now falls back to a vertical slab partition when the axis-edge special cases do not apply: it splits on the exterior and triangle vertex x coordinates, computes the triangle y-span at each slab boundary, and emits lower/upper slab rings outside the triangular hole. This avoids refining the interior of slanted triangular holes while keeping broader arbitrary polygon holes and multi-hole non-rectilinear cases as explicit follow-up work.

### 2026-06-14: hydro close-mask multiple triangular holes split into slab rings

Extended the triangular-hole close-mask decomposition from one interior triangle to multiple contained triangular holes. Rust now builds one vertical slab partition from the exterior and all triangle vertex x coordinates, computes each active triangular hole's y-span at slab boundaries, sorts active spans, and emits the outside slab segments as separate close-mask rings. This covers separated multi-triangle non-rectilinear holes without falling back to one exterior mask, while overlapping/stacked non-rectilinear holes and general non-triangular hole topology remain follow-up gaps.


### 2026-06-14: hydro close-mask single-span polygon holes split into slab rings

Extended the native hydro/coast close-mask hole exporter from triangular non-rectilinear interiors to bounded single-span polygon holes such as convex diamond/quadrilateral exclusions inside an axis-aligned rectangular exterior. Rust now partitions on exterior and hole vertex x coordinates, computes each active hole y-span at slab boundaries, and emits the lower/upper outside slab rings so the hole interior is not refined. Multi-span concave holes, overlapping/stacked non-rectilinear holes, and more complex multi-component topology remain explicit follow-up gaps.


### 2026-06-14: hydro close-mask overlapping single-span holes merge slab spans

Extended the bounded single-span polygon-hole slab exporter so overlapping non-rectilinear interiors such as overlapping diamond holes no longer force an exterior-mask fallback. Within each vertical slab, Rust now merges active hole spans that overlap at both slab boundaries before emitting the outside lower/upper close-mask rings. This covers stacked/overlapping single-span exclusions while still rejecting ambiguous crossing-order spans and leaving multi-span concave holes plus broader polygon topology for follow-up work.


### 2026-06-14: hydro close-mask concave multi-span holes preserve notch slabs

Generalized the bounded non-rectilinear hole slab exporter from one y-span per vertical slab to multiple ordered y-spans. Concave interior rings with a slanted notch can now preserve the open notch as its own close-mask slab instead of over-excluding everything between the lowest and highest ring intersections. The implementation samples slab boundaries just inside the slab when an exact vertex boundary changes span count, keeping single-span diamond/triangle cases and overlapping span merges intact while still leaving crossing-order slabs and fully general polygon topology as follow-up work.


### 2026-06-14: hydro close-mask crossing-order slabs split at hole-edge crossings

Extended the bounded non-rectilinear hole slab exporter for crossing-order cases where two interior hole spans exchange vertical order inside a slab. Rust now detects intersections between low/high boundary edges from different non-rectilinear holes, inserts those crossing x coordinates into the slab partition, and then applies the existing ordered span merge on the smaller slabs. This avoids falling back to one exterior close-mask ring for X-shaped overlapping non-rectilinear exclusions while still leaving multi-component polygon unions and fully general topology as follow-up work.


### 2026-06-14: hydro close-mask rectilinear multi-component unions preserve holes

Extended the native hydro/coast close-mask dissolve path so rectilinear multi-component unions that would create an interior hole are not collapsed into one coarse outer ring. The dissolve merge can now return multiple close-mask specs for rectilinear components when a precise single-ring merge would drop a component or over-refine an interior gap; contained-rectilinear checks now compare decomposed cells instead of trusting bbox-level point-in-ring containment. This preserves donut-like hydro/coast refinement masks while leaving fully general multi-component polygon topology as follow-up work.


### 2026-06-14: hydro close-mask non-rectilinear dissolve requires real union evidence

Tightened the native hydro/coast close-mask dissolve gate for non-rectilinear multi-component inputs. Same-class masks whose bounding boxes overlap with positive area are no longer automatically collapsed into one coarse envelope; Rust now requires a real rectilinear, containment, shared-edge, or crossing-edge union path before dissolving. This preserves separate close-mask rings for bbox-overlapping but geometrically disjoint slanted polygons and avoids over-refining the gap between components. Fully general non-rectilinear multi-component polygon topology remains a follow-up gap.


### 2026-06-14: hydro close-mask non-axis exterior holes split into slabs

Extended the native hydro/coast close-mask hole exporter beyond axis-aligned rectangular exteriors for a bounded arbitrary-polygon case. Rust now decomposes non-axis-aligned exterior rings with contained non-rectilinear holes into vertical slab close-mask rings, using exterior and hole y-spans at each slab boundary so the hole interior is not refined by one coarse exterior mask. This moves another polygon-with-holes path out of the Fortran-era preprocessing gap while still leaving fully general non-rectilinear multi-component unions and more complex topology as follow-up work.


### 2026-06-14: hydro close-mask GeometryCollection polygons are parsed

Extended the native hydro/coast close-mask GeoJSON reader to recurse into `GeometryCollection` geometries before generating EarthMesh `.nml` masks. Polygon and MultiPolygon members embedded by GIS preprocessing are now exported through the same Rust close-mask path instead of being silently dropped, while non-polygon members remain ignored. This improves the v3 data-source boundary for hydro/coast layers without reintroducing Python/Fortran preprocessing.


### 2026-06-14: hydro close-mask single Feature sources are parsed

Extended the native hydro/coast close-mask GeoJSON input boundary so a top-level `Feature` with class properties is accepted in addition to `FeatureCollection`. Single-feature GIS exports now flow through the same Rust close-mask `.nml` generation path instead of being ignored because they lack a `features` array. This keeps more v3 hydro/coast source-layer variants inside the Rust preprocessing path.


### 2026-06-14: v3 hydro GeoJSON summary accepts Rust exporter class keys

Aligned the v3 hydro/coast source summary reader with the Rust MERIT/CaMa source exporters. Hydro GeoJSON summaries now collect classes from `hydro_class`, `river_class`, and `mask_class`, so MERIT river masks written with `mask_class=R2/R3` and CaMa reach-derived layers using `river_class` are visible at the Rust data_preprocess boundary instead of producing empty class summaries. This removes another mismatch between the new Rust source exporters and the v3 source-state abstraction.


### 2026-06-14: v3 GeoJSON summaries use feature-scoped class parsing

Replaced the v3 hydro/coast GeoJSON summary reader's string-level class scan with the existing Rust JSON parser and feature-scoped `Feature.properties` traversal. Summary feature counts and class sets now come only from actual GeoJSON `Feature` objects, so collection-level metadata or other non-feature JSON properties can no longer pollute the Rust data_preprocess source-state summary. This tightens the native v3 source boundary before broader hydro/coast mesh generation consumes those summaries.

### 2026-06-14: restart-refine earthmesh final handoff is typed

Extended the mkgrd restart-refine final-domain handoff so `mesh_type='earthmesh'` is no longer rejected by the Rust helper layer. Restart-refine final `Get_Contain(0)` now maps earthmesh to the same `Loc` containment kind used by the migrated explicit Area_judge earth final-postprocess branch, and the final `mask_postproc_Earth` request carries selected-domain bounds plus `mask_sea_ratio` through a typed `EarthFromFinalGrid` option. The handoff reads the copied final gridfile at postprocess time to recover `num_mp_step`/`sjx_points`, avoiding another legacy `consts_coms` mesh-count argument while preserving land/ocean behavior.

### 2026-06-14: default patch-on mask-restart continues through Area_judge final postprocess

Extended the option-free top-level mkgrd dispatcher so `mask_restart=.true.` with `mask_patch_on=.true.` no longer stops at patch `Mask_make` when the restarted `Area_judge` grid already exists. The default handoff now runs the same configured global-source restart continuation used by explicit modes, preserving patch preprocessing, restarted `Area_judge`, persisted-boundary recovery, final `Get_Contain(0)`, and land final `mask_postproc` output generation. If the restart Area_judge grid is absent, the dispatcher still preserves the older patch-only preprocessing behavior instead of guessing a continuation target.

### 2026-06-14: data_preprocess source-state supports earthmesh final handoff

Extended the data_preprocess-derived source-state final-domain handoff so `mesh_type='earthmesh'` now builds both final `Get_Contain(0)` options and final `mask_postproc_Earth` runner options. The source-state path maps earthmesh containment to the Rust `Loc` containment kind and derives selected-domain bounds from the same source sea/land matrix used by landmesh, while `EarthFromFinalGrid` recovers final mesh counts after the result gridfile is copied. This removes another land/ocean-only assumption from the Rust source-state boundary needed by future sea-land integrated CoLM/EarthMesh workflows.

### 2026-06-14: default source-state restart-refine reports inferred final postprocess

Extended the default `earthmesh_cli <mkgrd.nml>` restart-refine binary path for compact source-state inputs so the emitted run summary identifies `restart_refine_source=source_state` and exposes final `Get_Contain(0)` plus final `mask_postproc` outputs when the persisted contain boundary already exists. This makes the option-free source-state restart-refine handoff observable at the same level as the landtype-source branch while preserving automatic `num_vertex` recovery from the existing contain file.

### 2026-06-14: mask_restart Area_judge dispatch forwards runtime state

Extended the Rust top-level `mkgrd` dispatch report for the restarted `Area_judge` continuation so it no longer drops runtime state at the `MaskRestartAreaJudge` enum boundary. The restarted Area_judge run report now carries an `EarthmeshRuntimeState` built from the parsed namelist config, and `MkgrdTopLevelDispatchRunReport::runtime_state()` forwards it to callers. This removes another `consts_coms`-style implicit-state gap for restart/postprocess orchestration while preserving existing final `Get_Contain(0)` and mask_postproc behavior.

### 2026-06-14: mask_restart patch and ocean dispatch forward runtime state

Extended the remaining executable `mask_restart` top-level dispatch branches so patch preprocessing and direct ocean postprocess reports also carry `EarthmeshRuntimeState` derived from the parsed namelist config. `MkgrdTopLevelDispatchRunReport::runtime_state()` now forwards state for patch, ocean, and restarted Area_judge branches, leaving plan-only reports as the only intentional `None`. This closes another top-level `consts_coms` runtime-state forwarding gap while keeping branch-specific file outputs unchanged.

### 2026-06-14: mask_restart final Get_Contain writes runtime mesh counts

Extended the restarted `Area_judge` final-domain handoff so final `Get_Contain(0)` no longer only returns `GetContainRuntimeCounts` in the contain report. The top-level runtime state now records the current mesh cell/vertex counts into `num_mp_step(1)` and `num_wp_step(1)` through `EarthmeshRuntimeState::record_mesh_counts_for_step`, matching the legacy `consts_coms` counter update without relying on hidden module globals.

### mkgrd default landtype refine dispatch

Extended the default no-explicit-mode `earthmesh_cli <mkgrd.nml>` path for non-restart `NL%refine=.true.` namelists that provide `NL%landtype_file`. The top-level default handoff now runs the migrated data-preprocess landtype-source refine stack, prints `refine_source=landtype_file`, and reports the executed refine steps/source branches instead of stopping after the initial gridinit output. This closes one more `mkgrd.x` production-entry gap where a Fortran-style namelist could already describe the source state without a debug/source-state flag.

### data_preprocess hydro_class close-mask metadata

Aligned the Rust hydro/coast close-mask NML exporter with the v3/MERIT source summary vocabulary by accepting `hydro_class` feature properties alongside the legacy `river_class` and `mask_class` keys. MERIT/CaMa-derived hydro GeoJSON can now flow directly from source-summary style metadata into EarthMesh close-mask refine NML generation without a separate property-renaming preprocessing step.

### 2026-06-14: direct top-level patch-on restart continues final postprocess

Extended the reusable Rust `run_mkgrd_top_level_namelist` dispatcher for `mask_restart=.true.` plus `mask_patch_on=.true.` when the restarted `Area_judge` grid already exists. The direct library entry now follows the same migrated continuation as the option-free binary/default handoff: apply patch preprocessing, rerun configured global-source restarted `Area_judge`, infer the persisted final contain boundary, and continue through final `Get_Contain(0)` plus land `mask_postproc`. Patch-only preprocessing remains the fallback when no restarted `Area_judge` grid exists.

### 2026-06-14: hydro close-mask non-rectilinear multi-component gap preservation

Extended the native hydro/coast close-mask dissolve guard for a bounded non-rectilinear multi-component topology case. When several same-class non-rectilinear polygons form a ring around an interior gap, Rust now rejects the unsafe single-ring/bbox-style dissolve and preserves the coverage as multiple close-mask specs. This prevents hydro/coast refinement from filling an interior gap that the `.nml` single-ring format cannot represent precisely, while still keeping existing crossing-edge and chained non-rectilinear merges when they produce a safe ring. Exact polygon-union rings with holes remain a follow-up topology gap.

### 2026-06-14: mask_restart plan reports preserve runtime state

`MkgrdMaskRestartPlanReport` now carries an `EarthmeshRuntimeState`, and `MkgrdTopLevelDispatchRunReport::runtime_state()` returns it even for `MaskRestartPlan` branches. The executable mask_restart patch and ocean runners now clone the plan-owned state instead of reconstructing fresh config-only state locally. This closes another `consts_coms` migration seam: deferred or plan-only restart continuations no longer drop explicit Rust runtime/config state at the API boundary.

### 2026-06-14: mask_restart runtime state records remask step

`plan_mkgrd_mask_restart_namelist` now initializes its `EarthmeshRuntimeState.step` from the Fortran restart handoff step `max_iter + 1` instead of leaving the state at the default initial-grid step. The plan also rejects non-positive remask steps at the Rust state boundary. Because executable patch/ocean restart runners and `MkgrdTopLevelDispatchRunReport::runtime_state()` reuse the plan-owned state, plan-only and executable mask_restart branches now preserve the same explicit remask step that Fortran previously kept in module-global state.

### 2026-06-14: mask_restart runtime state mirrors refine override

The Rust mask_restart plan now builds its `EarthmeshRuntimeState` from a restart-specific config where `refine` is forced to `false`, matching the top-level `mkgrd.F90` restart branch even if `NL%refine=.true.` was present in the namelist. The original parsed config remains available on the plan, while execution-facing runtime state now carries the Fortran module-global override explicitly together with the remask step.

### 2026-06-14: mask_restart Area_judge atmos MPAS-Simple final postprocess

Extended the mask_restart Area_judge final handoff to `atmosmesh` for `MPAS-Simple`: the Rust runner now writes the final Atmos `Get_Contain(0)` contain file and then invokes the existing MPAS-Simple atmosphere writer to produce `result/MPASOUT_NXP####_global_Simple.nc4`. This covers another `mkgrd.F90` ContinueMkgrd final postprocess combination beyond land/ocean/earth.

### 2026-06-14: mask_restart Area_judge atmos full MPAS final postprocess

Extended the `atmosmesh` mask_restart Area_judge final handoff beyond `MPAS-Simple` to the full `MPAS` branch. The Rust runner now carries the restart remask step into the MPAS nominal resolution calculation, reads the final atmosphere gridfile plus cellwidth file, and writes both `result/MPASOUT_NXP####_global.nc4` and `result/MPASOUT_NXP####_global.graph.info` through the migrated MPAS writer path. This closes another Fortran `mask_postproc_Atmos` output-format combination inside the `mkgrd.F90` ContinueMkgrd restart path.

### 2026-06-14: restart-refine atmos MPAS-Simple final handoff

Extended the generic refine-loop final-domain handoff so `atmosmesh` postprocessing can run through the MPAS branch instead of requiring a land/ocean/earth domain `mask_postproc` plan. `MkgrdFinalDomainPostprocOptions::Atmos` now dispatches `MPAS-Simple` through the migrated atmosphere writer after the final gridfile is copied into `result/gridfile_NXP####_<mode>.nc4`, and restart-refine final postprocess request construction now accepts `atmosmesh`. This moves another refine-enabled restart final postprocess combination out of the Fortran-only `mask_postproc_Atmos` path.

### 2026-06-14: data_preprocess atmos final postprocess handoff

Extended the data_preprocess-derived source-state final-domain handoff so `mesh_type='atmosmesh'` is no longer dropped at the postprocess request boundary. The reusable Rust request/options helpers now carry an Atmos request with the caller's `output_format`, and the refine-loop data_preprocess runner forwards `NL%output_format` into the existing Atmos MPAS/MPAS-Simple final handoff. A new execution regression covers `data_preprocess -> atmosmesh -> MPAS-Simple`, including final Get_Contain(0) generation and MPAS-Simple output writing.

### 2026-06-14: restart-refine atmos full MPAS final handoff coverage

Added explicit regression coverage for the migrated refine-loop final-domain handoff when `mesh_type='atmosmesh'` and `output_format='MPAS'`. The test verifies the Atmos branch bypasses land/ocean domain postprocess planning, copies the final gridfile, reads the full MPAS cellwidth input, and writes both `MPASOUT_NXP####_global.nc4` and `MPASOUT_NXP####_global.graph.info`. This locks the full MPAS variant beside the existing MPAS-Simple refine handoff coverage.


### 2026-06-14: landtype-source atmos MPAS-Simple final handoff covered

Added a direct `run_mkgrd_refine_landtype_source_namelist` regression for `NL%landtype_file -> mesh_type='atmosmesh' -> output_format='MPAS-Simple'`. The test runs the production landtype-source/data_preprocess refine entry, injects the legacy final-quality cellwidth side effect through the existing executor boundary, and verifies the final Atmos MPAS-Simple postprocess report plus `MPASOUT_NXP0002_global_Simple.nc4`. The MPAS adapters now normalize generated one-based gridfiles that omit Rust legacy placeholder rows before consuming connectivity/cellwidth, so production gridinit/refine outputs can reach the migrated MPAS writer instead of only hand-built placeholder fixtures.

### 2026-06-14: final quality accepts generated one-based gridfiles

Extended the migrated `Final_Grid_Quality_Check` executor so file-backed final quality and spring adjustment normalize generated one-based gridfiles that omit Rust legacy placeholder rows before running global/regional quality kernels. A compact-gridfile regression now writes a production-shaped no-placeholder `Unstructured_Mesh_Save` input, runs the real working-state final-quality executor, and verifies the adjusted output is persisted with the legacy placeholder topology expected by downstream quality, spring, and MPAS adapter paths. This closes the handoff mismatch where MPAS postprocessing could normalize generated gridfiles but final quality itself still rejected the same compact connectivity.

### 2026-06-14: landtype-source atmos full MPAS binary handoff covered

Extended the production `earthmesh_cli <mkgrd.nml> --run-refine-landtype-source` path with regression coverage for `NL%landtype_file -> mesh_type='atmosmesh' -> output_format='MPAS'`. The test drives real gridinit, source/refine execution, final quality, full MPAS NetCDF generation, and `graph.info` writing through the binary surface. This exposed and fixed two final-quality handoff mismatches: generated one-placeholder gridfiles now normalize by adding only the missing Rust placeholder row without shifting Fortran IDs, and `Grid_Quality_Check_Global` now passes M-point triangle-center coordinates to polygon quality groups instead of reusing W-point cell coordinates. The CLI report now prints both `refine_final_postproc_mpas` and `refine_final_postproc_mpas_graph`, making the full MPAS final-postprocess side effects visible at the top-level Rust replacement boundary.

### 2026-06-14: compact source-state atmos full MPAS binary handoff covered

Extended the production `earthmesh_cli <mkgrd.nml> --run-refine-source-state <state>` path so compact source-state files can request `final_domain_postproc=atmos` for `mesh_type='atmosmesh'` and `output_format='MPAS'`. The compact source-state postprocess parser/request builder now carries Atmos through to `MkgrdFinalDomainPostprocOptions::Atmos`, and the source-state CLI report prints both `refine_final_postproc_mpas` and `refine_final_postproc_mpas_graph`. A binary regression drives real gridinit, source/refine execution, final `Get_Contain(0)`, final quality, MPAS NetCDF writing, and `graph.info` generation from a compact source-state file, matching the already-covered landtype-source full MPAS handoff at the other production source boundary.

### 2026-06-14: restart-refine compact source-state atmos full MPAS binary report covered

Extended the production `earthmesh_cli <mkgrd.nml> --run-mask-restart-area-judge-refine --restart-refine-source-state <state>` path for `mesh_type='atmosmesh'` and `output_format='MPAS'`. The restart-refine prepare step now defers specified-mask validation for `mask_restart=.true.` until the restart branch applies the requested mask sources, matching the Fortran continuation ordering. The CLI report now prints both `restart_refine_final_postproc_mpas` and `restart_refine_final_postproc_mpas_graph`, and the regression verifies the generated `MPASOUT_NXP####_global.nc4` plus `graph.info` files through the migrated Rust stack.

### 2026-06-14: restart-refine Atmos full MPAS entry coverage completed

Added binary coverage for the restart-refine `atmosmesh` full `MPAS` postprocess through the remaining production entry variants: explicit landtype-source restart-refine, default compact source-state restart-refine, and default landtype-source restart-refine. These tests verify the generated `MPASOUT_NXP####_global.nc4` and `MPASOUT_NXP####_global.graph.info` files and assert that the CLI reports both `restart_refine_final_postproc_mpas` and `restart_refine_final_postproc_mpas_graph`. This closes the observable-reporting gap around the full MPAS restart-refine handoff; broader restart/ContinueMkgrd combinations remain tracked separately.

### 2026-06-14: mask_restart land/earth patchtype artifacts reported

Extended the migrated `mask_restart` restarted `Area_judge` final-postprocess CLI reporting for land and earth branches. The binary now prints `mask_restart_postproc_patchtype=` whenever the migrated land/earth `mask_postproc` runner writes the patchtype NetCDF output, so ContinueMkgrd restart handoffs expose all adapter-facing artifacts rather than only the final gridfile. The regression first failed on the missing stdout key and now covers the inferred-boundary default land restart path.

### 2026-06-14: non-restart refine land/earth patchtype artifacts reported

Extended the non-restart refine final-postprocess CLI reporting shared by compact source-state and landtype-source entries. Land and Earth final-domain postprocess reports now expose `refine_final_postproc_patchtype=` alongside the final gridfile output, and Earth additionally exposes `refine_final_postproc_earthmesh_info=`, so adapter-facing artifacts are visible at the same top-level Rust replacement boundary as ocean OBC and atmosphere MPAS outputs. The regression first failed on the missing land patchtype stdout key and now passes through the production binary landtype-source path.

### 2026-06-14: safe convex non-rectilinear close-mask dissolve

Extended the native hydro/coast close-mask dissolve path for a bounded non-rectilinear overlap case. When two same-class convex polygon masks overlap and their combined convex hull has the same area as the polygon union, Rust now emits one dissolved close-mask ring instead of preserving two separate masks. The merge is guarded by convexity, positive intersection area, and an area equality check between the hull and `area(left)+area(right)-area(intersection)`, so point-touch, disjoint bbox overlap, interior-gap, and hole-preserving cases remain on the existing conservative paths.

### 2026-06-14: Rust CoLM coupling CSV-to-NetCDF handoff

Added a Rust-native CoLM2024/CoLM20XX coupling metadata NetCDF writer behind `earthmesh_cli --colm-coupling-csv-to-netcdf`. The new path reads the package-style `colm_coupling_cells.csv` handoff table and writes the same core `earthmesh_colm_coupling_netcdf` numeric/code schema used by the Python package exporter: `cell`, `cell_index`, center lon/lat, surface/river/coast class codes, river/coast flags, fractions, and area metadata. This moves one more v3 hydro/coast/CoLM adapter boundary out of Python-only tooling and into the Rust CLI while keeping the file as coupling metadata, not a complete CoLM forcing/restart product.

### 2026-06-14: runtime state records Get_Contain num_vertex boundary

`EarthmeshRuntimeState` now carries the legacy `num_vertex` containment boundary explicitly instead of leaving it as an implicit `consts_coms` module-global handoff. Source-branch `Get_Contain` execution records `previous_num_vertex` beside `num_mp_step`/`num_wp_step`, and the mask-restart final `Get_Contain(0)` handoff writes the same boundary into runtime state when a persisted nonzero value is available. This closes another concrete `consts_coms` seam needed by restart/refine and postprocess callers that need to preserve containment boundaries across Rust execution APIs.

### 2026-06-14: refine-loop final Get_Contain writes runtime state

Extended the generic migrated refine-loop final-domain handoff so final `Get_Contain(0)` no longer only returns containment evidence in `MkgrdRefineLoopFinalDomainHandoffReport`. When the executor provides an `EarthmeshRuntimeState`, the execution report now records the final-domain `Get_Contain` mesh counts at the final mask-postprocess step and preserves a nonzero `previous_num_vertex` boundary. This closes the same `consts_coms` counter/num_vertex seam for ordinary refine-loop final handoffs that was already closed for source-branch and mask-restart final containment paths.

### 2026-06-14: compact source-state final contain exposes runtime counters

Extended the direct migrated refine report APIs so `MkgrdRefineLoopNamelistRunReport`, `MkgrdTopLevelNamelistRunReport`, `MkgrdRefineLandtypeSourceNamelistRunReport`, and `MkgrdRefineCompactSourceStateNamelistRunReport` expose final-domain `Get_Contain` runtime counters without callers traversing nested execution fields. The compact source-state final contain options now forward the persisted source-state `num_vertex` boundary into `Get_Contain(0)`, so final contain reports and runtime-state writeback preserve the same boundary that Fortran previously carried through `consts_coms`.

### 2026-06-14: compact source-state ocean postprocess preserves num_vertex

Extended the compact source-state final-domain ocean postprocess request so it carries the persisted `num_vertex` boundary from the source-state bundle into `MaskPostprocOceanRunOptions`. This removes the remaining `num_vertex: 0` reset on the compact source-state ocean postprocess path and keeps the boundary consistent with the already migrated final-domain `Get_Contain(0)` runtime-counter handoff.

### 2026-06-14: patch-on ocean restart final postprocess covered at top level

Added coverage for the option-free top-level `mkgrd` dispatcher when `mask_restart=.true.`, `mask_patch_on=.true.`, and `mesh_type='oceanmesh'` already has a persisted restart Area_judge grid and final contain boundary. The test verifies that Rust continues through patch preprocessing, restarted Area_judge, final `Get_Contain(0)`, and ocean `mask_postproc` OBC outputs from the normal top-level dispatcher rather than relying on explicit debug-mode CLI flags.

### 2026-06-14: CoLM restart-template NetCDF handoff added

Extended the Rust CoLM package handoff beyond metadata-only coupling NetCDF. The `earthmesh_cli --colm-coupling-csv-to-netcdf` path now accepts `--restart-template-netcdf` and writes a Rust-generated CoLM restart-template NetCDF carrying cell indices, centers, land fraction, river fraction, coastal fraction, and normalized cell area from the package CSV. This gives CoLM2024/CoLM20XX a concrete Rust-side restart seed boundary while the remaining CoLM gap stays focused on full model-specific forcing/restart products and parity fixtures.

### 2026-06-14: CoLM forcing-template NetCDF handoff added

Extended the Rust CoLM package handoff with `--forcing-template-netcdf`. The `earthmesh_cli --colm-coupling-csv-to-netcdf` path can now write a Rust-generated CoLM forcing-template NetCDF with cell indices, centers, land forcing area, river forcing area, and coastal forcing area derived from the package CSV. Empty or non-finite river-area fields are normalized to zero in this forcing handoff so downstream model inputs do not inherit CSV missing-value NaNs. This is still a template/handoff product, not the final fully model-specific CoLM forcing/restart package.

### 2026-06-14: CoLM package delivery manifest JSON added

Extended the Rust CoLM package handoff so `--delivery-manifest` now writes a real `earthmesh_colm_package_manifest` JSON file instead of only being copied into NetCDF metadata. The manifest records the case name, row count, coupling NetCDF path, and any restart/forcing template paths generated in the same CLI run. This makes the Rust-generated CoLM handoff products discoverable as a package while the remaining gap still includes full model-specific CoLM forcing/restart semantics and parity fixtures.

### 2026-06-14: direct restart Area_judge runtime state exposed

Extended `MkgrdRestartAreaJudgeGlobalSourceRunReport` with a typed `runtime_state()` accessor and routed `MkgrdTopLevelDispatchRunReport::runtime_state()` through it. Direct mask-restart global-source `Area_judge` callers can now observe the final `Get_Contain(0)` `num_mp_step`/`num_wp_step` writeback without reaching into nested restart internals, closing another explicit `consts_coms` runtime-counter seam at the Rust report boundary.

### 2026-06-14: restart-refine land patchtype artifact reported

Extended the restart-refine final-postprocess CLI reporting for the migrated land branch. `earthmesh_cli` now prints `restart_refine_final_postproc_patchtype=` when the restart-refine land `mask_postproc` handoff writes the CoLM patchtype NetCDF file, matching the already exposed normal refine and direct mask-restart report boundaries. This closes one more observable `mkgrd.x` replacement gap for restart/refine/ContinueMkgrd land handoffs.

### 2026-06-14: restart-refine earthmesh final outputs reported

Extended the default restart-refine landtype-source path for `mesh_type='earthmesh'`. Calculated-refine threshold planning, GetRef orchestration, and final `Get_Contain(0)` now treat `earthmesh` as the LOC-style land/ocean/atmos composite alias used elsewhere in the Rust port, instead of accepting only literal `LOCmesh`. The CLI also reports `restart_refine_final_postproc_patchtype=` and `restart_refine_final_postproc_earthmesh_info=` after the migrated Earth final postprocess writes CoLM patchtype and `earthmesh_info.nc4`, closing another restart/refine/ContinueMkgrd observability gap at the `mkgrd.x` replacement boundary.

### 2026-06-14: compact source-state earthmesh final postprocess request

Extended compact source-state restart/refine metadata so `final_domain_contain=earthmesh` and `final_domain_postproc=earthmesh` are accepted as LOC-style Earth handoff requests. The parser now records an Earth final-postprocess request, and the compact source-state runner maps it to `MkgrdFinalDomainPostprocOptions::EarthFromFinalGrid` with the namelist `mask_sea_ratio` plus reconstructed source axes. This removes another source-state-only limitation in the migrated `mkgrd.x` restart/refine path.

### 2026-06-14: compact source-state earthmesh binary final postprocess covered

Added binary coverage for `earthmesh_cli <mkgrd.nml> --run-refine-source-state <state>` with `mesh_type='earthmesh'`, `final_domain_contain=earthmesh`, and `final_domain_postproc=earthmesh`. The regression drives real gridinit, specified source/refine execution, LOC-style final `Get_Contain(0)`, Earth `mask_postproc`, CoLM patchtype NetCDF writing, and `earthmesh_info.nc4` generation through the compact source-state binary boundary. The test exposed a compact source-state Earth postprocess bounds mismatch: the final-domain Area_judge payload starts at north-latitude source index `1`, so the Earth patchtype context now uses the same `maxlat_dm_area=1` instead of the southern `nlats_source` edge.

### 2026-06-14: compact source-state land postprocess bounds aligned

Aligned compact source-state land final-postprocess context with the Area_judge selected-grid convention used by restart and data_preprocess paths. The compact land selected `seaorland` matrix now keeps latitude in north-to-south source-index order, and the compact land patchtype context starts from `maxlat_dm_area=1` for full compact source windows instead of using the southern `nlats_source` edge. This fixes the same latitude-boundary class of bug exposed by the compact Earth final-postprocess binary regression while leaving full binary land parity fixture construction as a separate coverage task.

### 2026-06-14: compact source-state land binary final postprocess covered

Added binary coverage for `earthmesh_cli <mkgrd.nml> --run-refine-source-state <state>` with `mesh_type='landmesh'`, `final_domain_contain=land`, and `final_domain_postproc=land`. The regression drives real gridinit, specified source/refine execution, final `Get_Contain(0)`, land `mask_postproc`, and CoLM patchtype NetCDF writing through the compact source-state binary boundary. The fixture uses a sparse land mask matching the generated final containment pixels, avoiding invalid all-land source windows where `mask_postproc_Lnd` must reject land pixels that do not map to any final unstructured cell.

### 2026-06-14: restart-refine compact source-state earthmesh binary covered

Added direct binary coverage for `earthmesh_cli <mkgrd.nml> --run-mask-restart-area-judge-refine --restart-refine-source-state <state>` with `mesh_type='earthmesh'` and final postprocess enabled. The regression drives the compact source-state Area_judge restart/refine handoff through LOC-style final `Get_Contain(0)`, Earth `mask_postproc`, CoLM patchtype NetCDF writing, and `earthmesh_info.nc4` generation, asserting the explicit `restart_refine_source=source_state`, final gridfile, patchtype, and earthmesh_info report lines. Added matching default-dispatcher coverage for `earthmesh_cli <mkgrd.nml> --restart-refine-source-state <state> --restart-refine-initial-gridfile <gridfile>` so the option-free `mkgrd.x` replacement boundary exercises the same Earth final postprocess outputs. The behavior was already present, so this closes direct matrix coverage gaps rather than changing production code.

### 2026-06-14: restart-refine earthmesh hex final postprocess role masks

Closed the hex-specific final postprocess mismatch exposed by the default `mkgrd.x` restart-refine compact source-state path for `mesh_type='earthmesh'` and `mode_grid='hex'`. In this handoff, the final hex grid layout reports `ustr_points` at the swapped polygon-grid size while `Get_Contain(0)`/Earth role masks remain at the center/cell-grain contain size. The Rust final compaction and `earthmesh_info.nc4` builders now use the role-mask grain for hex while keeping the existing stricter tri short-mask rejection. A binary regression now drives the option-free default dispatcher through LOC-style final contain, Earth `mask_postproc`, CoLM patchtype output, and `earthmesh_info.nc4` for the compact source-state hex case.

### 2026-06-15: default restart-refine landtype earthmesh hex coverage

Added explicit binary coverage for the production `NL%landtype_file` restart-refine handoff when `mesh_type='earthmesh'` and `mode_grid='hex'`. The option-free `earthmesh_cli <mkgrd.nml>` dispatcher now has a regression proving it can enter the landtype-file source path, reuse the supplied restart-refine initial gridfile, run the migrated LOC-style final contain and Earth final postprocess, and report/write the final gridfile, CoLM patchtype NetCDF, and `earthmesh_info.nc4` outputs for the hex case. This complements the compact source-state hex coverage and narrows the remaining `mkgrd.F90` matrix gap without claiming full `mkgrd.x` replacement parity.

### 2026-06-15: v3 Rust geometry concave-mask PyO3 overlay

Extended the `rust/earthmesh_geometry` PyO3 runtime path beyond convex-only clipping for simple polygon overlap. `intersection_area` now triangulates both simple polygons and sums triangle-pair intersections, so convex mesh cells clipped by concave hydro/coast masks such as L-shaped river corridors preserve the full overlap area instead of losing area through convex-clip assumptions. The new Rust crate regression first failed with `0.25` instead of the expected `1.75`, then passed after the triangulated path; the Python `RustGeometryBackend` now has a matching PyO3 boundary test for the same concave hydro mask fraction. This improves the v3 Python+Rust handoff for hydro/coast/CoLM mask attribution, while arbitrary close-mask dissolve/union with holes and complex multi-component topology remains an explicit follow-up gap.

### 2026-06-15: v3 Rust geometry overlay-cell PyO3 runtime boundary

Moved the v3 Rust geometry backend one layer closer to Rust-owned hydro/coast attribution. The PyO3 module now exposes `overlay_cell(cell_vertices, masks)` and returns the winning class, winning priority, per-class fractions, source feature ids, and quality flags from Rust. `RustGeometryBackend` now calls this single Rust boundary instead of looping in Python over scalar `intersection_area` calls. New tests cover the direct PyO3 payload for a concave R2 river mask, the Python `RustGeometryBackend` wrapper, and the Rust crate-level `overlay_cell` API. This reduces Python orchestration in the v3 hydro/coast/CoLM mask attribution path while preserving the broader follow-up gap for arbitrary close-mask dissolve/union and full model-specific CoLM forcing/restart semantics.

### 2026-06-15: v3 Rust geometry batched overlay-cells PyO3 boundary

Extended the v3 Rust geometry runtime from a single-cell PyO3 overlay call to a batched `overlay_cells(cells, masks)` boundary. Python now sends multiple cell ids and polygons plus shared hydro/coast masks across the PyO3 boundary once, and Rust returns per-cell winning class, priority, class fractions, source feature ids, and quality flags. `RustGeometryBackend.overlay_cells` now uses this batch call instead of looping over one Rust call per cell. New Rust and Python tests cover a wet cell with a concave R2 river overlap plus a dry cell that returns `UNKNOWN`/`missing_mask`. This continues moving v3 hydro/coast/CoLM attribution toward the intended Python-orchestration/Rust-computation split; fully general close-mask dissolve/union and model-specific CoLM forcing/restart products remain separate follow-up work.

### 2026-06-15: v3 geometry backend recorded in main manifest

Extended the v3 pipeline manifest contract so `manifest.json` records the effective geometry backend used for hydro/coast mask attribution. The pipeline already wrote the backend to `overlay_summary.json` and the MERIT smoke pipeline summary; now the core `V3RunManifest` carries the same `geometry_backend` field, so CoLM2024/CoLM20XX, MPAS, and FVCOM adapter packages can prove from their primary manifest whether attribution ran through the Python reference backend or the Rust/PyO3 backend. A regression first failed on the missing manifest field and missing sidecar key, then passed after wiring the backend name into `V3RunManifest` construction.

### 2026-06-15: MERIT v3 summary records resolved Rust backend

Tightened the MERIT-Hydro v3 regional pipeline backend provenance. When callers request the `rust` backend alias, `manifest.json` and `overlay_summary.json` already report the effective `rust_pyo3` backend; `pipeline_summary.json` now records that same resolved backend instead of the input alias. The new regression runs the real MERIT fixture through `geometry_backend="rust"` after building the PyO3 extension and verifies all three summary artifacts agree on `rust_pyo3`.

### 2026-06-15: MOD_Area_judge manifest status closed

Audited the `src/MOD_Area_judge.F90` migration manifest entry against the current Rust surfaces and tests. The entry already listed the pure geometry helpers, source-window helpers, area-mask builders, domain/seaorland composition, restart grid persistence, restart/refine handoffs, and MOD_data_preprocess landtype bridge as completed Rust surfaces, and it had no remaining Area_judge-specific surfaces. The manifest now marks `MOD_Area_judge.F90` as `completed`; a manifest consistency regression prevents future entries from staying in `started` status with an empty remaining list.

### 2026-06-15: data_preprocess source-grid globals moved into Rust runtime state

Migrated another `consts_coms` global-state handoff used by `MOD_data_preprocess.F90`: `EarthmeshRuntimeState` now owns a `SourceGridState` carrying `nlons_source`, `nlats_source`, and `maxlc`. The refine prepare path records source-grid dimensions from the typed source-grid bundle, and the landtype-source refine/restart paths write the real data_preprocess `maxlc` into the runtime state before execution, so callers no longer need to infer those values from implicit Fortran globals.

### 2026-06-15: mask counter globals copied into Rust runtime state

Migrated another `consts_coms` global-state handoff: `EarthmeshRuntimeState` now carries explicit mask counters for `mask_domain_ndm`, `mask_refine_ndm(0:9)`, and `mask_patch_ndm(0:9)`. The refine prepare path copies the counters produced by Rust `Mask_make` workspace execution into runtime state, so downstream Area_judge/refine handoffs can read the same values without relying on implicit Fortran module globals.

### 2026-06-15: scalar consts_coms runtime defaults and num_center handoff added

Moved another set of scalar `consts_coms` globals into explicit Rust runtime state. `EarthmeshRuntimeState` now carries `rinit`, `rinit8`, `iunit`, `io6`, and `num_center` defaults matching `consts_coms.F90` plus `mkgrd.F90` top-level initialization. It also provides a `record_num_center_from_previous_step` handoff mirroring `MOD_GetContain.F90`, where refine-area containment derives `num_center` from `num_wp_step(step-1)` rather than reading a hidden module global.

### 2026-06-15: impent pentagon-index scratch state moved into Rust runtime state

Migrated another `consts_coms` global-state handoff used by `icosahedron.F90`: `EarthmeshRuntimeState` now owns the twelve `impent(12)` pentagonal M-point indices as `pentagon_indices`. The Rust state starts with the legacy zero scratch defaults and records completed icosahedron values only through `record_pentagon_indices_from_icosahedron`, which rejects zero sentinels before replacing the hidden module-global array with explicit runtime state.

### 2026-06-15: consts_coms double-precision π constants named in Rust

Added explicit Rust names for the double-precision mathematical constants from `consts_coms.F90`: `PIO180_R8`, `PIU180_R8`, `PI2_R8`, and `PI_R8`. These mirror the Fortran `pio180_r8`, `piu180_r8`, `pi2_r8`, and `pi_r8` names so migrated grid-quality and spherical-area kernels can depend on typed Rust constants instead of reintroducing local copies or reaching back to module globals.

### 2026-06-15: generated gridinit writes impent into runtime state

Closed the production wiring gap for the newly explicit `impent(12)` state. `VoronoiGridState` now carries the pentagonal M-point indices copied from the relaxed icosahedron state, and `run_mkgrd_gridinit_global_namelist` records them into `EarthmeshRuntimeState::pentagon_indices` before returning the generated-grid report. The gridinit runtime state therefore preserves the same pentagon-index handoff that Fortran stored in `consts_coms:impent`, instead of leaving the Rust state at the zero scratch default after a real grid generation.

### 2026-06-15: restart-refine compact source-state ocean postprocess uses source num_vertex

Closed a default-dispatcher gap for `mesh_type='oceanmesh'` restart-refine handoffs driven by compact source-state files. The compact source-state metadata already carries the legacy `num_vertex` boundary, so the Rust runner now falls back to that value when no manual `--mask-postproc-num-vertex` argument or persisted contain file is available. A binary regression drives `earthmesh_cli <mkgrd.nml> --restart-refine-source-state <state> --restart-refine-initial-gridfile <gridfile>` through final `Get_Contain(0)`, ocean `mask_postproc`, and OBC/OBDv2 output reporting, narrowing the remaining `mkgrd.x` replacement gap to broader combination coverage and release-scale parity rather than this source-state boundary handoff.

### 2026-06-15: restart-refine compact source-state land postprocess uses source num_vertex

Closed the corresponding default-dispatcher gap for `mesh_type='landmesh'` restart-refine handoffs driven by compact source-state files. The compact source-state `num_vertex` field now drives final `Get_Contain(0)` and land `mask_postproc` when no manual `--mask-postproc-num-vertex` argument or persisted contain file is available. A binary regression drives `earthmesh_cli <mkgrd.nml> --restart-refine-source-state <state> --restart-refine-initial-gridfile <gridfile>` through final contain generation, land gridfile output, and CoLM patchtype reporting with the same sparse selected-domain fixture used by existing land postprocess coverage.

### 2026-06-15: restart-refine landtype-source ocean postprocess uses mode-grid num_vertex

Closed the matching default-dispatcher gap for `mesh_type='oceanmesh'` restart-refine handoffs driven directly by `NL%landtype_file`. When neither `--mask-postproc-num-vertex` nor a persisted final contain file is available, the landtype-source runner now falls back to the existing `NL%mode_grid` to `num_vertex` inference. A binary regression drives `earthmesh_cli <mkgrd.nml> --restart-refine-initial-gridfile <gridfile>` through final `Get_Contain(0)`, ocean `mask_postproc`, and OBC/OBDv2 output reporting without requiring the explicit debug execution flag or a manual postprocess boundary.

### 2026-06-15: restart-refine landtype-source land postprocess uses default num_vertex

Closed the non-ocean side of the default restart-refine `NL%landtype_file` handoff. The landtype-source restart runner now falls back from missing manual `--mask-postproc-num-vertex` and missing persisted final contain metadata to the land handoff's default `num_vertex=1`, while retaining the existing `NL%mode_grid` tri/hex fallback for ocean/Earth-style handoffs. The default binary landmesh regression no longer passes a manual postprocess boundary and still drives final `Get_Contain(0)`, land `mask_postproc`, and CoLM patchtype reporting from the landtype-derived source state.

### 2026-06-15: default mask_restart wrapper supplies mode-grid postprocess boundary

Tightened the option-free default wrapper used by the CLI before falling back to plain dispatch. When a mask-restart `ContinueMkgrd` path has a restart Area_judge grid but no manual postprocess boundary, the wrapper can now pass the `NL%mode_grid`-derived tri/hex `num_vertex` into the configured global-source Area_judge final handoff. The lower-level explicit Area_judge runner remains capable of area-only execution when callers omit the boundary deliberately.

### 2026-06-15: hydro close-mask LineString and MultiLineString centerlines

Extended the native Rust hydro/coast close-mask exporter beyond pre-polygonized GeoJSON inputs. `LineString` and `MultiLineString` source features are now accepted alongside Polygon/MultiPolygon/GeometryCollection/Feature inputs; when a refine degree has a configured `--buffer-deg-by-refine-degree` value, the line centerline is converted to a mitered corridor polygon and then follows the existing cumulative refine, simplify, dissolve, and `.nml` export path. This lets MERIT/CaMa-style river or coastline centerline layers drive EarthMesh close-mask refinement without a separate Python polygonization step. Lines without an explicit buffer remain skipped to avoid zero-area masks.

### 2026-06-15: FVCOM 2dm mesh save writer ported

Ported `MOD_file_preprocess.F90:FVCOM_Mesh_Save` into Rust as a typed `.2dm` writer. The new path writes the legacy `result/fvcom.2dm` file with `MESH2D`/`MESHNAME`, `E3T` triangle records, `ND` node records, and `NS` open-boundary segments read from either `obc.nc4` or `obc_patch.nc4` according to `mask_patch_on`. The writer keeps the Fortran-indexed EarthMesh placeholder convention and subtracts one only at the final FVCOM text boundary, matching the original `i=2..` Fortran loops without adding speculative `.dat` output.

### 2026-06-15: IAP mesh reader payload ported

Added a typed Rust reader for `MOD_file_preprocess.F90:IAP_Mesh_Read`. The reader loads `sjx_points`, `lbx_points`, `GLONW`, `GLATW`, `itab_m%im`, and `itab_m%iw`, reconstructs the legacy first placeholder row, converts radians to degrees, normalizes longitudes into `[-180, 180]`, and applies the Fortran `+1` offset to triangle-neighbor and triangle-vertex connectivity. This closes the direct IAP read payload boundary alongside the already migrated IAP-Ocean mode-file-to-EarthMesh converter.

### 2026-06-15: vendored BLAS/LAPACK externalized by reachability gate

Closed the `blas.F90` and `lapack.F90` migration entries without hand-translating Netlib routines or adding a speculative dependency. A static regression now enumerates every bundled BLAS/LAPACK entry point and proves no other `src/*.F90` EarthMesh source calls them. Under the manifest's `external_crate` strategy, that means the current Rust port needs no BLAS/LAPACK shim; if a future migrated kernel reintroduces one of those calls, the test fails and forces an explicit maintained-provider decision instead of silently depending on the vendored Fortran files.

### 2026-06-15: MOD_GetRef manifest completion guarded

Closed the `MOD_GetRef.F90` migration manifest entry after adding a static completion gate. The gate enumerates every `MOD_GetRef.F90` subroutine (`GetRef`, `GetRef_Lnd`, `GetRef_Ocn`, `GetRef_Atmos`, `GetRef_LOC`, `mean_std_cal2d`, and `mean_std_cal3d`) and requires each name to remain anchored in the Rust implementation or migration evidence before the manifest can stay marked `completed`. This records the already-migrated threshold builders, file runners, calculated/specified NetCDF readers and writers, LOC aggregation path, and fixture-parity checks as a closed GetRef surface while leaving broader `mkgrd.x` release parity under the `mkgrd.F90` entry.

### 2026-06-15: MOD_GetContain manifest completion guarded

Closed the `MOD_GetContain.F90` migration manifest entry after adding a static completion gate. The gate enumerates the file's subroutines (`Get_Contain`, `IsInArea_ustr_Calculation`, `Contain_Calculation`, and `Data_Updata`) and requires each name to remain anchored in Rust code or migration evidence before the manifest can stay marked `completed`. This records the already-migrated area selector, containment matrix core, dateline and south-pole handling, file-backed refine/final-domain containment runners, and runtime counter handoffs as a closed GetContain surface while broader restart/refine release parity remains tracked under `mkgrd.F90`.

### 2026-06-15: MOD_refine manifest completion guarded

Closed the `MOD_refine.F90` migration manifest entry after migrating the last unanchored placeholder, `orial_vertices_protect`, as an explicit Rust no-op. The Fortran routine contains no executable statements, so `earthmesh_mesh::refine_orial_vertices_protect_fortran_indexed` intentionally preserves caller-owned refinement markers unchanged and has fixture coverage for that behavior. A static completion gate now enumerates every `MOD_refine.F90` subroutine and requires each name to remain anchored in Rust code or migration evidence before the manifest can stay marked `completed`. This closes the per-subroutine refine surface while larger restart/refine production-matrix parity remains tracked under `mkgrd.F90`.
### 2026-06-15: MOD_file_preprocess bbox/circle mesh schemas ported

Added Rust NetCDF readers and writers for `MOD_file_preprocess.F90:bbox_Mesh_Read`, `bbox_Mesh_Save`, `circle_Mesh_Read`, and `circle_Mesh_Save`. These file-level mesh adapters intentionally preserve the legacy mesh schemas without the mask-only `bbox_refine` and `circle_refine` metadata, so bbox/circle geometry can round-trip independently from `mkgrd.F90` refinement masks. The existing mode4 reader is now explicitly anchored as `Mode4_Mesh_Read`; broader release-scale file-preprocess parity remains tracked under the MOD_file_preprocess manifest entry.

### 2026-06-15: MOD_data_preprocess threshold readers and initial quality check ported

Added direct Rust readers for `MOD_data_preprocess.F90:data_read_onelayer` and `data_read_twolayer`, preserving the Fortran one-based `start/count` window semantics for threshold NetCDF variables. The new `Threshold_Read_Lnd`, `Threshold_Read_Ocn`, and `Threshold_Read_Atmos` adapters keep the Fortran flag-pair selection behavior but return explicit Rust reports instead of allocating hidden module globals. Also added `run_mkgrd_initial_grid_quality_check` for `mkgrd.F90:Inital_Grid_Quality_Check`, composing the migrated unstructured grid reader and `Grid_Quality_Check_Global` writer for the legacy `_global_orial.nc4` initial quality side effect. The legacy `mkgrd.F90:CHECK` NetCDF status subroutine is represented by Rust `Result` propagation through `netcdf_to_io_error`, not by a separate panic/stop wrapper.

### 2026-06-15: MOD_file_preprocess manifest completion guarded

Closed `src/MOD_file_preprocess.F90` after adding a completion gate that enumerates all 25 Fortran file-preprocess subroutines and requires each to remain anchored in Rust code, migration notes, manifest evidence, or focused adapter fixtures. The gate also requires the key release-parity fixture files for bbox/circle/close mesh schemas, FVCOM `.2dm` output, IAP payload reads, MPAS full/simple/graph/edge-reference adapters, and mode4 mesh generation. This records the file-preprocess replacement as completed while broader `mkgrd.x`, `MOD_data_preprocess`, and `consts_coms` release parity remain tracked separately.

### 2026-06-15: MOD_data_preprocess manifest completion guarded

Closed `src/MOD_data_preprocess.F90` after adding a completion gate that requires all six Fortran data-preprocess subroutines to remain anchored and requires evidence across landtype loading, threshold window readers, MERIT/CaMa data sources, native hydro/coast close-mask topology, CoLM NetCDF/package handoffs, and v3 Rust/PyO3 hydro attribution. The gate fixes the previously broad release-parity remaining item to concrete tests for LineString/MultiLineString buffering, non-rectilinear holes, multi-component union gap preservation, shared/partial/chained polygon dissolve, bbox-overlap disjoint guards, convex overlap dissolve, CoLM delivery manifest output, and effective Rust geometry backend provenance.

### 2026-06-15: consts_coms manifest completion guarded

Closed `src/consts_coms.F90` after adding a completion gate for the Rust-owned replacements of legacy constants, memory allocators, and mutable module-global handoffs. The gate requires evidence for mathematical constants, `mem_grid`/`mem_ijtabs`/`mem_delaunay` allocation defaults, `EarthmeshRuntimeState` config/refine/grid/itab/delaunay ownership, mesh-count and `num_vertex` counters, scalar defaults and `num_center`, source-grid/maxlc, mask counters, `impent(12)` pentagon indices, gridinit writeback, source-branch/runtime-state propagation, final-domain writeback, and restart/top-level runtime-state forwarding.

### 2026-06-15: mkgrd manifest completion guarded

Closed `src/mkgrd.F90` after adding a completion gate for the Rust `mkgrd.x` replacement surface. The gate requires every mkgrd subroutine name to remain anchored and fixes the former restart/refine/ContinueMkgrd matrix remaining item to concrete evidence across gridinit, initial/final quality, calculated/specified source branches, compact source-state and landtype-source refine runners, default option-free dispatch, mask_restart patch/ocean/non-ocean Area_judge continuation, restart-refine compact/landtype handoffs, persisted and inferred `num_vertex` boundaries, land/ocean/atmos/earth final postprocess, MPAS full/simple outputs, graph.info, CoLM patchtype, and earthmesh_info outputs.

### 2026-06-15: root build entrypoint switched to Rust

Closed the delivery-layer gap after the migration manifest reached `completed`. The root `Makefile` now builds `rust/earthmesh_cli` through Cargo and copies the Rust binary to `./mkgrd.x`, preserving the old executable name without compiling `src/*.F90` objects. `make.sh`, `make_gnu.sh`, and `switch_compiler.sh` are retained only as compatibility wrappers/no-ops for old workflows, and a build-entrypoint regression ensures those scripts do not reintroduce active Fortran compiler or object-build hooks.
