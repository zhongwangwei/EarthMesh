# R3 — Geometry Safety Layer (MVP) 报告

> 阶段：R3（geometry safety layer；**不重写 geometry engine**）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [07_geometry_gis_audit.md](../07_geometry_gis_audit.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)
> 范围：在 `earthmesh_geometry`（干净、无 netcdf、可即时验证）加一个**附加**安全层；既有 `polygon_area`/`clip`/`intersection_area`/`overlay_cell` 行为不变，只新增校验与 flag。

## 1. Fixed flags（GeometryQualityFlag，13 个全实现）
`zero_area_cell` · `invalid_polygon` · `self_intersection` · `duplicate_vertex` · `dateline_crossing` · `polar_region_warning` · `planar_area_used_warning` · `projection_distortion_warning` · `mask_overlap_conflict` · `missing_mask` · `unresolved_fraction_sum_error` · `negative_area` · `non_finite_coordinate`。
- 定义于 `geometry/src/safety.rs` 的 `enum GeometryQualityFlag`，`as_str()` 映射到既有字符串（`zero_area_cell`/`missing_mask` 与旧 `overlay_cell` 完全一致 → `Vec<String>` 消费者与 pyo3 绑定不破）。

## 2. 实现内容
**polygon validation** — `safety::validate_polygon(&[Point]) -> Vec<GeometryQualityFlag>`：
- finite coordinates（`non_finite_coordinate`）
- ≥3 unique vertices（否则 `invalid_polygon`）
- no duplicate consecutive vertices（`duplicate_vertex`，含 wrap 首尾）
- non-zero area（`zero_area_cell`）
- self-intersection（`self_intersection`，复用既有 `area_judge_first_self_intersection_fortran_indexed`，**独立于面积**——对称 bow-tie shoelace 面积为 0 仍能检出）
- ring orientation（`ring_is_clockwise()` 提供；CW 非错误，不发硬 flag，文档说明本 crate 用 open ring）
- dateline-crossing（`dateline_crossing`，lon 跨度 >180° 启发式）
- high-latitude（`polar_region_warning`，|lat| ≥ 75°）

**overlay validation** — 增强 `overlay_cell`（向后兼容）：
- 非有限 cell 坐标早退 → `non_finite_coordinate`（修原 bug：NaN 面积会绕过 `<=0.0`）
- `negative_area`（防御性分支；`polygon_area` 无符号故当前不触发，留作面积模型升级）
- NaN 面积拒绝 → `zero_area_cell`
- `mask_overlap_conflict`：≥2 个贡献 mask 在最高 priority **并列**（歧义 winner）
- flags 挂在 `OverlayCellResult.quality_flags`
- **关键判定**：overlay 的 per-class fraction 跨**不同类**可合法 >1（重叠覆盖，既有测试 sum=1.4375 仍 `is_empty()`）→ 故 **fraction-sum 校验不放在 overlay**，而是独立给互斥分区用：

**fraction partition validation** — `safety::validate_fraction_partition(&[f64], tol)`：互斥分区（land/ocean/coast 应和=1）；`unresolved_fraction_sum_error`（|sum−1|>tol）+ `negative_area`（负分数）+ `non_finite_coordinate`。供 R5 coupling 守恒用。

**degree-buffer 警告** — `safety::degree_buffer_warnings(buffer_deg, max_abs_lat, lon_span)`：
- 总是 `planar_area_used_warning`
- 高纬（|lat|≥60°）或大跨度（lon_span≥30°）→ `projection_distortion_warning`
- |lat|≥75° → `polar_region_warning`
- 附 `KM_BUFFER_ADVICE` 常量：建议 meter/km buffer + 局地等面积投影后反投

## 3. preview / production / planar / spherical 区分（已在代码与文档明确）
- `safety::GeometryKind { Preview, Production }`：Preview（GUI 地图绘制/瓦片）**永不**用于面积/fraction/守恒；Production（mask/overlay/fraction）需校验。
- `safety::AreaModel { PlanarDegree, SphericalMeters, LocalEqualAreaProjected }`：当前全部 `PlanarDegree`（平面近似）；后两者为未来路径。
- 模块级文档（`safety.rs` 顶部）写明四者关系与"小单元 fraction 比值≈OK、绝对面积/高纬/大跨度失真"。

