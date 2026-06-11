# Package Complete Surface Mask Design

**Goal:** When a hydro/coast refinement delivery package receives a raw LAND/OCEAN surface mask, it must package an EarthMesh-cell keyed complete mask so adapters can classify every cell without recomputing raster or polygon overlays.

**Requirements:**
- Preserve existing package behavior when `--surface-geojson` is absent.
- When `--surface-geojson` is supplied, write `<case>_complete_cell_mask.geojson` containing one feature per background cell.
- Complete mask properties must include `surface_class` for LAND/OCEAN-resolved cells and preserve hydro/coast flags from sparse overlays.
- `delivery_manifest.json` must list the complete mask under `files.complete_cell_mask_geojson` while keeping the original input under `source_files.surface_geojson`.
- CoLM package coupling must prefer `files.complete_cell_mask_geojson` over raw `source_files.surface_geojson`, because raw masks can be dissolved polygons and not keyed by `cell_id`.
- Backward compatibility: if only `source_files.surface_geojson` exists, continue reading it as before.

**Validation:**
- Unit tests prove package writes the complete mask and manifest path.
- Unit tests prove CoLM uses complete mask from `files` even when raw source is missing or unsuitable.
- Smoke tests run targeted pytest and a real package/coupling export when input data is available.
