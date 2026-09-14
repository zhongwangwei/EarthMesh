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

rust/earthmesh_refine_redgreen   retained refinement backend, depends on
                                    earthmesh_mesh, used by earthmesh_cli
```

Arrows indicate the normal direction toward higher-level orchestration, not a
complete Cargo dependency graph. Shared physical constants live in
`earthmesh_core`; geometry and h-field code reuse that source of truth.

## Three retained refinement backends

`earthmesh_refine_redgreen` is red-green refinement: mark any set of triangles,
split each of them into four, and close the seams by halving the neighbours the
split left hanging. `earthmesh_refine_method_c` is Method-C, which subdivides a
closed region and surrounds it with transition rows. `earthmesh_refine_certified`
is CMRC, which starts from a certified mother grid and only accepts
reverse-coarsening changes that preserve the active certificate contract.

HARP-DV was retired before this active architecture contract: `harp_dv` backend
names and `&harp_dv` namelist sections now fail explicitly instead of falling
back to another backend.

The backends consume shared demand data through algorithm-specific adapters;
they do not promise identical admissible markings. Red-green grows the marked
set to close refinement seams. Canonical Method-C additionally requires its
stride-3 lattice and supported transition patches. CMRC refuses coarsening
changes that violate its primal/dual/physical/balance contracts. Algorithm
success alone does not establish that the final model product is valid.

Every published HEX product must have 5, 6, or 7 edges per physical cell,
regardless of backend. TRI products publish 3-edge triangles; their intermediate
W-point fans are not subject to the HEX degree contract. Raw mother grids and
intermediate representations are distinct from published physical cells.

## Requirement, algorithm, scope, and delivery

The production GUI / `--project` route is:

```text
Project demand and source configuration
  -> validation and shared lowering
  -> selected refinement backend
  -> final domain extraction / AutoRefine / hydro selection
  -> final physical-cell, topology, geometry and policy admission
  -> requested model-format adapter and explicit delivery report
