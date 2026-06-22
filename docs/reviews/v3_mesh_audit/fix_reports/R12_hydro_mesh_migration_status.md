# R12 — util/hydro_mesh Python → Rust 迁移状态 + colm_coupling 补齐

> 缘起:用户指出 river/coast fraction、邻接在 GBA/长三角早实现过——纠正我先前(看 Rust stub)的误判,并把"全部转换 Python → Rust"落到实处。本阶段先**核清真实迁移状态**,再补齐确认未迁的纯逻辑。
> 日期：2026-06-22 · 全部**真跑验证**。

## 1. 关键纠正
我先前判"river/coast fraction 不存在 / R7 强接=空壳",是因为只看了 Rust 的半成品 stub `write_colm_coupling_csv_from_mesh`(river/coast 写死 0)。事实:
- **几何 overlay 早已在 Rust**:`earthmesh_geometry::{clip_convex_polygon(Sutherland-Hodgman), intersection_area, overlay_cell}`(`overlap_area = intersection_area(cell, mask.polygon)`)——即 Python `cell_mask_merge.py`/`earthmesh_intersection.py` 用 shapely 做的逐 cell 交叠面积,Rust **用自写几何实现了,无需 shapely**。
- Python 包(5755 行/25 模块)**零重型依赖**(无 numpy/geopandas/pyproj/netCDF4);只有交叠那两个模块惰性 `import shapely`,而那正是 Rust 已替代的部分。

## 2. 逐模块迁移状态(核查后)
| Python 模块 | Rust | 状态 |
|---|---|---|
| classifier.py | `classify_merit_cell`/`classify_merit_hydro_window` | ✅ |
| MERIT tile 读取/选择 | `read_merit_hydro_window`/`select_merit_hydro_tiles`/`merit_tile_bounds_from_name` | ✅(本轮另修 dateline) |
| geojson_export / mask 图层 | `write_merit_hydro_mask_geojson_layers` | ✅ |
| cell_mask_merge.py / earthmesh_intersection.py（shapely 交叠） | `overlay_cell`/`overlay_cells`/`intersection_area`/`clip_convex_polygon` | ✅(无 shapely) |
| refine_mask_export / refinement_recipe / composite_refine_mask_export | `write_hydro_close_mask_nmls`/recipe/`write_hydro_composite_close_mask_nmls` | ✅ |
| cama_*（binary/inventory/sample/contract/surface_mask） | `read_cama_*`/`build_cama_reach_inventory`/`classify_cama_reach_record` | ✅ |
| **colm_coupling.py** | `colm_coupling_rows_from_intersections` + `write_colm_coupling_csv_from_intersections` + `--colm-coupling-from-intersections` | ✅ **本轮补齐** |
| colm_coupling netcdf | `write_colm_coupling_netcdf_from_csv` | ✅ |
| refinement_package / merit_package_bridge | 仅 `write_colm_package_delivery_manifest` | ◐ 半（manifest 写有，组包编排无） |
| qa_gates.py | R7 `quality::coupling` 为并行重写 | ◐ 非忠实 port |
| refinement_eval.py | — | ❌ 未迁 |
| refinement_sweep.py | — | ❌ 未迁 |
| merit_mesh_regeneration.py | — | ❌ 未迁(编排) |
| coastal_band.py | — | ❌ 未迁 |
| geojson_map.py / corridor_preview.py | — | ❌ 未迁(HTML/leaflet 可视化,GUI 自绘) |

## 3. 本轮补齐:colm_coupling.py → Rust
忠实 port `intersections_to_coupling_rows` + `write_colm_coupling_csv`:读 cell×river intersection GeoJSON 的 `properties` → CoLM coupling 行;`river_fraction >= min_fraction` 且 `cell_id`、`river_class` 非空才保留;按 `(cell_id, river_class)` 排序。复用 cli 现成 `JsonParser`/`geojson_feature_nodes`(无新依赖)。新增 CLI `--colm-coupling-from-intersections <geojson> <out.csv> [min_fraction]`。

