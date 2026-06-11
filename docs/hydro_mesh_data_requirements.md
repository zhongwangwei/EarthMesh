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
- Many sampled records have `downstream_x=-9999` and `downstream_y=-9999`; this is preserved in output and should be interpreted carefully before using it as a definitive estuary flag.

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
