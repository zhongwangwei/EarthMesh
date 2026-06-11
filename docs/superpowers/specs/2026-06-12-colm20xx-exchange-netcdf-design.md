# CoLM20XX Exchange NetCDF Adapter Design

**Goal:** Formalize the first CoLM20XX-facing v3 artifact as a NetCDF exchange metadata file emitted by the `colm20xx` adapter. The file reserves sea-land-hydro coupling fields while remaining independent of triangle, hexagon, or mixed polygon topology.

## Scope

This phase writes an adapter-level metadata artifact, not a complete future CoLM20XX runtime grid, forcing, restart, or coupler file. It complements the existing CoLM2024 package coupling CSV/NetCDF and the MPAS/FVCOM adapter artifacts.

## Artifact

`adapter_colm20xx_exchange.nc`

Global attributes:

- `kind = earthmesh_colm20xx_exchange_netcdf`
- `adapter_name = colm20xx`
- `schema_version = 0.1`
- class-code meaning attributes for surface, hydro, and coast classes

Dimensions:

- `cell`: one row per canonical EarthMesh cell

Required cell variables:

- `cell_id`
- `cell_index`
- `center_lon`, `center_lat`
- `area_m2`
- `surface_class_code`
- `hydro_class_code`
- `coast_class_code`

Reserved exchange variables:

- `land_fraction`
- `ocean_fraction`
- `river_fraction`
- `coastal_fraction`
- `supports_land_ocean_exchange`
- `supports_river_land_exchange`
- `supports_river_ocean_exchange`
- `supports_land_atmos_exchange`
- `supports_ocean_atmos_exchange`

## Semantics

Fractions come from `CanonicalCell.source_fractions` when available. If a fraction is absent, the writer falls back to canonical class labels: LAND/OCEAN surface classes imply full land/ocean fraction, non-`NONE` hydro classes imply river fraction, and coast/delta/estuary labels imply coastal fraction.

Exchange support flags are derived from positive fractions:

- land-ocean: land and ocean fractions are both positive
- river-land: river and land fractions are both positive
- river-ocean: river and ocean fractions are both positive
- land-atmos: land fraction is positive
- ocean-atmos: ocean fraction is positive

## Pipeline integration

`V3PipelineResult.write_sidecars()` records this artifact as `files.exchange` in `adapter_colm20xx.json` and returns it as `adapter_colm20xx_exchange` in the sidecar path map.

## Validation

Tests must prove:

- `write_adapter_model_artifacts("colm20xx", ...)` writes the NetCDF with expected attrs, dimensions, class codes, fractions, and exchange flags.
- The v3 pipeline includes `adapter_colm20xx_exchange.nc` and records `files.exchange` in the adapter JSON sidecar.

## Non-goals

- Direct CoLM20XX runtime ingestion.
- Conservative cell-to-cell exchange weights between separate meshes.
- Replacing the CoLM2024 package coupling table.
