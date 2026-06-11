# EarthMesh v3 Architecture Design

## Purpose

EarthMesh v3 should become a mode-independent mesh and coupling product platform for atmosphere, land, ocean, and future Earth-system workflows. The current `v3-hydro-mesh-cama` work is one important component, not the whole v3 architecture.

The v3 design must keep current MPAS, CoLM2024, and FVCOM support while preparing for a future CoLM20XX family where CoLM may expand from a land model into an integrated land-ocean-hydro model.

## Design Position

EarthMesh v3 is organized around three boundaries:

1. **Core schema and orchestration**: model-independent case recipes, cell metadata, component outputs, run manifests, and QA reports.
2. **Compute kernels**: heavy geometry, spatial indexing, mask intersection, conservative weights, and eventually mesh refinement kernels.
3. **Model adapters**: translation from canonical v3 products into MPAS, CoLM2024, FVCOM, CoLM20XX, or generic exchange-grid outputs.

The recommended technical stack is:

```text
Python: recipe, orchestration, I/O, adapters, QA, case management
Rust: heavy geometry, spatial index, mask merge, intersection, conservative weights
Fortran: existing v2 mesh/refinement kernel during transition
```

This avoids a risky full rewrite while allowing v3 to modernize around tested interfaces.

## Current v2 Constraints That v3 Should Address

The current Fortran v2 flow is valuable but tightly coupled:

```text
read namelist
-> initialize or read mesh
-> preprocess reference data
-> judge domain/refinement areas
-> calculate containment
-> derive refinement targets
-> run refinement loop
-> postprocess masks and model output
```

The main limitations for v3 are:

- Mode and output are hard-bound in the control layer: `landmesh` expects CoLM, `oceanmesh` expects FVCOM, and `atmosmesh` expects MPAS or MPAS-Simple.
- Configuration is deeply nested in Fortran namelists with many per-mode switches.
- New scientific products, such as hydro/coast coupling metadata, are easier to develop in Python/Rust than directly inside the Fortran global-state flow.
- Existing hydro-CaMa work already shows that future workflows require more than a mesh file: they need masks, class labels, crosswalks, QA maps, and coupling tables.

v3 should keep the proven Fortran mesh kernel available, but move orchestration and extensible products into a flatter layer.

## v3 Target Architecture

```text
earthmesh_v3/
  core/
    recipe.py
    schema.py
    manifest.py
    registry.py
    qa.py

  components/
    hydro_cama/
    coastline/
    land_surface/
    ocean_surface/
    atmosphere_forcing/
    estuary_delta/

  coupling/
    adjacency.py
    exchange_table.py
    conservative_weights.py

  adapters/
    mpas/
    fvcom/
    colm2024/
    colm20xx/
    generic_esmf/

  kernels/
    rust/
    fortran_legacy/
```

The repository does not need to move into this exact tree immediately. This is the conceptual boundary for v3 planning and incremental migration.

## Canonical Cell Schema

Every v3 cell product should carry a model-independent identity and classification layer. At minimum:

```text
cell_id
cell_index
center_lon
center_lat
area_m2
geometry_ref
surface_class
hydro_class
coast_class
mesh_priority
component_roles
source_fractions
quality_flags
```

Recommended class vocabularies:

```text
surface_class:
  LAND, OCEAN, COAST, LAKE, ICE, WETLAND, UNKNOWN

hydro_class:
  NONE, R0, R1, R2, R3, ESTUARY, DELTA

coast_class:
  NONE, COAST_LAND, COAST_OCEAN, ESTUARY, DELTA, TIDAL_FLAT, SHELF

component_roles:
  colm_land, colm_ocean, colm_coast, cama_river,
  mpas_atmos, fvcom_ocean, exchange_cell
```

A cell may carry multiple roles. For example, a river-mouth coastal cell may be:

```text
surface_class = COAST
hydro_class = ESTUARY
coast_class = ESTUARY
component_roles = [colm_land, colm_ocean, cama_river, exchange_cell]
```

This is essential for future CoLM20XX sea-land integration.


## Mesh Topology Compatibility

v3 must support triangular, hexagonal, and general polygon cells through the same canonical schema.

The core rule is:

```text
v3 core operates on polygon cells;
triangle and hexagon are special cases of polygon topology.
```

Canonical topology fields should include:

