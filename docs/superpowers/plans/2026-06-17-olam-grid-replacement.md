# OLAM Grid Replacement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace EarthMesh grid construction with an OLAM-style Delaunay/Voronoi pipeline instead of continuing to patch the legacy `MOD_refine` flow.

**Architecture:** Introduce a reusable OLAM Delaunay mesh layer in `earthmesh_mesh` that owns `M/U/W` tables, full topology validation, and neighbor rebuilding. Make global gridinit, specified refinement, regional masking, spring smoothing, and NetCDF output consume this layer through narrow adapters until the old EarthMesh refine path can be removed.

**Tech Stack:** Rust workspace crates `earthmesh_mesh`, `earthmesh_cli`, and `earthmesh_gui`; OLAM Fortran sources under `/Users/zhongwangwei/Library/Mobile Documents/com~apple~CloudDocs/olam-model-code-r1095-trunk.zip`; targeted `cargo test` validation.

## Global Constraints

- Do not overwrite current GUI/mask/refine edits in the dirty worktree.
- Preserve current EarthMesh NetCDF output schema while the OLAM internals are phased in.
- Use TDD for each production-code change: add failing Rust tests before adding implementation.
- Keep OLAM concepts, not Fortran global-module structure: no new global mutable state and no `stop`-style panics for recoverable topology errors.
- Preserve Fortran-compatible placeholder indexing at the adapter boundary until all downstream writers are migrated.

---

## File Structure

- Modify: `rust/earthmesh_mesh/src/lib.rs` - add the OLAM Delaunay mesh model, topology validator, global expansion, `spawn_nest`-style refinement, and OLAM spring routines.
- Test: `rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs` - unit tests for `M/U/W` topology invariants and OLAM global initialization.
- Test: `rust/earthmesh_mesh/tests/olam_spawn_nest.rs` - specified-region refinement tests for one and multiple regions, boundary rows, and transition rows.
- Modify: `rust/earthmesh_cli/src/lib.rs` - route gridinit/refine/spring through the OLAM mesh layer and keep existing NetCDF writers as adapters.
- Test: `rust/earthmesh_cli/tests/olam_mkgrd_pipeline.rs` - end-to-end CLI tests for global, global+refine, regional mask, land, and ocean cases.
- Modify: `rust/earthmesh_gui/src/main.rs` - keep GUI options but ensure refinement level, max passes, and region lists map to the OLAM region model.

## Task 1: OLAM Delaunay Mesh Foundation

**Files:**
- Modify: `rust/earthmesh_mesh/src/lib.rs`
- Test: `rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs`

**Interfaces:**
- Produces: `OlamDelaunayMesh::from_icosahedron(...) -> Option<OlamDelaunayMesh>`
- Produces: `OlamDelaunayMesh::validate_topology(&self) -> io::Result<OlamTopologyValidation>`
- Consumes: existing `IcosahedronRelaxedGrid`, `IcosahedronUEdge`, `IcosahedronWFace`, and `IcosahedronMPointNeighbors`

- [x] **Step 1: Write failing topology test**

```rust
use earthmesh_mesh::OlamDelaunayMesh;

#[test]
fn olam_delaunay_mesh_from_icosahedron_has_closed_muw_topology() {
    let mesh = OlamDelaunayMesh::from_icosahedron(1, 0, 1.0, 0.25, 100)
        .expect("valid OLAM icosahedron mesh");
    let report = mesh.validate_topology().expect("closed topology");
    assert_eq!(report.checked_m_points, mesh.nmd - 1);
    assert_eq!(report.checked_u_edges, mesh.nud - 1);
    assert_eq!(report.checked_w_faces, mesh.nwd - 1);
}
```

- [x] **Step 2: Run failing test**

Run: `cargo test -p earthmesh_mesh --test olam_delaunay_mesh`
Expected: compile failure because `OlamDelaunayMesh` does not exist.

- [x] **Step 3: Implement the smallest generic OLAM mesh wrapper**

Add `OlamDelaunayMesh` and `OlamTopologyValidation` around the existing relaxed icosahedron tables, then validate reciprocal references:

```rust
pub struct OlamDelaunayMesh {
    pub nmd: usize,
    pub nud: usize,
    pub nwd: usize,
    pub m_points: Vec<CartesianPoint>,
    pub u_edges: Vec<IcosahedronUEdge>,
    pub w_faces: Vec<IcosahedronWFace>,
    pub m_neighbors: Vec<IcosahedronMPointNeighbors>,
}
```

- [x] **Step 4: Run test to green**

Run: `cargo test -p earthmesh_mesh --test olam_delaunay_mesh`
Expected: PASS.

## Task 2: OLAM Global Expansion

