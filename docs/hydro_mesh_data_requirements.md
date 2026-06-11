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
- Each close-mask refinement degree is capped at `999` masks by default. EarthMesh
  now writes and reads close-mask temporary files with three-digit numbering such
  as `mask_refine_close_1_100.nc4`, so 1min river/coastline experiments are no
  longer forced into the old two-digit `99`-mask limit.
- When a user-specified cap is active, higher target-refinement classes such as `R3` are retained
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

## Verified reusable refinement evaluation report

The R3 degree-3 smoke result can now be reduced to a compact JSON evaluation report.
This makes future threshold, buffer, coastline-mask, and target-resolution tests
directly comparable without re-reading large GeoJSON files or manually parsing
EarthMesh logs.

```bash
python3 -m util.hydro_mesh.refinement_eval \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_earthmesh_cell_intersections_preview.background_cells.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_earthmesh_cell_intersections_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_refinement_eval.json \
  --log-path /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_hydro_close_r3d3_smoke.log
```

Observed report path:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_refinement_eval.json
```

Observed JSON summary:

- Background/domain cells: `438`.
- Equivalent cell-size range: `14.9057 km` to `55.0720 km`.
- Equivalent median cell size: `18.7462 km`.
- River-overlap records: `200`.
- River-overlap class counts: `R2=124`, `R3=76`.
- River-overlap fraction range: `1.84e-06` to `0.7930`, median `0.0592`.
- Estimated river-overlap area sum: about `7.52e9 m2`.
- Parsed EarthMesh retained triangles: level 1 `96`, level 2 `86`, level 3 `83`.

Use this report as the current regression/evaluation artifact for v3 hydro-mesh
experiments. A better candidate recipe should improve one or more of these metrics
while keeping mesh growth bounded and preserving river/coastline continuity.

## Verified interactive EarthMesh-cell Leaflet map

The R2/R3 point preview HTML shows CaMa candidates on a web basemap, but it does
not show the actual refined EarthMesh cells. The same Leaflet workflow can now
embed the mesh-cell GeoJSON layers directly:

```bash
python3 -m util.hydro_mesh.geojson_map \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_earthmesh_cell_intersections_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_earthmesh_cells_leaflet.html \
  --background-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_earthmesh_cell_intersections_preview.background_cells.geojson \
  --title "Yangtze Delta EarthMesh hydro-refined cells"
```

Observed output:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_earthmesh_cells_leaflet.html
```

This HTML embeds both layers in the file:

- gray `land/background cells`: all selected EarthMesh cells in the bbox;
- yellow `R2 river-overlap cells`;
- red `R3 river-overlap cells`;
- popups with `cell_id`, `river_class`, `river_fraction`, and available area
  metadata;
- a Leaflet layer control so background cells and river cells can be toggled.

This is the interactive counterpart to the PNG preview and is the preferred visual
QA artifact when checking whether river/coastal corridors are actually refined on
the mesh rather than only present as CaMa point candidates.

## Verified CaMa elevation-derived coastal-band refinement

The previous hydro-refinement smoke cases refined only CaMa river corridors. The
provided `glb_01min` package also contains `elevtn.bin`, whose valid values mark
CaMa land/domain cells and whose `-9999` values mark ocean/undefined cells in the
current window. This can be used to derive a separate land/ocean transition band
instead of pretending that river corridors also represent the coast.

Generate a dissolved coastal-band mask for EarthMesh close refinement:

```bash
python3 -m util.hydro_mesh.coastal_band \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/glb_01min \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_coastal_band_radius3.geojson \
  --bbox 118 28 123 33 \
  --radius-cells 3
```

Generate an explicit 1 arcmin coastal-cell layer for visual QA:

```bash
python3 -m util.hydro_mesh.coastal_band \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/glb_01min \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_coastal_band_radius3_cells.geojson \
  --bbox 118 28 123 33 \
  --radius-cells 3 \
  --no-dissolve
```

Observed coastal-band statistics for the 118-123E, 28-33N window:

- Coastal-band cells: `8705`.
- Land-side cells: `4255`.
- Ocean-side cells: `4450`.
- Radius: `3` CaMa 1 arcmin cells, so the QA layer preserves the 1min coastline
  structure while the dissolved layer remains suitable for close-mask export.

The close-mask exporter now accepts `mask_class=COAST` as well as `river_class`.
The first coast-aware smoke was run while close-mask temporary files still had the
old `99`-mask per-degree practical limit, so it allocated degree-1 capacity as:

- `COAST_d1=20`
- `R2_d1=60`
- `R3_d1=19`
- `R3_d2=19`
- `R3_d3=19`

The resulting prefix is:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro_r3d3_coast20
```

The successful coast-aware EarthMesh smoke run used:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/Atmos_hex_NXP64_hydro_close_yangtze_r3d3_coast20_smoke.nml
```

and wrote:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/cases/ATMOS_hex_N64_hydro_close_yangtze_r3d3_coast20_smoke/result/MPASOUT_NXP0064_global.nc4
```

Smoke-run retained-refinement evidence:

- Level 1 selected `94` triangles and retained `94`.
- Level 2 selected `185` triangles and retained `94`.
- Level 3 selected `283` triangles and retained `90`.
- EarthMesh finished successfully.

Compared with the river-only R3 degree-3 smoke, the coast-aware run changes the
regional mesh:

- Background/domain cells increase from `438` to `482`.
- River-overlap records increase from `200` to `204`.
- Equivalent median cell size decreases from about `18.75 km` to `17.86 km`.
- Level-2 retained triangles increase from `86` to `94`.
- Level-3 retained triangles increase from `83` to `90`.

The corresponding three-layer interactive HTML is:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_coast20_rivers_and_coast_leaflet.html
```

It embeds gray background EarthMesh cells, yellow/red R2/R3 river-overlap cells,
and cyan CaMa `elevtn` coastal-band cells. This is the first visual QA artifact in
this branch that shows both river refinement and explicit coastline parsing on the
same basemap.

