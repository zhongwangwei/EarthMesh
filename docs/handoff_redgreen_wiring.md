# Task: route `refinement.backend = RedGreen` through the refinement pipeline

Repository: `EarthMesh` (Rust). Branch: `ocean-carve-topology-and-spring-defaults`.
Start from a clean tree — `cargo test --workspace --release` is currently
**1565 passed / 0 failed**, and `cargo clippy --workspace --all-targets` and
`cargo fmt --all -- --check` are clean. Keep it that way at every commit.

## What already exists

A second refinement backend, **red-green** (`rust/earthmesh_refine_redgreen`),
is complete and tested: mark any set of triangles, split each into four, close
the seams by halving the neighbours left hanging, Lawson-flip the angles back.
Unlike Method-C it never refuses a region for its shape — its judge chain grows
a marking until the triangulation closes. That is the whole reason it exists:
Method-C refuses coastal regions whose shape came from data (a global run had
25 of 59 tiles refused), and criteria-driven refinement is currently suspended
on it for that reason.

The backend choice already travels end to end:

```
refinement.backend (project YAML)
  -> earthmesh_project lowering  -> mkgrd.refine_backend
  -> NL%refine_backend in project.nml
  -> EarthmeshConfig::refine_backend  ("method_c" | "red_green")
  -> rust/earthmesh_cli/src/refine_pipeline/global_source.rs   <-- refused here
```

Everything between a marking and a gridfile exists, each piece with its own
test plus one test that says they compose:

| step | function |
|---|---|
| mesh bridge in | `earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&TriangularMesh, &[IcosahedronMPointNeighbors]) -> io::Result<RedGreenMesh>` |
| settings per level | `earthmesh_cli::redgreen_bridge::redgreen_settings_for_level(&RefineConfig, level) -> RedGreenSettings` |
| regions to marking | `earthmesh_cli::redgreen_bridge::redgreen_marking_from_regions(&RedGreenMesh, &[RefinementRegion], level) -> Vec<i32>` |
| one whole level | `earthmesh_cli::redgreen_bridge::refine_redgreen_level(&RedGreenMesh, &[RefinementRegion], &RefineConfig, level, previous_marks: Option<&[i32]>) -> io::Result<(UnstructuredMesh, RedGreenOutcome)>` |
| mesh bridge out | `earthmesh_cli::redgreen_bridge::unstructured_mesh_from_redgreen(&RedGreenMesh) -> io::Result<UnstructuredMesh>` |

Three prerequisites in `global_source.rs` are already done:

- `pentagon_indices` is taken from the base mesh (`mesh.impent`), not from the
  refined Voronoi `state`.
- `realized_max_level` and all eight Method-C metadata arrays live behind one
  `Option<MethodCMetadataOwned>` bound before the call that borrows them.
- `write_refined_outputs` (renamed from `write_method_c_refined_outputs`) takes
  `&UnstructuredMesh` and an **optional** `MethodCMetadataSlices`, so a backend
  with no generations or ancestry to report passes `None`.

Past the `state` block, `state` therefore has exactly **one** consumer that is
not the metadata: `output_mesh`.

## The work, in two commits

### Commit 1 — extract, no behaviour change

In `rust/earthmesh_cli/src/refine_pipeline/global_source.rs` the refinement is a
single `if / else if` expression of roughly 250 lines, running from the adaptive
branch (~line 450), through the h-field branch (~520–632), to the spring /
cartesian-xy / atmosphere tail branches (~633–681), and binding its result to
`mesh`. It ends immediately before:

```rust
let transition_faces = mesh.boundary_rows().len();
```

Lift that whole expression into a private function in the same module, e.g.

```rust
fn refine_with_method_c(/* the values it closes over */)
    -> io::Result<(TriangularMesh, Vec<RefinePass>, HfieldDiagnostics)>
```

Return whatever the current expression assigns plus the two values it writes by
side effect (`hfield_diagnostics`, and the `adaptive_run` tuple if it is set
inside). Change nothing else. This commit must leave every test passing and the
diff readable as a pure move.

### Commit 2 — the branch

```rust
let (state, output_mesh, method_c_metadata) = match config.refine_backend.trim() {
    "red_green" => { /* below */ }
    _           => { /* today's path: refine_with_method_c(..) -> Voronoi/PCVT
                        -> gridfile_mesh_from_one_based_state -> Some(metadata) */ }
};
```

The red-green arm:

1. `redgreen_mesh_from_triangular(&mesh, &m_neighbors)` on the **unrefined** mesh.
2. For `level` in `1..=max_level`, `refine_redgreen_level(...)`, passing the
   previous level's marking as `previous_marks` so a deeper level stays inside
   the one above (that is what `RedGreenSettings::halo` erodes against).
   **A round renumbers**: carry the previous marking through
   `RedGreenOutcome::cell_renumbering` before handing it to the next level.
   Do not reuse the array you built last time.
3. Result: `state = None`, `output_mesh` from the last level's `UnstructuredMesh`,
   `method_c_metadata = None`.

Then delete the `refine_backend == "red_green"` refusal that currently sits at
the adaptive call site in the same file.

`state` becomes `Option`; the metadata block already is. `realized_max_level`
already returns 0 when the metadata is absent, which is the honest answer — it
means "not measured from this mesh".

## Verification

Test only what you changed while iterating; run the full suite before each
commit. This repository has **two Cargo workspaces** — `gui-tauri/src-tauri` is
not a member of the root one, so a root-level `cargo test --workspace` reports
success without covering it. You are not touching the GUI, so the root gates
suffice:

```
cargo test --workspace --release
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```

There is no end-to-end fixture for the red-green route yet. Add one: a small
project (NXP ~21) with a named circle and `refinement.backend: RedGreen`,
asserting the gridfile has more cells than the unrefined mesh. Without it the
branch is untested and a wrong `output_mesh` produces a file that opens fine.

## Things that will bite

- **Do not reuse a marking across levels without the renumbering.** It will not
  error; it will refine the wrong triangles.
- **Method-C's five-table pattern.** Repeatedly in this port the Fortran
  allocates a table by *count* and pads it with a placeholder, while the Rust
  returned only the rows it filled. Five such tables have been fixed. If a
  consumer refuses a length that looks correct, suspect this before suspecting
  the consumer.
- **`realized_max_level = 0` for red-green is deliberate.** Do not synthesise a
  level count to make a log line look better; the requested `max_level` travels
  separately.
- Read `CLAUDE.md` and `docs/mesh_construction_technical_guide.md` sections 3, 4
  and 11 before starting. Section 11.1 lists five silent-failure classes found
  in this code; every one produced a mesh that was valid, passed its quality
  gates, and was not what the project asked for. That is the failure mode to
  design against here.
