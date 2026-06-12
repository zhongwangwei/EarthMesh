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

Extended `rust/earthmesh_mesh` with `spherical_centroid_degrees`, a tested Rust port of `MOD_grid_preprocess:centroid_spherical_single`. It preserves the Fortran method: lon/lat vertices are converted to unit Cartesian vectors, averaged component-wise, then converted back to lon/lat. The grid-wide `centroid_spherical_calculation` wrapper over connectivity remains unported.

### 2026-06-12: `MOD_grid_preprocess.F90` longitude normalization and arc length ported

Extended `rust/earthmesh_mesh` with tested ports of `CheckLon` and `arc_length`. `CheckLon` preserves the Fortran single-step +/-360 behavior rather than full modulo wrapping. `arc_length_unit_sphere` preserves the MPAS-compatible mixed precision behavior in the Fortran implementation by squaring the half-angle sine terms as `f32` before converting back to `f64`.