## Verified three-digit close-mask numbering and mask-allocation lesson

EarthMesh close-mask temporary files now use three-digit numbering consistently
for close masks generated from external masks and from automatic patch boundaries.
This removes the old `I2.2` bottleneck that prevented degree-1 mask counts above
`99`.

Validation evidence:

- `mkgrd.x` compiled after changing the close-mask counters.
- `refine_spc_hydro_r3d3_riverall`, with `123` degree-1 masks, successfully read
  `mask_refine_close_1_100.nc4`.
- `refine_spc_hydro_r3d3_coastall`, with `401` degree-1 masks, successfully read
  `mask_refine_close_1_400.nc4`.

However, more low-level masks are not automatically better. The current smoke
comparison is:

| recipe | degree-1 masks | bbox cells | median cell size | river-overlap records | retained L1/L2/L3 |
| --- | ---: | ---: | ---: | ---: | --- |
| `r3d3` river-only | `99` | `438` | `18.75 km` | `200` | `96/86/83` |
| old `coast20` | `99` | `482` | `17.86 km` | `204` | `94/94/90` |
| `r2cap80_coast20` | `119` | `438` | `18.75 km` | `200` | `96/86/83` |
| `riverall` | `123` | not promoted | not promoted | not promoted | `100/72/51` |
| `coastall` | `401` | `364` | `22.16 km` | `174` | `100/72/51` |

The lesson is that the three-digit numbering is necessary infrastructure, but the
v3 recipe still needs hydrologic ranking. Full R2 or full coastline masks can make
the transition/weak-concavity cleanup remove more high-level R3 refinement. For
the current Yangtze-delta smoke, the old `coast20` remains the best visual/metric
candidate because it adds explicit coastline refinement while preserving or
improving retained level-2/level-3 refinement.

## Composite river/coast close-mask recipes

The close-mask exporter can now cap source rings independently by class:

```bash
python3 -m util.hydro_mesh.refine_mask_export \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro_r3d3_river_ranked \
  --class-refine R2=1 R3=3 \
  --max-rings-by-class R2=60 R3=19 \
  --buffer-deg-by-refine-degree 1=1.5 2=1.0 3=0.5 \
  --simplify-tolerance-deg 0.005
```

For mixed river/coast tests, use the composite recipe driver instead of hand
copying `.nml` files.  The recipe schema is intentionally small: each component
points at one GeoJSON source, declares its `class_refine`, and optionally sets
`max_rings_by_class`, `buffer_deg`, `buffer_deg_by_refine_degree`,
`simplify_tolerance_deg`, and `cumulative_refine`.

Example for the current 99-mask degree-1 allocation:

```json
{
  "max_masks_per_refine_degree": 999,
  "components": [
    {
      "name": "coastline_support",
      "input_geojson": "/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_coastal_band_radius3.geojson",
      "class_refine": {"COAST": 1},
      "max_rings_by_class": {"COAST": 20},
      "simplify_tolerance_deg": 0.005
    },
    {
      "name": "ranked_river_corridors",
      "input_geojson": "/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_dissolved_corridor_preview.geojson",
      "class_refine": {"R2": 1, "R3": 3},
      "max_rings_by_class": {"R2": 60, "R3": 19},
      "buffer_deg_by_refine_degree": {"1": 1.5, "2": 1.0, "3": 0.5},
      "simplify_tolerance_deg": 0.005
    }
  ]
}
```

Run it with:

```bash
python3 -m util.hydro_mesh.composite_refine_mask_export \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_composite_recipe_r3d3_ranked_coast20.json \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/refine_spc_hydro_r3d3_ranked_coast20 \
  --summary-json /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_composite_recipe_r3d3_ranked_coast20_summary.json
```

The summary JSON records the actual allocation, for example
`COAST_d1=20`, `R2_d1=60`, and `R3_d1/d2/d3=19`.  With the same river buffers
as the verified R3 recipe (`1=1.5,2=1.0,3=0.5`), the composite
`ranked_coast20` smoke reproduced the old `coast20` metrics exactly:
`482` bbox cells, median equivalent cell size `17.86 km`, `204` river-overlap
records, and retained `94/94/90` triangles for levels 1/2/3.  The matching
interactive map with raw 1 arcmin coastal QA cells is:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_ranked_coast20_rivers_and_coast_leaflet.html
```

For user-facing mesh QA, do not overlay the raw CaMa coastal rectangles as the
final coast layer.  First intersect the CaMa coastal band with the generated
EarthMesh cells, then render those EarthMesh cells:

```bash
python3 -m util.hydro_mesh.earthmesh_intersection \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_coastal_band_radius3.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_ranked_coast20_coastal_earthmesh_cell_intersections_preview.geojson \
  --mpas-mesh /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/cases/ATMOS_hex_N64_hydro_close_yangtze_r3d3_ranked_coast20_smoke/result/MPASOUT_NXP0064_global.nc4 \
  --bbox 118 28 123 33 \
  --classes COAST \
  --unit-sphere-area

python3 -m util.hydro_mesh.geojson_map \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_ranked_coast20_earthmesh_cell_intersections_preview.geojson \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_ranked_coast20_rivers_and_integrated_coast_leaflet.html \
  --background-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_ranked_coast20_earthmesh_cell_intersections_preview.background_cells.geojson \
  --coast-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_ranked_coast20_coastal_earthmesh_cell_intersections_preview.geojson \
  --title "Yangtze delta hydro/coast EarthMesh cells"
```

The integrated coast layer currently contains `84` coastal-overlap EarthMesh
cells in the 118-123E, 28-33N QA window.  These features keep EarthMesh cell
geometry and carry `mask_class=COAST`, `overlap_class=COAST`, and
`coastal_fraction` properties, so they are visibly embedded in the mesh instead
of appearing as separate 1 arcmin CaMa quadrilaterals.  The integrated map is:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_r3d3_ranked_coast20_rivers_and_integrated_coast_leaflet.html
```

