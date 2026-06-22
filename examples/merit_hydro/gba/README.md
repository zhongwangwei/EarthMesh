# MERIT-Hydro Greater Bay Area example

This example records the current 90 m MERIT-Hydro based Greater Bay Area hydro/coast mesh case.

Files:

- `case.nml` — runnable EarthMesh namelist copied from the MERIT regeneration smoke case.
- `delivery_manifest.json` — delivery-package manifest for the generated inspection/package artifacts.

Large generated GeoJSON/HTML artifacts are intentionally not copied into the repository.

## External data (required)

This is a **template** case: it needs MERIT-Hydro source data that is not shipped in the
repo. Paths use the `${EARTHMESH_DATA}` placeholder — set it to your data root before running:

```bash
export EARTHMESH_DATA=/path/to/your/earthmesh_data
# then substitute ${EARTHMESH_DATA} in case.nml / delivery_manifest.json, or edit them to your paths
```

Expected layout under `${EARTHMESH_DATA}`:

- MERIT-Hydro tiles (set your own mount, e.g. a `MERIT_Hydro` directory)
- `${EARTHMESH_DATA}/merit_package_bridge_smoke` — generated inspection/package artifacts
- `${EARTHMESH_DATA}/merit_gba_regeneration_smoke` — regeneration case workdir

The committed `case.nml` and `delivery_manifest.json` are repo-local templates with placeholder
paths only (no personal machine paths). `delivery_manifest.json` records the package layout used
to produce/inspect the current package; regenerate it on your machine.
