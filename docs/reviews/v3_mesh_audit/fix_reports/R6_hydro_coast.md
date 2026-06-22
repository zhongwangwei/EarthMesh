# R6 — MERIT-Hydro / Hydro-Coast Validation Report (MVP) 报告

> 阶段：R6（hydro-coast 可靠性/可诊断/可复现 + validation report MVP）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [06_merit_hydro_hydro_coast_audit.md](../06_merit_hydro_hydro_coast_audit.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)
> 放置：`earthmesh_quality::hydro_coast`（纯诊断层，**无 netcdf**、可即时 `cargo test`；复用 R3 `geometry::safety`）。
> 边界（遵守"不要做"）：**不全面重写 MERIT reader、不引大型 GIS 依赖、不改 refinement 核心**。校验层接收 cli MERIT pipeline 已提取的事实(tiles/bbox/counts/close-mask rings),产出结构化诊断 —— 大多数"优先修"项的修法是**提供诊断/警告**,而非改 reader 行为。

## 1. HydroCoastValidationReport（字段齐全）
selected_tiles · bbox(+bbox_valid/crosses_dateline) · stride · nodata_count · river_feature_count · coast_feature_count · overlap_count · river_mouth_candidate_count · masks_by_class · features_dropped_by_simplify · expected_tile_count/coverage_fraction · warnings · geometry_flags · recommended_fixes · severity(Pass/Warn/Fail) · river_width_unit/upstream_area_unit。
**score 占位**(`HydroCoastScores`,不实现 optimizer)：hydro_score · coast_score · river_mouth_priority · estuary_priority · coupling_priority。
输出：`to_json` / `write_json`（`kind=earthmesh_hydro_coast_validation`）。

## 2. 优先修 15 项 → 落地方式
| # | 项 | R6 落地 |
|---|----|---------|
| 1 | MERIT root path validation | `merit_root_exists`→缺失即 Fail + fix |
| 2 | tile selection coverage report | `tile_coverage`(5° 网格,跨180°分段)→ expected_tile_count + coverage_fraction(<1 Warn) |
| 3 | bbox validation | `validate_bbox`(south<north/经纬范围)→ 非法 Fail |
| 4 | dateline warning | `crosses_dateline`(west>east 或 span>180)→ Warn + fix |
| 5 | high-lat degree buffer warning | 复用 `geometry::safety::degree_buffer_warnings` → projection_distortion/polar flag + km-buffer fix |
| 6 | nodata summary | `nodata_count` + warning |
| 7 | stride warning(漏窄河) | stride>1 → Warn + "narrow rivers may be skipped" + 聚合建议 |
| 8 | river width/upstream area units | `river_width_unit`/`upstream_area_unit` 记录,缺失告警 |
| 9 | river/coast class priority | 文档化 river>coast 优先(overlap fix 文案);冲突计数见 #10 |
| 10 | river/coast overlap conflict flags | `overlap_count`>0 → Warn + 拆 river-mouth 建议 |
| 11 | river mouth candidate detection | `river_mouth_candidates(river_pts,coast_pts,dist_km)` 纯启发式 + `river_mouth_candidate_count` |
| 12 | close polygon validation | 对每个 close-mask ring 跑 `validate_polygon`;自交→Fail(reject) |
| 13 | simplify tolerance warning | `features_dropped_by_simplify`>0 → Warn + 降容差/保护窄道建议 |
| 14 | composite mask duplicate/overlap report | `masks_by_class` 同类重复计数 → warning |
| 15 | hydro-coast eval summary | 整个 report = eval summary(severity + warnings + fixes + counts) |

severity 规则：root 缺失/bbox 非法/close-mask 自交 = **Fail**;coverage<1/stride>1/overlap/高纬 buffer/simplify drop/dateline = **Warn**。

## 3. Tests（`cargo test -p earthmesh_quality` 全绿：26 测试 0 failed;fmt PASS）
| 要求测试 | 实现(hydro_coast::tests) | 结果 |
|----------|---------------------------|------|
| no tiles found | `no_tiles_found_reports_zero_coverage`(coverage 0) | ✅ |
| bbox selects expected tiles | `bbox_selects_expected_tiles` | ✅ |
| high-lat warning | `high_latitude_degree_buffer_warns` | ✅ |
| stride warning | `stride_gt_one_warns` | ✅ |
| river/coast overlap | `river_coast_overlap_warns` | ✅ |
| self-intersecting close mask rejected | `self_intersecting_close_mask_rejected`(Fail) | ✅ |
| validation report written | `validation_report_written_and_has_fields`(JSON 字段 + write_json) | ✅ |
| (附加) invalid bbox=Fail / river_mouth_candidates | `invalid_bbox_is_fail`,`river_mouth_candidates_counts_near_coast` | ✅ |

## 4. Files changed
| 文件 | 改动 |
|------|------|
| `rust/earthmesh_quality/src/hydro_coast.rs` | 新增:LonLatBbox/TileBounds/HydroCoastScores/HydroCoastInputs/HydroCoastValidationReport + validate_bbox/tile_coverage/river_mouth_candidates/build_report + to_json/write_json + 9 测试 |
| `rust/earthmesh_quality/src/lib.rs` | `+pub mod hydro_coast` |

## 5. cli 集成（下一步,API 已就绪）
cli 的 `--merit-hydro-geojson` / hydro pipeline 已产出 `MeritHydroRegionWorkflowReport`(tiles/分类计数等)。集成:从该报告 + bbox/stride/close-mask rings 构建 `HydroCoastInputs` → `build_report` → `write_json("hydro_coast_validation.json")`。建议新增 `--hydro-coast-validate` 子命令或在 merit 生成后自动写。**未在本期接线**(需把 merit report 字段映射进 inputs,且 cli 需 netcdf 构建)——纯模块 + 测试为可验证 MVP,接线为薄后续。

## 6. Remaining / next
1. **cli 接线**(§5):把 MERIT pipeline 产物喂入 `HydroCoastInputs` 并写报告;`river_mouth_candidate_count` 用 `river_mouth_candidates` 从 classified 河/岸点算。
2. **实际 reader 行为修复**(非诊断):跨180° tile 选择(`merit_bbox_intersects`,[06](../06_merit_hydro_hydro_coast_audit.md) H-B1)、stride 改聚合、km buffer —— 本期只**诊断/警告**,真正改 reader 数学是更大工作(明确不做全面重写)。
3. **score 实现**:占位字段接入 [04](../04_physical_refinement_audit.md) 的 score_hydro_coast + [09](../09_score_based_refinement_design.md) planner（R7+）。
