# MERIT-Hydro Greater Bay Area example

This example records the current 90 m MERIT-Hydro based Greater Bay Area hydro/coast mesh case.

Files:

- `case.nml` — runnable EarthMesh namelist copied from the MERIT regeneration smoke case.
- `delivery_manifest.json` — delivery-package manifest for the generated inspection/package artifacts.

Large generated GeoJSON/HTML artifacts are intentionally not copied into the repository. The manifest keeps the absolute scratch paths used to produce and inspect the current package.

Source data expected on this machine:

- MERIT-Hydro tiles: `/Volumes/Data01/MERIT_Hydro`
- Scratch package: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_package_bridge_smoke`
- Regeneration case: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_gba_regeneration_smoke`
