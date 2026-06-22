# MERIT-Hydro Yangtze Delta example

This example records the current 90 m MERIT-Hydro based Yangtze Delta hydro/coast mesh package.

Files:

- `case_or_manifest.json` — small repo-local pointer to the recommended delivery case.
- `delivery_manifest.json` — delivery-package manifest for generated inspection/package artifacts.

Large generated GeoJSON/HTML artifacts are intentionally not copied into the repository.

## External data (required)

This is a **template** case needing MERIT-Hydro source data not shipped in the repo. Paths use
the `${EARTHMESH_DATA}` placeholder — `export EARTHMESH_DATA=/path/to/your/earthmesh_data` and
substitute/edit it before running. The committed JSON files are repo-local templates with
placeholder paths only (no personal machine paths).

Source data expected under `${EARTHMESH_DATA}`:

- MERIT-Hydro tiles: `/Volumes/Data01/MERIT_Hydro`
- Scratch package: `${EARTHMESH_DATA}/merit_yangtze_N112_bridge_stride5_compact_surface`
- Background EarthMesh cells/log: `${EARTHMESH_DATA}/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.background_cells.geojson` and `${EARTHMESH_DATA}/earthmesh_hydro_close_N112_r3d3_cst20_smoke.log`

Runnable comparison namelist:

- `${EARTHMESH_DATA}/Atmos_hex_NXP112_hydro_close_yangtze_r3d3_cst20_smoke.nml`
- Fortran comparison result dir: `${EARTHMESH_DATA}/cases/ATMOS_hydro_N112_r3d3_cst20/result`
