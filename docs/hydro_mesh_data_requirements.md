# Hydro Mesh Data Requirements for CaMa-Flood and CoLM2024

EarthMesh v3 hydro-mesh support starts with a conservative hybrid representation:
small channels are aggregated or represented as 1D river edges, medium channels are
1D edges plus refinement buffers, and major rivers/estuaries/coastal wetlands become
2D river-corridor candidates.

## Data needed now

To move from the current tested CSV classifier to real CaMa-Flood ingestion, provide
one small representative CaMa-Flood map directory or NetCDF subset for the target
resolution and domain.

Required river fields:

- Reach or grid-cell center longitude.
- Reach or grid-cell center latitude.
- Downstream topology, such as a downstream cell index or `nextxy`-style pair.
- Upstream drainage area, preferably in square kilometers or with clear units.
- River channel width, preferably in meters or with clear units.
- River segment length, preferably in meters or kilometers with clear units.

Useful optional river/floodplain fields:

- Floodplain width or floodplain area.
- Floodplain elevation or bankfull elevation.
- Mean or bankfull discharge.
- Inundation fraction or floodplain storage diagnostics.
- Basin identifier or river order.
- River mouth, estuary, or ocean outlet flags if available.

CoLM2024 and EarthMesh fields:

- Target EarthMesh domain: global, China, a named basin, or a coastal bounding box.
- Target minimum EarthMesh cell size for the first case.
- CoLM2024 landtype/surface dataset path used by the intended case.
- Whether the first 2D river corridors should include only estuaries and mainstems,
  or also floodplains and coastal wetlands.

## Current implemented file formats

The first implementation accepts a CSV reach inventory with these columns:

```csv
reach_id,upstream_area_km2,width_m,floodplain_width_m,target_dx_km,is_estuary,is_delta,is_coastal_wetland,is_major_confluence,user_force_2d
```

Boolean columns accept `true`, `false`, `1`, `0`, `yes`, `no`, and empty values.
Missing optional boolean columns default to false.

## Current output classes

- `R0`: ignored or aggregated subgrid channel.
- `R1`: explicit 1D river edge only.
- `R2`: 1D river edge plus EarthMesh refinement buffer.
- `R3`: explicit 2D river corridor candidate plus 1D topology.

## Validation expectations

Before using the products in an EarthMesh/CoLM2024 workflow, validate that:

1. Required CaMa-Flood fields are detected by `util.hydro_mesh.cama_contract`.
2. Classified reaches include continuous R3 mainstem/estuary segments rather than isolated fragments.
3. R2/R3 classes do not create a mesh-size explosion for the target domain.
4. Generated masks remain inside the EarthMesh domain.
5. CoLM2024 receives both land mesh metadata and river-to-land mapping metadata.

## Probe result for `/Users/zhongwangwei/Desktop/glb_01min.tar.gz`

The provided global 1 arcmin CaMa-Flood archive is usable as a cautious next-step
input, but it is a traditional CaMa binary map package rather than a NetCDF file.
It should not be fully extracted into the repository.

Observed metadata from the archive:

- Map directory: `glb_01min`.
- Grid size: `21600 x 10800`.
- Grid spacing: `0.01666667` degrees.
- Domain: west `-180`, east `180`, south `-90`, north `90`.
- Floodplain layers: `10`.
- Byte order from control files: little endian.
- Required first-pass binary fields present: `nextxy.bin`, `uparea.bin`, `width.bin`, `rivlen.bin`.
- Important caveat: `rivwth.ctl` is present, but `rivwth.bin` was not observed in the tar member inventory; the first reader should treat `width.bin` as a river-width candidate and report that assumption explicitly.

Recommended handling:

1. Keep the `.tar.gz` outside the git repository.
2. For development, extract only a regional window or a small fixture generated from the global binaries.
3. Implement windowed binary reads instead of loading full global arrays into memory.
4. Verify the scientific meaning and units of `width.bin` before using it as the final R2/R3 river-width criterion.