```

`target.kind`, domain, data/threshold sources, and model format express the
land/atmosphere/ocean differences; they do not select separate copies of the
refinement algorithms. Backend capability limits remain explicit:

- Canonical Method-C consumes its structured route or gradient-limited HField.
- Method-C LEPP-Delaunay and Red-Green consume supported named/adaptive demands,
  not the canonical Method-C HField route.
- CMRC consumes certifiable named/threshold/hydro requirements, not Project's
  point-radius adaptive or canonical Method-C HField route. Its internal demand
  construction may still use an HField; that is not a public route substitution.
  Regional Earth/atmosphere/land publication accepts bbox/circle/close regions
  and unions of their whole-cell selections for TRI or HEX, using the same
  selector after CMRC. The publication stage requires landtype data only for
  surface-masked land/ocean targets; unmasked regional Earth/atmosphere
  publication applies no land mask
  (refinement data requirements remain independent). TRI retains its centre and
  all vertices inside the same member; HEX retains whole cells by centre.
  Overlapping or repeated members do not duplicate cells. Project/GUI inputs
  reuse multipart Shapefile (or Close PolygonShp) boundary sources; no separate
  union editor is required. This is not polygon Boolean merging: TRI selection
  can omit cells crossing member seams, even if members overlap.
  Regional ocean publication remains TRI + a single close polygon with the
  existing clean-ocean boundary path; ocean unions remain unsupported.
  Regional products retain the closed parent and scope remap/certification to it;
  only HEX may claim a certified dual-cell subset, while TRI claims a face subset.
- Active statistical thresholds require an enabled consumer even without an
  independent threshold region. Turning adaptive off without selecting HField
  or CMRC is valid for named regions only, not for active statistical demands.
  Dormant algorithm parameters remain preserved for round-trip editing.
- Active Method-C LEPP / post-quality ownership is checked before legacy backend
  dispatch, including CMRC; another selected backend cannot silently ignore it.
- `quality_policy=domain_export` remains preflight-only and is rejected by
  lowering. Existing regional extraction/export adapters are a separate feature.

Only unmasked global Earth/Atmosphere Projects require one closed sphere
(Euler characteristic 2, no boundary edges). Regional and surface-masked outputs
have their own boundary/component semantics. CMRC's closed mother-grid
certificate does not replace checks on an extracted region.

Project model writers run after `admit_project_final_gridfile`. ICON/FVCOM
require TRI; MPAS-family delivery requires HEX. CoLM raster delivery supports
either cell kind but must be configured explicitly. A `native_only` delivery
report is not a claim that a specialized model artifact was produced.
Regional MPAS/ICON mesh delivery does not supply atmospheric boundary forcing or
validate a model solver run.

Direct legacy namelist and library entrypoints retain some local output
orchestration. Their refined publication helpers reuse the physical HEX degree
gate, but that alone is not Project's full final admission or delivery report.
The restart final handoffs now share full native admission as described below;
this does not cover every standalone producer. Keep raw/intermediate writers usable; do not claim entrypoint equivalence merely
because both paths call the same mesh algorithm.

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
- Candidate HEX diagnostics may order corners in a local spherical tangent
  plane. Final Project admission instead audits the stored `itab_w%im`/`n_ngrwm`
  rings without per-cell reordering; a single whole-mesh winding conversion is
  allowed, but mixed winding and shared-edge defects remain visible.
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

The naming check rejects explicit source-origin labels such as `Fortran reference`,
`reference_fortran`, and `v2_reference`, plus modules named `reference` or
`reference_*`. Ordinary mathematical reference values and persisted file-format
keys are allowed; existing NetCDF keys must not be renamed to satisfy a naming
lint. `make check-architecture-selftest` exercises both accepted and rejected
fixtures, including failed checks and unwritable reports.

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
- A run reports what it produced, not what it was asked for. `refine_max_level`
  is derived from configuration before any refinement happens, so it cannot
  distinguish a fully realized run from one that refined nothing;
  `refine_realized_max_level` is measured from the produced mesh, and the
  h-field anchor counts (`requested`/`covered`/`boundary_clipped`) say whether a
  shortfall came from an empty demand or from demand that was dropped.
- The h-field raster is derived from the target resolution rather than fixed,
  and both ends of its usable range fail. Too coarse aliases the level map and
  Method-C rejects the mask; too fine resolves demand narrower than one rad3
  footprint, which can only be refined where a footprint happens to fit. The
  selection therefore measures unmet demand directly — demanded faces that the
  selection does not cover — and refuses to deliver a mesh missing most of what
  was asked for. Clipping the parent apron row stays deliberate: those anchors
  are refined by the footprint anchored elsewhere in their own component.
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

### Legacy final-delivery boundary

`project_quality::admit_final_gridfile` shares the stored physical-cell,
geometry, topology, and domain-scope checks without constructing or lowering a
Project. Project retains its own quality policy, candidate diagnostics and
AutoRefine loop. Raw restart carriers and preprocessing outputs are not final
products and are not admitted by this API automatically.

The legacy atmosphere restart MPAS and MPAS-Simple adapters now call this gate
before reading cellwidth or writing model files. They deliver **W polygons**,
even when the legacy source filename uses `mode_grid='tri'`; admission therefore
checks HEX 5–7-sided cells on one closed sphere. This does not change Project's
TRI + MPAS native-only capability. Open/regional products must use the regional
adapter with its parent-mesh context, not these `_global` adapters.

The existing model filenames and cellwidth-derived densities remain unchanged.
`result/final_quality/<MPAS|MPAS-Simple>/legacy_delivery.json` records the physical
target, source mode, final quality and artifacts returned by this attempt only.
The previous completion record is retired before a new final attempt, so an
admission/adapter failure cannot leave a current success marker. MPAS mesh and
graph files are staged and published together with that completion record last.

The Earth/land/ocean restart final handoffs (including both ocean restart
routes) also call shared admission. Earth requires a closed sphere only when
`mask_domain_global && !mask_patch_on`; masked/regional Earth and land/ocean
use boundary-aware checks without imposing χ=2. Physical cells follow
`mode_grid`: TRI triangles or HEX 5–7-sided polygons. Final native mesh and
embedded ocean boundary order are staged first, then admitted, then patchtype,
Earth-info and OBC sidecars are staged. Completion is recorded under
`result/final_quality/<final-gridfile-stem>/legacy_delivery.json` only after
these outputs succeed. Patchtype/info/OBC are `auxiliary_artifacts`, not proof
that a CoLM/FVCOM adapter ran; delivery remains `native_only`.

Public low-level composition helpers retain unchecked candidate/preprocessor
use, including Project clean-ocean processing. `defer_model_exports` does not
bypass native admission in the Earth/land/ocean final handoffs. Successful
low-level outputs are unchanged. The final-only wrappers share
`project_delivery::LegacyDeliveryStage`: private staging is on each output's
filesystem, output/input/diagnostic aliases are rejected, and I/O publication
failure restores previous native/model/auxiliary files. A completion path that
aliases an input or output is rejected before retirement to protect its bytes.
Quality files remain live **last-attempt diagnostics**, not transactional
readiness; their mesh name refers to the intended published path. Consumers
must use `legacy_delivery.json`, not the presence of individual files, as the
completion signal. Rollback errors retain recovery backups and report their
paths without restoring readiness prematurely. This is not crash-atomic
publication or isolation for concurrent readers/writers.

The standalone, no-Project default CLI now also admits and stages the **unmasked
global base gridinit** output (generated or imported). Explicit final-base
ownership is passed through the existing dispatcher, independently of
`defer_model_exports`; raw library gridinit and Project candidate calls remain
unchecked. This native-only handoff uses TRI/HEX physical cells and closed-sphere
χ=2 admission, and writes `gridfile/final_quality/<stem>/legacy_delivery.json`
last. It does not claim that the requested MPAS/FVCOM/etc. adapter ran.

Workspace destinations are authorized before creating staging or retiring
readiness. This final-base path preserves the previous workspace rather than
deleting it, protects source/native files from `namelist.save` aliases, and
redirects only converter/generator output paths into staging. Failure preserves
previous native bytes and withdraws readiness; successful reports contain only
published paths. Workspace setup and quality diagnostics are not transactional.

Standalone simple **regional base** clipping (bbox/circle/close) and land/sea
centre-sample carving now use that same explicit final ownership. Generation
and import share one raw carrier helper with the original input config; the
existing clip/carve kernel runs privately before boundary-aware admission
(`expected_euler_characteristic=None`). The full mother is retained as an
unchecked `raw_parent` auxiliary under `tmpfile/*_clip_raw_*.nc4`, including
pure landtype carving: it must not overwrite an already admitted global base
or leave that base's completion record certifying unchecked replacement bytes.
Only selected native + raw parent are published together, with readiness last.
The typed region reader consumes domain files directly, so this final-only
path omits redundant legacy Mask_make caches; raw APIs retain their preprocessing.
Empty clip/carve failures preserve the prior native and mother and withdraw
readiness. Neither native-only completion nor raw-parent ancestry implies a
model adapter ran.

Standalone clean-ocean TRI close+landtype uses the same final-base handoff and
the existing clean boundary algorithm in private staging. Native metadata and
embedded OBC are completed before regional admission; native, unchecked raw
parent, OBC/OBCv2 and any requested FVCOM output publish as one rollback bundle.
Only `output_format='FVCOM' && !defer_model_exports` runs the final-file FVCOM
adapter and records `model_delivered`; other/deferred formats record native and
auxiliary delivery only. Historical model files are not evidence of this
attempt's delivery. A close boundary transformed into a cap/union uses simple
clip/carve, without inventing clean OBC context or claiming FVCOM delivery.

Standalone **refined final output** now uses one final-only wrapper for
Method-C (including HField, adaptive and LEPP), Red-Green and CMRC. The default
CLI, restart-to-refine handoff and explicit refinement flags select it; raw
public refinement APIs and Project candidate calls do not. Only producer output
paths move into a private workspace: original demand configuration and source
paths stay unchanged. Current returned native, raw-parent, coupled, LEPP and
CMRC artifacts are staged together, with JSON references remapped to published
paths. An unchecked initial carrier lives under `tmpfile/refine_source_*`, not
over a previously admitted global base.

The selected spherical product and any delivered land/ocean or LEPP native
siblings pass shared physical-cell, topology and geometry admission. A full
mother uses χ=2; regional/masked children use boundary-aware checks. Then the
existing final-file adapters deliver only supported formats with actual
producer context: MPAS needs persisted widths (and a parent for regional
extraction), bounded FVCOM needs embedded OBC, and regional ICON needs a parent.
Missing context is recorded as native-only, not invented. Legacy CoLM coupling
files remain auxiliary metadata, not raster/model readiness. CMRC retains its
own certificate and existing model products. `defer_model_exports` does not
bypass native admission. The whole bundle publishes with
`result/final_quality/refinement/legacy_delivery.json` last; a failed rerun
preserves prior data bytes but withdraws that readiness record.

Patch-on **base** delivery reuses the same global/regional final flows. Legacy
Mask_make patch caches are staged as auxiliary files and explicitly labelled
`patch_preprocessing_only`: they do not change base geometry. Such a global
base still requires χ=2, rather than being misclassified as an extracted patch.

This is not blanket certification of low-level APIs. Explicit preprocessing
and raw public/Project composition remain unchecked carriers. Cartesian-XY
refinement keeps transactional native output but does not run spherical
admission or claim final/model readiness; it emits an explicit diagnostic and
no `legacy_delivery.json`. Do not gate intermediate writers to simulate
coverage or apply closed-sphere checks to masked/Cartesian products.
