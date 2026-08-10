# Method-C LEPP-Delaunay inventory

This inventory records the repository state and staged LEPP-Delaunay work.
The implementation now covers the read-only LEPP walk, transactional
terminal-midpoint insertion, protected regional boundaries, deterministic
parallel evaluation, the explicit PostQuality path, and the selectable
AdaptiveHybrid production path. It reuses the generic dynamic mesh rather than
introducing a second half-edge implementation. Canonical Method-C remains the
default.

## Current Method-C layout

Method-C already has its own crate at `rust/earthmesh_refine_method_c`.
`src/lib.rs` keeps the canonical algorithm in a flat module tree rather than
the proposed `canonical/` subtree. Moving those modules is unnecessary for the
read-only kernel and would create a large regression surface.

The canonical implementation is split across:

- `method_c_mesh`: `MethodCMesh`, the Method-C-owned wrapper around the shared
  `TriangularMesh`.
- `method_c_spawn` and `method_c_spawn_internal`: public spawn APIs and the
  validate/pass/retry driver.
- `method_c_spawn_pass`, `method_c_spawn_retry`, and
  `method_c_spawn_retry_scaled`: pass execution and recovery.
- `method_c_selection*`: requested-region selection and canonical topology
  traversal.
- `method_c_perimeter*`: perimeter discovery, repair, and transition rows.
- `method_c_tables`, `method_c_emit`, and `method_c_dump`: canonical M/U/W
  table maintenance, output, and diagnostics.
- `method_c_spawn_hfield` and `method_c_nest_spring*`: h-field demand and the
  optional canonical nest spring.

The new kernel lives beside those modules at
`rust/earthmesh_refine_method_c/src/lepp_delaunay/`. It is named explicitly so
it cannot be mistaken for canonical Method-C.

## Canonical entry points and call chain

The global CLI path dispatches from
`rust/earthmesh_cli/src/refine_pipeline/global_source.rs` through
`rust/earthmesh_cli/src/refinement_demand/nest.rs` into `MethodCMesh`.
`MethodCMesh::spawn_nest*` in `method_c_spawn` calls
`spawn_nest_internal`, which validates the request, executes canonical passes,
and applies its existing retry/repair policy. Output conversion remains the
shared `TriangularMesh`/Voronoi path in `earthmesh_mesh`.

Canonical Method-C remains the default route. `&method_c
algorithm='lepp_delaunay'` selects AdaptiveHybrid instead; it consumes named
regions and `&adaptive` point+radius demand, writes the normal gridfile plus
`method_c_lepp_report.json` and `unresolved_demand.json`, and explicitly marks
the result as non-canonical. When explicitly enabled in
`&quality`, the CLI copies the completed canonical triangulation into
`MeshState`, runs the bounded PostQuality pass, and writes a separate `_lepp`
gridfile plus JSON report. It never substitutes that mesh for the canonical
output or runtime state.

## Mesh and topology representations

- `earthmesh_mesh::TriangularMesh` is the shared primal mesh and carries the
  canonical M/U/W-compatible data used by Method-C and output conversion.
- `earthmesh_refine_method_c::MethodCMesh` owns Method-C methods and its last
  transition-row list while dereferencing to `TriangularMesh`.
- `earthmesh_mesh::MeshState` is the generic dynamic triangular topology. It
  stores vertices, triangles, and triangle neighbours with reserved slots 0
  and 1 (`MESH_STATE_FIRST_ID == 2`), converts from `TriangularMesh`, derives
  adjacency in `from_parts`, and validates local or global topology.
- Phase 1 LEPP reads `MeshState` only. It does not modify either representation
  and does not translate LEPP results into M/U/W rows.

## Reusable geometry, Delaunay, Voronoi, and validation

The hard generic machinery already exists in `earthmesh_mesh`:

- Cartesian sphere helpers: `coordinates` (`dot`, `cross`, `magnitude`,
  lon/lat conversion and normalization).