## Windowed reader workflow

The current code supports a safe first pass for binary CaMa maps that have already
been extracted outside the git repository. The intended workflow is:

1. Extract only the required map files to an external scratch directory, not into
   the EarthMesh repository:
   - `params.txt`
   - `nextxy.bin`
   - `uparea.bin`
   - `width.bin`
   - `rivlen.bin`
2. Create a `CamaGridSpec` from `params.txt` metadata.
3. Convert a geographic bounding box into `(x_start, y_start, width, height)` with
   `CamaGridSpec.window_for_bbox(...)`.
4. Read only that binary window with `read_reach_inventory_window(...)`.
5. Classify the returned `RiverReach` records with the existing R0/R1/R2/R3
   classifier.

This deliberately avoids full global-array loading. For the provided 1 arcmin
global package, one scalar layer is about 933 MB and `nextxy.bin` is about 1.87 GB,
so full-array reads are unnecessary for regional development and risky for routine
iteration.

## Verified regional sample command

After extracting the required files to `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/glb_01min`,
the current sampler can generate classified reach records for a regional bbox without
loading the full global map:

```bash
python3 -m util.hydro_mesh.cama_sample \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/glb_01min \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_classified.jsonl \
  --bbox 118 28 123 33 \
  --target-dx-km 10
```

Observed for this first Yangtze-delta-adjacent probe:

- Output records: `13389`.
- Classification counts with current conservative thresholds: `R0=9988`, `R1=1764`, `R2=876`, `R3=761`.
- `uparea.bin` is treated as square meters and converted with `--uparea-to-km2 1e-6`.
- CaMa control files use `yrev`; the sampler defaults to reversed binary row order.
- After interpreting `nextxy.bin` as two planar CaMa variables (`varx`, then `vary`) with `yrev` conversion and one-based index normalization, all `13389` sampled records have valid downstream indices in this regional window.

This proves the windowed reader path is feasible for regional development. The next
scientific validation step is to compare sampled river locations/classes against a
known basin or coastline reference before generating 2D river-corridor masks.

## Verified R2/R3 GeoJSON export

The classified regional sample can be converted to a lightweight GeoJSON point
layer for visual inspection against river and coastline references:

```bash
python3 -m util.hydro_mesh.geojson_export \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_classified.jsonl \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.geojson \
  --classes R2 R3
```

Observed output for the same bbox:

- GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.geojson`.
- Feature count: `1637`.
- Class counts: `R2=876`, `R3=761`.

This file is an inspection layer, not yet an EarthMesh refine mask. Use it to verify
that high-priority river points align with expected rivers, estuaries, and coastal
features before constructing buffered corridors or 2D river cells.

## Verified HTML map preview

The R2/R3 GeoJSON inspection layer can also be wrapped as a local Leaflet HTML map:

```bash
python3 -m util.hydro_mesh.geojson_map \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.html \
  --title "Yangtze Delta CaMa R2/R3 Candidates"
```

Observed output:

- HTML path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.html`.
- File size for the current sample: about `755 KB`.
- The GeoJSON features are embedded directly in the HTML file.
- Leaflet and OpenStreetMap tiles are loaded from public CDNs when the file is opened;
  the basemap therefore needs network access unless this preview is later adapted to
  local/offline tiles.

Use this preview as a visual QA gate before generating buffered river corridors or
EarthMesh 2D river masks. If the point cloud appears shifted, mirrored, or dominated
by false coastal outlets, re-check CaMa `yrev`, longitude/latitude center convention,
width units, and downstream outlet interpretation before proceeding.

## Verified corridor preview export

After the point-level visual QA passes, the R2/R3 point layer can be converted into
approximate preview corridor polygons. This is deliberately a QA product, not the
final EarthMesh river mask: each retained point is represented by a local circular
buffer in lon/lat space, without topology-aware unioning, centerline smoothing, or
coastline clipping.

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.corridor_preview \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_corridor_preview.geojson \
  --segments 20 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_corridor_preview.png \
  --title "Yangtze Delta R2/R3 corridor preview"
