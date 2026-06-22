# RELEASE_TRACKER — EarthMesh v3 版本计划

> 配套：[FIX_QUEUE.md](./FIX_QUEUE.md) · [FIX_PLAN.md](./FIX_PLAN.md)。当前分支：`v3.0.0-alpha1`。
> 原则：每个版本只承诺能验收的 R 阶段；release blocker 未清不发版。

---

## v3.0.0-alpha2 — "正确性 + 可诊断"
- **必须完成的 R 阶段**：R0（队列）+ R1（P0 修复 + 构建卫生）+ R2（验证器/质量骨架）。
- **关键交付**：`make test`/`make fmt` 全绿；coupling 占位列诚实化（fill 而非 0）；workspace + 固定工具链；`earthmesh_quality` 几何/拓扑/数值 metrics + 门禁；overlay Σ=1 校验。
- **Release blockers**：
  - [~] OLAM 工作树外部改动合并并定版（前置 EM3-P0-001）— **代码已修复(mesh 全绿)，待 commit**。
  - [~] `cargo test` 全 crate 绿（含 olam_delaunay）— core/geometry/mesh **绿**；cli/gui 待 CI 验证（static-netcdf 编译时长）。
  - [ ] coupling NetCDF/CSV 不再以 0 冒充未计算值（R1/R5）。
  - [ ] `make fmt` 绿（待 WIP commit 后一次性 fmt）+ `cargo build --workspace` 绿（待加 workspace）。
- **Open risks**：cli/gui 全量测试在 CI 时间内能否跑完（static-netcdf 编译，[01](./01_build_and_crate_audit.md)）；olam 根因是容差还是内核归一化未定。

## v3.0.0-alpha3 — "几何 + hydro-coast 正确"
- **必须完成的 R 阶段**：R3（几何球面/投影/dateline/pole）+ R4（MERIT-Hydro/hydro-coast 正确性）。
- **关键交付**：球面/等面积面积；跨180° tile；stride 聚合；composite 几何去重；km buffer/投影。
- **Release blockers**：
  - [ ] `fraction_ratio_stable_across_latitude` 通过。
  - [ ] 跨180°/stride/composite/buffer 测试通过。
  - [ ] buffer 参数 degree→km 的迁移说明 + 兼容路径。
- **Open risks**：是否需要第三方几何库（robust clipping 推迟到 R10）；buffer 单位变更对既有算例的影响。

## v3.0.0-alpha4 — "守恒耦合 + 项目配置"
- **必须完成的 R 阶段**：R5（陆海耦合守恒）+ R6（ProjectConfig + presets + manifest）。
- **关键交付**：overlay 守恒 fraction + cell_area；CaMa estuary/river-mouth；orphan/outlet；mass conservation 门禁；`earthmesh_project` 四层 + lower（零迁移）+ save/load + 12 模板。
- **Release blockers**：
  - [ ] mass conservation 残差≤1e-6 门禁通过。
  - [ ] `lower_matches_legacy_namelist` 逐字节回归通过。
  - [ ] 旧 .nml 可 import 为 project。
- **Open risks**：物理保真参考数据可得性；coupling 行为变更对下游 CoLM 适配的影响。

## v3.0.0-beta1 — "评分细化 + 质量看板 + GUI 平台"
- **必须完成的 R 阶段**：R7（score 细化 planner）+ R8（质量 dashboard/score preview）+ R9（GUI 工作流平台）。
- **关键交付**：RefinementCriterion planner（布尔退化为 criterion，回归）+ composite score + budget + repair loop；GUI 10 步向导 + 模板 + 数据图层管理 + 质量仪表盘 + score 热力图 + project save/load + hydro GUI flow。
- **Release blockers**：
  - [ ] `threshold_criterion_matches_legacy_bool` 回归通过。
  - [ ] repair loop 收敛（min angle/守恒）。
  - [ ] GUI 工作流/仪表盘/i18n 测试通过；Expert 模式保留旧能力。
- **Open risks**：planner 性能（大网格预算优化）；GUI 重排回归风险。

## v3.x（post-beta，R10）
- 拆分超大 lib.rs；robust geometry backend；plugin + 多目标；物理保真全量；Rust 化 hydro eval/ranking/HTML（收敛 Rust/Python 双实现）；GUI 全量。非 beta1 blocker。

---

## 全局 release blockers（任何版本前）
- [ ] **改代码单线执行**：确认无多 agent 并行改同一批文件（工程原则 7/8）。
- [ ] 每个落地 patch 有 `fix_reports/R{n}_*.md`（改了什么/为什么/如何验证/行为变化）。
- [ ] refinement 相关变更对照修复总原则五条。

## 全局 open risks
- 工作树长期存在未提交 OLAM 改动 → 审查/修复基线漂移（应尽快定版）。
- v3 Rust+Python 混合：Python `util/` 是否长期保留未定，影响 R10 范围。
- static-netcdf 跨平台（尤其 Windows）打包风险（[01](./01_build_and_crate_audit.md) #6）。