## 4. Tests（geometry `--all-targets` 全绿：30 测试 0 failed；fmt PASS）
| 要求测试 | 实现 | 结果 |
|----------|------|------|
| zero area polygon | `zero_area_polygon_flagged` | ✅ |
| duplicate vertex | `duplicate_consecutive_vertex_flagged` | ✅ |
| self-intersecting polygon | `self_intersecting_polygon_flagged`（bow-tie，零面积仍检出） | ✅ |
| dateline-crossing warning | `dateline_crossing_flagged` | ✅ |
| high-latitude degree buffer warning | `high_latitude_degree_buffer_warns_projection_distortion` | ✅ |
| overlay fraction sum warning | `fraction_partition_sum_over_one_flagged` | ✅ |
| missing mask flag | 既有 `overlay_cells_batches_..._missing_mask_flags`（仍绿） | ✅ |
| (附加) invalid/<3、non-finite、polar polygon、low-lat buffer、sum=1 clean、flag 字符串、valid square 无 flag | 见 safety::tests | ✅ |
| 既有几何不回归 | `overlay_cell_returns_class_fractions`(is_empty)、polygon/intersection/concave | ✅ |

下游：`cargo build -p earthmesh_cli --features static-netcdf` 与 `-p earthmesh_gui`（geometry 改动后）见 §6（后台验证）。

## 5. Files changed
| 文件 | 改动 |
|------|------|
| `rust/earthmesh_geometry/src/safety.rs` | 新增（enum + validators + buffer 警告 + AreaModel/GeometryKind + 14 测试） |
| `rust/earthmesh_geometry/src/lib.rs` | `+pub mod safety;`；`overlay_cell` 增强（非有限早退 + 负面积防御 + 并列冲突），既有行为不变 |

## 6. Remaining geometry limitations（本 MVP 只"诊断"未"修复"几何数学）
1. **平面面积仍是平面**：`polygon_area` 仍 degree² shoelace；safety 层只**标记** `planar_area_used_warning`，不替换为球面/等面积（R3 范围=safety，几何重写在后）。
2. **`intersection_area` 三角化失败仍静默回退凸裁剪**：`validate_polygon` 能**检出**自交 mask，但 `intersection_area` 本身未改（不重写引擎）；建议调用方先 `validate_polygon` 再 overlay。
3. **凸 clip 限制**：`clip_convex_polygon` 仍要求 clip 凸；robust clipping 未做。
4. **dateline 仅启发式检测**（lon 跨度>180°）：检出但**不分裂**处理；overlay/area 跨 180° 仍不正确（只警告）。
5. **overlay 不自动校验 mask 多边形**：需调用方显式调 `validate_polygon`（避免改变 overlay 既有签名/性能）。
6. **flags 尚未接入 hydro/coupling 生产路径**：safety 层就绪，hydro buffer 警告、coupling fraction 校验的**接线**在 R4/R5。

## 7. Future robust geometry recommendation
- **球面/等面积面积**（替换平面 shoelace）：`AreaModel::SphericalMeters` 或局地 LAEA 投影后平面积分 → 修高纬失真（[07 G3](../07_geometry_gis_audit.md)）。
- **robust clipping**（凹/holes/multipolygon）：评估引入 `geo`/`geo-clipper`（许可/体量）或自研 Weiler-Atherton → 取代静默回退（[07 G4](../07_geometry_gis_audit.md)）。
- **dateline 分裂 + 极区投影**：统一经度归一化 + split-at-antimeridian + 极方位投影（[07 G5/G6](../07_geometry_gis_audit.md)）。
- **接线**：R4 hydro buffer 改 km + `degree_buffer_warnings`；R5 coupling 用 `validate_fraction_partition` + overlay 守恒；overlay 入口自动 `validate_polygon` 并把 flag 汇入 `MeshQualityReport`/`run_manifest`。
- **合并两套平面几何**（geometry crate vs cli 内联）为单一后端（[07 G7](../07_geometry_gis_audit.md)）。
