# FIX_PLAN — EarthMesh v3 修复阶段计划（R0–R10）

> 配套：[FIX_QUEUE.md](./FIX_QUEUE.md) · [RELEASE_TRACKER.md](./RELEASE_TRACKER.md) · 审查 [FINAL](./FINAL_V3_MESH_AUDIT_REPORT.md)。
> 工程原则：先修 compile/run/result/data；再补验证器与质量报告，不先大重构；新功能必有 example/测试/validation；GUI 不绑底层；hydro-coast/coupling 是核心；每 patch 说明改了什么/为什么/如何验证/是否改变用户行为；**改代码单线执行，多 agent 只审查/设计**。
> 阶段递进：R1-R2 修正确性+补验证器（alpha2）→ R3-R5 几何/hydro/coupling 正确性（alpha3-4）→ R6-R9 配置/score/质量/GUI（beta1）→ R10 长期架构（v3.x）。

---

## R0 — 建立修复队列与规则（本阶段，不改代码）
- **目标**：把审查报告变成可执行修复队列与规则。
- **允许修改**：仅 `docs/reviews/v3_mesh_audit/`（FIX_QUEUE/FIX_PLAN/RELEASE_TRACKER/fix_reports/）。
- **禁止**：改任何 `src/rust`、`Cargo.toml`、Makefile。
- **验收**：4 文件齐备；FIX_QUEUE 含 P0-P3 全字段 item；FIX_PLAN 含 R0-R10；RELEASE_TRACKER 含 alpha2-beta1。

## R1 — P0 修复 + 构建卫生（alpha2 核心）
- **目标**：消除编译/运行/结果/数据问题；让 `make test`/`make fmt` 绿、构建自洽。
- **Items**：EM3-P0-001(olam,先合并 OLAM 外部改动), EM3-P0-002(coupling 占位列诚实化), EM3-P1-007(default /tmp), EM3-P1-012(FVCOM 告警), EM3-P2-001(workspace), EM3-P2-002(fmt), EM3-P2-010(版本/元数据), EM3-P2-011(feature 一致)。
- **允许修改**：`mesh/tests/olam_*`(经定版后), `core/src/lib.rs`(default), `cli/src/lib.rs`(coupling 占位→fill), `gui/src/main.rs`(FVCOM 告警), 根 `Cargo.toml`, `rust-toolchain.toml`, 各 `Cargo.toml`(版本), Makefile/CI, README。
- **禁止**：算法重构；改 refine/geometry 内核逻辑；新增 crate。
- **验收**：`make test` 全绿（含 olam）；`make fmt` 绿；`cargo build --workspace` 绿；coupling CSV 占位列为显式 fill 而非 0；FVCOM regional 跳过有告警。

## R2 — 验证器与质量报告骨架（alpha2/alpha3）
- **目标**：先有"诊断能力"，再谈修几何（工程原则 2）。
- **Items**：EM3-P1-004(Σ=1 校验), EM3-P1-003(intersection 失败 flag), EM3-P1-009(earthmesh_quality 几何/拓扑/数值 metrics + 门禁)。
- **允许修改**：新 crate `earthmesh_quality`；`earthmesh_geometry`(加 flag/校验，不改既有结果)；NetCDF/CSV writer。
- **禁止**：改几何面积算法本身（留 R3）；改 refine 决策。
- **验收**：`GeometryQualityFlag`(11) 可用；overlay Σ≠1 报 flag；quality crate 输出几何/拓扑/数值 metrics + Pass/Warn/Fail + 至少 1 个 validation 命令 `earthmesh quality <gridfile>`。

## R3 — 几何正确性（球面/投影/dateline/pole）（alpha3）
- **目标**：修平面 GIS 失真，面积守恒可信。
- **Items**：EM3-P1-001(球面/等面积面积), EM3-P1-003(robust 收尾), dateline/pole 处理。
- **允许修改**：`earthmesh_geometry`；调用方按需切换面积函数。
- **禁止**：引入重型第三方几何库（先自研局地投影+球面面积；robust clipping 评估留 R10）。
- **验收**：`fraction_ratio_stable_across_latitude` 通过；球面面积 vs 解析值误差<阈；dateline/pole 单元有 flag。