- General arc/area helpers: `mesh_area_primitives` and
  `mesh_spherical_area`.
- Robust predicates: `mesh_predicates::{orient3d, orientation_on_sphere,
  in_circle_on_sphere}` with explicit ambiguity rather than guessed signs.
- Dynamic topology: `mesh_state::{MeshState, MeshStateError}`.
- Point location, Delaunay cavity, degree forecast, and insertion:
  `mesh_insertion`.
- Patch snapshot/rollback: `mesh_patch`.
- Edge flip and Lawson legalization: `mesh_flip`.
- Spherical circumcentres and local Voronoi cells:
  `spherical_circumcenter*` and `mesh_voronoi`.
- Canonical M/U/W and Euler/topology checks:
  `mesh_topology_validation::MethodCTopologyValidation` and
  `MeshState::validate`.

LEPP edge ranking nevertheless uses its specified
`R * atan2(norm(cross(a, b)), dot(a, b))` formula locally. This avoids the
`acos` conditioning of the existing Method-C selection helper and makes the
Phase 1 numerical contract explicit.

## Actual capability gap

The repository was not missing a generic insertion or cavity implementation:
`MeshState::{delaunay_cavity, insert_site}`, patch rollback, edge legalization,
and Voronoi reconstruction already exist. What was missing at the start of
this work was specifically:

- a deterministic read-only LEPP traversal and its result/error/report types;
- a LEPP terminal-edge insertion policy and one common transaction entry;
- the later AdaptiveHybrid execution policy and boundary-constrained variant.

The current implementation supplies stable ids, transaction gates,
terminal-midpoint insertion, protected-segment marker inheritance and rollback,
AdaptiveHybrid target/balance/quality cycles, deterministic parallel scans,
explicit project/namelist configuration, auditable unresolved-demand reports,
and CLI integration without duplicating the existing geometry. Detail arrays
are bounded samples; their adjacent aggregate counters remain exact.

## Canonical regression surface

The unchanged canonical anchors are:

- unit tests under `rust/earthmesh_refine_method_c/src/tests.rs`;
- `tests/method_c_delaunay_mesh.rs`;
- `tests/method_c_spawn_nest.rs`;
- `tests/method_c_disjoint_regions.rs`;
- `tests/method_c_lineage.rs`;
- `tests/mesh_state_from_refined.rs`;
- `tests/point_radius_coastal_demand.rs`;
- `tests/progress_hook.rs`;
- shared insertion, rollback, predicates, topology, and Voronoi tests in
  `earthmesh_mesh`.

The repository gates are:

```text
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The project wrappers `make check-architecture` and `make clippy-full` remain
additional mandatory local gates.

## Phase 1 changed files

- `docs/method_c_lepp_inventory.md`
- `rust/earthmesh_refine_method_c/src/lib.rs`
- `rust/earthmesh_refine_method_c/src/lepp_delaunay/` (kernel and focused tests)

## Phase 2 dynamic mesh continuation

`earthmesh_mesh::MeshState` remains the dynamic structure. Phase 2 adds only
the capabilities the existing type lacked:

- typed `VertexId`, `FaceId`, and canonical endpoint-based `EdgeId`;
- slot generations, so a reused or rolled-back slot cannot make a stale id
  silently name a later entity;
- row-local generation restoration in `MeshPatch`;
- `insert_site_transactionally`, which snapshots, inserts, validates, then
  commits or restores;
- topology validation that checks neighbour reciprocity across the same edge
  and recomputes non-manifold edge claims.

The existing `MeshState::from_triangular_mesh` and
`MeshState::to_triangular_mesh` remain the conversion bridge. Existing point
location, cavity, insertion, flips, predicates, and Voronoi code are reused.

## Terminal midpoint continuation

`insert_lepp_terminal_midpoint` now composes:

```text
face -> LEPP -> interior terminal edge -> spherical midpoint
     -> degree/protected-site gates -> transactional Delaunay insertion