```text
cell_id
cell_type        # TRI, HEX, POLYGON, MIXED
vertices
edges
neighbors
area_m2
center_lon
center_lat
orientation
source_mesh_type
```

Topology-specific behavior belongs in kernels and adapters, not in component science logic.

Expected adapter usage:

- `mpas`: primarily hexagonal or general polygon dual meshes.
- `fvcom`: primarily triangular ocean meshes.
- `colm2024`: should consume canonical cells plus patch and coupling metadata without assuming tri or hex.
- `colm20xx`: should consume canonical land, ocean, coast, river, and exchange cells without assuming one fixed cell shape.
- `generic_esmf`: should handle arbitrary polygon exchange grids.

Hydro, coast, land, ocean, and atmosphere components should therefore emit masks and fractions against general polygon cells. This ensures that river/coast/coupling logic works for both MPAS-style hex meshes and FVCOM-style triangular meshes, and remains usable if future cases contain mixed or clipped coastal polygons.


## Hydro, Coast, and Coupling Semantics

In v3, hydro, coast, and coupling are canonical product layers rather than mesh-shape-specific features.

### Hydro

Hydro means river and hydrologic network information. The first implementation source is CaMa-Flood, but the schema should not depend on CaMa alone.

Typical hydro fields include:

```text
reach_id
hydro_class          # NONE, R0, R1, R2, R3, ESTUARY, DELTA
upstream_area_km2
river_width_m
river_length_m
downstream_reach_id
downstream_cell_id
river_fraction
river_area_m2
linked_land_cell_ids
linked_ocean_cell_ids
```

Hydro products answer questions such as: which mesh cells contain rivers, how large those rivers are, where they flow next, and how river water maps onto land or ocean cells.

### Coast

Coast means land-ocean transition information. It is not only a line; for Earth-system use it should be treated as a zone with land-side, ocean-side, estuary, delta, tidal-flat, and shelf candidates.

Typical coast fields include:

```text
surface_class        # LAND, OCEAN, COAST, LAKE, ICE, WETLAND
coast_class          # NONE, COAST_LAND, COAST_OCEAN, ESTUARY, DELTA, TIDAL_FLAT, SHELF
land_fraction
ocean_fraction
coast_fraction
coast_source
coast_distance_m
```

Coast products answer questions such as: which cells are pure land, pure ocean, mixed coast, estuary, or delta, and where refinement or exchange interfaces should be emphasized.

### Coupling

Coupling means the exchange relationships between model roles or physical components. It is independent of whether the source mesh is triangular, hexagonal, or mixed polygonal.

Typical coupling fields include:

```text
source_cell_id
target_cell_id
source_role          # river, land, ocean, atmosphere, coast
target_role
interface_type       # river_land, river_ocean, land_ocean, land_atmos, ocean_atmos
exchange_area_m2
exchange_fraction
weight
conservative
quality_flags
```

Coupling products answer questions such as: which river cell drains to which ocean cell, which land and ocean cells share a coastline interface, and what conservative exchange weight should be used.

For CoLM2024 and CoLM20XX, the adapter should consume these semantic layers instead of assuming a fixed triangle or hexagon shape. The shape matters for geometry calculations, but the model-facing product should be cell metadata plus exchange tables.

## Component Contract

Components generate canonical v3 products. They do not write model-specific final outputs directly.

A component should declare:

```text
name
version
input_sources
required_fields
output_layers
classification_vocab
spatial_coverage
quality_checks
```

Example components:

### hydro_cama

Responsibilities:

- Read CaMa-Flood binary or NetCDF maps.
- Classify reaches as R0/R1/R2/R3.
- Identify estuary and delta candidates.
- Produce river corridor masks.
- Produce river-to-mesh intersections.
- Produce CoLM-oriented river coupling metadata.

### coastline

Responsibilities:

- Generate land/ocean/coast masks from CaMa `elevtn.bin` or higher-quality shoreline products.
- Distinguish land-side and ocean-side coastal cells.
- Provide coastal refinement candidates.

### land_surface

Responsibilities:

- Attach land type, vegetation, soil, topography, LAI, and land heterogeneity information.
- Produce CoLM2024 patch and landunit mapping inputs.

### ocean_surface

Responsibilities:

- Attach bathymetry, shelf, sea-surface gradients, EKE, SST, SSH, or ocean mask data.
- Produce FVCOM or future CoLM ocean candidates.

### atmosphere_forcing

Responsibilities:

- Attach orographic, coast-transition, typhoon, precipitation-gradient, or other atmosphere-oriented refinement masks.
- Support MPAS mesh generation and Earth-system exchange products.

## Adapter Contract

Adapters translate canonical v3 products into model-specific files.

Adapters should not own scientific classification logic. They should read canonical cell/coupling products and emit the format expected by a target model.

Required initial adapters:

### mpas

Scope:

- MPAS and MPAS-Simple mesh outputs.
- Atmospheric hex mesh metadata.
- Optional surface coupling metadata for future Earth-system workflows.

### fvcom

Scope:

- FVCOM ocean triangular mesh products.
- Coastline/open-boundary metadata.
- Future river-mouth runoff boundary metadata.

### colm2024

Scope:

- Current CoLM2024 land-oriented products.
- Land cell metadata.
- Patch, landunit, soil, vegetation, and mask mappings.
- River-to-land coupling table.
- Coast and estuary metadata as optional future-facing fields.

### colm20xx

Scope:

- Future CoLM integrated land-ocean-hydro products.
- This adapter should begin as a schema reserve and validation target, not as a complete model writer.
- It should reserve fields for land-ocean, river-land, river-ocean, and atmosphere-surface exchange.

### generic_esmf

Scope:

- Generic conservative exchange-grid products.
- SCRIP/ESMF/xESMF-style weights where possible.
- Useful for coupled Earth-system testing independent of a single model.

## Coupling and Exchange Tables

v3 should produce coupling tables as first-class outputs, not as side effects.

Minimum exchange table fields:

```text
source_cell_id
target_cell_id
source_role
target_role
interface_type
exchange_area_m2
exchange_fraction
weight
conservative
quality_flags
```

Recommended interface types:

```text
land_ocean
river_land
river_ocean
coast_ocean
land_atmos
ocean_atmos
river_atmos
```

For CoLM2024, the first required exchange product is river-to-land mapping. For CoLM20XX, the schema should already reserve river-ocean and land-ocean exchange rows.

## Python and Rust Boundary

Python should own:

- CLI and case orchestration.
- YAML/JSON recipe parsing.
- Component and adapter registry.
- NetCDF, GeoJSON, CSV, JSONL, HTML, and report I/O.
- Calling the legacy Fortran kernel.
- Calling Rust compute functions.
- User-facing QA and diagnostics.

Rust should own:

- Spatial indexing.
- Large-scale geometry intersection.
- Cell-mask overlay and priority merge.
- Conservative remapping weights.
- Fast adjacency graph construction.
- Large regional/global mask operations.
- Shape-agnostic polygon operations that work for triangular, hexagonal, and mixed cells.

For Python/Rust integration, the preferred interface is:

```text
PyO3 + maturin
```

The first Rust crate should be small and focused, not a full mesh generator.

Recommended Rust MVP:

```text
Given mesh cell polygons and multiple classified mask layers,
return per-cell class fractions, winning class labels, source fractions,
and QA counters.
```

This MVP directly supports hydro-CaMa, coastline refinement, CoLM2024 coupling, and future CoLM20XX exchange tables.

## Fortran Legacy Kernel Bridge

The v2 Fortran kernel remains important during v3 development.

Keep using Fortran for:

- Existing tri/hex mesh generation.
- Current refinement loop.
- Spring adjustment.
- Weak-concavity cleanup.
- Existing MPAS/FVCOM/CoLM output compatibility.

Do not rewrite the full Fortran kernel at the start. Instead, wrap it from Python:

```text
v3 recipe -> generated namelist/masks -> mkgrd.x -> canonical postprocessing -> adapters/QA
```

Later, after v3 schema and QA are stable, individual Fortran responsibilities can be replaced by Rust kernels if tests show a clear benefit.

## Hydro-CaMa Position in v3

`v3-hydro-mesh-cama` should become a component, not a branch-specific workflow.

Its long-term name should be close to:

```text
components/hydro_cama
```

or, if expanded to coast/exchange logic:

```text
components/hydro_coast_coupling
```

Required outputs:

```text
R0/R1/R2/R3 river classification
LAND/OCEAN/COAST surface classification
ESTUARY/DELTA candidates
mesh refinement masks
river-to-cell intersections
CoLM2024 river coupling table
CoLM20XX reserved exchange metadata
HTML/PNG/JSON QA
```

## Recipe Shape

A v3 case should be configured with a flat recipe rather than a long mode-specific namelist.

Example:

```yaml
case:
  name: china_coast_hydro_v1
  output_dir: /path/to/case

mesh:
  grid: hex
  base_resolution: N160
  kernel: fortran_legacy

region:
  bbox: [73, 3, 136, 54]

components:
  - type: hydro_cama
    source: /path/to/glb_01min
    classes: [R2, R3]

  - type: coastline
    source: cama_elevtn
    coast_radius_cells: 3

adapters:
  - mpas
  - colm2024
  - fvcom
  - colm20xx_schema

qa:
  html: true
  png: true
  summary_json: true
```

The recipe should compile to legacy Fortran namelists and external masks when the Fortran kernel is used.

## Migration Plan

### Phase 1: v3 schema and wrapper

- Define canonical cell and coupling schemas.
- Define component and adapter interfaces.
- Add Python recipe parser.
- Wrap existing Fortran `mkgrd.x` runs.
- Keep current outputs unchanged.

### Phase 2: formalize hydro-CaMa component

- Move current hydro-CaMa utilities behind component APIs.
- Preserve current tests and add schema-level tests.
- Emit canonical cell masks and coupling products.
- Keep HTML/PNG QA outputs.

### Phase 3: add Rust geometry MVP

- Implement fast mask-cell intersection.
- Implement priority/fraction merge.
- Compare Rust output against current Python/Shapely outputs on small fixtures.
- Use Rust only where results match and performance improves.

### Phase 4: adapter family

- Formalize MPAS adapter.
- Formalize FVCOM adapter.
- Formalize CoLM2024 adapter.
- Add CoLM20XX schema-only adapter for future integration.
- Add generic exchange-grid adapter.

### Phase 5: future kernel migration

- Identify Fortran kernel bottlenecks.
- Replace isolated geometry or adjacency functions only after schema-level regression tests exist.
- Defer full mesh refinement rewrite until the v3 platform is stable.

## Validation Strategy

Every v3 run should produce a manifest with:

```text
recipe hash
component versions
adapter versions
input source inventory
mask counts
cell count by class
missing mask count
cell size distribution
intersection/coupling row counts
QA artifact paths
warnings
```

Required tests:

- Schema validation tests.
- Component contract tests.
- Adapter output smoke tests.
- Rust/Python parity tests for geometry operations.
- Legacy Fortran wrapper smoke tests.
- CoLM2024 coupling table tests.
- CoLM20XX reserved schema tests.

## Design Decisions

1. v3 core is model-independent.
2. MPAS, CoLM2024, FVCOM, and CoLM20XX are adapters, not separate cores.
3. CoLM2024 support remains current and concrete.
4. CoLM20XX support starts as a future-facing schema and exchange-table reserve.
5. hydro-CaMa is a v3 component, not the whole v3.
6. Python is the orchestration layer.
7. Rust is introduced first for heavy geometry and mask/coupling computation.
8. The existing Fortran kernel remains active until replacement is justified by tests and parity evidence.
9. v3 internal geometry is shape-agnostic polygon topology; triangular and hexagonal meshes are both first-class supported cases.

## Non-Goals for the First v3 Implementation

- Full rewrite of the Fortran mesh refinement kernel.
- Full implementation of a future CoLM20XX model writer.
- Removing existing MPAS, CoLM2024, or FVCOM support.
- Replacing all Shapely/Python utilities before Rust parity tests exist.
- Changing scientific thresholds without documented QA cases.

## Immediate Next Step

Create an implementation plan for Phase 1:

1. Add canonical schema definitions.
2. Add a v3 recipe parser.
3. Add a run manifest format.
4. Add a legacy Fortran wrapper boundary.
5. Add adapter stubs for MPAS, CoLM2024, FVCOM, and CoLM20XX schema validation.
6. Keep `v3-hydro-mesh-cama` as an existing component candidate and connect it only after the schema boundary is stable.

## CoLM2024 and CoLM20XX Adapter Specification v0.2