This is now the reproducible control surface for a small recipe sweep such as
`R2={40,60,80}` by `COAST={10,20,40}`.

Two failed smoke attempts exposed an important geometry lesson:

- A narrow-buffer river recipe (`1=1.0,2=0.2,3=0.08`) completed but degraded the
  Yangtze-delta subset to `109` bbox cells, median `49.30 km`, and retained only
  `70/4/0` triangles.  The high-level R3 geometry was too thin for the nested
  cleanup.
- `ranked_coast20_cells`, using individual 1 arcmin coastal cells for the 20
  coast masks, also completed but produced the same degraded `109`-cell subset
  and retained `68/4/0` triangles.  The 1min cells are valuable for visual QA,
  but close-mask control needs smoother, hydrologically ranked envelopes.

So the next v3 step should not be "add every 1min object".  It should keep the
composite recipe driver, rank river/coast source geometries explicitly, and judge
candidate masks by the retained R2/R3 refinement plus the embedded Leaflet QA
layer.

## Finer coastal mesh candidates

The first integrated-coast map was still visually coarse because the coastline
was embedded into the generated EarthMesh cells, but the underlying global base
mesh was only `NXP=64`.  Raising the COAST close-mask target alone did not help:
`COAST=2` and `COAST=3` masks were read, but the final Yangtze-delta subset was
unchanged from the N64 `ranked_coast20` case.  The effective path is to increase
the base grid resolution while keeping the same verified hydro/coast mask recipe.

Observed smoke comparison for 118-123E, 28-33N:

| case | status | bbox cells | background median dx | river-overlap cells | retained L1/L2/L3 | coastal EarthMesh cells | coastal median dx |
| --- | --- | ---: | ---: | ---: | --- | ---: | ---: |
| `N64 ranked_coast20` | pass | `482` | `17.86 km` | `204` | `94/94/90` | `84` | `27.31 km` |
| `N96 r3d3 cst20` | pass | `1110` | `10.31 km` | `333` | `183/211/283` | `134` | `20.44 km` |
| `N112 r3d3 cst20` | pass | `2574` | `8.41 km` | `500` | `246/367/660` | `374` | `8.96 km` |
| `N128 r3d3 cst20 guardfix` | pass | `2934` | `7.40 km` | `592` | `307/513/989` | `441` | coast fraction median `0.71` |

The original `N128` smoke hit EarthMesh's close-curve segmentation guard:
`ERROR! num_sum must same as sum(n_close_curve)-1`.  The failing third refinement
step had two closed curves with `n_close_curve-1 = 29, 82`; the guard was checking
the first curve's local segment sum (`29`) against the global total (`112`).  The
guard now checks each closed curve against `n_close_curve(i)-1`, and the same N128
namelist completes successfully in an isolated validation case.

The current lighter promoted package remains `N112 r3d3 cst20`:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_rivers_and_integrated_coast_leaflet.html
```

The matching mesh is:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/cases/ATMOS_hydro_N112_r3d3_cst20/result/MPASOUT_NXP0112_global.nc4
```

This candidate is much heavier than the N64 smoke but is the first one where the
coastal overlap layer is mostly around 9 km in the QA window while retaining the
river R3 refinement.

The finer N128 guardfix validation output is:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/n128_guard_fix_validation/cases/ATMOS_hydro_N128_r3d3_cst20_guardfix/result/MPASOUT_NXP0128_global.nc4
```

The repaired run log contains `!! Successfully Make Grid End !!` and no
`ERROR! num_sum` guard failure.  Its integrated QA artifacts are:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/n128_guard_fix_validation/yangtze_delta_N128_r3d3_cst20_guardfix_with_coast_eval.json
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/n128_guard_fix_validation/yangtze_delta_N128_guardfix_rivers_and_integrated_coast_leaflet.html
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/n128_guard_fix_validation/package/delivery_manifest.json
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/n128_guard_fix_validation/package/colm_coupling/colm_coupling_cells.nc
```

The N128 delivery package writes `2934` coupling rows, `427` river cells, `441`
coast cells, and a CoLM coupling NetCDF.  It is the best current high-resolution
Yangtze-delta candidate when the extra N128 cost is acceptable.

Machine-readable evaluation artifacts were refreshed with the Phase 27 coast metrics and
ranked with the Phase 26 sweep scorer:

```bash
python3 -m util.hydro_mesh.refinement_sweep rank \
  --reports \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_N64_r3d3_ranked_coast20_with_coast_eval.json \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_N96_r3d3_cst20_with_coast_eval.json \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_N112_r3d3_cst20_with_coast_eval.json \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_N128_r3d3_cst20_failed_eval.json \
  --output-json \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_r3d3_cst20_with_coast_sweep_ranking_including_N128_failure.json \
  --max-background-cells 3000
```

After replacing the failed N128 row with the guardfix evaluation report, the
ranking recommends the N128 guardfix candidate under the same `--max-background-cells
3000` cap:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/n128_guard_fix_validation/yangtze_delta_r3d3_cst20_with_coast_sweep_ranking_guardfix.json
```

The current rank order is N128 guardfix, N112, N96, then N64.

The recommended N112 candidate can now be promoted into a reproducible delivery
package with one command:

```bash
python3 -m util.hydro_mesh.refinement_package \
  --case-name N112_r3d3_cst20 \
  --background-geojson \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.background_cells.geojson \
  --river-geojson \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.geojson \
  --coast-geojson \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_coastal_earthmesh_cell_intersections_preview.geojson \
  --log-path \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_hydro_close_N112_r3d3_cst20_smoke.log \
  --output-dir \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20 \
  --title "Yangtze delta N112 hydro/coast EarthMesh cells" \
  --comparison-reports \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_N64_r3d3_ranked_coast20_with_coast_eval.json \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_N96_r3d3_cst20_with_coast_eval.json \
  --failed-reports \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_N128_r3d3_cst20_failed_eval.json \
  --max-background-cells 3000
