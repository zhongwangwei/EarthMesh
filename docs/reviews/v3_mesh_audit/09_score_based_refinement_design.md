# 09 — Score-Based / Physics-Aware / Budget-Constrained Refinement Design (EarthMesh v3)

> Phase P5/P7 衔接（capstone 设计提案，可提 patch，不落地）· 未修改任何 `src/rust` 代码
> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md)（总原则五条）
> 整合：[03 ProjectConfig/criteria/budget](./03_config_schema_audit.md) · [04 score_land/ocean/atmos](./04_physical_refinement_audit.md) · [05 coupling](./05_coupled_mesh_audit.md) · [06 hydro](./06_merit_hydro_hydro_coast_audit.md) · [07 geometry](./07_geometry_gis_audit.md) · [08 quality](./08_mesh_quality_metrics_design.md)
> 审查对象：EarthMesh v3，`v3.0.0-alpha1`，仅当前项目，不引用旧版本。
> 证据：`mesh.rs:19334-20031`(refine_iter_b/c/d/e/f/g judge)、`cli:4413-4814`(area_judge `>=` 阈值)、grep：budget/quantile/pareto/cost/target_level/resolution_ratio/max_cells = **0 命中**。
> 日期：2026-06-22 · 证据等级见 AUDIT_PRINCIPLES §5。

---

## 0. 核心结论（先读）

当前细化 = **纯布尔阈值**：每变量"超阈值→把该区域按配置度数细化"（`refine_iter_*_judge` `mesh.rs:19334+`、area_judge `>=` `cli:4413+`），过渡靠 `HALO`/`max_transition_row`，平滑靠 spring。**无 score、无预算、无收益/成本、无质量驱动的 repair、无 target-level 优化**（budget/quantile/pareto/cost/target_level 全 0 命中）。

本设计提出**统一的细化规划层（planner）**，置于现有细化引擎之上：DataLayers → 特征 → 每 criterion score → 复合 score → target level → 预算分配 → 质量约束 → repair loop → 最终 target level → 喂给现有 `spawn_nest`/refine loop。**现有布尔阈值判据退化为一个 criterion**，specified-region 退化为另一个 criterion → **10 种目标方法全部统一**，且 **v3 内部零迁移**（planner 输出现有引擎已消费的"每区域/每 cell 细化度"）。

10 种目标方法如何被统一引擎覆盖：

| 方法 | 实现方式 |
|------|----------|
| 1 specified-region | `RegionCriterion`（bbox/circle/close→score=1 区域内） |
| 2 threshold | `ThresholdCriterion`（包装现有布尔判据） |
| 3 score-based | composite score（§3） |
| 4 multi-objective | 多 criterion 加权 + Pareto（§4） |
| 5 physics-aware | criterion 声明 `physical_process` + `quality_contribution`（§2） |
| 6 cost-constrained | `RefinementBudget`（§4） |
| 7 quality-constrained | `QualityConstraint`（§5） |
| 8 plugin-defined | `trait RefinementCriterion`（§2） |
| 9 adaptive repair | repair loop（§7） |
| 10 GUI-guided | GUI score 热力图 + 交互权重（§8） |

---

## 1. Why Threshold-Only Is Insufficient

