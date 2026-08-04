# EarthMesh architecture contract

This document describes the current Rust v3 boundaries. It is intentionally
short: implementation details belong next to the code and in
`mesh_construction_technical_guide.md`.

## Dependency direction

```text
earthmesh_core        shared constants, namelists, runtime configuration
    ↑
earthmesh_geometry    small geometry and overlay kernels
earthmesh_hfield      continuous target cell-width field
    ↑
earthmesh_mesh        mesh construction and refinement kernels
    ↑
earthmesh_quality     geometry/topology reports and gates
earthmesh_project     versioned user intent and lowering
earthmesh_refine_planner measured-cell target levels consumed through HField
    ↑
earthmesh_cli         executable orchestration and file-format adapters
    ↑
EarthMesh Studio      Tauri adapter and static frontend

extends/earthmesh_grid_preprocess   compatibility port, depends on earthmesh_mesh,
                                    depended on by nothing above
```

Arrows indicate the normal direction toward higher-level orchestration, not a
complete Cargo dependency graph. Shared physical constants live in
`earthmesh_core`; geometry and h-field code reuse that source of truth.

## `extends/`

Crates under `extends/` are verification assets, not engine layers. They may
depend on `rust/` crates; nothing in `rust/` or EarthMesh Studio may depend on
them, so removing one can never change mesh output. `earthmesh_grid_preprocess`
is the port of the `MOD_grid_preprocess.F90` triangle refinement pipeline, kept
because discrete integer topology is what makes table-level exact comparison
against the Fortran reference possible; production refinement is Method-C.

## Canonical execution paths

- A YAML/JSON project is parsed and validated by `earthmesh_project`.
- `ProjectConfig::try_lower` is the shared Project-to-engine lowering contract.
- `mkgrd.x --project` owns production CLI orchestration, including the project
  quality policy and reproducibility manifest.
- EarthMesh Studio uses the same project schema and lowering. It still stages
  GUI-created regional mask files and its run directory before launching the
  engine; it must not override lowering defaults or quality thresholds.
- Regional Projects run one bounded hydro closed loop: coarse gridfile → exact
  Project footprint → MERIT R2/R3/coast and optional CaMa linked reach corridors/mouths →
  cell-local Lambert azimuthal equal-area overlay → target-level plan → shared
  HField/Method-C engine → final overlay, coupling, and quality recomputation.
  The Method-C rerun consumes that exact measured coarse gridfile as its parent;
  it must not regenerate a nominal NXP parent because doing so invalidates cell
  identity and changes the far-field mesh. CLI and GUI invoke the same
  implementation and return the final gridfile.
- Deep hydro refinement is quality-feedback controlled rather than threshold
  relaxed: a level-3-or-deeper result whose per-cell edge-length CV gate warns
  is rebuilt once with a stricter `hfield_g=0.1` graded skirt. The manifest
  records `quality_retry_applied`; the final report retains the exact adapter
  HField target-vs-actual diagnostics and still has to pass the original gate.
- Production Project coupling requires native MERIT `stride=1`. NetCDF variables
  are read as bbox hyperslabs rather than full tiles; a native-cell halo and a
  cross-window surface index preserve coast adjacency at footprint and tile
  seams. Sparse stride values are rejected instead of being interpreted as
  physical neighbors.
- Final LOCmesh land/ocean/coupling outputs sample production landtype at the
  required mesh points with grouped, one-tile-at-a-time hyperslabs; they do not
  expand the 86,400×43,200 raster into repeated global `Vec<Vec<i32>>` copies.
  HField landtype and mean/std threshold masks likewise stream longitude
  stripes, preserve the north-to-south source axis, and retain only HField-bin
  aggregates plus one stripe.
- Same-cell hydro class overlaps remain separate coupling rows but are grouped
  by canonical `cell_id` for refinement planning and budget accounting. CaMa
  estuary source/reach metadata and conservative estuary fractions survive the
  class union into the production CoLM CSV.
- Project hydro resolves its enabled landtype layer, runs coupling-quality on
  both measured and final gridfiles, records the final coupling verdict, and
  applies a `Block` policy to coupling failures as well as mesh failures.
  That generic report declares `signal_scope=landtype_grid_only`; hydro-specific
  observability is reported separately as `estuary_coupling_rows` and in the
  CoLM `is_estuary/estuary_fraction/reach_ids` columns.
- Project hydro levels are clamped to Method-C's supported level 5. Final-grid
  Project quality is the only Block-policy gate for hydro runs; the coarse mesh
  is not accepted or rejected as though it were the final product.
- Hex quality consumes the authoritative `itab_w%im`/`n_ngrwm` W-cell rings and
  orders corners in a local spherical tangent plane, so antimeridian cells are
  part of the topology report instead of being discarded by raw longitude.
- Bbox (including antimeridian), circle, shapefile, and close domains are
  supported. Multipart domains remain multipart; hole-bearing shapefile rings
  are rejected explicitly until the domain interface carries hole topology.
  Circle domains are minor-hemisphere disks and therefore reject radii above
  one quarter of Earth's circumference.

## Flatness rule

The public architecture is flat by responsibility: each crate owns one layer,
wildcard public re-exports, deprecated output facades, and single-child forwarding
directories are forbidden by `make check-architecture`. Narrow legacy **input**
aliases may remain at parser boundaries while old Project files are supported;
they are not exposed as current GUI choices or output identifiers. GUI/CLI policy
is sourced from the Project model.

The large `earthmesh_cli` and `earthmesh_mesh` modules remain internally split
by algorithm and file-format responsibility. Moving them into a single flat
module would not reduce behavior or dependencies and would make the numerical
and topology kernels harder to verify, so physical directory flattening is not
an architecture goal.

## Quality and topology scope

- Connectivity failures include invalid indices, non-manifold edges,
  disconnected cell components, and disconnected vertex fans.
- Ocean projects carve by centre sample, which strands narrow bays and river
  mouths as orphan cells or vertex-only contacts that no refinement pass can
  repair. `NL%isolated_ocean` (on by default for `oceanmesh`, overridable via
  `expert.isolated_ocean`) keeps only the largest edge-connected water body and
  splits pinched vertex fans. It removes cells, so the run log always reports how
  many went and how many components were found.
- Guarded AutoRefine comparisons separate float noise from meaningfulness:
  exact-valued metrics (counts, `max_adjacent_resolution_ratio`) use a 1e-9
  guard, continuous whole-mesh extrema (`aspect_ratio.max`, `min_angle_deg`, …)
  use 1e-4 relative. Touching one cell in a 10^5-cell mesh moves an extremum by
  ~1e-6 in an effectively random direction; scoring that as a regression rolled
  back otherwise sound passes.
- Euler characteristic `V-E+F` is always reported and becomes a gate only when
  the input explicitly supplies an expectation. Before final mask topology is
  known, Projects supply χ=2 only for unmasked global Earth/atmosphere meshes;
  land, ocean, coupled, and regional meshes remain infer-only because masking
  may introduce boundaries, holes, or multiple components.
- Spherical area, great-circle edges, tangent-plane interior angles, and
  spherical compactness are exact spherical metrics at every valid scale.
  Euclidean triangle eta/NSR remain explicitly local compatibility metrics and
  are excluded when any cell edge exceeds 15 degrees.