```

The package writes `delivery_manifest.json`, a coast-aware eval JSON, the integrated
Leaflet QA map, and a ranking JSON into the package directory.  Treat
`delivery_manifest.json` as the next stable input boundary for CoLM2024/CoLM20XX
coupling-table generation.

The delivery package can now be converted into the first CoLM2024/CoLM20XX-style
all-cell coupling table and NetCDF metadata artifact:

```bash
python3 -m util.hydro_mesh.colm_coupling package \
  --delivery-manifest \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20/delivery_manifest.json \
  --output-dir \
    /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20/colm_coupling
```

The smoke output writes:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20/colm_coupling/colm_coupling_cells.csv
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20/colm_coupling/colm_coupling_cells.nc
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20/colm_coupling/colm_coupling_summary.json
```

Observed summary for the N112 package:

- `rows_written = 2574` all background EarthMesh cells.
- `river_overlap_record_count = 500`, aggregated to `river_cell_count = 372` unique cells.
- `coast_overlap_record_count = 374`, aggregated to `coast_cell_count = 374` unique cells.
- `32` cells carry both river and coast coupling flags.

This is still a metadata handoff table, not final CoLM NetCDF.  It is the first
stable all-cell boundary for the later CoLM2024/CoLM20XX writer: every background
cell is present, sparse river/coast overlaps are joined by `cell_id`, and repeated
overlap records in the same cell are aggregated before export.

Surface classification is now an optional package/coupling input.  The input
surface mask can be either a raw LAND/OCEAN polygon layer, such as a CaMa
`elevtn.bin`-derived dissolved mask, or an already EarthMesh-cell keyed layer.  When
`--surface-geojson` is supplied, the package writer derives an adapter-ready
`<case>_complete_cell_mask.geojson` with exactly one feature per background
EarthMesh cell:

```bash
python3 -m util.hydro_mesh.cama_surface_mask \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/glb_01min \
  /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_cama_surface_land_ocean.geojson \
  --bbox 118 28 123 33

python3 -m util.hydro_mesh.refinement_package \
  --case-name N112_r3d3_cst20 \
  --background-geojson <background_cells.geojson> \
  --river-geojson <river_overlap_cells.geojson> \
  --coast-geojson <coast_overlap_cells.geojson> \
  --surface-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_cama_surface_land_ocean.geojson \
  --log-path <mkgrd.log> \
  --output-dir /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20_surface
```

The delivery manifest keeps the raw input under `source_files.surface_geojson` and
records the derived cell-keyed product under `files.complete_cell_mask_geojson`.
The CoLM package coupling export prefers `files.complete_cell_mask_geojson`, falling
back to legacy `source_files.surface_geojson` only for older packages.  It normalizes
`COAST_LAND` to `LAND` and `COAST_OCEAN` to `OCEAN`, while keeping coast overlap as
separate `has_coast/coast_class` fields.

Verified N112 CaMa surface-aware smoke output:

- Complete mask path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20_surface/N112_r3d3_cst20_complete_cell_mask.geojson`.
- Surface-aware HTML path: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20_surface/N112_r3d3_cst20_rivers_and_integrated_coast_leaflet.html`.
- The HTML embeds a `surfaceCells` layer, a `complete LAND/OCEAN cell mask` layer toggle, and LAND/OCEAN legend entries.
- `surface_source_kind = complete_cell_mask_geojson` in `colm_coupling_summary.json`.
- `surface_cell_count = rows_written = 2574`, so every background cell has a surface row.
- `surface_class_counts` in `colm_coupling_summary.json`: `LAND=2288`, `OCEAN=286`, `UNKNOWN=0`.
- The same counts are present in `colm_coupling_cells.csv` by row-level `surface_class`.
- Hydro flags remain separate: `has_river=372` cells and `has_coast=374` cells.

The package coupling export now writes both `colm_coupling_cells.csv` and
`colm_coupling_cells.nc`.  The NetCDF file is still a coupling metadata artifact,
not a complete CoLM forcing or restart file, but it gives downstream CoLM2024/CoLM20XX
work a typed `cell` dimension with longitude/latitude, class-code variables, river
fraction, coastal fraction, and area metadata.

## v3 MPAS/FVCOM/CoLM adapter bundle contracts

The v3 adapter sidecar writer now emits model-named artifacts and a machine-readable
`adapter_<name>_bundle.json` contract for every requested adapter.  The bundle groups
the canonical adapter cell CSV, run manifest, overlay summary, and any model-named
artifact with explicit artifact roles, readiness level, warnings, and limitations.
This closes the earlier schema/CSV-only handoff gap while still avoiding an unsafe
claim that the files have been ingested by production model runtimes.

Observed local v3 demo smoke:

```bash
OUT=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/v3_adapter_bundle_smoke
python3 -m util.v3_core.cli \
  --demo gba \
  --case-name v3_adapter_bundle_smoke \
  --recipe-hash adapter_bundle_smoke \
  --adapters colm2024,mpas,fvcom,colm20xx \
  --output-dir "$OUT" \
  --html-map "$OUT/gba_demo.html"
```

Observed adapter artifacts:

