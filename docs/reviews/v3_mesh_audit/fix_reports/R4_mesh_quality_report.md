# R4 — MeshQualityReport MVP 报告

> 阶段：R4（mesh quality report MVP）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [08_mesh_quality_metrics_design.md](../08_mesh_quality_metrics_design.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)

## 1. 放置决策
新建 **`earthmesh_quality` crate**（依赖仅 `earthmesh_core` + `earthmesh_geometry`，**无 netcdf** → 编译快、可即时 `cargo test` 验证；已加入 workspace 第 6 成员）。它定义自己的轻量 **`QualityMeshInput`**（engine-agnostic：vertices + cells，不依赖 mesh 重类型），故 API 天然可独立。复用 R3 的 `geometry::safety::validate_polygon` 做几何校验、`haversine_km` 算边长、`polygon_area` 算面积。

## 2. MVP 指标（全部实现）
**Geometry**（`GeometryMetrics`）：cell_count · vertex_count · edge_count · cell_area(min/max/mean/std/CV) · edge_length_km(min/max/mean/std/CV，大圆距离) · min_angle_deg · max_angle_deg · aspect_ratio(Stat5) · compactness(4πA/P², Stat5) · zero_area_cell_count · negative_area_cell_count · self_intersection_count · invalid_polygon_count。
**Topology**（`TopologyMetrics`）：invalid_vertex_index_count · invalid_cell_index_count · duplicate_edge_count(非流形,>2 cell 共边) · dangling_edge_count(退化/越界边) · orphan_cell_count(不与任何 cell 共边) · neighbor_reciprocity_failure_count · abnormal_polygon_edge_count(<3 顶点) · isolated_refined_cell_count(refined cell 邻居全更粗) · max_adjacent_resolution_ratio(2^Δlevel) · transition_continuity_warning_count(Δlevel>1)。

## 3. 输出（4 种，`io` 模块，手写无 serde）
- `quality_summary.json`（`kind`/verdict/geometry/topology/gates）
- `quality_summary.csv`（一行一指标 + gates + `summary,verdict`）
- `worst_cells.geojson`（最差 cell 的闭合 Polygon + metric/value/level 属性）
- `quality_report.md`（人可读：geometry/topology/gates 表 + Verdict）
- `io::write_all(report, dir)` 一次写全 4 个。

## 4. Pass/Warn/Fail（`QualityThresholds`，保守默认 + 可配覆盖）
- **Fail（catastrophic topology/geometry）**：任一 invalid index / duplicate edge / dangling / orphan / neighbor-reciprocity / abnormal polygon / self-intersection / invalid polygon / zero/negative area > 0；或 min_angle < 5° / aspect ≥ 10。
- **Warn（suspicious degradation）**：min_angle < 20° / aspect ≥ 4 / area CV ≥ 1.5 / max_adjacent_resolution_ratio > 2 / transition 警告 / isolated refined。
- verdict = 所有 gate 的最坏级（Fail>Warn>Pass）。阈值是 `QualityThresholds` 字段，未来 config 可覆盖。

## 5. CLI 接线（`--mesh-quality`，每次可对 gridfile 出报告）
新增 cli 子命令 `--mesh-quality <gridfile.nc4> [out_dir]`：`read_gridfile_mesh_points` → 用三角形（M→W，**1-based** 索引、跳过 sentinel）视图建 `QualityMeshInput` → `compute` → `io::write_all`（默认写到 gridfile 所在目录）。打印 `mesh_quality_verdict/cells/min_angle_deg/output=...`。cli 加 `earthmesh_quality` 依赖。
> 自动 post-run（每次 mesh run 结束自动出报告）为下一步：在 run() 各 gridinit/refine 分支产物处调用同一入口（需逐分支接，已有 subcommand 入口可复用）。