```

Observed output for the same bbox:

- Corridor GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_corridor_preview.geojson`.
- Corridor PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_corridor_preview.png`.
- Feature count: `1637` polygons, preserving `R2=876` and `R3=761`.
- Preview radius range: `700 m` to about `15098 m` with the current width-driven rule.

Current v0 radius rule:

- `R2`: at least `700 m`, capped at `1500 m`, with wider source widths allowed to
  increase the buffer up to that cap.
- `R3`: at least `1800 m`, otherwise driven by the source `width_m` value.

Large R3 blobs are useful QA signals rather than immediate errors. They may indicate
true wide estuary/floodplain candidates, but they may also indicate that CaMa `width.bin`
needs unit, semantic, or threshold calibration before this preview is promoted into an
EarthMesh mask-generation step.

## Verified linked corridor fallback

Before the `nextxy.bin` planar/yrev interpretation was corrected, the safer fallback
was to connect only nearby same-class candidates and mark the geometry source
explicitly as `nearest_neighbor_segment`. This mode remains useful when a clipped
input layer lacks downstream indices, but it is superseded by the CaMa downstream
segment preview when `downstream_x` and `downstream_y` are available.

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.corridor_preview \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_linked_corridor_preview.geojson \
  --neighbor-links \
  --max-link-distance-km 3.0 \
  --max-radius-m 2500 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_linked_corridor_preview.png \
  --title "Yangtze Delta linked corridor preview"
```

Observed output:

- Linked corridor GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_linked_corridor_preview.geojson`.
- Linked corridor PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_linked_corridor_preview.png`.
- Feature count: `1063` nearest-neighbor segment polygons.
- Class counts: `R2=541`, `R3=522`.
- Link distance range: about `1.56 km` to `2.48 km` under the `3 km` cap.
- Preview radius range after capping: `700 m` to `2500 m`.

This linked fallback is visually cleaner than the independent point-circle preview, but
it is still not a hydrologic topology product. The next scientific correction should
repair or reinterpret CaMa `nextxy.bin`/downstream indices so segment corridors can be
built from actual flow direction instead of nearest-neighbor proximity.


## Verified CaMa downstream corridor preview

The corrected `nextxy.bin` reader treats the file according to `nextxy.ctl`: two
planar int32 variables (`varx` followed by `vary`), little endian, `yrev`, and
one-based CaMa indices. `varx` is converted to zero-based `x_index` by subtracting
one; `vary` is converted from north-to-south storage coordinates back to EarthMesh's
south-to-north `y_index` convention.

With that correction, the R2/R3 point layer can be converted into segment buffers
following explicit CaMa downstream links:

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.corridor_preview \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_R2R3.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_downstream_corridor_preview.geojson \
  --downstream-links \
  --max-radius-m 2500 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_downstream_corridor_preview.png \
  --title "Yangtze Delta CaMa downstream corridor preview"
```

Observed output:

- Downstream corridor GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_downstream_corridor_preview.geojson`.
- Downstream corridor PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_downstream_corridor_preview.png`.
- Feature count: `1516` CaMa downstream segment polygons.
- Class counts: `R2=761`, `R3=755`.
- Link distance range: about `1.56 km` to `5.11 km`.
- Preview radius range after capping: `700 m` to `2500 m`.
- Geometry source marker: `corridor_source_geometry=cama_downstream_segment`.

This is now a hydrologic-topology preview rather than a nearest-neighbor fallback,
but it is still not a final EarthMesh mask. The next step is to union/smooth
connected segment buffers, clip them against a coastline/domain reference, and then
rasterize or intersect them with EarthMesh cells for CoLM2024 coupling metadata.

## Verified dissolved corridor preview

The CaMa downstream segment buffers can be dissolved by river class into coarse QA
mask candidates. This uses Shapely at runtime when available and keeps the output
marked as a preview product rather than a final EarthMesh mask.

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.corridor_preview \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_downstream_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  --dissolve \
  --simplify-tolerance-deg 0.0002 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.png \
  --title "Yangtze Delta dissolved corridor preview"
```

