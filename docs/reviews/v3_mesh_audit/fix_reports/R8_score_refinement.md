# R8 — Score-based / Physics-aware Refinement Skeleton 报告

> 阶段：R8（refinement score 框架 skeleton）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [09_score_based_refinement_design.md](../09_score_based_refinement_design.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)
> 放置：**新 crate `earthmesh_refine_planner`**（依赖 core + geometry + **quality**，无 netcdf，可即时 `cargo test`）。
> 边界（遵守"不要做"）：**不替换现有 refinement workflow、不加大型外部数据依赖、不做完整 optimizer**。这是一个 skeleton:criteria 读"预提取的 feature 列",planner 产出 target_level 图 + 决策报告,**不接管引擎**。

## 1. 11 个类型/API（全实现）
`trait RefinementCriterion` · `CriterionContext` · `CriterionMetadata`(含 `physical_process`,强制可解释) · `CellFeatureTable`(centroids/columns/neighbors/regions) · `CellScore`(raw/demand 0..1/confidence/reason) · `CompositeScoreConfig`(weights/combine/max_passes) · `RefinementBudget`(max_refined_cells/max_adjacent_level_jump) · `QualityConstraint`(min_angle/min_cell_area/max_adjacent_ratio/no_isolated/smooth) · `TargetLevelMap`(level+source) · `RefinementDecision`(raw/normalized/composite/target/final/rejection_reason/top_reason) · `RefinementReport`(decisions+target_levels+budget_used)。

## 2. Criteria（简单/mock，6 个）
- **`SpecifiedRegionCriterion`**(具体):centroid 在 bbox/circle 内→demand 1。
- **`land_cover_entropy_criterion`** / **`hydro_coast_score_criterion`** / **`coupled_coast_criterion`**(placeholder):读预提取列(landcover_entropy / hydro_coast_score / coupled_coast_score),clamp 0..1。
- **`distance_to_river_criterion`** / **`distance_to_coast_criterion`**(placeholder):demand = exp(-d/L),d 取自 km 距离列。
- 每个 criterion 带 `physical_process`(GUI "why refine here" 的来源)。

## 3. 目标功能（pipeline，全实现）
输入 features → 每 criterion `score`(raw+demand) → weighted composite(Σw·demand/Σw,0..1) → `target_level=round(composite·max_passes)` → **budget**(top-by-score 保留,超出 rejection="budget") → **quality floor**(min_cell_area:area/4^level<min→降级,rejection="min_cell_area") → **smooth/transition/de-isolate**(复用 `quality::topology` 的 `smooth_target_levels`/`enforce_max_adjacent_level_jump`/`remove_isolated_refined_cells`,降级 rejection="transition/isolation") → `final_level` + 每 cell `RefinementDecision`。

**必须保证项**:
- no isolated refined cells → `remove_isolated_refined_cells`（R5 复用）。
- smooth transition / max adjacent jump → `smooth_target_levels` + `enforce_max_adjacent_level_jump`。
- min angle / min area **可表示** → `QualityConstraint` 字段;min_area 已实际 enforce(降级)。
- rejection reason 可输出 → `RefinementDecision.rejection_reason`。
- GUI "why refine here" → `RefinementDecision.top_reason`(主导 criterion + 其 reason) + geojson `why` 属性。

## 4. 输出（3 种）
- `refinement_score.csv`:cell,composite_score,target_level,final_level,rejection_reason,top_reason。
- `target_levels.geojson`:每 cell centroid Point + {final_level,target_level,composite_score,rejection_reason,why}。
- `refinement_decision_report.json`:`kind=earthmesh_refinement_decision` + budget summary + per-cell decisions。
- `io::write_all` 一次写全 3 个。

## 5. Tests（`cargo test -p earthmesh_refine_planner` 全绿：6 测试 0 failed;fmt PASS）
| 要求测试 | 实现 | 结果 |
|----------|------|------|
| one criterion score | `one_criterion_score`(SpecifiedRegion→composite 1, level 3) | ✅ |
| multiple criteria weighted score | `multiple_criteria_weighted_score`(entropy + distance-to-river) | ✅ |
| budget constraint | `budget_constraint_caps_refined_cells`(max_refined=2,低分 cell rejection=budget) | ✅ |
| quality constraint rejection | `quality_constraint_rejection_min_area`(area/4<min→final 0,rejection=min_cell_area) | ✅ |
| target level smoothing | `target_level_smoothing_limits_adjacent_jump`(相邻级差≤1) | ✅ |
| report output | `report_output_csv_geojson_json` | ✅ |

## 6. Files changed
| 文件 | 改动 |
|------|------|
| `Cargo.toml`(root) | workspace 加 `earthmesh_refine_planner` 成员 |
| `rust/earthmesh_refine_planner/Cargo.toml` | 新 crate(core+geometry+quality 依赖) |
| `rust/earthmesh_refine_planner/src/lib.rs` | 11 类型 + 6 criteria + `plan()` + 6 测试 |
| `rust/earthmesh_refine_planner/src/io.rs` | csv/geojson/json writers + write_all |

## 7. Remaining / next
1. **接入真实 features**:cli 从 mesh + landtype/MERIT/CaMa 构建 `CellFeatureTable`(centroids/neighbors/feature 列),调 `plan()` 写 target_levels;真实 criteria(替换 placeholder 读列→从数据计算)。
2. **lower 到引擎**:把 `TargetLevelMap` 转成现有引擎消费的"每区域/每 cell 细化度"(doc 09 lowering)——**本期不做**(不替换 workflow)。
3. **multi-objective / Pareto / repair loop**:完整 optimizer 留后续(doc 09);本期 skeleton 只做加权 + 预算 + 约束。
4. **geojson 用 polygon**:当前 centroid Point;有 cell ring 时可升级为 Polygon。
