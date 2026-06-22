# TASK_STATUS — EarthMesh v3 审查进度追踪

> 配套：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md) · [PROJECT_AUDIT_PLAN.md](./PROJECT_AUDIT_PLAN.md) · [REPORT_TEMPLATE.md](./REPORT_TEMPLATE.md)
> 状态取值：`Not started` / `In progress` / `Blocked` / `Done`。
> 日期格式：YYYY-MM-DD（绝对日期）。Owner 默认 `audit`（人或代审 agent）。

## 阶段总表

| Phase | 名称 | Status | Owner | Started | Completed | Output file | Open questions | Next action |
|-------|------|--------|-------|---------|-----------|-------------|----------------|-------------|
| P0 | 工作区初始化 | Done | audit | 2026-06-22 | 2026-06-22 | 本目录 4 文件 | 无 | 进入 P1：架构与 workflow 测绘 |
| P1 | 架构与 workflow 测绘 / 构建体检 | Done | audit | 2026-06-22 | 2026-06-22 | [`01_build_and_crate_audit.md`](./01_build_and_crate_audit.md) | olam 4 红是容差还是内核未归一化？fmt diff 性质？netcdf/workspace 策略？ | 进入 P2：以 #1（olam 半径）根因 + cli 全量测试分片为切入做正确性审查 |
| P1b | Workflow 自洽性审查（5 workflow + 5 Mermaid） | Done | audit | 2026-06-22 | 2026-06-22 | [`02_workflow_consistency_audit.md`](./02_workflow_consistency_audit.md) | v3 是否承认 Python util 层？coupling 占位实现是否本期补守恒？colm_coupling 双实现以谁为准？ | 进入 P2/P3：coupling 守恒缺口 + refine 物理依据为重点 |
| P1c | 配置系统 / Project Schema 审查（提案） | Done | audit | 2026-06-22 | 2026-06-22 | [`03_config_schema_audit.md`](./03_config_schema_audit.md) | 是否引入 `earthmesh_project` 新 crate？是否支持 YAML/JSON project？preset 列表是否认可？ | 进入 P2/P3 正确性+物理审查；schema 提案待 P8 批准（S1→S8） |
| P2 | 正确性 / Bug 审查 | Not started | audit | — | — | `AUDIT_REPORT.md §2`（Bug Table） | Method-C 邻接与闭合性是否有未覆盖边界？P1 已登记 #1(olam 半径)/#4(/tmp 默认) 待根因 | 先查 #1 根因 + #4 默认配置；再从 `earthmesh_mesh` refine/topology 内核入手 |
| P3 | 物理一致性审查（land/ocean/atmos score 设计） | Done | audit | 2026-06-22 | 2026-06-22 | [`04_physical_refinement_audit.md`](./04_physical_refinement_audit.md) | 19 个 preset 权重是否认可？P0 低垂果实(MERIT/CaMa 已读未用)是否优先接入？质量门禁阈值定多少？ | 落地归 P8（先 [03](./03_config_schema_audit.md) S1-S3 框架，再按 P0→P3 增量加 criterion） |
| P4 | 陆海耦合 (LOCmesh) 审查 | Done | audit | 2026-06-22 | 2026-06-22 | [`05_coupled_mesh_audit.md`](./05_coupled_mesh_audit.md) | 是否本期补守恒 fraction(overlay)？CaMa estuary/outlet 是否优先接入？守恒门禁阈值定多少(建议 1e-6)? | 落地归 P8(C1→C9)；MERIT-Hydro 专项(close 闭合/陆海互补)可作 P4b |
| P4b | MERIT-Hydro / Hydro-Coast 深度审查（20 项检查） | Done | audit | 2026-06-22 | 2026-06-22 | [`06_merit_hydro_hydro_coast_audit.md`](./06_merit_hydro_hydro_coast_audit.md) | buffer 是否改 km/等面积投影？跨180°是否本期修？stride 抽样改聚合？eval/ranking 是否 Rust 化？ | 落地归 P8(H1→H10)；H1(dateline)/H3(stride) 为快速修 |
| Pg | Geometry / GIS / mask overlay 审查 | Done | audit | 2026-06-22 | 2026-06-22 | [`07_geometry_gis_audit.md`](./07_geometry_gis_audit.md) | 是否引入球面/等面积后端？是否引入 robust clipping 库(依赖体量)？两套平面几何何时合并？ | 落地归 P8(G1→G9)；G1/G2(flags+守恒)为低风险即时收益 |
| P5 | 网格质量度量体系设计 | Done | audit | 2026-06-22 | 2026-06-22 | [`08_mesh_quality_metrics_design.md`](./08_mesh_quality_metrics_design.md) | 是否建 `earthmesh_quality` 新 crate？物理保真指标(Q5)需哪些参考数据？门禁阈值是否认可？ | 落地归 P8(Q1→Q9)；Q1-Q4(几何/拓扑/数值,无需外部数据)优先 |
| P5b | Score-based / budget / physics-aware refinement 设计 | Done | audit | 2026-06-22 | 2026-06-22 | [`09_score_based_refinement_design.md`](./09_score_based_refinement_design.md) | 是否建 `earthmesh_refine_planner` crate？预算分配默认用 quantile 还是 greedy？repair loop 最大迭代数？ | 落地归 P8(R1→R10)；R3(包装现有布尔判据保回归)是关键先决 |
| P6 | GUI 重设计提案 | Done | audit | 2026-06-22 | 2026-06-22 | [`10_gui_redesign_proposal.md`](./10_gui_redesign_proposal.md) | 是否采用 10 步向导 + Normal/Expert 双模式？12 模板/17 图层是否认可？地图画 polygon 优先级？ | 落地归 P8(G-1→G-10)；G-1/G-2(project+模式,保功能不丢)先做 |
| PF | 终版汇总报告 | Done | audit | 2026-06-22 | 2026-06-22 | [`FINAL_V3_MESH_AUDIT_REPORT.md`](./FINAL_V3_MESH_AUDIT_REPORT.md) | 先合并 OLAM 外部改动定版；P8 落地顺序与批准范围 | 进入 P8：按 Patch Plan A(小patch)→B(架构,需批准) 落地，先决 OLAM 定版 |
| P7 | 重构路线图（已并入 FINAL §8） | Done | audit | 2026-06-22 | 2026-06-22 | [`FINAL_V3_MESH_AUDIT_REPORT.md`](./FINAL_V3_MESH_AUDIT_REPORT.md) §8 | 拆分 22k/36k lib.rs 的安全边界 | 渐进拆分(long-term)，绿测约束 |
| R0 | 建立修复队列与规则 | Done | audit | 2026-06-22 | 2026-06-22 | [`FIX_QUEUE.md`](./FIX_QUEUE.md)·[`FIX_PLAN.md`](./FIX_PLAN.md)·[`RELEASE_TRACKER.md`](./RELEASE_TRACKER.md)·[`fix_reports/R0_fix_queue.md`](./fix_reports/R0_fix_queue.md) | OLAM 外部改动定版；是否建 3 个新 crate；Python util 去留 | 进入 R1（先合并 OLAM，再做 olam 容差/default-tmp+fmt/workspace 三 patch，单线执行） |
| R1 | 修 build/crate/API/CLI-GUI 基础 | Done | dev | 2026-06-22 | 2026-06-22 | [`fix_reports/R1_build_crate_api.md`](./fix_reports/R1_build_crate_api.md) | OLAM WIP 待 commit 才能一次性 fmt/版本统一；`" /tmp"` parity 决策；cli/gui 需 CI 验证(netcdf-c) | 已做:workspace+cli元数据+examples/unwrap审查 |
| R2 | 路径解析/examples 可复现/run_manifest MVP | Done | dev | 2026-06-22 | 2026-06-22 | [`fix_reports/R2_paths_manifest_examples.md`](./fix_reports/R2_paths_manifest_examples.md) | run-manifest workdir 级增强；`${EARTHMESH_DATA}` 是否自动展开；fmt-apply 待 OLAM commit | 已做并**全部验证**:core PathResolver/RunManifest/InputDataCheck(测试绿)+examples 占位化+gui home_dir 接线(gui 34 测试绿)+cli main run_manifest 接线(编译+运行时冒烟绿)。下一步:R3(geometry 球面/Σ=1/flags) |
| R3 | Geometry Safety Layer MVP | Done | dev | 2026-06-22 | 2026-06-22 | [`fix_reports/R3_geometry_safety.md`](./fix_reports/R3_geometry_safety.md) | safety flags 接入 hydro(R4)/coupling(R5) 生产路径；球面面积/robust clipping 待几何重写 | 已做并验证:`geometry::safety`(13 flags + validate_polygon/validate_fraction_partition/degree_buffer_warnings)+overlay_cell 增强;geometry 30 测试绿+cli/gui 下游编译绿。下一步:R4 hydro(km buffer+跨180°+stride) |
| P8 | 补丁实施（= R1–R10，见 FIX_PLAN） | Not started | dev | — | — | `fix_reports/R{n}_*.md` + 源码改动 | 见 FIX_QUEUE 各 item Deps | R1：单线落地 P0+构建卫生 |