**Files:**
- Modify: `rust/earthmesh_mesh/src/lib.rs`
- Test: `rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs`

**Interfaces:**
- Produces: `OlamDelaunayMesh::expand_global2(&self) -> io::Result<OlamDelaunayMesh>`
- Produces: `OlamDelaunayMesh::expand_global3(&self) -> io::Result<OlamDelaunayMesh>`
- Produces: `OlamDelaunayMesh::expand_by_factor(&self, factor: usize) -> io::Result<OlamDelaunayMesh>`

- [x] **Step 1: Add tests for legal and illegal expansion factors**

Run: `cargo test -p earthmesh_mesh --test olam_delaunay_mesh expand`
Expected before implementation: compile failure for missing expansion APIs.

- [x] **Step 2: Port OLAM `expand_delaunay_mesh`, `expand_global2`, and `expand_global3` semantics**

Use `/tmp/olam-src.Mca9eh/olam-model-code-r1095-trunk/omodel/expand_global.f90` as the reference. Expansion factors must be only products of 2 and 3, and each expansion must end with full topology rebuild plus validation.

Progress: `expand_global2`, `expand_global3`, and 3-first/2-second factor dispatch are ported with topology-validation tests. `gridinit_voronoi_state_fortran` now applies OLAM `get_factors` before building the Voronoi adapter state.

## Task 3: OLAM Specified-Region Refinement

**Files:**
- Modify: `rust/earthmesh_mesh/src/lib.rs`
- Test: `rust/earthmesh_mesh/tests/olam_spawn_nest.rs`
- Modify: `rust/earthmesh_cli/src/lib.rs`

**Interfaces:**
- Produces: `OlamRefinementRegion::{Circle, Corridor, Bbox}`
- Produces: `OlamDelaunayMesh::spawn_nest(&self, regions: &[OlamRefinementRegion], max_level: usize) -> io::Result<OlamDelaunayMesh>`
- Produces: `OlamDelaunayMesh::boundary_rows(&self) -> &[usize]`

- [x] **Step 1: Test one circle, two circles, bbox, and different per-region levels**

Run: `cargo test -p earthmesh_mesh --test olam_spawn_nest`
Expected before implementation: compile failure for missing `OlamRefinementRegion` and `spawn_nest`.

- [ ] **Step 2: Port OLAM region selection and topology-safe perimeter tracing**

Use OLAM `spawn_nest.f90` routines `ngr_area`, `thirdm`, `fill_rad3`, `perim_map2`, `perim_ngr`, and `perim_mrow`. The backend must canonicalize GUI circles/bboxes into legal topology regions before subdividing.