| # | 缺陷 | 证据 | 违背原则 |
|---|------|------|----------|
| 1 | 布尔判据：超阈值即细化，无"强度/优先级" | `refine_iter_*_judge :19334+`；area_judge `>=` `:4413` | 1,2 |
| 2 | 无预算：无法限 cell 数/计算成本，可无限细化 | `max_cells/budget/cost`=0 命中 | 3 |
| 3 | 无收益/成本权衡 | `benefit/pareto`=0 | 3 |
| 4 | 无质量反馈：细化后不回看质量、不修复 | 质量仅事后写 NetCDF（[08](./08_mesh_quality_metrics_design.md)），不驱动决策 | 4 |
| 5 | 多判据无法合成：各变量独立开关，无加权/多目标 | `refine_onelayer_*[N]` 平行开关（[04](./04_physical_refinement_audit.md)） | 1,2 |
| 6 | 无 target-level 优化：度数靠人工配置 | `target_level`=0 命中 | 3 |
| 7 | 过渡/隔离仅靠 HALO 固定行数，非自适应 | `HALO`/`max_transition_row`（[04](./04_physical_refinement_audit.md)） | 4 |
| 8 | 不可解释：无 per-cell"为什么细化"+"代价" | 无 score/reason | 5 |
| 9 | 数值脆性已显现：olam 半径 4 红（[01](./01_build_and_crate_audit.md)#1） | `olam_delaunay_mesh.rs` | 4 |

**结论**：threshold-only 能表达"哪里超标"，但不能回答"在预算内、保证质量的前提下，最该细化哪里、细化到几级"——后者正是地球系统网格的核心需求（原则 1–5）。

---

## 2. RefinementCriterion API

```rust
/// 一个 criterion = 一种细化驱动力（threshold / region / river / coastline / typhoon ...）。
pub trait RefinementCriterion: Send + Sync {
    fn metadata(&self) -> CriterionMetadata;           // 名字 / 物理过程 / 版本
    fn applicable_mesh_types(&self) -> Vec<MeshDomainKind>;  // land/ocean/atmos/coupled
    fn required_data(&self) -> Vec<CriterionDataSource>;     // 声明数据依赖([03](./03_config_schema_audit.md))
    fn score(&self, ctx: &CriterionContext, cell: usize) -> CellScore;  // per-cell
    fn score_units(&self) -> &'static str;             // "fraction"/"km"/"dimensionless"
    fn quality_contribution(&self) -> CriterionQualityContribution;  // improves/may_degrade([04](./04_physical_refinement_audit.md))
    fn gui_spec(&self) -> CriterionGuiSpec;            // label/help/range/默认([03](./03_config_schema_audit.md))
    fn cli_schema(&self) -> CriterionCliSchema;        // 参数名/类型/默认，供 CLI 解析
}

pub struct CriterionContext<'a> {
    pub mesh: &'a UnstructuredMesh,
    pub features: &'a CellFeatureTable,    // 预采样的每 cell 特征
    pub domain: MeshDomainKind,
    pub neighbors: &'a CellAdjacency,      // 邻接（供梯度/连通类判据）
}
/// 所有 criterion 共享的、预先从 DataLayers 采样/聚合好的 per-cell 特征表（避免每 criterion 重复读数据）。
pub struct CellFeatureTable {
    pub cell_count: usize,
    pub center: Vec<LonLat>,
    pub area_m2: Vec<f64>,                  // 球面/等面积([07](./07_geometry_gis_audit.md))
    pub columns: BTreeMap<String, Vec<f64>>,  // "lai_std","slope_std","upa","wth","dist_coast_km",...
    pub categorical: BTreeMap<String, Vec<i32>>,  // "landtype","river_class",...
    pub provenance: BTreeMap<String, InputFingerprint>, // 可复现([03](./03_config_schema_audit.md))
}
pub struct CellScore {
    pub raw: f64,           // 原始量（带 units）
    pub demand: f64,        // 归一化 0..1 细化需求
    pub confidence: f64,    // 0..1 数据可信度
    pub reason: String,     // "wth=420m≥R3" 可解释（原则5）
}
pub struct CompositeScoreConfig {
    pub criteria: Vec<(String /*criterion_id*/, CriterionWeight)>,
    pub combine: CombineRule,           // WeightedSum | WeightedMax | Pareto
    pub normalization: Normalization,   // RobustQuantile{lo,hi} | MinMax | Fixed(th)
}
pub enum CombineRule { WeightedSum, WeightedMax, Pareto }

pub struct RefinementBudget {            // 见 §4 / [03](./03_config_schema_audit.md)
    pub max_cells: Option<u64>,
    pub max_compute_cost: Option<f64>,   // 代理：Σ cell_count·iter
    pub min_edge_km: Option<f64>,        // CFL
    pub max_adjacent_resolution_ratio: Option<f64>,
    pub allocation: AllocationMethod,    // Quantile | GreedyBenefitCost | Pareto | Constrained
}
pub struct QualityConstraint {           // 见 §5 / [08](./08_mesh_quality_metrics_design.md)
    pub min_angle_deg: f64,
    pub min_cell_area_m2: f64,
    pub max_adjacent_resolution_ratio: f64,
    pub enforce_topology: bool,          // 闭合/邻接/无孤立
    pub enforce_coastline: bool,
    pub enforce_river_connectivity: bool,
    pub enforce_coupling_map: bool,
    pub on_violation: ViolationPolicy,   // Warn | Block | Repair
}
pub struct TargetLevelMap {
    pub level: Vec<u8>,        // per-cell 目标细化级
    pub source: Vec<String>,   // 主导 criterion（可解释）
}
pub struct RefinementDecision {
    pub cell: usize,
    pub raw_scores: Vec<(String, f64)>,
    pub normalized_scores: Vec<(String, f64)>,
    pub composite_score: f64,
    pub target_level: u8,          // 预算前
    pub final_level: u8,           // 预算+质量+repair 后
    pub rejection_reason: Option<String>,  // 为何没细化到 target（budget/quality）
    pub quality_flags: Vec<String>,        // 接 [07](./07_geometry_gis_audit.md)/[08](./08_mesh_quality_metrics_design.md)
}
pub struct RefinementReport {
    pub decisions: Vec<RefinementDecision>,
    pub budget_used: BudgetUsage,          // cells_before/after, cost, 是否触顶
    pub repair_iterations: u32,
    pub quality_before: Option<MeshQualityReport>,   // [08](./08_mesh_quality_metrics_design.md)
    pub quality_after: Option<MeshQualityReport>,
    pub verdict: QualityVerdict,
}
```

