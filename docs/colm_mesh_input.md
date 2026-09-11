# CoLM mesh input from an EarthMesh native polygon grid

```sh
earthmesh_cli --colm-mesh-from-gridfile \
  gridfile_NXP0144_hex_landmesh.nc4 colm_mesh.nc \
  --pixels-per-degree 240
```

Use `colm_mesh.nc` as `DEF_file_mesh` for an **UNSTRUCTURED** CoLM build.
The input is the final native hex/polygon grid, after regional selection and
land-cell selection. This explicit adapter is independent of CMRC generation:
it also accepts valid native polygon grids from other supported generators.
It does not regenerate the mesh, change its angles, or extend an existing
`certified_ready` certificate to the raster.

## What is written

- `elmindex(nlat,nlon)` on disk: int32 IDs; CoLM's Fortran NetCDF API reads
  this as `val(nlon,nlat)`. The reverse disk order is **not** compatible.
- `lon_w/lon_e(nlon)` and `lat_s/lat_n(nlat)`, in degrees, with west-to-east
  longitudes and north-to-south latitudes. The resolution is explicitly chosen
  by `--pixels-per-degree`; it is not inferred from threshold statistics.
- `cell_id`, `pixel_count`, and source lineage when present: trace raster
  elements back to final native W cells. Canonical IDs are retained; dummy
  rows are not elements. These additional variables are not required by CoLM.

Every positive pixel is assigned by its center's membership in the native
**spherical, great-circle-edged** cell. Shared-edge ties use the lowest native
ID. Every delivered cell must own at least one pixel; otherwise export fails
and requests a finer raster. Invalid geometry and interior overlaps fail rather than silently assigning
another cell. Candidate polygons are intersected before sampling; the relative
intersection-area tolerance is 1e-9 for shared-edge roundoff.

Zero denotes pixels outside the represented whole-cell footprint. There is
**no nearest-cell filling**, polygon clipping to a requested boundary, or
landtype masking at this stage. Coastal cells can contain water pixels;
CoLM's mesh reader includes these positive pixels and land-patch preprocessing
classifies them later. This is not a land-only coastline raster.

The raster uses pixel-center sampling, **not conservative overlap weights**.
Pixel areas and polygon areas need not agree exactly. Reducing pixel size
reduces boundary discretization but does not certify area conservation or
simulation accuracy. The output window is limited to 268,435,456 pixels and written one row at a time.
Antimeridian or polar cells conservatively use full-longitude windows, which can
reach that limit sooner than ordinary regional meshes.
Small unsupported geometries and resource limits fail
explicitly; existing output is replaced only after successful file closure.

## Compatibility evidence and limits

Consumer source checked: CoLM202X `ebe6de998692f075216037810ce9184fa407e27b`,
`share/MOD_Grid.F90` (coordinate reading), `share/MOD_NetCDFBlock.F90`
(`nf90_get_var` x/y start/count), and `share/MOD_Mesh.F90` (positive IDs).
A nonsquare Fortran NetCDF read establishes disk dimension order independently
of the Rust writer. The shared legacy PatchID writer uses the same corrected
schema; its Rust in-memory `[lon][lat]` API is unchanged. Previously exported
transposed PatchID files need regeneration.

This is a mesh-input handoff, **not** a complete CoLM running-data package.
Raw surface fields, generated landdata, initial states, forcing, full CoLM
preprocessing, and solver validation remain separate steps. Coupling CSV/NetCDF
and forcing/restart *templates* are not substitutes for `DEF_file_mesh`.