Progress: mesh-layer foundation is in place for `Circle`, `Bbox`, and `Corridor` requests with per-region levels. The current pass uses OLAM-style midpoint subdivision for selected W faces plus local transition-face closure to keep M-point incidence within the 7-slot OLAM table limit. The atmospheric global specified-refine dispatcher now has a direct OLAM output branch and the gridfile adapter preserves 7-corner transition cells via `itab_w%npoly`. The first Rust `perim_mrow` slice is in place: local spawn now records positive/negative transition rows, preserves `mrow` through seed rebuilds and global 2/3 expansions, and keeps recursive GUI levels usable by treating current level rows as an override instead of Fortran's hard nested-grid out-of-bounds stop. The Rust selector now also ports the `fill_rad3` seed expansion used after OLAM builds the inside-M-point list plus OLAM's iterative sharp-concavity closure around nearly filled M-point rings, so tiny or vertex-centered specified regions expand beyond the immediate one-ring and close one-face gaps before subdivision. `fill_rad3` coverage now locks both current-M `npoly` handling and the Fortran im1/im2/im3 distant-M expansion that marks all six neighboring W faces. Region selection now follows OLAM's `thirdm` seed-lattice intent instead of selecting every M/W center inside the geometric mask, and focused coverage locks the straight opposite-edge third-neighbor walk plus far-end reciprocal `jdone` marking. Global start-point selection now matches OLAM's contained `impent` preference and covers the near-pentagon `impen` march-to-region branch for circle/corridor close halos, including the Fortran `impen`/`imcent` `mrlm` equality gate and the later `mrlo = ltab_md(imbeg)%mrlm` ownership rule when marched IMBEG differs from IMPEN/IMCENT. The Rust spawn path orders closed directed perimeter loops, rotates each loop to an OLAM-style `nwdiv == 2` convex corner, and promotes adjacent boundary faces until every Method-C loop length is divisible by three. The spawn path then walks the loop in three-segment groups and identifies the center-segment outside W face, matching the `nest_wd(jw2)%iw(3) = -1` suppression step before OLAM `perim_fill3`; transition faces now use a Method-C edge-midpoint-only triangulation instead of adding synthetic face-center fan points. The over-valence closure now promotes adjacent transition faces, and when a `perim_fill3` suppressed edge blocks edge-level closure it upgrades the old W face into the selected set and recomputes perimeter/suppression state until all old M points remain within the 7-slot OLAM table limit. Method-C now builds an explicit OLAM numbering plan (`imnew`, `iunew`, `iwnew`, `nest_ud`, and `nest_wd`) and assigns midpoint M ids in the Fortran `im = 2..nmd` plus first-seen `ltab_md(im)%iu` order, with focused topology coverage for split-U midpoint IDs, full-interior sphere/native midpoint edge-average coordinates, and full-subdivision central-parent/child-W vertex geometry, split-U M metadata child ownership, split-U second-half U ids that share the midpoint and adjacent W-face family rewrites, exact child-W adjacency rewrites for full-subdivision split-U half-edges, suppressed split-U reuse/no-midpoint behavior, `iwnew` parent-plus-three-child W ids with parent/child `mrlw_orig` distinction, central parent-W midpoint-triangle topology, final child-W M vertices rebuilt from U endpoints, `iunew` internal U ids for fully subdivided W faces and their midpoint-pair endpoints, `impent = imnew(impent)` remapping, prognostic partner remapping through `imnew/iunew/iwnew`, and final closed-table validation without placeholder U/W/M neighbor IDs, plus multi-shape/multi-region including Polygon allocation-count and projected-output closure, plus public sphere/Cartesian spawn entrypoint Method-C table-path coverage including `niter > 0` Cartesian spring and native-XY deltax spring, OLAMIN-style NXP=66 atmosphere multilevel corridor signature/closure plus Voronoi handoff coverage, and CLI compact gridfile handoff coverage that locks Method-C `itab_w%npoly` into `n_ngrwm` through NetCDF write/read. The final W/U/M topology is emitted through the Method-C table path (`emit_method_c_tables`, `fill_method_c_full_subdivision`, and `perim_fill3_method_c`) rather than the old triangle-seed rebuild; `olam_method_c_pass_uses_fortran_table_numbering_counts` locks the Fortran allocation counts, and the full `earthmesh_mesh` lib suite now passes with 78 tests, and the focused `olam_method_c` test group passes with 28 tests. Larger per-point edge-geometry parity fixtures remain pending; final Fortran OLAM .h5 gridfile tolerance comparison is still blocked by the local OLAM build environment (default h5pfc/Intel flags unavailable; Homebrew gfortran reaches mem_lp.f90 but rejects OLAM nonstandard IMPORT usage).

## Task 4: OLAM Spring Smoothing

**Files:**
- Modify: `rust/earthmesh_mesh/src/lib.rs`
- Test: `rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs`
- Modify: `rust/earthmesh_cli/src/lib.rs`

**Interfaces:**
- Produces: `OlamDelaunayMesh::spring_global(&self, nxp: usize, niter: usize) -> io::Result<OlamDelaunayMesh>`
- Produces: `OlamDelaunayMesh::spring_global_with_controls(&self, nxp: usize, niter: usize, beta: f64, relax: f64) -> io::Result<OlamDelaunayMesh>`
- Produces: `OlamDelaunayMesh::spring_nest(&self, nxp: usize, niter: usize, ngr: usize, move_interior: bool) -> io::Result<OlamDelaunayMesh>`
- Produces: `OlamDelaunayMesh::spawn_nest_with_spring(&self, regions: &[OlamRefinementRegion], max_level: usize, nxp: usize, niter: usize) -> io::Result<(OlamDelaunayMesh, usize)>`

- [x] **Step 1: Test that global spring keeps pentagons fixed and projects all active points back to Earth radius**

Run: `cargo test -p earthmesh_mesh --test olam_delaunay_mesh spring`
Expected before implementation: compile failure for missing OLAM spring APIs.

- [ ] **Step 2: Port OLAM `spring_dynamics_globe` and `spring_dynamics_nest`**

Use fixed pentagon behavior, angle-based target-distance adjustment, `mrow` transition handling, and per-iteration sphere projection.

