# MERIT-Hydro Yangtze Delta example

This example records the current 90 m MERIT-Hydro based Yangtze Delta hydro/coast mesh package.

Files:

- `case_or_manifest.json` — small repo-local pointer to the recommended delivery case.
- `delivery_manifest.json` — delivery-package manifest for generated inspection/package artifacts.

Large generated GeoJSON/HTML artifacts are intentionally not copied into the repository. The manifest keeps the absolute scratch paths used to produce and inspect the current package.

Source data expected on this machine:

- MERIT-Hydro tiles: `/Volumes/Data01/MERIT_Hydro`
- Scratch package: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride5_compact_surface`
- Background EarthMesh cells/log: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.background_cells.geojson` and `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_hydro_close_N112_r3d3_cst20_smoke.log`

Runnable comparison namelist:

- `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/Atmos_hex_NXP112_hydro_close_yangtze_r3d3_cst20_smoke.nml`
- Fortran comparison result dir: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/cases/ATMOS_hydro_N112_r3d3_cst20/result`