---

## 3. Composite Score Design

per-cell 流水（每 cell 产出一个 `RefinementDecision`）：

```
1. raw_scores[c]        = { criterion_id -> CellScore.raw }          // 各 criterion 原始
2. normalized_scores[c] = norm(raw) per CompositeScoreConfig.normalization  // 0..1, 稳健分位([04 §3.0](./04_physical_refinement_audit.md))
3. composite_score[c]   = combine(normalized · weight)              // WeightedSum / WeightedMax / Pareto
4. target_level[c]      = ceil(composite_score[c] · max_passes)     // 预算前的"理想"级
```

- **multi-objective**：`CombineRule::Pareto` 时保留非支配解集，预算阶段在 Pareto 前沿上取（§4）。
- **physics-aware**：每 criterion 的 `quality_contribution`（improves/may_degrade）参与决策——若一 criterion 的细化会 `may_degrade` MinEdge 且触及预算 `min_edge_km`，其贡献被抑制。
- **可解释**：`RefinementDecision` 保留 raw/normalized/composite + 主导 source + reason（原则 5）。

---

## 4. Budget Constraint Design

输入：`max_cells`、`min_angle`、`max_resolution_jump`、`max_compute_cost`。目标：在约束内选"收益最大"的细化集合。

| 方法 | 说明 | 适用 |
|------|------|------|
| **Score quantile** | 取 composite_score 的 top-q% 细化（q 由 max_cells 反推） | 快速、默认 |
| **Greedy benefit/cost** | 按 `Δscore / Δcells` 降序贪心加入，直到预算耗尽 | 收益/成本权衡 |
| **Pareto frontier** | 多目标：在 (score, cost) 前沿取非支配点 | multi-objective |
| **Constrained optimization** | 整数规划/松弛：max Σscore·level s.t. Σcost≤budget, 质量约束 | 高精度、慢 |
| **Target-level smoothing** | 对 target_level 做空间平滑（避免棋盘/突变） | 必跑 |
| **Transition-zone enforcement** | 相邻级差≤1，自动插入过渡环 | 必跑 |
| **Isolated-cell removal** | 去孤立细化 cell（邻居都低级则降级或补环） | 必跑 |