## 阶段权限提醒（来自 PROJECT_AUDIT_PLAN §8）

- **只读阶段**：P0–P4（禁止改 `src/rust`，仅写 `docs/`）。
- **可提 patch（不落地）**：P5–P7（及 P2 的 bug 草案）。
- **可直接改代码**：仅 P8，且需用户批准 + 绿测 + surgical。

## 全局 Open Questions（待用户/后续阶段澄清）

1. 审查产出最终是否聚合为单一 `AUDIT_REPORT.md`，还是每 phase 独立文件？（当前计划：聚合）
2. P8 落地补丁应在 `v3.0.0-alpha1` 直接进行，还是新开 `audit/patch-*` 分支？
3. 是否需要将慢测试（`make test-slow`，`--ignored`）纳入每阶段证据基线，还是仅在 P2/P4 跑？
4. GUI 重设计是否有目标用户画像 / 优先模式（land vs ocean vs coupled）的偏好？

## P1 关键发现速览（详见 01_build_and_crate_audit.md）

- **#1 [High,A]** `olam_delaunay_mesh` 4 测试红：半径 ±1e-6 m 容差过紧（相对 ~1.5e-13），平台敏感。
- **#2 [High,A]** 无 workspace 根 Cargo.toml → 5 份 target（≈27G）、依赖重复编译。
- **#3 [Med,A]** `cargo fmt --check` 在 mesh/cli/gui 失败 → `make fmt` 会红。
- **#4 [Med,A]** `EarthmeshConfig::default()` 用 `"/tmp"`，`base_dir`/`mode_file` 带前导空格。
- **#5 [Med,A]** GUI 80 处依赖 `earthmesh_cli::`，workflow/IO 未抽独立库。
- **#6 [Med,B]** `static-netcdf` 源码构建 netcdf-c，Windows 打包高风险、CI 极慢。
- 实跑：core ✅39 · geometry ✅15 · mesh ❌(olam 4 红/其余~230 绿) · cli/gui 编译超时未验证。