Observed output:

- Dissolved corridor GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson`.
- Dissolved corridor PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.png`.
- Feature count: `2` class-level `MultiPolygon` features: one `R2`, one `R3`.
- Source segment counts: `R2=761`, `R3=755`.
- Geometry source marker: `corridor_source_geometry=dissolved_corridor`.
- Simplification: topology-preserving `0.0002` degrees for visual QA size reduction.

This is the first output that resembles a mask candidate, but it is still missing the
coastline/domain clip and EarthMesh cell intersection. Use it to inspect whether the
river corridors are spatially coherent before converting to mesh-cell metadata.

## Verified bbox clip and regular-grid river-cell preview

The dissolved corridor mask can now be clipped to a regional lon/lat bounding box
and intersected with a regular lon/lat grid. This is a bridge product for visual QA
and metadata prototyping: each output feature is a full grid cell with fractional
river-overlap metadata. Areas and fractions are computed in planar degree-square
space, so this is not yet the final geodesic EarthMesh cell intersection.

Bbox-clipped dissolved corridor preview:

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.corridor_preview \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_clipped_corridor_preview.geojson \
  --clip-bbox 118 28 123 33 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_clipped_corridor_preview.png \
  --title "Yangtze Delta bbox-clipped corridor preview"
```

Observed output:

- Clipped corridor GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_clipped_corridor_preview.geojson`.
- Clipped corridor PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_clipped_corridor_preview.png`.
- Feature count: `2` class-level features, preserving `R2=1` and `R3=1`.
- Geometry source marker: `corridor_source_geometry=bbox_clipped_corridor`.

Regular-grid river-cell intersection preview:

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.corridor_preview \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_grid_intersections_preview.geojson \
  --grid-cell-size-deg 0.05 \
  --clip-bbox 118 28 123 33 \
  --min-fraction 0.0 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_grid_intersections_preview.png \
  --title "Yangtze Delta regular-grid river cells preview"
```

Observed output:

- Grid-cell GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_grid_intersections_preview.geojson`.
- Grid-cell PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_grid_intersections_preview.png`.
- Feature count: `970` regular lon/lat cells with nonzero corridor overlap.
- Class counts: `R2=542`, `R3=428`.
- Cell size: `0.05` degrees.
- River-overlap fraction range: about `4.85e-05` to `0.9898`.
- Geometry source marker: `corridor_source_geometry=regular_grid_intersection_preview`.

This proves that the river corridor can be represented as cell-level metadata rather
than only as vector masks. The remaining v3 production step is to replace this
regular lon/lat preview grid with actual EarthMesh cell polygons, compute geodesic
cell/corridor overlap, and clip against an explicit coastline/domain mask before
writing CoLM2024 coupling metadata.

## Verified EarthMesh cell intersection preview

The dissolved CaMa corridor can also be intersected with actual MPAS/EarthMesh cell
polygons from an existing mesh NetCDF. The current reader supports MPAS-style
variables `lonCell`, `latCell`, `lonVertex`, `latVertex`, `verticesOnCell`,
`nEdgesOnCell`, and optional `indexToCellID`/`areaCell`. Cells are selected by center
point inside a bbox, converted to GeoJSON polygons, and intersected against the R2/R3
corridor polygons.

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.earthmesh_intersection \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_preview.geojson \
  --mpas-mesh cases/ATMOS_hex_N64_refine2_global_LOM67_251027/result/MPASOUT_NXP0064_global.nc4 \
  --bbox 118 28 123 33 \
  --min-fraction 0.0 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_preview.png \
  --title "Yangtze Delta EarthMesh river-cell intersections preview"