```
allocate(target_level, budget):
  cand = cells sorted by composite_score desc
  match budget.allocation:
    Quantile        -> keep top cells until Σcells_after ≤ max_cells
    GreedyBenefitCost -> add while benefit/cost best and within budget
    Pareto          -> pick on (score,cost) frontier within budget
    Constrained     -> ILP/relaxation under all constraints
  return capped_level  // 未入选的 cell 记 rejection_reason="budget"
```

`compute_cost ≈ Σ (4^level)·base_cells`（每升一级 hex/tri 约 ×4）；`max_compute_cost` 防爆。

---

## 5. Quality Constraint Design

细化级必须满足（否则降级/插过渡/触发 repair，对接 [08](./08_mesh_quality_metrics_design.md) 门禁）：

| 约束 | 默认 | 违反动作 |
|------|------|----------|
| min angle | tri≥30° / hex≥100° | 降级该 cell |
| min cell area | ≥ `min_edge_km²` 量级 | 降级 |
| max adjacent resolution ratio | ≤2 | 插过渡环 |
| no isolated refined cells | — | 补环或降级 |
| smooth transition | 级差≤1 | 插过渡 |
| no topology break | 闭合/邻接一致 | repair（§7） |
| no broken coastline | 岸线连续 | repair / 锁定岸线 cell |
| no broken river connectivity | 河网连通 | repair / 锁定河道 cell |
| no invalid coupling map | fraction 守恒 Σ=1（[05](./05_coupled_mesh_audit.md)） | repair / 阻断 |

`ViolationPolicy`：`Warn`（记录继续）/`Block`（失败退出）/`Repair`（进 §7 循环）。

---

## 6. Target Level Assignment Algorithm

`target_level = f(score, budget, physical_priority, quality_constraints)`：

```
ASSIGN(features, criteria, composite_cfg, budget, quality):
  1. for each cell c:  score[c] = composite(criteria, c, composite_cfg)
  2. raw_level[c]   = ceil(score[c] · max_passes)                 // 理想级
  3. priority[c]    = max over criteria of physical_priority      // 河口/岸线/河道置顶
  4. capped_level   = allocate(raw_level, priority, budget)       // §4 预算
  5. smoothed       = smooth_levels(capped_level)                 // 空间平滑
  6. transitioned   = enforce_transition(smoothed, quality.max_adjacent_resolution_ratio)
  7. deisolated     = remove_isolated(transitioned)               // 无孤立
  8. constrained    = apply_quality_floor(deisolated, quality)    // min angle/area→降级
  9. final_level    = repair_loop(constrained, quality)           // §7
  return TargetLevelMap{ level: final_level, source }
```

保证项（逐步强制）：no isolated（7）· smooth transition（6）· max adjacent ratio（6）· min angle/area（8）· topology/coastline/river/coupling（9 repair）。

---

## 7. Repair Loop（adaptive repair after quality check）

```
repair_loop(level, quality):
  for iter in 0..MAX_REPAIR:                      // 收敛或触顶
     plan  = engine_refine(level)                 // 调现有 spawn_nest/refine loop
     qr    = run_quality(plan, quality)           // [08](./08_mesh_quality_metrics_design.md)
     fails = qr.gates.filter(Fail)
     if fails.empty(): return level               // 收敛
     for f in fails:
        match f.metric:
          MinAngle | MinCellArea     -> downgrade offending cells
          AdjacentResolutionRatio    -> insert transition ring
          IsolatedRefinedCell        -> downgrade or fill neighbor ring
          NeighborReciprocity|Orphan -> topology repair (re-LOP / ngr_renew)
          CoastlineDiscontinuity     -> lock coastline cells, re-level
          RiverPathDiscontinuity     -> lock river cells, re-level
          UnresolvedFractionSumError -> recompute overlay fractions([05](./05_coupled_mesh_audit.md))
     level = apply_repairs(level, fails)
  // 触顶仍未收敛：按 on_violation = Block/Warn 处理，报告残余 fails
  return level
```

