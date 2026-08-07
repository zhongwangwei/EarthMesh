# Mesh Quality Report — WARN

- mesh: `/private/tmp/claude-501/-Users-zhongwangwei-Downloads-EarthMesh-3-0-0-alpha2/278d9a0f-5697-4b05-97c5-3549d4156fe7/scratchpad/run/g_100/result/gridfile_NXP0021_hex.nc4`
- cell view: `tri`
- tool: earthmesh_quality 3.0.0-alpha3

## Geometry

- cells: 9942 · vertices: 4974 · edges: 14913
- cell area (spherical km²): mean 5.1308e4, CV 0.317, max/min 16.11
- edge length (km): min 82.319, mean 338.225; max per-cell edge CV 0.255
- min angle: 32.33° · max angle: 112.06° · max angle deviation: 52.05° · max aspect: 1.84 · min compactness: 0.435
- local shape metric samples: 9942 · excluded coarse cells: 0
- local triangle quality: eta min 0.675 · NSR min 0.566
- zero-area: 0 · self-intersect: 0 · invalid: 0

## Topology

- invalid vertex idx: 0 · invalid cell idx: 0 · duplicate edges: 0 · dangling edges: 0 · boundary edges: 0
- orphan cells: 0 · neighbor-reciprocity fails: 0 · neighbor-degree mismatch: 0 · misoriented shared edges: 0 · abnormal polygons: 0
- Euler characteristic: 2 · expected: n/a · connected components: 1 · non-manifold vertex fans: 0
- cell sides: triangles 9942 · quads 0 · pentagons 0 · hexagons 0 · heptagons 0 · other 0
- cell-side counts are informational; quality gates are listed below
- isolated refined: 0 · max adjacent res ratio: 2.00 · transition warnings: 0

## Gates

| Metric | Value | Level |
|--------|-------|-------|
| invalid_vertex_index_count | 0 | pass |
| invalid_cell_index_count | 0 | pass |
| duplicate_edge_count | 0 | pass |
| dangling_edge_count | 0 | pass |
| misoriented_shared_edge_count | 0 | pass |
| neighbor_degree_mismatch_count | 0 | pass |
| orphan_cell_count | 0 | pass |
| neighbor_reciprocity_failure_count | 0 | pass |
| abnormal_polygon_edge_count | 0 | pass |
| boundary_vertex_degree_violation_count | 0 | pass |
| self_intersection_count | 0 | pass |
| invalid_polygon_count | 0 | pass |
| zero_area_cell_count | 0 | pass |
| negative_area_cell_count | 0 | pass |
| non_finite_cell_count | 0 | pass |
| min_angle_deg | 32.32581641494327 | pass |
| aspect_ratio_max | 1.835062931303399 | pass |
| cell_edge_length_cv_max | 0.2554459864474142 | pass |
| angle_deviation_deg_max | 52.05187755760762 | warn |
| cell_area_cv | 0.3174294983662944 | pass |
| cell_area_ratio | 16.111904513317413 | pass |
| max_adjacent_resolution_ratio | 2 | pass |
| transition_continuity_warning_count | 0 | pass |
| isolated_refined_cell_count | 0 | pass |

## Refine-level groups

| Level | Cells | Area CV | Edge CV max | Angle dev max | Tri eta local min | Tri NSR local min |
|-------|-------|---------|-------------|---------------|-------------|-------------|
| 0 | 8470 | 0.060 | 0.213 | 34.155 | 0.808 | 0.764 |
| 1 | 1472 | 0.550 | 0.255 | 52.052 | 0.675 | 0.566 |

**Verdict: WARN**

28 worst cell(s) in `worst_cells.geojson`.
