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