要点：repair **复用现有引擎**（`spawn_nest`/`refine_delaunay_lop`/`ngr_renew`）作为"执行器"，planner 只调级别；每轮跑 [08](./08_mesh_quality_metrics_design.md) 质量回看 → 真正的"质量驱动自适应"（原则 4），补上当前"细化后不回看"的缺陷。

---

## 8. GUI Mapping

| GUI 元素 | 数据 | 满足 |
|----------|------|------|
| Intent/preset 选择 | `CompositeScoreConfig` 预设权重（[04](./04_physical_refinement_audit.md)/[06](./06_merit_hydro_hydro_coast_audit.md)） | 原则 1 |
| 每 criterion 开关+权重滑杆 | 由 `CriterionGuiSpec` 自动渲染 | 原则 5 |
| **score 热力图** | `composite_score[c]` 着色 | 原则 5 |
| 点击 cell → reason | `RefinementDecision.reason`+raw/normalized | 原则 2/5 |
| 预算滑杆（max cells/min edge） | `RefinementBudget`，实时显示预计 cell 数/成本 | 原则 3 |
| **target vs final level** 对比图 | `target_level` vs `final_level` + rejection_reason | 原则 3 |
| **before/after 质量卡片** | `RefinementReport.quality_before/after` | 原则 4 |
| worst cells 高亮 | `quality_flags`（[08](./08_mesh_quality_metrics_design.md)） | 原则 4/5 |

