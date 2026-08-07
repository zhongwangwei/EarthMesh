# Mesh Quality Report — WARN

- mesh: `/private/tmp/claude-501/-Users-zhongwangwei-Downloads-EarthMesh-3-0-0-alpha2/278d9a0f-5697-4b05-97c5-3549d4156fe7/scratchpad/run/cmp_harp_dv/result/gridfile_NXP0021_hex.nc4`
- cell view: `tri`
- tool: earthmesh_quality 3.0.0-alpha3

## Geometry

- cells: 14736 · vertices: 7371 · edges: 22104
- cell area (spherical km²): mean 3.4616e4, CV 0.763, max/min 77.33
- edge length (km): min 35.900, mean 252.317; max per-cell edge CV 0.292
- min angle: 28.12° · max angle: 123.60° · max angle deviation: 63.59° · max aspect: 2.10 · min compactness: 0.370
- local shape metric samples: 14736 · excluded coarse cells: 0
- local triangle quality: eta min 0.565 · NSR min 0.418
- zero-area: 0 · self-intersect: 0 · invalid: 0

## Topology

- invalid vertex idx: 0 · invalid cell idx: 0 · duplicate edges: 0 · dangling edges: 0 · boundary edges: 0
- orphan cells: 0 · neighbor-reciprocity fails: 0 · neighbor-degree mismatch: 0 · misoriented shared edges: 0 · abnormal polygons: 0
- Euler characteristic: 2 · expected: n/a · connected components: 1 · non-manifold vertex fans: 0
- cell sides: triangles 14736 · quads 0 · pentagons 0 · hexagons 0 · heptagons 0 · other 0
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
| min_angle_deg | 28.1208842023318 | pass |
| aspect_ratio_max | 2.0977466038023787 | pass |
| cell_edge_length_cv_max | 0.29195258226940657 | pass |
| angle_deviation_deg_max | 63.59498806390581 | warn |
| cell_area_cv | 0.7630719356441835 | pass |
| cell_area_ratio | 77.32899213141029 | pass |
| max_adjacent_resolution_ratio | 2 | pass |
| transition_continuity_warning_count | 0 | pass |
| isolated_refined_cell_count | 0 | pass |

## Refine-level groups

| Level | Cells | Area CV | Edge CV max | Angle dev max | Tri eta local min | Tri NSR local min |
|-------|-------|---------|-------------|---------------|-------------|-------------|
| 0 | 8183 | 0.059 | 0.101 | 12.772 | 0.962 | 0.959 |
| 1 | 6553 | 1.099 | 0.292 | 63.595 | 0.565 | 0.418 |

**Verdict: WARN**

50 worst cell(s) in `worst_cells.geojson`.