## P1b 关键发现速览（详见 02_workflow_consistency_audit.md）

- **W1 [Blocker,A]** coupling 用单点采样非守恒分类 land/ocean；CSV 的 river/coast/fraction/area 全为占位符 `lib.rs:28992-29021`。
- **W2 [High,A]** `overlay_cell`/`intersection_area` 已实现但 coupling 未调用 → 无守恒 fraction `geometry/lib.rs:114-210`。
- **W3 [High,A]** v3 实为 Rust+Python 混合：hydro-coast eval/ranking/HTML/QA/CaMa/守恒在 `util/`，recipe 内嵌 `python3 -m util...` `lib.rs:1070-1072`。
- **W4 [High,A]** `colm_coupling` Rust 与 Python 双实现，易漂移。
- **W5 [High,A]** 无 coastline / river-mouth 显式识别（CaMa `is_estuary` 未接入 `lib.rs:4571`）。
- **W6 [High,A]** 无几何质量门禁；GUI 不展示任何质量指标（违原则5）。
- **W7 [Med,A]** MERIT tile 选择仅 bbox 不支持 polygon `lib.rs:864`；ocean landtype 硬编码 IGBP 0/17。

## 变更日志

| 日期 | 事件 |
|------|------|
| 2026-06-22 | 创建审查工作区，完成四个基准文件（P0 Done）。|
| 2026-06-22 | 完成 P1 构建与 crate 体检，产出 `01_build_and_crate_audit.md`（9 节）；实跑 core/geometry/mesh 测试 + fmt + clippy(core/geo)；cli/gui 编译超时记录在案。|
| 2026-06-22 | 完成 P1b workflow 自洽性审查，产出 `02_workflow_consistency_audit.md`（5 Mermaid + 一致性表 + Top20 风险 + GUI/template/质量报告建议）。核心发现：v3 实为 Rust+Python 混合（hydro-coast eval/ranking/HTML/QA/守恒在 util/）；coupling 为点采样占位骨架（无守恒/无 coast/river-mouth/无质量校验）。|
| 2026-06-22 | 完成 P1c 配置/Project Schema 审查，产出 `03_config_schema_audit.md`（10 节：风险/Schema/Rust types/JSON/YAML/GUI/CLI/校验/兼容/补丁）。核心：当前 64 个扁平底层字段过于 engine-level；提议新 crate `earthmesh_project` 分四层（friendly/expert/plan/repro）+ plugin `RefinementCriterion` trait + 11 种 mesh intent preset，v3 内部零迁移（lower→现有 config）。|
| 2026-06-22 | 完成 P3 物理细化审查，产出 `04_physical_refinement_audit.md`（8 节：能力表/缺口/score 公式/19 preset/数据源/GUI/质量指标/路线图）。核心：当前细化=每变量 mean/std 阈值开关，覆盖 Land 6/24·Ocean 5/21·Atmos 1/17（仅台风）；设计 score_land(14)/score_ocean(12)/score_atmos(10) 加权归一化 + plugin criteria；P0 低垂果实=接入已读未用的 MERIT/CaMa 数据。|
| 2026-06-22 | 完成 P4 陆海耦合审查，产出 `05_coupled_mesh_audit.md`（10 节：现状/缺失概念/8类分类/score_coupled/输出schema/15质量指标/GUI/各格式建议/测试/补丁）。核心：v3 是单网格+逐格 land/ocean 二分类，非 fractional 耦合；coastline/river_mouth/outlet/orphan/sea_fraction/mass_conserv 全 0 命中；16 项能力仅 2✅3🟡11❌；设计 overlay 守恒分类 + 守恒门禁(残差≤1e-6 Block)。|
| 2026-06-22 | 完成 P4b MERIT-Hydro 深度审查，产出 `06_merit_hydro_hydro_coast_audit.md`（11 节：workflow图/4风险表/score_hydro_coast(14项)/更优mask法/eval指标/GUI13步/测试/补丁）。20项检查 ✅9🟡6❌5：最严重=跨180°不支持(:4401)、几何全 degree(高纬失真/窄河道破坏)、stride 抽样漏河、composite 无几何去重、无 river-mouth/estuary 类、Rust 无 eval/ranking(在 Python util)。|
| 2026-06-22 | 完成 Pg geometry/GIS 审查，产出 `07_geometry_gis_audit.md`（9 节：清单/平面vs球面/dateline-pole/overlay风险/后端建议/preview-production分离/GeometryQualityFlags(11)/测试/补丁）。核心：球面网格+平面GIS割裂——geometry crate 全 lon/lat 平面(polygon_area/clip/intersection/overlay)，仅 haversine 球面；overlay 无 Σfraction=1 校验、三角化失败静默回退、quality_flags 仅2种；preview 已与生产分离(好)。|
| 2026-06-22 | 完成 P5 质量度量体系设计，产出 `08_mesh_quality_metrics_design.md`（11 节：crate/MeshQualityReport/metric表(A几何B拓扑C数值D物理)/门禁/NetCDF/CSV/GeoJSON worst cells/MD-HTML/GUI dashboard/测试/补丁）。核心：现状仅算角度+边长(~3-4/80指标)，连 cell 面积统计都无；设计 `earthmesh_quality` 新 crate + plugin metric + Pass/Warn/Fail 门禁 + before/after 收益。|
| 2026-06-22 | 完成 P5b score-based refinement 设计，产出 `09_score_based_refinement_design.md`（11 节：为何 threshold 不够/Criterion API/composite score/预算/质量约束/target level 算法/repair loop/GUI/CLI/测试/路线图）。核心：现状=纯布尔阈值 judge(budget/quantile/pareto/target_level 全0命中)；设计统一 planner(criterion→score→预算→质量→repair)置于现有引擎之上,10种细化方法统一,布尔阈值退化为一个 criterion,零迁移。|
| 2026-06-22 | 完成 P6 GUI 重设计提案，产出 `10_gui_redesign_proposal.md`（13 节：现状风险/信息架构/10步向导/8 tab/字段/Normal-Expert/12模板/17图层管理/hydro GUI流/质量仪表盘/i18n键/路线图/测试）。核心：GUI 从"参数表单"(暴露64字段无质量反馈)升级为"工作流平台"(向导+模板+数据图层+score热力图+质量仪表盘),承载 ProjectConfig,引擎复用 start_run,旧能力进 Expert 模式不丢,零迁移。|
| 2026-06-22 | 完成终版汇总，产出 `FINAL_V3_MESH_AUDIT_REPORT.md`（9 节：Executive Summary+Top10/Bug表/Workflow一致性/物理一致性/MERIT-Hydro/质量度量/GUI重设计/重构路线图/Patch Plan）。综合 01-10 全部审查。三大系统短板：球面网格+平面GIS割裂、布尔阈值非物理感知细化、二分类占位非守恒耦合。审查阶段(P0-PF)完成，转入 P8 落地。|