> 当前 GUI 直接暴露 64 个 engine 字段且无质量反馈（[02](./02_workflow_consistency_audit.md)/[03](./03_config_schema_audit.md)）；本映射用 preset+score 取代，engine 旋钮进 Expert（[03 §6](./03_config_schema_audit.md#6-gui-mapping)）。

---

## 9. CLI Mapping

| 命令（提案） | 作用 |
|--------------|------|
| `earthmesh refine plan project.yaml` | 计算 score + target level，输出 `RefinementReport`（不执行） |
| `earthmesh refine explain project.yaml --cell N` | 打印某 cell 的 raw/normalized/composite + reason |
| `earthmesh refine run project.yaml` | plan → 预算/质量/repair → 执行（调现有引擎） |
| `earthmesh refine budget project.yaml --max-cells 200000` | 覆盖预算，预览取舍 |
| `mkgrd.x <legacy.nml>` | **保持不变**（旧布尔阈值路径） |

> 兼容：planner 产出现有引擎消费的"每区域/每 cell 细化度"；旧 namelist 布尔阈值经 `ThresholdCriterion` 包装为一个 criterion → 行为可逐字节回归（[03 §9](./03_config_schema_audit.md#9-migration-free-compatibility-strategyv3-内部无破坏)）。

---

## 10. Tests

| 测试 | 目的 |
|------|------|
| `criterion_score_normalization_robust_quantile` | 归一化稳健、跨纬度一致 |
| `composite_weighted_sum_and_pareto` | 复合规则正确 |
| `threshold_criterion_matches_legacy_bool` | 包装后与现有布尔判据逐字节一致（回归） |
| `region_criterion_specified_bbox_circle_close` | specified-region 等价旧行为 |
| `budget_quantile_respects_max_cells` | 预算上限被遵守 |
| `budget_greedy_benefit_cost_orders` | 贪心顺序正确 |
| `budget_pareto_frontier_nondominated` | Pareto 前沿正确 |
| `target_level_smoothing_no_checkerboard` | 平滑去棋盘 |
| `transition_enforced_max_ratio_2` | 相邻级差≤1 |
| `isolated_refined_cell_removed` | 无孤立细化 |
| `repair_loop_converges_min_angle` | repair 收敛到 min angle 达标 |
| `repair_preserves_coastline_and_river` | 岸线/河网连通锁定 |
| `repair_coupling_fraction_conserved` | 耦合守恒（[05](./05_coupled_mesh_audit.md)） |
| `rejection_reason_recorded_when_budget_hit` | 触顶 cell 记原因 |
| `refinement_report_before_after_benefit` | 收益量化 |

> 现状：refine 测试覆盖布尔判据/spawn_nest（`area_judge_*`/`refine_*`，[01](./01_build_and_crate_audit.md)）；score/budget/repair 测试全缺。

---

## 11. Implementation Roadmap

| 阶段 | 内容 | 先决 | 风险 |
|------|------|------|------|
| R1 | `earthmesh_refine_planner` crate 骨架 + trait/structs（§2） | [03](./03_config_schema_audit.md) S1 | 低（新增） |
| R2 | `CellFeatureTable` 采样器（从 DataLayers，球面面积） | [07 G3](./07_geometry_gis_audit.md) | 中 |
| R3 | `ThresholdCriterion` + `RegionCriterion`（包装现有，回归比对） | R1 | 中（回归关键） |
| R4 | composite score + normalization（§3） | R1-R3 | 低 |
| R5 | budget allocation（quantile→greedy→pareto）（§4） | R4 | 中 |
| R6 | target-level assignment + smoothing/transition/de-isolation（§6） | R5 | 中 |
| R7 | quality constraints + repair loop（§5/§7，接 [08](./08_mesh_quality_metrics_design.md)） | R6, [08](./08_mesh_quality_metrics_design.md) Q1-Q6 | 高 |
| R8 | 物理 criteria（river/coastline/estuary/typhoon...，接 [04](./04_physical_refinement_audit.md)/[05](./05_coupled_mesh_audit.md)/[06](./06_merit_hydro_hydro_coast_audit.md)） | R4 | 中 |
| R9 | CLI plan/explain/run（§9） | R6 | 低 |
| R10 | GUI score 热力图 + 预算 + before/after（§8） | R7, [08](./08_mesh_quality_metrics_design.md) Q9 | 中（归 P6） |

> 顺序：R1→R2→**R3（先把现有布尔判据包成 criterion，保证回归绿）**→R4→R5→R6→R7（repair，最重）→R8（物理）→R9/R10。每步独立 PR + 回归比对，符合 surgical change。先决全部指向 [03](./03_config_schema_audit.md) S1（project crate）与 [08](./08_mesh_quality_metrics_design.md)（quality）。

---

## 关键证据索引（file:line）

- 现有布尔判据：`mesh.rs:19334`(refine_iter_b)、`:19444`(g)、`:19517`(e)、`:19694`(f)、`:19807`(c)、`:20031`(d)；area_judge `>=` 阈值 `cli:4413-4814`
- 缺失概念（grep 0 命中）：budget / quantile / pareto / cost / benefit / target_level / resolution_ratio / max_cells
- 过渡/平滑现状：`HALO`/`max_transition_row`/spring（[04](./04_physical_refinement_audit.md)）
- 设计落点：[03 criteria/budget/QualityConstraintConfig](./03_config_schema_audit.md#3-rust-type-sketches)、[04 score](./04_physical_refinement_audit.md#3-score-formulas)、[05 coupling repair](./05_coupled_mesh_audit.md)、[06 hydro](./06_merit_hydro_hydro_coast_audit.md)、[07 球面面积](./07_geometry_gis_audit.md)、[08 quality/repair 回看](./08_mesh_quality_metrics_design.md)

*本报告为细化引擎 capstone 设计提案；现状结论基于实际源码与 grep 全量核查。未修改任何 `src/rust` 代码。*