- MPAS mesh NetCDF handoff: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/v3_adapter_bundle_smoke/adapter_mpas_mesh.nc`.
- FVCOM mesh text handoff: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/v3_adapter_bundle_smoke/adapter_fvcom_mesh.dat`.
- CoLM20XX exchange NetCDF handoff: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/v3_adapter_bundle_smoke/adapter_colm20xx_exchange.nc`.
- Bundle manifests: `adapter_mpas_bundle.json`, `adapter_fvcom_bundle.json`,
  `adapter_colm2024_bundle.json`, and `adapter_colm20xx_bundle.json`.
- `adapter_mpas_bundle.json` has `readiness_level=model_handoff_contract` and
  `artifact_roles.mesh=mpas_unstructured_mesh_netcdf`.
- `adapter_fvcom_bundle.json` has `readiness_level=model_handoff_contract` and
  `artifact_roles.mesh=fvcom_unstructured_mesh_dat`.
- `adapter_colm20xx_bundle.json` has `readiness_level=exchange_schema_contract` and
  `artifact_roles.exchange=colm20xx_land_ocean_river_exchange_netcdf`.

The MPAS/FVCOM artifacts preserve cell IDs, centers, areas, vertex coordinates, and
v3 surface/hydro/coast class codes for downstream adapter development.  The bundle
contracts make their scope explicit: they are EarthMesh model-handoff contracts, not
validated runtime ingestion files, and they do not yet include model-specific forcing,
depth/open-boundary/control-deck products.

## CoLM20XX exchange NetCDF schema contract

The `colm20xx` adapter now emits a first formal exchange schema NetCDF:

```text
adapter_colm20xx_exchange.nc
```

The file has `kind=earthmesh_colm20xx_exchange_netcdf`, `adapter_name=colm20xx`,
and `schema_version=0.1`.  It stores one `cell` row per canonical EarthMesh cell
with center coordinates, area, surface/hydro/coast class codes, land/ocean/river/coast
fractions, and reserved exchange support flags for `land_ocean`, `river_land`,
`river_ocean`, `land_atmos`, and `ocean_atmos` coupling.  `adapter_colm20xx.json`
records this file as `files.exchange`, and `adapter_colm20xx_bundle.json` records it as the `colm20xx_land_ocean_river_exchange_netcdf` artifact role.

This is the first concrete CoLM20XX exchange schema contract.  It is still not a
validated future CoLM20XX runtime input file, because final model-side naming and
ingestion are outside the current EarthMesh repository, but it fixes the field names,
NetCDF boundary, bundle role, and coupling-support flags that future sea-land-hydro
integration work can consume.

Design note: `docs/superpowers/specs/2026-06-12-colm20xx-exchange-netcdf-design.md`.

## MERIT-Hydro 90m delivery-package bridge smoke

The current package boundary can now be driven by MERIT-Hydro 90m masks instead of
CaMa 1 arcmin surface masks.  The bridge intentionally reuses the existing package
and CoLM coupling handoff instead of creating a parallel format:

1. `util.v3_components.hydro_merit.write_merit_mask_outputs()` reads MERIT tiles for
   a bbox and writes raw `R2/R3`, `COAST_LAND/COAST_OCEAN`, and `LAND/OCEAN` masks.
2. `util.hydro_mesh.earthmesh_intersection.write_earthmesh_intersection_geojson()`
   projects MERIT river/coast masks onto a supplied EarthMesh/background cell layer.
3. `util.hydro_mesh.refinement_package.write_refinement_delivery_package()` writes
   the same delivery manifest, complete cell mask, surface-aware HTML, and ranking
   artifacts used by the CaMa package path.
4. `util.hydro_mesh.colm_coupling package` reads the manifest unchanged.

Small-window local smoke command:

```bash
SMOKE=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_package_bridge_smoke
python3 -m util.v3_core.grid \
  --bbox 113.8 22.2 114.0 22.4 \
  --nx 8 --ny 8 \
  --output "$SMOKE/background_cells.geojson" \
  --cell-id-prefix merit_gba

python3 -m util.hydro_mesh.merit_package_bridge \
  --case-name merit_gba_90m_bridge_smoke \
  --background-geojson "$SMOKE/background_cells.geojson" \
  --merit-root /Volumes/Data01/MERIT_Hydro \
  --bbox 113.8 22.2 114.0 22.4 \
  --log-path "$SMOKE/mkgrd_placeholder.log" \
  --output-dir "$SMOKE/package" \
  --title "MERIT GBA 90m bridge smoke" \
  --max-background-cells 100

python3 -m util.hydro_mesh.colm_coupling package \
  --delivery-manifest "$SMOKE/package/delivery_manifest.json" \
  --output-dir "$SMOKE/package/colm_coupling"
```

Observed smoke output:

- Manifest: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_package_bridge_smoke/package/delivery_manifest.json`.
- HTML: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_package_bridge_smoke/package/merit_gba_90m_bridge_smoke_rivers_and_integrated_coast_leaflet.html`.
- Complete mask: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_package_bridge_smoke/package/merit_gba_90m_bridge_smoke_complete_cell_mask.geojson`.
- MERIT source masks in the smoke: `river=84`, `coast=2986`, `surface=54770` features.
- EarthMesh intersections: `river_intersection_features=13`, `coast_intersection_features=75`.
- CoLM rows: `64`; `river_cell_count=10`; `coast_cell_count=38`.
- Surface counts: `LAND=22`, `OCEAN=42`, `UNKNOWN=0`.

This proves the 90m MERIT path can feed the same package/adapter handoff as the CaMa
path.  A full N112 Yangtze or China run should still be staged carefully because
MERIT stride-1 windows over multiple 5-degree tiles can produce very large raw mask
GeoJSON files before the final EarthMesh-cell package is compacted.

### Yangtze-delta N112 MERIT bridge stride-50 smoke

After adding a spatial index for complete surface-mask assignment, the bridge was
scaled from the 0.2 degree GBA test to the existing N112 Yangtze-delta background
cell layer over bbox `118 28 123 33` with `--stride 50`:

```bash
OUT=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride50
python3 -m util.hydro_mesh.merit_package_bridge \
  --case-name merit_yangtze_N112_stride50 \
  --background-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.background_cells.geojson \
  --merit-root /Volumes/Data01/MERIT_Hydro \
  --bbox 118 28 123 33 \
  --log-path /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_hydro_close_N112_r3d3_cst20_smoke.log \
  --output-dir "$OUT/package" \
  --title "MERIT Yangtze N112 bridge stride50 smoke" \
  --max-background-cells 3000 \
  --stride 50
```

Observed output:

- Runtime for package plus CoLM export: about `4` seconds on the local workstation.
- Manifest: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride50/package/delivery_manifest.json`.
- HTML: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride50/package/merit_yangtze_N112_stride50_rivers_and_integrated_coast_leaflet.html`.
- Complete mask: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride50/package/merit_yangtze_N112_stride50_complete_cell_mask.geojson`.
- MERIT source masks: `river=81`, `coast=2362`, `surface=12198` features.
- EarthMesh intersections: `river_intersection_features=176`, `coast_intersection_features=1444`.
- CoLM rows: `2574`; `river_cell_count=171`; `coast_cell_count=908`.
- Surface counts: `LAND=2257`, `OCEAN=317`, `UNKNOWN=0`.

Implementation note: complete cell-mask generation now builds a shapely `STRtree` for
surface polygons, so surface assignment queries only intersecting candidates instead
of scanning every raw MERIT surface polygon for every EarthMesh cell.  MERIT
`COAST_LAND` and `COAST_OCEAN` intersections also derive `surface_class=LAND/OCEAN`
for the complete mask while retaining coast as a separate coupling flag.

### Yangtze-delta N112 MERIT stride comparison

After the spatial-index fix, the same N112 Yangtze-delta package bridge was run at
finer MERIT sampling strides.  All runs use the existing N112 background EarthMesh
cells and bbox `118 28 123 33`.

| MERIT stride | Runtime | Output size | MERIT river masks | MERIT coast masks | MERIT surface masks | River cells | Coast cells | Surface counts |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 50 | ~4 s | small smoke output | 81 | 2362 | 12198 | 171 | 908 | LAND=2257, OCEAN=317, UNKNOWN=0 |
| 20 | ~10 s | 170M | 526 | 9361 | 80714 | 597 | 942 | LAND=2204, OCEAN=370, UNKNOWN=0 |
| 10 | ~33 s | 641M | 2033 | 24787 | 334381 | 1139 | 998 | LAND=2184, OCEAN=390, UNKNOWN=0 |
| 5 | ~97 s | 1.2G | 8189 | 61717 | 1372495 | 1673 | 1043 | LAND=2189, OCEAN=385, UNKNOWN=0 |

The `stride=5` output is the first finer-than-10 QA run.  It increases river-cell
coverage from `1139` to `1673` and coast-cell coverage from `998` to `1043`, but the
raw MERIT surface GeoJSON grows to about `1.2G`.  For `stride=1`, do not write raw
LAND/OCEAN surface polygons; use the compact-surface package mode below.

### Slim MERIT package mode

`util.hydro_mesh.merit_package_bridge` now accepts `--raw-merit-output-dir`.  When
this option is supplied, the bridge writes large raw MERIT-derived mask GeoJSON files
outside the final delivery package while keeping compact EarthMesh-cell artifacts,
the HTML QA map, the complete LAND/OCEAN cell mask, and the CoLM coupling export
inside `--output-dir`.  By default the bridge also skips the duplicate combined
`merit_masks.geojson` total layer; use `--write-combined-raw-mask` only when forensic
debugging needs that redundant combined file.  For fine runs, add
`--skip-raw-surface-mask`: the bridge then samples MERIT `landtype_igbp` at each
EarthMesh cell center and writes a compact cell-keyed complete mask, avoiding the
massive raw LAND/OCEAN surface GeoJSON.  The package manifest and bridge summary
still reference the external raw river/coast MERIT files for provenance.

`--compress-raw-merit` writes the raw MERIT river/coast/surface layers as
`.geojson.gz`.  The downstream EarthMesh intersection reader can consume these
compressed layers directly, so fine MERIT runs no longer need to keep large
uncompressed raw GeoJSON files between mask generation and package creation.

Example stride-10 Yangtze-delta command:

```bash
OUT=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride10_slim_layers
python3 -m util.hydro_mesh.merit_package_bridge \
  --case-name merit_yangtze_N112_stride10_slim_layers \
  --background-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.background_cells.geojson \
  --merit-root /Volumes/Data01/MERIT_Hydro \
  --bbox 118 28 123 33 \
  --log-path /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_hydro_close_N112_r3d3_cst20_smoke.log \
  --output-dir "$OUT/package" \
  --raw-merit-output-dir "$OUT/raw_merit_source" \
  --title "MERIT Yangtze N112 bridge stride10 slim layers smoke" \
  --max-background-cells 3000 \
  --stride 10

python3 -m util.hydro_mesh.colm_coupling package \
  --delivery-manifest "$OUT/package/delivery_manifest.json" \
  --output-dir "$OUT/package/colm_coupling"
```

Observed slim stride-10 output:

- Runtime for package plus CoLM export: about `26` seconds on the local workstation.
- Delivery package size: `15M`.
- External raw MERIT source size: `313M` without the duplicate combined raw mask.
- Manifest: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride10_slim_layers/package/delivery_manifest.json`.
- HTML: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride10_slim_layers/package/merit_yangtze_N112_stride10_slim_layers_rivers_and_integrated_coast_leaflet.html`.
- Complete mask: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride10_slim_layers/package/merit_yangtze_N112_stride10_slim_layers_complete_cell_mask.geojson`.
- Raw surface source recorded in the manifest: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride10_slim_layers/raw_merit_source/merit_surface_masks.geojson`.
- Bridge summary records `files.merit_masks = null`, confirming the duplicate combined raw mask was skipped.
- Raw layer sizes: `river=1.8M`, `coast=22M`, `surface=289M`, `summary=4K`.
- MERIT source masks: `river=2033`, `coast=24787`, `surface=334381` features.
- EarthMesh intersections: `river_intersection_features=1350`, `coast_intersection_features=1853`.
- CoLM rows: `2574`; `river_cell_count=1139`; `coast_cell_count=998`.
- Surface counts: `LAND=2184`, `OCEAN=390`, `UNKNOWN=0`; `surface_source_kind=complete_cell_mask_geojson`.

The package can therefore be handed to model adapters without carrying the raw MERIT
source polygons in the same directory.  Keep the external raw directory when
reproducibility or visual forensic QA is needed; archive only the `package/` directory
when the downstream consumer only needs EarthMesh-cell masks and coupling tables.

### Compact-surface MERIT stride-1 smoke

The compact-surface mode is the required path for fine MERIT sampling.  It keeps raw
MERIT river/coast masks for provenance, but does not write raw LAND/OCEAN surface
polygons.  Instead, the package contains the cell-keyed complete mask used by HTML
and CoLM coupling.

```bash
OUT=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride1_compact_surface
python3 -m util.hydro_mesh.merit_package_bridge \
  --case-name merit_yangtze_N112_stride1_compact_surface \
  --background-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.background_cells.geojson \
  --merit-root /Volumes/Data01/MERIT_Hydro \
  --bbox 118 28 123 33 \
  --log-path /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_hydro_close_N112_r3d3_cst20_smoke.log \
  --output-dir "$OUT/package" \
  --raw-merit-output-dir "$OUT/raw_merit_source" \
  --skip-raw-surface-mask \
  --title "MERIT Yangtze N112 bridge stride1 compact surface smoke" \
  --max-background-cells 3000 \
  --stride 1