Progress: `spring_dynamics_globe` is now represented in the OLAM mesh layer. `OlamDelaunayMesh::from_icosahedron` builds the unsmoothed Delaunay topology first, then applies the OLAM global spring so the twelve `impent` pentagon M points stay fixed. A minimal `spring_dynamics_nest` core is also available and is wired into `spawn_nest_with_spring`: it moves transition-row M points after each successful nest pass, scales target edge length by `mrlu`, applies the OLAM opposite-angle and minimum-area corrections, applies OLAM's `mrow` target-length multipliers for `-2/-1/+1` transition combinations, and projects moved M points back to the sphere. The Method-C spring entrypoint now has focused `niter > 0` coverage proving the actual nest spring pass runs after Method-C refinement without changing Fortran allocation counts, breaking topology closure, or moving M points off the active radius; the Cartesian/native-XY spring entrypoint has matching `niter > 0` coverage proving it keeps the Method-C table path, topology closure, and finite native coordinates, including the native-XY deltax path that uses Fortran's `deltax * sqrt(2/sqrt(3))` target spacing. Method-C perimeter grouping, local `perim_fill3` M/W `ngr` ownership, transition W-face edge rewrites including the `iu33` special case, transition U-edge endpoint/face rewrites, `thirdm` opposite-edge third-neighbor traversal with reciprocal `jdone` marking, Fortran `perim_mrow` alternating half-step row growth, the Fortran split between unprojected linear `perim_fill3` coordinates and later radius projection, the six explicit `perim_fill3` weighted transition-coordinate formulas plus local `mrlm_orig` assignments, and the exact radial expansion projection formula now have focused regression coverage; remaining work is broader per-point Fortran-output edge-geometry parity fixtures and end-to-end file-output tolerance checks.

## Task 5: CLI and GUI Handoff

**Files:**
- Modify: `rust/earthmesh_cli/src/lib.rs`
- Modify: `rust/earthmesh_gui/src/main.rs`
- Test: `rust/earthmesh_cli/tests/olam_mkgrd_pipeline.rs`

**Interfaces:**
- Consumes: `OlamDelaunayMesh`
- Produces: existing `UnstructuredMesh` NetCDF payloads and GUI result previews

- [ ] **Step 1: Add end-to-end tests for `00_quickstart_n16.nml` global, global+refine, land, ocean, and coupled output**

Run: `cargo test -p earthmesh_cli --test olam_mkgrd_pipeline`
Expected before implementation: failure because the top-level pipeline still routes through the legacy refine adapters.

- [ ] **Step 2: Route mkgrd gridinit/refine/spring through OLAM internals**

Keep existing NetCDF readers/writers as boundary adapters. Results must show only current run outputs, preserve case naming from experiment name, and keep regional/land/ocean subsetting topology-consistent.

Progress: `atmosmesh`, `landmesh`, `oceanmesh`, and `LOCmesh` global specified-region refine route to `OlamRefineGlobalSource`. GUI specified-refine staging preserves the visible/global `max_iter_spc` default of 5, allows per-region levels 1..5, and emits empty higher-degree mask files so the global pass cap remains consistent with generated inputs. The OLAM direct path now reads specified-refine circle, bbox, and GUI close-polygon masks directly into OLAM refinement regions instead of falling back to the legacy refine adapter. It also accepts calculated-refine circle/bbox/close region sources through `RL%mask_refine_cal_*`; calculated masks with degree 0 are promoted to the active `max_iter_cal` OLAM level so calculated-only land/ocean/LOC global source runs no longer need the legacy geometric refine loop. It honors `RL%SpringGlobal_type=1` with positive `RL%niter_refine` by running OLAM nest spring after each actual nest pass and reports `spring_nest_passes` plus `spring_nest_iterations`; `default_atmos_global_specified_refine_uses_olam_spawn_nest` now verifies this from namelist dispatch through output gridfile readback and topology consistency. Regional OLAM direct runs now read `NL%mask_domain_fprefix` for bbox/circle/close masks, combine multiple regions as a union, write the full OLAM mesh to `tmpfile/`, then generate the public `result/` gridfile through the existing regional mask-postproc compaction/reindex path. Land/ocean runs with a real `landtype_file` now write a raw OLAM gridfile under `tmpfile/`, optionally domain-subset it first, then generate the public result gridfile through the landtype mask/subset path with compacted topology and placeholder-preserving vertex coordinates. `LOCmesh` runs with a real `landtype_file` now write the raw/domain-filtered OLAM result gridfile, land and ocean subset gridfiles, and a CoLM coupling CSV/NetCDF/manifest from the same visible mesh. Method-C perimeter semantics, in-place OLAM table numbering parity, `thirdm` straight-path seed traversal, calculated-refine level selection, LOCmesh calculated threshold output ordering, calculated threshold component-file OR aggregation, GetRef_Lnd mainland-fraction `maxlc` denominator behavior, unprojected-to-projected edge coordinate staging, `perim_fill3` weighted transition-coordinate formulas, exact radial expansion projection, and top-level OLAM dispatch now have focused and pipeline coverage; the full `olam_mkgrd_pipeline` CLI suite passes with 62 tests; deeper per-point Fortran-output edge-coordinate fixtures and real Fortran threshold-output file comparisons remain open.