## 6. Tests（`cargo test -p earthmesh_quality` 全绿：10 测试 0 failed；fmt PASS）
| 要求测试 | 实现 | 结果 |
|----------|------|------|
| tiny valid mesh | `tiny_valid_mesh_passes`（2 方格，verdict=Pass，edge_count=7，min_angle 90°） | ✅ |
| invalid index | `invalid_vertex_index_is_fail` | ✅ |
| duplicate edge | `duplicate_edge_non_manifold_is_fail`（3 cell 共边） | ✅ |
| zero area cell | `zero_area_cell_is_fail`（共线三角） | ✅ |
| bad neighbor reciprocity | `bad_neighbor_reciprocity_is_fail` | ✅ |
| quality JSON output | `quality_json_output_has_required_fields` | ✅ |
| worst_cells.geojson output | `worst_cells_geojson_output_for_bad_mesh`（bow-tie→feature） | ✅ |
| (附加) CSV 输出 / write_all 四件 / abrupt transition warn | `quality_csv_*`,`write_all_produces_four_artifacts`,`abrupt_transition_warns` | ✅ |

cli 接线编译/回归：见 §8（后台 static-netcdf 构建）。

## 7. Files changed
| 文件 | 改动 |
|------|------|
| `Cargo.toml`(root) | workspace 加 `earthmesh_quality` 成员 |
| `rust/earthmesh_quality/Cargo.toml` | 新 crate（core+geometry 依赖） |
| `rust/earthmesh_quality/src/lib.rs` | 新增：模型 + compute + 阈值 + verdict + 6 单测 |
| `rust/earthmesh_quality/src/io.rs` | 新增：JSON/CSV/GeoJSON/MD writers + write_all |
| `rust/earthmesh_quality/tests/quality.rs` | 新增：4 集成测试 |
| `rust/earthmesh_cli/Cargo.toml` | 加 earthmesh_quality 依赖 |
| `rust/earthmesh_cli/src/main.rs` | `--mesh-quality` 子命令 + gridfile→QualityMeshInput |

## 8. 验证结果（含真实 gridfile 运行时冒烟）
- `cargo fmt -p earthmesh_quality --check` → **PASS**；`cargo test -p earthmesh_quality --all-targets` → **10 passed / 0 failed**。
- `cargo build -p earthmesh_cli --features static-netcdf`（mesh-quality 接线）→ **exit 0**；`cli_help` 回归 → **pass**。
- **运行时冒烟**（真实全球网格 `cases/quickstart_n32/gridfile/gridfile_NXP0016_01_hex.nc4`）：写出全 4 件;5120 cells / 2563 verts / 7680 edges;min_angle **53.42°** / max 71.96° / max aspect **1.18** / orphan 0 / abnormal 0 → verdict **warn**。

### 运行时发现并已修复（避免误导用户）
1. **平面角度在全球网格上假阳性**：首次冒烟 min_angle=**0.27°**、verdict=fail —— 因 M→W 三角形含**跨 180° 日期线**的,在平面 (lon,lat) 下被测成窄 sliver。**修复**:角度改用 **3D 单位球弦向量**(`lonlat_to_unit`),aspect 改用 **haversine km 边长**(比值跨日期线安全)→ min_angle 0.27°→**53.42°**(近等边,正确)。
2. **OLAM sentinel/dummy 三角**:1 个退化三角(Fortran 1-based 哑元)→ 1 orphan + 1 abnormal,误致 fail。**修复**:cli `quality_input_from_gridfile` 跳过顶点不互异的三角 → orphan/abnormal 归 0。
3. **残留 warn = `cell_area_cv`**:平面 degree² 面积在全球网格上被跨日期线三角放大 → CV 偏高触发 Warn(非 Fail)。属 R3 标注的**平面面积限制**;球面/等面积面积是后续(§9.3)。报告对好网格给"warn 而非 fail",方向正确。

## 9. Remaining / next
1. **自动 post-run 报告**：把 `--mesh-quality` 逻辑接入 cli run() 各 mesh 产物分支（每次 run 自动出报告 + 写进 run_manifest 的 `quality_report` 字段）。
2. **hex 视图**：当前 cli 接线用三角形（M→W）视图；hex（W→M）cell 视图可加，给六边形网格更贴切的质量。
3. **物理保真 / 球面面积**：area 仍平面 degree²（已在 R3 标注 planar_area_used）；refine_level/neighbors 当前 cli 接线未填（→ refinement/transition 指标在 cli 路径暂为 0），需从 gridfile 读细化级与邻接后填充。
4. **接 QualityConstraintConfig**：阈值未来由 [03 ProjectConfig](../03_config_schema_audit.md) 的 `QualityConstraintConfig` 覆盖；门禁 Block 可阻断 run。