## R4 — MERIT-Hydro / hydro-coast 正确性（alpha3）
- **目标**：核心场景 mask 正确。
- **Items**：EM3-P1-002(跨180° tile), EM3-P1-005(stride 聚合), EM3-P1-006(composite 几何去重), EM3-P1-008(km buffer/投影)。
- **允许修改**：`cli/src/lib.rs` 的 merit/hydro_close/composite/buffer/simplify 函数。
- **禁止**：改 mesh 内核 refine；改 coupling（留 R5）。
- **验收**：跨180°/stride/composite/buffer 各自测试通过；buffer 参数 degree→km（文档+迁移说明）。

## R5 — 陆海耦合守恒（alpha4）
- **目标**：从二分类占位升级为守恒耦合。
- **Items**：EM3-P0-002(守恒补全), EM3-P1-010(8 类 + CaMa estuary/river-mouth + orphan/outlet)。
- **允许修改**：`cli` coupling 函数；接 `geometry::overlay_cell`；CaMa 接入。
- **禁止**：改 GUI（留 R8）；改 refine planner（留 R7）。
- **验收**：`coupled_overlay_fraction_sums_to_one`、`coupled_estuary_from_cama`、`coupled_orphan_cell_detected`、mass conservation 门禁(残差≤1e-6)通过；CSV/NetCDF 填实 fraction/area。

## R6 — 配置 ProjectConfig + presets + manifest（alpha4）
- **目标**：高层配置层，零迁移。
- **Items**：EM3-P2-003(ProjectConfig 四层), EM3-P2-005(save/load), presets。
- **允许修改**：新 crate `earthmesh_project`；cli 加 `lower()`/`import`；不动 core 64 字段。
- **禁止**：改 GUI（留 R8/R9）；改引擎内核。
- **验收**：`project_config_roundtrip`、`lower_matches_legacy_namelist`(逐字节回归)；12 模板可加载；旧 .nml 可 import。

## R7 — score-based refinement planner（beta1）
- **目标**：布尔阈值 → 物理感知评分 + 预算 + repair。
- **Items**：EM3-P1-011(criterion/composite/budget/repair)。
- **允许修改**：新 crate `earthmesh_refine_planner`；planner 输出现有引擎消费的细化度（不改内核）。
- **禁止**：改 mesh 内核 refine 算法；删除旧 namelist 路径。
- **验收**：`threshold_criterion_matches_legacy_bool`(回归)、`budget_quantile_respects_max_cells`、`repair_loop_converges_min_angle` 通过；提供 `earthmesh refine plan/explain` 命令。

## R8 — 质量 dashboard + score preview + project save/load（beta1）
- **目标**：GUI 能看"为何细化/是否更好"。
- **Items**：EM3-P1-013, EM3-P2-006/007, EM3-P2-005(GUI 侧)。
- **允许修改**：`gui`。
- **禁止**：改引擎/几何/coupling 逻辑。
- **验收**：质量仪表盘、score 热力图、project save/load 的 GUI 测试通过。

## R9 — GUI 工作流平台（beta1）
- **目标**：参数表单 → 工作流平台。
- **Items**：EM3-P2-004(向导/模板/双模式), EM3-P2-008(hydro GUI), EM3-P2-009(画 polygon)。
- **允许修改**：`gui`,`i18n.rs`。
- **禁止**：删除 Expert 模式中的旧能力。
- **验收**：10 步向导、12 模板、17 图层管理、hydro flow、画 polygon、i18n 全 en/zh 测试通过。

## R10 — 长期架构（v3.x，post-beta）
- **目标**：系统性重构。
- **Items**：EM3-P3-001..006（拆 lib.rs / robust geometry / plugin+多目标 / 物理保真 / Rust 化 Python / GUI 全量）。
- **允许修改**：广泛，但**绿测约束、小步、可回滚**。
- **禁止**：一次性大爆改；在无回归测试覆盖处重写。
- **验收**：每步全量回归绿；行为不变（除明确新功能）。

---

## 跨阶段规则
- **单线改代码**：任一时刻仅一个执行者修改某批文件（工程原则 7/8）。
- **每 patch 报告**：写入 `fix_reports/R{n}_*.md`，含 改了什么/为什么/如何验证/是否改变用户行为。
- **回归优先**：R3-R7 改既有逻辑前，先确保该路径有测试（必要时 R2 补）。
- **refinement 相关 patch** 必须对照修复总原则五条。