This section turns the future-facing CoLM notes above into an adapter contract.
The key design rule is that CoLM adapters consume canonical v3 polygon products;
they must not bind EarthMesh v3 to triangular or hexagonal topology.  Triangle,
hexagon, and mixed coastal polygons are geometry details below the semantic
handoff layer.

### CoLM2024 adapter: concrete current handoff

CoLM2024 remains the current, concrete land-model target.  The adapter should emit
land-oriented products that can be validated now:

- canonical all-cell table with stable `cell_id`, `cell_index`, geometry,
  `surface_class`, `hydro_class`, `coast_class`, `component_roles`,
  `source_fractions`, and `quality_flags`;
- land/river/coast mask tables for CoLM preprocessing;
- river-to-land coupling rows for CaMa/MERIT river cells that intersect CoLM land
  cells;
- optional coastal metadata (`COAST_LAND`, `COAST_OCEAN`, `ESTUARY`, `DELTA`) as
  future-facing fields, while still allowing a land-only CoLM2024 run to ignore
  them;
- QA report proving complete cell coverage, no `UNKNOWN` surface class in the
  promoted package, non-empty river/coast overlays when requested, and row-count
  consistency with the background mesh.

CoLM2024 should therefore be strict about operational completeness but conservative
about model coupling: it can carry ocean/coast metadata without requiring the
current model executable to ingest all future fields.

### CoLM20XX adapter: reserved integrated Earth-system contract

CoLM20XX is a reserved contract for a future land-ocean-hydro CoLM family.  It
should start as a schema and validation target, not as a claim that the future
model I/O is finalized.  Its first deliverable is an exchange NetCDF plus manifest
that makes the following relationships explicit:

```text
cell_id, cell_index, cell_type
surface_class: LAND/OCEAN/COAST/LAKE/ICE/WETLAND/UNKNOWN
hydro_class: NONE/R0/R1/R2/R3/ESTUARY/DELTA
coast_class: NONE/COAST_LAND/COAST_OCEAN/ESTUARY/DELTA/TIDAL_FLAT/SHELF
component_roles: colm_land, colm_ocean, colm_coast, cama_river, exchange_cell
source_fractions: LAND, OCEAN, COAST, R2, R3, ESTUARY, DELTA
exchange booleans: supports_land_ocean_exchange, supports_river_land_exchange, supports_river_ocean_exchange
```

The current `colm20xx` adapter artifact is intentionally named as an exchange
schema (`adapter_colm20xx_exchange.nc`).  It reserves model-facing concepts while
keeping final variable names and model-side control files outside the current
scope.

### Shape compatibility requirements

CoLM2024 and CoLM20XX adapters must accept:

- `TRI` cells from FVCOM-style ocean meshes;
- `HEX` cells from MPAS/EarthMesh dual meshes;
- `POLYGON` cells from clipped coast, estuary, delta, or exchange-grid products;
- `MIXED` collections when a case combines multiple source meshes.

Adapter QA should reject missing semantics, not valid topology shapes.  A triangle
or hexagon with complete surface/hydro/coast roles is acceptable; a hexagon with
missing mask coverage or an `UNKNOWN` promoted surface class is not.

### Promotion gates

A CoLM2024 or CoLM20XX handoff is promotable only when the run manifest records:

1. complete canonical cell coverage for the target background mesh;
2. explicit LAND/OCEAN/COAST or documented land-only scope;
3. no unexpected `UNKNOWN` cells in promoted products;
4. river/coast overlap counts above the case-specific minimum;
5. all adapter-required fields present, including `component_roles` and
   `source_fractions`;
6. topology bounds appropriate for the target mesh family, for example final
   EarthMesh hex regeneration should keep `n_ngrwm` within 5-7 before MPAS/CoLM
   promotion;
7. reproducible input inventory and adapter bundle manifest paths.

### Near-term implementation order

1. Keep CoLM2024 package export concrete and QA-gated.
2. Keep CoLM20XX as schema-only plus exchange NetCDF until model-side naming is
   known.
3. Add exchange-link tables after canonical cell masks are stable.
4. Move heavy overlay/intersection/fraction calculations to Rust only after Python
   parity tests pass.
5. Preserve MPAS/FVCOM adapters as first-class outputs; CoLM adapters should reuse
   the same canonical cells rather than introduce a separate mesh core.
