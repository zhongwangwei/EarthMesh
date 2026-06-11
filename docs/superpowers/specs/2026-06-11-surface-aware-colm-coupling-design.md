# Surface-Aware CoLM Coupling Design

## Purpose

Extend the hydro/coast delivery package and CoLM coupling table so land/ocean surface class can be supplied explicitly. This closes the current metadata gap where the package-level CoLM table can mark river and coast cells but cannot distinguish ordinary LAND and OCEAN cells.

## Scope

This phase adds optional `surface_geojson` plumbing only. It does not infer land/ocean from elevation, does not rasterize MERIT masks onto EarthMesh cells, and does not change the N112 CaMa smoke outputs when no surface layer is provided.

## Architecture

Two existing modules are extended:

1. `util/hydro_mesh/refinement_package.py` accepts optional `surface_geojson` and records it in `delivery_manifest.json` under `source_files.surface_geojson`.
2. `util/hydro_mesh/colm_coupling.py` reads that optional path when present and joins surface properties by `cell_id` into the all-cell table.

The surface layer is expected to be EarthMesh-cell keyed GeoJSON. Each feature should carry either `surface_class=LAND/OCEAN/COAST/UNKNOWN` or `mask_class=LAND/OCEAN/COAST_LAND/COAST_OCEAN`. The coupling table writes normalized `surface_class` values: `LAND`, `OCEAN`, `COAST`, or `UNKNOWN`. Coast overlap remains a separate `has_coast/coast_class/coastal_fraction` field so land/ocean and coast flags are not conflated.

## Behavior

- If no `surface_geojson` is present, behavior remains unchanged: cells with coast overlap use `surface_class=COAST`; other cells use `UNKNOWN`.
- If `surface_geojson` is present, `surface_class` comes from the surface layer whenever the cell has a matching surface feature.
- `COAST_LAND` surface masks normalize to `LAND`; `COAST_OCEAN` normalizes to `OCEAN`.
- Summary JSON reports `surface_cell_count` and `surface_geojson` when available.

## Tests

Add tests proving:

1. Package writer records optional `surface_geojson` in `delivery_manifest.json`.
2. Package CLI accepts `--surface-geojson`.
3. CoLM package coupling uses surface layer values for LAND/OCEAN while retaining separate coast flags.
4. Existing N112 package without surface layer still works.