python3 -m util.hydro_mesh.colm_coupling package \
  --delivery-manifest "$OUT/package/delivery_manifest.json" \
  --output-dir "$OUT/package/colm_coupling"
```

Observed compact-surface results over the same N112 Yangtze-delta window:

| MERIT stride | Runtime | Package size | Raw source size | Raw surface GeoJSON | MERIT river masks | MERIT coast masks | River cells | Coast cells | Surface counts |
| --- | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | --- |
| 5 | ~16 s | 18M | 62M | skipped | 8189 | 61717 | 1673 | 1043 | LAND=2165, OCEAN=409, UNKNOWN=0 |
| 1 | ~134 s | 20M | 495M | skipped | 204039 | 358012 | 2143 | 992 | LAND=2173, OCEAN=401, UNKNOWN=0 |

Observed stride-1 artifact paths:

- Manifest: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride1_compact_surface/package/delivery_manifest.json`.
- HTML: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride1_compact_surface/package/merit_yangtze_N112_stride1_compact_surface_rivers_and_integrated_coast_leaflet.html`.
- Complete mask: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride1_compact_surface/package/merit_yangtze_N112_stride1_compact_surface_complete_cell_mask.geojson`.
- Raw river masks: `179M`; raw coast masks: `315M`; raw surface masks: not written.
- Manifest `source_files` has no `surface_geojson`; CoLM uses `surface_source_kind=complete_cell_mask_geojson`.

This closes the Yangtze-delta stride sweep through native MERIT resolution for the
existing N112 background mesh.  The China-region scaling step below uses the same
compact-surface handoff with `--skip-raw-surface-mask`.

### Compressed raw MERIT layer smoke

The compact-surface mode can now add `--compress-raw-merit` to reduce raw
river/coast provenance storage while keeping the delivery package unchanged:

```bash
OUT=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_yangtze_N112_bridge_stride5_compact_gzip
python3 -m util.hydro_mesh.merit_package_bridge \
  --case-name merit_yangtze_N112_stride5_compact_gzip \
  --background-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/yangtze_delta_hydro_close_N112_r3d3_cst20_earthmesh_cell_intersections_preview.background_cells.geojson \
  --merit-root /Volumes/Data01/MERIT_Hydro \
  --bbox 118 28 123 33 \
  --log-path /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_hydro_close_N112_r3d3_cst20_smoke.log \
  --output-dir "$OUT/package" \
  --raw-merit-output-dir "$OUT/raw_merit_source" \
  --skip-raw-surface-mask \
  --compress-raw-merit \
  --title "MERIT Yangtze N112 stride5 compact gzip" \
  --max-background-cells 3000 \
  --stride 5
```

Observed stride-5 compressed output:

- Delivery package size: `18M`.
- External raw MERIT source size: `2.4M` instead of the previous `62M` uncompressed
  compact-surface raw river/coast layers.
- Raw compressed layer sizes: `river=421078 bytes`, `coast=2073750 bytes`.
- MERIT source masks: `river=8189`, `coast=61717`, `surface=0` because raw surface
  output was skipped.
- EarthMesh intersections: `river_intersection_features=2180`,
  `coast_intersection_features=2008`.
- CoLM rows: `2574`; `river_cell_count=1673`; `coast_cell_count=1043`.
- Surface counts: `LAND=2165`, `OCEAN=409`; `surface_source_kind=complete_cell_mask_geojson`.

### China-region N160 MERIT compact-surface package

The first China-region MERIT package reuses the existing N160 background cell layer
covering China mainland, Taiwan, and surrounding seas.  The background layer has
`19737` EarthMesh cells and bbox approximately `72.69E-136.39E`, `2.74N-54.22N`;
the source refinement domain was bbox `73 3 136 54`.  Use the `nowce` smoke log for
package metadata because the non-`nowce` and tiled China logs hit a Fortran runtime
index error during refinement cleanup.

```bash
OUT=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface
python3 -m util.hydro_mesh.merit_package_bridge \
  --case-name merit_china_N160_stride20_compact_surface \
  --background-geojson /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/china_region_N160_background_cells.geojson \
  --merit-root /Volumes/Data01/MERIT_Hydro \
  --bbox 73 3 136 54 \
  --log-path /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/earthmesh_china_region_N160_d3_nowce_smoke.log \
  --output-dir "$OUT/package" \
  --raw-merit-output-dir "$OUT/raw_merit_source" \
  --skip-raw-surface-mask \
  --title "MERIT China N160 bridge stride20 compact surface" \
  --max-background-cells 25000 \
  --stride 20

python3 -m util.hydro_mesh.colm_coupling package \
  --delivery-manifest "$OUT/package/delivery_manifest.json" \
  --output-dir "$OUT/package/colm_coupling"
```

Observed China-region output:

- Runtime for package plus CoLM export: about `411` seconds on the local workstation.
- Delivery package size: `97M`; external raw MERIT source size: `232M`; total output size: `329M`.
- Manifest: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/delivery_manifest.json`.
- HTML: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/merit_china_N160_stride20_compact_surface_rivers_and_integrated_coast_leaflet.html`.
- Complete mask: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/merit_china_N160_stride20_compact_surface_complete_cell_mask.geojson`.
- Raw river masks: `16M`; raw coast masks: `216M`; raw surface masks: not written.
- MERIT source masks: `river=18082`, `coast=246213`, `surface=0` because raw surface output was skipped.
- EarthMesh intersections: `river_intersection_features=8276`, `coast_intersection_features=9085`.
- CoLM rows: `19737`; `river_cell_count=6518`; `coast_cell_count=4872`.
- Surface counts: `LAND=11908`, `OCEAN=7829`, `UNKNOWN=0`; `surface_source_kind=complete_cell_mask_geojson`.
- CoLM NetCDF: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/colm_coupling/colm_coupling_cells.nc` (`2.9M`), with `cell=19737`, `kind=earthmesh_colm_coupling_netcdf`, and code variables for surface, river, and coast classes.

This is the first full China-region package generated through the MERIT bridge.  It
proves that the compact-surface handoff scales from the Yangtze test window to the
China/Taiwan/surrounding-seas N160 background layer without carrying raw surface
polygons in the package.  The separate MERIT-driven regeneration smoke below proves
the close-mask regeneration loop; applying that regeneration path to the full China
N160 domain remains a production-scale QA exercise rather than a missing code path.

### Hydro/coast mesh QA gates

Before promoting a hydro/coast delivery package to a production regeneration or
adapter handoff, run the package-level QA gates.  The gates require a complete
cell mask for every background cell, explicit LAND/OCEAN classes with no UNKNOWN
surface cells, non-empty river and coast overlaps, and optional CoLM all-cell row
count consistency.

```bash
python3 -m util.hydro_mesh.qa_gates \
  --delivery-manifest /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/delivery_manifest.json \
  --colm-summary-json /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/colm_coupling/colm_coupling_summary.json \
  --output-json /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/hydro_mesh_qa_report.json \
  --min-river-cells 1 \
  --min-coast-cells 1
```

Observed China N160 MERIT package QA result:

- QA report: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_china_N160_bridge_stride20_compact_surface/package/hydro_mesh_qa_report.json`.
- Status: `pass`.
- Background cells: `19737`; complete mask cells: `19737`; CoLM rows: `19737`.
- River overlap cells: `8276`; coast overlap cells: `9085`.
- Surface class counts: `LAND=11908`, `OCEAN=7829`, `UNKNOWN=0`.
- Passing checks: `complete_mask_present`, `complete_mask_cell_count_matches_background`,
  `surface_classes_known`, `land_ocean_both_present`, `river_cells_present`,
  `coast_cells_present`, `colm_rows_match_background`, and `colm_surface_unknown_zero`.

These gates are the minimum promotion threshold before a large-domain
MERIT-driven `mkgrd.x` regeneration run.  They do not replace visual inspection or
river/coast scientific threshold tuning; they prevent the known operational failure
classes such as missing cell masks, UNKNOWN surface cells, empty river/coast overlays,
and CoLM row-count drift.

### MERIT-driven mesh regeneration loop

The MERIT bridge now has a direct regeneration entry point for turning MERIT-Hydro
features into EarthMesh close-mask refinement inputs, rather than only projecting
MERIT masks onto an already-generated background mesh:

```bash
RUN=/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_gba_regeneration_smoke
python3 -m util.hydro_mesh.merit_mesh_regeneration \
  --case-name ATMOS_merit_gba_regeneration_smoke \
  --merit-root /Volumes/Data01/MERIT_Hydro \
  --bbox 113.8 22.2 114.0 22.4 \
  --output-dir "$RUN" \
  --template-nml /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/cases/ATMOS_hydro_N128_r3d3_cst20/result/namelist.save \
  --case-base-dir "$RUN/cases" \
  --stride 1 \
  --r2-cap 4 \
  --r3-cap 4 \
  --coast-cap 4

./mkgrd.x "$RUN/ATMOS_merit_gba_regeneration_smoke.mnl" > "$RUN/mkgrd.log" 2>&1
```

The utility writes compressed raw MERIT river/coast GeoJSON layers, a
`merit_close_mask_recipe.json`, EarthMesh close-mask NML files under the
`refine_spc_merit` prefix, and an optional patched mkgrd namelist with
`RL%mask_refine_spc_fprefix` and `RL%max_iter_spc` set from the generated masks.
`composite_refine_mask_export` reads both plain `.geojson` and `.geojson.gz`
inputs, so the regeneration loop can use the compressed raw MERIT layers
directly.

Observed GBA bounded smoke result:

- Summary: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_gba_regeneration_smoke/merit_mesh_regeneration_summary.json`.
- Patched namelist: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_gba_regeneration_smoke/ATMOS_merit_gba_regeneration_smoke.mnl`.
- Generated mesh: `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/merit_gba_regeneration_smoke/cases/ATMOS_merit_gba_regeneration_smoke/result/MPASOUT_NXP0128_global.nc4`.
- mkgrd log ended with `!! Successfully Make Grid End !!`.
- Close-mask components: `merit_coast=8`, `merit_river=16`, total close files `24`.
- Class/degree split: `COAST_LAND_d1=4`, `COAST_OCEAN_d1=4`, `R2_d1=4`, `R3_d1=4`, `R3_d2=4`, `R3_d3=4`.
- Compressed raw MERIT source size: about `92K` for this bounded smoke.

This proves the first end-to-end MERIT -> compressed raw masks -> close-mask NML
prefix -> patched mkgrd namelist -> `mkgrd.x` -> MPASOUT mesh loop.  It is still
a bounded smoke, not a promoted China/Yangtze production regeneration.  Before a
large-domain regenerated mesh is treated as a deliverable, the R2/R3/coast caps,
buffer distances, and visual/metric QA thresholds should be reviewed on the target
domain.
