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
| `N128 r3d3 cst20` | failed | n/a | n/a | n/a | n/a | n/a | n/a |

The `N128` smoke hit EarthMesh's close-curve segmentation guard:
`ERROR! num_sum must same as sum(n_close_curve)-1`.  Do not promote it until the
segment ordering issue is fixed.  The current finer visual QA candidate is
therefore `N112 r3d3 cst20`:

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

The ranking report recommends `N112_r3d3_cst20`.  `N128_r3d3_cst20` is kept as a
failed row with the close-curve error summary so it is not mistaken for a missing
or merely unscored candidate.

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
all-cell coupling table:

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
- Surface counts in `colm_coupling_cells.csv`: `LAND=2288`, `OCEAN=286`, `UNKNOWN=0`.
- Hydro flags remain separate: `has_river=372` cells and `has_coast=374` cells.

This is still a metadata handoff table, not final CoLM NetCDF, but the surface-aware
package now gives CoLM2024/CoLM20XX an all-cell LAND/OCEAN base plus independent
river/coast coupling flags.