```

Observed output:

- EarthMesh cell-intersection GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_preview.geojson`.
- EarthMesh cell-intersection PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_preview.png`.
- Bbox-selected MPAS/EarthMesh source cells: `333`.
- Output features with nonzero corridor overlap: `133`.
- Class counts: `R2=83`, `R3=50`.
- River-overlap fraction range: about `1.0e-04` to `0.5501`.
- Geometry source marker: `corridor_source_geometry=earthmesh_cell_intersection_preview`.

Each output feature keeps the original cell polygon and adds overlap metadata:
`river_class`, `river_fraction`, `intersection_area_deg2`, `cell_area_deg2`,
`source_areaCell`, `source_areaCell_units`, and `source_estimated_river_area` when
`areaCell` is available. The `source_areaCell` fields deliberately preserve the mesh
file's raw area values instead of promising true square meters. In the current sample
file, `areaCell` is labeled `m^2` but has unit-sphere-scale values, so production
CoLM2024 coupling still needs an explicit geodesic area normalization step.

This is the first prototype that represents CaMa-Flood river corridors directly on
real EarthMesh cells. The remaining scientific steps are coastline/domain clipping,
geodesic overlap areas, and export of stable CoLM2024 river-to-land coupling tables.

## Verified domain-mask interface and normalized area estimate

The EarthMesh cell intersection utility now has two optional safeguards for the
CoLM2024 coupling path:

1. `--domain-geojson` clips R2/R3 corridor polygons to an external domain or
   coastline mask before cell intersection. The example below uses a bbox polygon as
   a placeholder only; replace it with a real land/coastline/domain mask for
   production.
2. `--unit-sphere-area` treats source `areaCell` values as unit-sphere areas and
   adds `normalized_cell_area_m2` plus `estimated_river_area_m2`. This is appropriate
   for the current sample mesh, where `areaCell` values are near `1e-5`.

Placeholder bbox-domain file used for the first smoke test:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_bbox_domain.geojson
```

Domain-clipped and area-normalized command:

```bash
MPLCONFIGDIR=/tmp/matplotlib python3 -m util.hydro_mesh.earthmesh_intersection \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_domain_area_preview.geojson \
  --mpas-mesh cases/ATMOS_hex_N64_refine2_global_LOM67_251027/result/MPASOUT_NXP0064_global.nc4 \
  --bbox 118 28 123 33 \
  --domain-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_bbox_domain.geojson \
  --unit-sphere-area \
  --min-fraction 0.0 \
  --preview-png /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_domain_area_preview.png \
  --title "Yangtze Delta EarthMesh cells with domain clip and normalized area"
```

Observed output:

- Domain-area GeoJSON path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_domain_area_preview.geojson`.
- Domain-area PNG path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_domain_area_preview.png`.
- Output features: `133`.
- Class counts: `R2=83`, `R3=50`.
- River-overlap fraction range: about `1.0e-04` to `0.5501`.
- Normalized cell area range: about `7.90e8` to `8.02e8 m2`.
- Estimated river-overlap area range: about `7.93e4` to `4.37e8 m2`.
- Metadata markers: `domain_clip_applied=True` and
  `area_normalization=unit_sphere_area_to_m2`.

This step gives CoLM2024 a more realistic metadata shape: each EarthMesh cell can now
carry a river class, overlap fraction, normalized cell area, and estimated river area.
The remaining blocker is scientific input quality, not software plumbing: the
placeholder bbox domain should be replaced with a coastline/domain mask and the
planar lon/lat overlap fraction should be replaced or validated with a true geodesic
intersection method before production use.

## Verified CoLM-style coupling table preview

The EarthMesh cell-intersection GeoJSON can now be converted to a stable long-table
coupling preview for CoLM2024-side experiments. Each row is one
`EarthMesh cell x river class` overlap record. The table intentionally stays simple:
it does not yet choose a dominant class, aggregate multiple river classes into one
cell, or write a final CoLM NetCDF file.