```

The midpoint rejects near-antipodal edges. The insertion report carries stable
site/face ids and the locally affected sites. Method-C gates preserve the
twelve original pentagon degrees and the writer's degree-seven limit. A
committed test insertion remains closed, Delaunay, deterministic, local, and
round-trips through the Method-C table rebuild.

## PostQuality production continuation

`improve_lepp_post_quality` repeatedly selects the worst violating face by a
stable deterministic order, follows its LEPP, and commits a terminal midpoint
only when the global normalized `(worst, total)` quality objective strictly
improves. Rejected candidates retain their stable face id and error in the
report, so a clean stop cannot hide gate, geometry, or transaction failures.

The production switch is explicit:

```text
&quality
  NL%lepp_post_quality = .TRUE.
  NL%lepp_post_quality_max_insertions = 50
  NL%lepp_post_quality_max_edge_km = 0.0
/
```

The quality block's `min_angle_warn_deg` is the spherical minimum-angle target;
the maximum-edge target is optional. This path currently requires a global,
spherical, closed Method-C mesh. It preserves the ordinary
`gridfile_NXP####_<mode>.nc4` and writes
`gridfile_NXP####_<mode>_lepp.nc4` plus
`method_c_lepp_post_quality.json`.

## AdaptiveHybrid and constrained-boundary continuation

AdaptiveHybrid evaluates physical target-size demand, current neighbour-size
balance, and mesh quality in parallel. It orders the collected candidates by
hardness, normalized violation, criterion id, and stable face id, then commits
transactions serially. Quality-driven insertions reuse PostQuality's global
normalized `(worst, total)` objective and roll back unless it strictly improves.
Serial commit is intentional: adjacent Delaunay
cavities mutate shared topology, while ordered read-only evaluation provides
parallel speedup without changing results across Rayon thread counts.

The constrained insertion API accepts an `earthmesh_boundary::SegmentList`.
Boundary-terminal insertion is allowed only on a protected segment; splitting
inherits its marker, and mesh plus marker state roll back together on failure.
If a proposed terminal midpoint encroaches another protected segment, that
segment is split first and the original demand remains for the next cycle.
`refine_adaptive_hybrid_constrained` drives the same AdaptiveHybrid loop after
verifying that every listed segment is a real mesh edge and every open edge is
protected. The CLI discretises named-region and output-domain boundaries as
straddling mesh edges and selects this constrained driver whenever that list is
non-empty.

The production selector is:

```text
&method_c
  NL%algorithm = 'lepp_delaunay'
  NL%max_cycles = 8
  NL%target_size_tolerance = 1.20
  NL%maximum_neighbor_size_ratio = 1.75
  NL%maximum_vertices = 5000000
  NL%maximum_insertions_per_cycle = 500000
  NL%maximum_path_length = 100000
  NL%stop_at_source_resolution = .TRUE.
  NL%minimum_triangle_angle_deg = 0.0
/
```

Quality repair is opt-in for AdaptiveHybrid: zero disables it. It is not
silently inherited from the ordinary 25-degree quality warning threshold.

## Deliberate limits

- Bowyer-Watson cavity changes or a duplicate edge-legalization path;
- M/U/W/mrow mutation inside the LEPP module;
- post-insertion relaxation;
- concurrent topology commits or conflict-graph colouring: evaluation is
  parallel and commits are serial/deterministic;
- a spherical demand spatial index for very large demand lists. The evaluator
  still tests faces against demands, but retains at most one winning candidate
  per face and reduces thread-local batches instead of materialising an empty
  result tuple for every face;
MeshState adjacency now gives LEPP a path-local incidence fast path; the
fixture-only diagnostic walker keeps its full incidence scan for malformed
non-manifold test inputs.

Those are later optimisations and must use the existing generic mesh machinery
rather than duplicate it. PostQuality remains explicit and non-default;
AdaptiveHybrid and its regional constrained route are production-wired behind
the explicit `lepp_delaunay` selector.
