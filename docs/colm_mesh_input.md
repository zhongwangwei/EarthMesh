# CoLM mesh input from an EarthMesh native polygon grid

```sh
earthmesh_cli --colm-mesh-from-gridfile \
  gridfile_NXP0144_hex_landmesh.nc4 colm_mesh.nc \
  --pixels-per-degree 240 --kind hex

earthmesh_cli --colm-mesh-from-gridfile \
  gridfile_NXP0192_tri_oceanmesh.nc4 colm_mesh_tri.nc \
  --pixels-per-degree 240 --kind tri
```

Use `colm_mesh.nc` as `DEF_file_mesh` for an **UNSTRUCTURED** CoLM build.
The input is the final native grid, after regional selection and domain-cell
selection. `--kind hex` (the backward-compatible default) rasterizes native W
polygon cells from `itab_w%im`; `--kind tri` rasterizes native M triangular cells
from `itab_m%iw`. A gridfile can carry both M and W views, so topology is
an explicit option and is never inferred from the filename. This adapter is
independent of CMRC generation: it also accepts valid native grids from other
supported generators.
It does not regenerate the mesh, change its angles, or extend an existing
`certified_ready` certificate to the raster.

## What is written

- `elmindex(nlat,nlon)` on disk: int32 IDs; CoLM's Fortran NetCDF API reads
  this as `val(nlon,nlat)`. The reverse disk order is **not** compatible.
- `lon_w/lon_e(nlon)` and `lat_s/lat_n(nlat)`, in degrees, with west-to-east
  longitudes and north-to-south latitudes. The resolution is explicitly chosen
  by `--pixels-per-degree`; it is not inferred from threshold statistics.
- `longitude(nlon)` and `latitude(nlat)`: float64 pixel-center coordinates,
  computed as edge midpoints, matching the supplied Pearl River PatchID example.
- `cell_id`, `pixel_count`, and source lineage when present: trace raster
  elements back to the selected native cells. For `--kind hex` they are final W
  cell IDs and `earthmesh_w_lineage`; for `--kind tri` they are final M triangle
  cell IDs and `earthmesh_m_lineage`. Canonical IDs are retained; dummy rows are
  not elements. These additional variables are not required by CoLM.

Every positive pixel is assigned by its center's membership in the selected
native **spherical, great-circle-edged** cell. TRI mode uses genuine M triangles,
not synthetic three-vertex W cells or W dual polygons. HEX mode preserves the
existing W polygon semantics. Shared-edge ties use the lowest selected native
ID. Every delivered cell must own at least one pixel; otherwise export fails
and requests a finer raster. Invalid geometry and interior overlaps fail rather than silently assigning
another cell. Candidate polygons are intersected before sampling; the relative
intersection-area tolerance is 1e-9 for shared-edge roundoff.

Zero denotes pixels outside the represented whole-cell footprint. There is
**no nearest-cell filling**, polygon clipping to a requested boundary, topology
conversion, or landtype masking at this stage. Coastal cells can contain water pixels;
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

## Real CoLM mesh-stage smoke

With an existing CoLM source checkout, GNU `mpifort`, `make`, `nf-config`,
`nc-config`, and Python 3.12+ with NumPy/netCDF4 already installed:

```sh
python -B scripts/test_colm_mesh_smoke.py
python -B scripts/run_colm_mesh_smoke.py \
  --colm-repo /path/to/CoLM202X \
  --revision ebe6de998692f075216037810ce9184fa407e27b \
  --out /tmp/earthmesh-colm-serial \
  --mesh regional=/path/to/colm_mesh.nc
# Repeat with a NEW --out and --mpi-ranks 3 for MPI IO/worker coverage.
```

The runner archives the specified committed CoLM source into a new directory;
local model edits are recorded but are neither used nor changed. It switches
only the archived spatial macros to UNSTRUCTURED and selects serial/MPI,
leaving other physics macros untouched. It builds the real CoLM modules with
bounds/FPE checks, calls `mesh_build` and `landelm_build`, saves native mesh and
land-element files, and reloads them in a **fresh process**. No CoLM routine is
replaced by a mock. `2005` is only the restart directory label for this smoke,
not an assertion about the raster's land-cover epoch.

Each phase checks model `landelm` consistency and dumps worker memberships.
The independent Python audit maps CoLM's union-grid **edges** back to input
pixels and checks all cell IDs, counts and ownerships, including missing and
duplicate pixels. A zero exit or model marker alone is not acceptance; only
`EARTHMESH_COLM_MESH_ROUNDTRIP_OK` after both audits indicates success.
The model's block-vector landelm files are block-suffixed; there is no required
`landelm/2005/landelm.nc` index file.

Use short, shell-safe output paths, as stock CoLM uses fixed-length filenames
and unquoted internal directory commands. Outputs must be new. The runner
records source/input/binary hashes, compiler options, commands, logs and timing.
It reads the regional raster for audit and stores full membership dumps; this
is a bounded acceptance tool, not a streaming global-data pipeline. CoLM block
boundaries must align with the raster for exact one-pixel roundtrip; the smoke
uses 5-degree blocks (including the tested 240 pixels/degree exports). A split
pixel fails explicitly instead of being counted as an equivalent input pixel.

This stops before land-only filtering, landpatch/PFT/crop construction and
surface aggregation. Coastal water pixels are retained as in the input.
A full default CoLM preprocessing run additionally needs compatible landtype,
`plant_15s` PFT/LAI/SAI/height tiles, `global_CFT_surface_data.nc`, lake depth,
soil fields and topography. Atmospheric forcing is not a prerequisite for this
mesh stage or for surface-data generation; it is a later simulation input.

Observed compatibility limit (2026-09-11): the pinned CoLM revision passed the
serial tiny and L1/L2/L3 mesh roundtrips on GNU 16.1.0/macOS arm64. The 3-rank
Open MPI 5.0.9 build instead failed during CoLM's landelm vector setup
(`MOD_Pixelset.F90`, `vecgs%vcnt`), after mesh construction. An isolated O0
Pixelset diagnostic also failed with invalid component bounds; the root cause
is unresolved. `--mpi-ranks` exposes this check, not a supported-MPI claim;
it does not silently fall back to serial or skip landelm construction.