**验证(真跑)**:
- 单元/集成测试 `colm_coupling_from_intersections.rs`:min_fraction 过滤、缺 cell_id/river_class 丢弃、排序、越界 min_fraction 报错 —— 3 测试绿。
- **与真 Python 对照**:同一份 intersection geojson,`earthmesh_cli --colm-coupling-from-intersections` 与 `util.hydro_mesh.colm_coupling.write_colm_coupling_csv` 输出**字节级完全一致**。
- 完整 cli 套件 0 failed(纯增量,无回归)。

## 4. 仍未迁(诚实清单,供后续逐项补)
1. `refinement_eval.py`(244)— 细化评估(overlap cells / retained triangles 等 eval json)。
2. `refinement_sweep.py`(275)— 细化参数扫描 → ranking。
3. `qa_gates.py`(170)— hydro mesh QA 门禁(R7 是并行重写,可二选一对齐)。
4. `refinement_package.py`/`merit_package_bridge.py`/`merit_mesh_regeneration.py`— 组包/编排(manifest 写已迁,编排未)。
5. `coastal_band.py`(201)— 海岸带集成。
6. `geojson_map.py`(364)/`corridor_preview.py`(795)— HTML/leaflet 可视化(GUI 自绘,优先级低)。

> 数据/几何核心(分类·读取·掩膜·交叠·CaMa·coupling)已全在 Rust。剩余主要是 eval/ranking/编排 与可视化。可按 1→6 逐个忠实 port + Python 对照。

## 5. 后续迁移进度(逐模块忠实 port + 真 Python 对照)

| # | Python 模块 | Rust(cli) | CLI | 对照验证 |
|---|---|---|---|---|
| 1 | `qa_gates.py` | `evaluate_hydro_mesh_qa`/`write_hydro_mesh_qa_report` | `--hydro-mesh-qa` | status + 全 6 项 check id:status 一致 ✅ |
| 2 | `refinement_eval.py` | `write_refinement_eval_json`/`parse_refinement_log`/summaries | `--hydro-refinement-eval` | 背景/河流摘要数值一致 ✅ |
| 3 | `refinement_sweep.py` | `write_sweep_recipes`/`write_sweep_ranking` | `--hydro-sweep-recipes`/`--hydro-sweep-rank` | 排序顺序 + recommended 一致 ✅ |
| 4 | `coastal_band.py`(纯核心) | `coastal_land_mask_from_elevation`/`coastal_band_cells` | — | band 网格 + land mask 一致 ✅(dissolve 用 shapely union,Rust 无 union,未迁) |
| 5 | `refinement_package.py`(manifest) | `write_hydro_delivery_manifest` | `--hydro-delivery-manifest` | manifest metrics + recommended 一致 ✅(端到端编排另需 §下方两 writer) |
| 6 | `geojson_map.py`/`corridor_preview.py`(可视化) | — | — | **不迁**:leaflet HTML 无 Rust 消费方,GUI 用 walkers/egui 自绘地图 |

**仍未迁(真实剩余,需 Rust 多边形并集/几何 IO)**:
- `cell_mask_merge.py::write_complete_cell_mask_geojson`、`earthmesh_intersection.py::write_earthmesh_intersection_geojson` —— overlay 原语(`overlay_cell`/`intersection_area`)已在 Rust,但**把 overlay 结果写成 complete-cell-mask / cell×river intersection geojson 的 writer + MPAS cell 多边形读取 + 面积归一化**未迁;它们是 colm_coupling(#已迁,消费方)的上游生产者,也是 #5 端到端编排的依赖。
- `coastal_band` 的 dissolve、`composite_refine_mask_export` 的并集类操作 —— 需 Rust 多边形 union(当前只有 clip/intersection,无 union)。
- CaMa 编排(`merit_mesh_regeneration.py` 的 namelist patch 写出)—— 纯组合,可后续接。

> 结论:本轮把 qa_gates/eval/sweep/coastal-核心/package-manifest 五块纯逻辑忠实迁到 Rust 并逐一与真 Python 对照一致。真正还差的是 **overlay→geojson 的两个 writer**(需补 MPAS cell 读取 + geojson 写出;overlay 数学已具备)与 **多边形 union**(dissolve/composite),以及编排胶水与可视化(可不迁)。