CSV export:

```bash
python3 -m util.hydro_mesh.colm_coupling \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_domain_area_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_colm_coupling_preview.csv \
  --format csv \
  --min-fraction 0.0
```

JSONL export:

```bash
python3 -m util.hydro_mesh.colm_coupling \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_earthmesh_cell_intersections_domain_area_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_colm_coupling_preview.jsonl \
  --format jsonl \
  --min-fraction 0.0
```

Observed output:

- CSV path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_colm_coupling_preview.csv`.
- JSONL path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_colm_coupling_preview.jsonl`.
- Rows: `133`.
- Class counts: `R2=83`, `R3=50`.
- River-overlap fraction range: about `1.0e-04` to `0.5501`.
- Estimated river-overlap area range: about `7.93e4` to `4.37e8 m2`.

Current stable columns:

```text
cell_id,cell_index,river_class,river_fraction,estimated_river_area_m2,
normalized_cell_area_m2,center_lon,center_lat,domain_clip_applied,
area_normalization
```

This is not yet a final CoLM2024 input file, but it is the first compact coupling
artifact that can be inspected, filtered, versioned, and compared across threshold
or coastline-mask experiments.

## Verified EarthMesh close-refinement mask export

The EarthMesh cell-intersection previews above used the existing
`MPASOUT_NXP0064_global.nc4` mesh. That mesh is useful for overlap/coupling QA, but
it is not hydro-refined in the Yangtze-delta test window: the selected cells are
roughly uniform, with normalized areas near `7.9e8 m2` to `8.0e8 m2` (about
`28 km` equivalent cell size). To make the mesh itself finer around CaMa-Flood river
corridors, the dissolved corridor polygons must be supplied back to EarthMesh as
specified-refinement input.

EarthMesh already supports `RL%mask_refine_spc_type = 'close'`. The hydro-mesh
exporter converts dissolved `R2`/`R3` corridor `Polygon`/`MultiPolygon` features into
EarthMesh-compatible close-mask `.nml` files:

```bash
python3 -m util.hydro_mesh.refine_mask_export \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro \
  --class-refine R2=1 R3=2 \
  --buffer-deg-by-refine-degree 1=1.0 2=0.2 \
  --simplify-tolerance-deg 0.005
```

Observed output for the current Yangtze-delta smoke case:

- Output prefix for EarthMesh namelists:
  `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro`.
- Files written: `118` close-mask `.nml` files.
- Mask counts: `R2_d1=80`, `R3_d1=19`, `R3_d2=19`.
- The export is cumulative by default: a class mapped to target degree `2` is
  emitted at both `close_refine = 1` and `close_refine = 2`, because EarthMesh can
  only apply a finer refinement level inside the previous level's refined interior.
- `--buffer-deg` is a mesh-generation envelope, not a river-area definition. It
  widens narrow CaMa corridors enough for coarse base triangles and transition/halo
  logic to select connected refinement regions.
- `--buffer-deg-by-refine-degree` can make that envelope hierarchical. The current
  smoke case uses a wide `1=1.0` degree level-1 support envelope and a narrower
  `2=0.2` degree R3 level-2 envelope.
- Each refinement degree is capped at `99` masks because the current Fortran
  close-mask temporary filename uses a two-digit `I2.2` counter.
- When the cap is active, higher target-refinement classes such as `R3` are retained
  before lower target-refinement classes such as `R2`.
- Each GeoJSON ring is written without its duplicate final closure coordinate;
  EarthMesh closes each curve internally when it reads the mask.
- The exporter removes stale files matching the same prefix before writing, so
  old `_100.nml`-style files are not accidentally consumed by `ls prefix*`.
- Keep non-mask files away from the same prefix because EarthMesh currently lists
  inputs with `ls <prefix>*`; for example, do not leave a
  `refine_spc_hydro_summary.json` beside these `.nml` files.

Example generated file:

```text
close_num = 1051
close_refine = 1
118.00615917 31.17665972
118.00615917 31.19169091
...
```

Use these files from an EarthMesh namelist like:

```fortran
RL%refine_spc              = .TRUE.
RL%max_iter_spc            = 2
RL%mask_refine_spc_type    = 'close'
RL%mask_refine_spc_fprefix = '/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro'
```

This step generates the refinement masks only; it does not by itself produce a new
refined mesh. The next verification step is to run EarthMesh with the close-mask
namelist and then re-run the EarthMesh cell-size/intersection preview on the newly
generated mesh to confirm that river/coastal corridors are actually refined.

Smoke-run evidence with
`/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/Atmos_hex_NXP64_hydro_close_yangtze_smoke.nml`:

- EarthMesh read the close masks and completed successfully.
- With no refinement-envelope buffer, `R3=2` masks were too narrow: level 2 selected
  only `35` triangles and the transition/halo cleanup removed all of them.
- With cumulative masks and degree-specific buffers `1=1.0 2=0.2`, level 1
  retained `68` refined triangles and level 2 retained `4` refined triangles after
  cleanup.
- The resulting preview image is:
  `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_degreebuffer_earthmesh_cells_with_land_background_preview.png`.
- Bbox cell-size smoke statistics for that mesh: `109` cells in the 118-123E,
  28-33N bbox, equivalent cell-size range about `29.7 km` to `64.6 km`, median
  about `49.3 km`.

This proves the close-mask route is executable, but it also shows that final v3
mesh design should not use raw river width as the only refinement mask. A practical
CoLM2024 workflow needs at least two layers: a broad level-1 hydrologic/coastal
envelope for transition support, and a narrower higher-level R3 river/estuary
envelope for true 2D corridor detail.

## Verified R3 degree-3 refinement smoke recipe

The close-mask workflow can now be captured as a small JSON recipe so the mask
generation command, EarthMesh namelist overrides, and smoke-run command stay
reproducible:

```bash
python3 -m util.hydro_mesh.refinement_recipe \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro_r3d3 \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_refinement_recipe_r3d3.json \
  --class-refine R2=1 R3=3 \
  --buffer-deg-by-refine-degree 1=1.5 2=1.0 3=0.5 \
  --simplify-tolerance-deg 0.005 \
  --example-namelist /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/Atmos_hex_NXP64_hydro_close_yangtze_r3d3_smoke.nml
```

The matching close-mask export uses the command recorded in that JSON recipe. For
the current Yangtze-delta smoke case it writes `137` `.nml` files:

- `R2_d1=80`
- `R3_d1=19`
- `R3_d2=19`
- `R3_d3=19`

The successful R3 degree-3 EarthMesh smoke run used:

```fortran
RL%refine_spc              = .TRUE.
RL%max_iter_spc            = 3
RL%mask_refine_spc_type    = 'close'
RL%mask_refine_spc_fprefix = '/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro_r3d3'
```

Smoke-run retained-refinement evidence:

- Level 1 selected `96` triangles and retained `96` after cleanup.
- Level 2 selected `185` triangles and retained `86` after cleanup.
- Level 3 selected `275` triangles and retained `83` after cleanup.
- EarthMesh finished successfully.

The resulting preview image is:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_earthmesh_cells_with_land_background_preview.png
```

Bbox statistics for the 118-123E, 28-33N window:

- Background/domain cells: `438`.
- R2/R3 river-overlap cells: `200` (`R2=124`, `R3=76`).
- Equivalent cell-size range: about `14.9 km` to `55.1 km`.
- Equivalent median cell size: about `18.7 km`.

This is the first smoke result that visibly creates a finer local mesh around the
CaMa-Flood corridor system. It is still a mesh-control experiment rather than final
CoLM2024 production input: the wide staged buffers should be calibrated with a real
coastline/domain mask and the final CoLM coupling table should continue to use the
unbuffered corridor overlap fractions for river area metadata.
