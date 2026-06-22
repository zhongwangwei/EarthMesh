# FIX_QUEUE — EarthMesh v3 修复队列

> 来源：综合 [FINAL_V3_MESH_AUDIT_REPORT.md](./FINAL_V3_MESH_AUDIT_REPORT.md) 与 01–10 审查报告。本文件不改代码。
> 阶段计划见 [FIX_PLAN.md](./FIX_PLAN.md)；版本计划见 [RELEASE_TRACKER.md](./RELEASE_TRACKER.md)。
> 优先级：P0 编译/运行/结果错误/数据损坏 · P1 几何/拓扑/质量/hydro/coupling/物理/误导 · P2 配置/GUI/可用性/文档 · P3 长期架构。
> 每 item 字段：ID/Title/Priority/Files/Problem/Why/Fix/Tests/Risk/Size/Deps/Phase/Milestone。Size：S(<50行)/M(50-300)/L(>300 或跨文件)。

## 修复总原则（细化黄金法则，所有 refinement-相关 patch 必须满足）
任何 refinement 同时满足：(1) 重要空间异质性；(2) 影响目标模式关键物理过程；(3) 物理收益>计算成本；(4) 细化后仍满足几何/拓扑/数值/耦合质量；(5) GUI 可解释为何细化+是否变好。

## 工程原则
1. 先修 compile/run/result/data-corruption；2. 再补验证器与质量报告，不先大重构；3. 新功能必须有可运行 example/测试/validation 命令；4. GUI 不绑定底层（普通用户看 workflow，高级看 expert config）；5. MERIT-Hydro/hydro-coast/coupling 是核心场景；6. 每 patch 说明：修了什么/为什么/如何验证/是否改变用户行为；7. 不允许多 agent 改同一批文件；8. 并行 agent 只审查/设计/报告，改代码单线执行。

---

## 状态更新（R2 后，2026-06-22）
- **EM3-P0-001（olam 半径）→ 根因已定位并由 OLAM WIP 修复**：根因**不是**测试容差过紧，而是**内核投影/外心用了固定半径**。WIP 引入 radius-aware 内核（`active_voronoi_grid_radius`、`project_to_polar_stereographic_with_radius`、`spherical_circumcenter_from_barycenter_with_radius`、多区域 `scale_olam_refinement_regions_radius`），点精确落在 `EARTH_RADIUS_METERS`。`olam_delaunay_mesh` 18/0，mesh 全量 43 套件绿。**待 commit**。
- **cli/gui 可验证性澄清**：此前"预算内无法验证"= 单条命令超时假象。**后台构建 cli(static-netcdf) exit code 0 已证 cli 可编译/测试**；首次编 netcdf-c 慢、之后缓存命中快。验证方式：后台 `cargo build/test -p earthmesh_cli --features static-netcdf`。
- **EM3-P2-001（workspace）→ R1 完成**：根 `Cargo.toml`+`Cargo.lock`，metadata/build 验证通过。待清理 5 个 per-crate lock。
- **EM3-P2-010（cli 元数据）→ R1 部分**：cli 补 `description`/`license`；版本统一待 commit 后。
- **R2 路径/manifest/examples → 完成**：core `PathResolver`/`RunManifest`/`InputDataCheck`（测试全绿）；examples 占位化；**接线已落地**：gui `runtime_workdir` 用 `home_dir()`（修 Windows）、cli `main()` 每次 run 写 `run_manifest.json`（编译验证见 R2 报告）。
- **R3 geometry safety → 完成**：`geometry::safety` 13 个 `GeometryQualityFlag` + `validate_polygon`/`validate_fraction_partition`/`degree_buffer_warnings` + `overlay_cell` 增强（非有限/负面积/并列冲突）。**EM3-P1-004(Σ=1 校验)** = `validate_fraction_partition`（互斥分区，供 R5）；**EM3-P1-003(自交检测)** = `validate_polygon` 自交 flag（`intersection_area` 数学未改，仅检出）。geometry 30 测试绿 + cli/gui 下游编译绿。
- 详见 [R1](./fix_reports/R1_build_crate_api.md) / [R2](./fix_reports/R2_paths_manifest_examples.md) / [R3](./fix_reports/R3_geometry_safety.md)。

## P0 — 编译/运行/结果错误/数据损坏（先修）

### EM3-P0-001 — olam_delaunay 半径容差过紧致 mesh 测试红
- Priority: P0 | Phase: R1 | Milestone: v3.0.0-alpha2 | Size: S | Risk: Low | Deps: 先合并工作树 OLAM 外部改动定版
- Files: `rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs:31,246,271,336`（可能含 `mesh/src/lib.rs` renormalize）
- Problem: 断言"每点半径=EARTH_RADIUS_METERS ±1e-6 m"（相对~1.5e-13）过紧，arm64/Homebrew rustc 上 4 测试红。
- Why: `cargo test mesh`/`make test` 失败 → CI、quickstart 信心受损；几何基线不稳，后续 patch 无法回归。
- Fix: 相对容差 `<=1e-6·R`（~6.4mm）或对 spring/expand 输出强制 renormalize 到地球半径（先定根因）。
- Tests: `cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --test olam_delaunay_mesh`
- 用户行为变化: 无（仅测试稳定）。

### EM3-P0-002 — coupling CSV 占位列被下游误读为真值（静默错误数据）
- Priority: P0 | Phase: R1(诚实化) → R5(守恒补全) | Milestone: alpha2(诚实化)/alpha4(守恒) | Size: M | Risk: Med | Deps: —
- Files: `rust/earthmesh_cli/src/lib.rs:29001-29021`（CSV 写出）, `:28992-29009`（点采样分类）, `:29226/29311`（restart/forcing 模板）
- Problem: cell 用单点采样二分类 land/ocean；CSV 的 `has_river/river_class/river_fraction/estimated_river_area/has_coast/coastal_fraction/normalized_cell_area_m2` 全写占位 `false/none/0.0`；forcing 模板 area=占位 0 ⇒ 无意义。
- Why: 下游模型把"无河流/无海岸/面积0"当真值 → 静默错误耦合数据（数据损坏类）。违原则 4。
- Fix(R1 诚实化): 占位列改为显式 `_FillValue`/`missing` 标记 + manifest 注明"未计算"，杜绝误读；(R5) 接 `geometry::overlay_cell` 求守恒 fraction、填实 `cell_area_m2`。
- Tests: `colm_coupling_csv_from_mesh`（断言占位列为 fill 而非 0）；R5 加 `coupled_overlay_fraction_sums_to_one`。
- 用户行为变化: 是（CSV/NetCDF 列语义从"0"变为"missing"；需告知下游）。

---

## P1 — 几何/拓扑/质量/hydro/coupling/物理/GUI 误导

### EM3-P1-001 — GIS/overlay 全平面 lon/lat（无球面/投影/守恒面积）
- Priority: P1 | Phase: R3 | Milestone: alpha3 | Size: L | Risk: High | Deps: EM3-P1-004
- Files: `rust/earthmesh_geometry/src/lib.rs:74`(polygon_area),`:86`(clip),`:114`(intersection),`:155`(overlay)
- Problem: 面积/裁剪/相交/overlay 全在 (lon,lat) 欧氏空间，无 cos(lat)/投影/球面面积。
- Why: 高纬/大单元/跨经面积失真；绝对 cell_area 无物理意义；守恒耦合不可信。违原则 4。
- Fix: 加球面多边形面积 + 局地等面积投影（LAEA）；fraction 在投影下计算；保留接口。
- Tests: `polygon_area_spherical_matches_reference`,`fraction_ratio_stable_across_latitude`
- 用户行为变化: 否（结果更准，接口不变）。

### EM3-P1-002 — MERIT tile 选择不支持跨 180°（静默选错 tile）
- Priority: P1（对太平洋域为 P0） | Phase: R4 | Milestone: alpha3 | Size: S | Risk: Med | Deps: —
- Files: `rust/earthmesh_cli/src/lib.rs:4401`(merit_bbox_intersects), `:864`(select_merit_hydro_tiles)
- Problem: 经度线性比较 `west<east && east>west`，跨 ±180° bbox 选错/漏 tile。
- Why: 太平洋/白令海/东西伯利亚静默生成错误 mask。违原则 2/4。
- Fix: 复用已有 `shift_longitudes_for_dateline_crossing`(`geometry:598`) 或拆两段 bbox。
- Tests: `merit_bbox_crosses_antimeridian_selects_both_sides`
- 用户行为变化: 否（修复正确性）。

### EM3-P1-003 — intersection_area 三角化失败静默回退凸裁剪
- Priority: P1 | Phase: R2(flag)/R3(robust) | Milestone: alpha2(flag)/alpha3 | Size: M | Risk: Med | Deps: —
- Files: `rust/earthmesh_geometry/src/lib.rs:115-120`
- Problem: triangulate→None（自交/带洞/multipolygon）时回退 `clip_convex_polygon(a,b)`，结果错且无 flag。
- Why: 复杂海岸线/河网 mask 面积静默错。网格质量不可诊断。违原则 4。
- Fix(R2): 失败时返回/标记 `InvalidPolygon`/`SelfIntersection` flag，不静默；(R3) robust clipping。
- Tests: `intersection_self_intersecting_flagged_not_silent`
- 用户行为变化: 否（增加诊断）。

### EM3-P1-004 — overlay_cell 无 Σ fraction=1 守恒校验
- Priority: P1 | Phase: R2 | Milestone: alpha2 | Size: S | Risk: Low | Deps: —
- Files: `rust/earthmesh_geometry/src/lib.rs:198-200`
- Problem: 各 fraction 各自 clamp≤1，但和不校验，无 `UnresolvedFractionSumError`。
- Why: 守恒失败不可见（coupling Blocker 根源）。违原则 4。
- Fix: 加 Σ 校验 + `GeometryQualityFlag::UnresolvedFractionSumError`（[07](./07_geometry_gis_audit.md)）。
- Tests: `overlay_fraction_sum_flags_when_not_one`
- 用户行为变化: 否（增加诊断）。

### EM3-P1-005 — stride 抽样跳过窄河道
- Priority: P1 | Phase: R4 | Milestone: alpha3 | Size: M | Risk: Med | Deps: —
- Files: `rust/earthmesh_cli/src/lib.rs:746`(indices_between_inclusive)
- Problem: `.step_by(stride)` 抽样而非聚合，宽度<stride 的支流/窄道丢失。
- Why: 河网连通断裂，hydro mask 错。违原则 2。
- Fix: 抽样改窗口聚合（max upa / 河道命中即保留）。
- Tests: `merit_stride_preserves_narrow_river`
- 用户行为变化: 否（更完整河网）。

### EM3-P1-006 — composite mask 跨组件无几何去重
- Priority: P1 | Phase: R4 | Milestone: alpha3 | Size: M | Risk: Med | Deps: —
- Files: `rust/earthmesh_cli/src/lib.rs:1453-1489`(apply_composite_refine_degree_cap)
- Problem: 去重键仅 `refine_degree`，不含几何 → 多组件同区域同级重复细化。
- Why: 同区域被重复细化，违配置意图与原则 3（成本）。
- Fix: 同 degree 重叠多边形先 union 再 cap。
- Tests: `composite_dedup_geometric_overlap`
- 用户行为变化: 否（去除冗余细化）。

### EM3-P1-007 — EarthmeshConfig::default 的 " /tmp" 前导空格 + POSIX-only
- Priority: P1 | Phase: R1 | Milestone: alpha2 | Size: S | Risk: Low | Deps: —
- Files: `rust/earthmesh_core/src/lib.rs:766-790`
- Problem: `base_dir:" /tmp"`/`mode_file:" /tmp"` 带前导空格（typo）；`/tmp` 仅 POSIX。
- Why: 用 Default 时生成错误目录名 / Windows 失效（结果/路径错误）。
- Fix: 去前导空格；默认改 `std::env::temp_dir()` 或空串占位。
- Tests: `cargo test core --all-targets`
- 用户行为变化: 否（仅默认值，真实 run 由 namelist 覆盖）。

### EM3-P1-008 — hydro buffer/simplify 全 degree（高纬失真/窄河道破坏）
- Priority: P1 | Phase: R4 | Milestone: alpha3 | Size: L | Risk: High | Deps: EM3-P1-001
- Files: `rust/earthmesh_cli/src/lib.rs:2911`(buffer),`:2734`(simplify_closed_ring)
- Problem: buffer offset 与 Douglas-Peucker tolerance 均 degree，高纬实际尺度不一致，窄河弯被抹平。
- Why: "距河 X km 细化"在 degree 下各向异性且随纬度变。违原则 4。
- Fix: 改 km，在局地方位等距/等面积投影下 offset/简化后投回；河道宽度下限保护点。
- Tests: `buffer_km_consistent_across_latitude`,`simplify_preserves_narrow_channel`
- 用户行为变化: 是（buffer 参数语义 degree→km）。

### EM3-P1-009 — 网格质量仅角度+边长，无门禁/不可诊断
- Priority: P1 | Phase: R2 | Milestone: alpha2/alpha3 | Size: L | Risk: Med | Deps: —
- Files: `rust/earthmesh_cli/src/lib.rs:19253`(write_grid_quality), `QualityClassMetrics`
- Problem: 仅算每类边长+角度(min/max/avg/std+less/more)，连 cell 面积统计都无；无 aspect/skew/拓扑/数值/物理/耦合；无 Pass/Warn/Fail 门禁。
- Why: 网格质量不可诊断、无法阻断坏网格。违原则 4/5。
- Fix: 新 `earthmesh_quality` crate（几何/拓扑/数值先行），门禁聚合 verdict（[08](./08_mesh_quality_metrics_design.md) Q1-Q6）。
- Tests: `quality_geometry_stats`,`quality_min_angle_gate_pass_warn_fail`,`quality_topology_orphan_detected`
- 用户行为变化: 否（新增输出）。

### EM3-P1-010 — 无 river-mouth/estuary/coastline/orphan 概念（CaMa is_estuary 已读未用）
- Priority: P1 | Phase: R5 | Milestone: alpha4 | Size: L | Risk: Med | Deps: EM3-P1-001/004
- Files: `rust/earthmesh_cli/src/lib.rs:4571`(is_estuary 未接), coupling `:28965+`; grep coastline/river_mouth/outlet/orphan=0
- Problem: 缺陆海过渡关键单元类型；CaMa `is_estuary` 存在却不进分类/score。
- Why: 河口/海岸/孤立单元不可辨识，耦合不正确。违原则 2/4。
- Fix: `CoupledCellClass` 8 类 + 接 CaMa estuary/river-mouth + orphan/outlet 检测（[05](./05_coupled_mesh_audit.md) C3/C4）。
- Tests: `coupled_estuary_from_cama`,`coupled_orphan_cell_detected`,`coupled_river_outlet_matches_ocean`
- 用户行为变化: 是（新增分类列/概念）。

### EM3-P1-011 — 细化为纯布尔阈值，无 score/预算/质量反馈（违物理/数值约束）
- Priority: P1 | Phase: R7 | Milestone: beta1 | Size: L | Risk: High | Deps: EM3-P2-003(project crate), EM3-P1-009(quality)
- Files: `rust/earthmesh_mesh/src/lib.rs:19334-20031`(refine_iter judge), `cli:4413-4814`(area_judge `>=`)
- Problem: 超阈值即细化，无 score/预算/收益-成本/质量回看（budget/quantile/target_level 全 0 命中）。
- Why: 可无限细化、不保证收益>成本、细化后不回看质量。违原则 1/2/3。
- Fix: `RefinementCriterion` planner（布尔阈值退化为一个 criterion，保回归）+ composite score + budget + repair loop（[09](./09_score_based_refinement_design.md) R1-R7）。
- Tests: `threshold_criterion_matches_legacy_bool`,`budget_quantile_respects_max_cells`,`repair_loop_converges_min_angle`
- 用户行为变化: 是（新增 score/预算模式；旧 namelist 路径保留）。

### EM3-P1-012 — FVCOM regional 开边界静默跳过，无告警
- Priority: P1 | Phase: R1 | Milestone: alpha2 | Size: S | Risk: Low | Deps: —
- Files: `rust/earthmesh_gui/src/main.rs:1303-1305`
- Problem: regional+open boundary 时 FVCOM .2dm 静默跳过，用户只得 regional gridfile。
- Why: 用户以为成功实则缺输出（GUI 误导）。违原则 5。
- Fix: 显式告警/状态提示 + 日志说明原因。
- Tests: gui 集成测试（断言告警出现）。
- 用户行为变化: 是（新增可见告警）。

### EM3-P1-013 — GUI 无质量反馈（仅 cell/vertex 计数）
- Priority: P1 | Phase: R8 | Milestone: beta1 | Size: M | Risk: Low | Deps: EM3-P1-009
- Files: `rust/earthmesh_gui/src/main.rs:2969-2983`
- Problem: 结果仅显示 cell/vertex 计数，无任何几何/拓扑/质量指标、无 before/after。
- Why: 用户盲跑，不知网格好坏。违原则 5。
- Fix: 质量仪表盘（verdict 徽章/四类卡片/worst cells/before-after，[08](./08_mesh_quality_metrics_design.md)§9）。
- Tests: `gui_quality_dashboard_verdict_colors`
- 用户行为变化: 是（新增仪表盘）。

---

## P2 — 配置/GUI/可用性/文档

### EM3-P2-001 — 无 workspace 根 Cargo.toml（5 份 target≈27G）
- Priority: P2 | Phase: R1 | Milestone: alpha2 | Size: S | Risk: Low | Deps: —
- Files: 仓库根（新增 `Cargo.toml [workspace]`）, 各 crate Cargo.toml, Makefile
- Problem: 无 workspace，5 份 target、依赖重复编译、无统一 cargo 命令。
- Why: 磁盘/CI 成本高、可维护性差。
- Fix: 加根 `[workspace] members`，共享 target；调整 Makefile 假设。
- Tests: `cargo build --workspace`,`make`
- 用户行为变化: 否（target 路径变，需更新文档）。

### EM3-P2-002 — cargo fmt --check 失败 / 无固定工具链
- Priority: P2 | Phase: R1 | Milestone: alpha2 | Size: S | Risk: Low | Deps: —
- Files: `rust-toolchain.toml`(新增), mesh/cli/gui 源（一次性 fmt）, CI
- Problem: `make fmt` 在 mesh/cli/gui 失败（1560/516/22 行 diff）。
- Why: CI fmt 门禁红、风格漂移。
- Fix: 固定 rustfmt 版本 + 一次性 `cargo fmt`（独立纯格式提交）+ CI 门禁。
- Tests: `make fmt`
- 用户行为变化: 否。

### EM3-P2-003 — 配置 64 扁平 engine 字段，无 ProjectConfig 项目层
- Priority: P2 | Phase: R6 | Milestone: alpha4 | Size: L | Risk: Med | Deps: —
- Files: 新 crate `earthmesh_project`（不动 `core` 64 字段）
- Problem: `EarthmeshConfig`(25)+`RefineConfig`(39) 扁平 Fortran 字段，无项目/意图层。
- Why: 用户被迫懂 NXP/HALO/spring；无单一 project 入口。违原则 5。
- Fix: `ProjectConfig` 四层(friendly/expert/plan/repro) + `lower()`→现有 config（[03](./03_config_schema_audit.md) S1-S3，零迁移）。
- Tests: `project_config_roundtrip`,`lower_matches_legacy_namelist`
- 用户行为变化: 是（新增 project.yaml；旧 .nml 保留）。

### EM3-P2-004 — GUI 参数表单，无向导/模板/数据图层管理
- Priority: P2 | Phase: R9 | Milestone: beta1 | Size: L | Risk: Med | Deps: EM3-P2-003
- Files: `rust/earthmesh_gui/src/main.rs`, `i18n.rs`
- Problem: 3 tab 暴露 ~64 engine 字段，无向导/12 模板/17 图层管理。
- Why: 专家门槛高、新手难用。违原则 5。
- Fix: 10 步向导 + Normal/Expert 双模式 + 模板 + 数据图层管理器（[10](./10_gui_redesign_proposal.md) G-1~G-6）。
- Tests: `gui_wizard_step_navigation`,`gui_target_template_fills_defaults`
- 用户行为变化: 是（GUI 重排；Expert 保留旧能力）。

### EM3-P2-005 — 无 project manifest save/load
- Priority: P2 | Phase: R6 | Milestone: alpha4 | Size: M | Risk: Low | Deps: EM3-P2-003
- Files: `earthmesh_project` + gui 顶栏
- Problem: save 只存 namelist，无单一 project save/load。
- Why: 算例不可整体复用/复现。
- Fix: ProjectConfig YAML/JSON 存取 + 顶栏 Project 菜单 + 旧 .nml 导入。
- Tests: `gui_project_save_load_roundtrip`
- 用户行为变化: 是（新增 project 文件）。

### EM3-P2-006 — 无质量 dashboard / EM3-P2-007 无 score preview
- Priority: P2 | Phase: R8 | Milestone: beta1 | Size: M | Risk: Low | Deps: EM3-P1-009/011
- Files: gui
- Problem: 无质量仪表盘、无 score 预览热力图。
- Why: 看不到"为何细化/是否更好"。违原则 5。
- Fix: 质量 dashboard（[08](./08_mesh_quality_metrics_design.md)§9）+ score 热力图（[09](./09_score_based_refinement_design.md)/[10](./10_gui_redesign_proposal.md) G-7/G-8）。
- Tests: `gui_score_preview_renders_heatmap`,`gui_quality_worst_cells_overlay`
- 用户行为变化: 是（新增视图）。

### EM3-P2-008 — MERIT-Hydro/hydro-coast 不可在 GUI 触发
- Priority: P2 | Phase: R9 | Milestone: beta1 | Size: L | Risk: Med | Deps: EM3-P2-004
- Files: gui
- Problem: hydro/MERIT/composite/coupling-package 只能 CLI+Python。
- Why: 核心场景无 GUI（违工程原则 5）。
- Fix: hydro-coast GUI flow（13 步，[06](./06_merit_hydro_hydro_coast_audit.md)§9/[10](./10_gui_redesign_proposal.md) G-9）。
- Tests: `gui_merit_hydro_flow_end_to_end`
- 用户行为变化: 是（GUI 新增 hydro 流程）。

### EM3-P2-009 — 区域不能地图上画 polygon
- Priority: P2 | Phase: R9 | Milestone: beta1 | Size: M | Risk: Low | Deps: EM3-P2-004
- Files: `gui/main.rs:3231-3258`
- Problem: 区域只能数值输入，不能在地图画 polygon。
- Why: 复杂流域/海岸交互困难。
- Fix: walkers 地图框选/画多边形→domain。
- Tests: `gui_domain_draw_polygon_to_config`
- 用户行为变化: 是（新增交互）。

### EM3-P2-010 — 版本/元数据/文档不完整
- Priority: P2 | Phase: R1 | Milestone: alpha2 | Size: S | Risk: Low | Deps: EM3-P2-001
- Files: 各 `Cargo.toml`(version 0.1.0→3.0.0-alpha.x; cli 缺 description/license), README/examples
- Problem: crate 版本仍 0.1.0；cli 缺元数据；examples/docs 不完整。
- Why: 不自述为 v3；可复现性差。
- Fix: 统一版本 + 补元数据 + 完善 examples README。
- Tests: `cargo metadata`
- 用户行为变化: 否。

### EM3-P2-011 — 测试 feature 不一致致 netcdf 缓存抖动
- Priority: P2 | Phase: R1 | Milestone: alpha2 | Size: S | Risk: Low | Deps: —
- Files: Makefile, README, CI
- Problem: `cargo test cli`(动态 netcdf) 与 `make test`(static-netcdf) feature 不一致 → 重建 netcdf-sys。
- Why: CI 极慢、本地构建抖动（[01](./01_build_and_crate_audit.md)）。
- Fix: 文档统一 cli 测试 feature；CI 固定一种。
- Tests: `make test`(计时)
- 用户行为变化: 否。

---

## P3 — 长期架构（不现在做）

### EM3-P3-001 — 拆分超大 lib.rs（mesh 22k / cli 36k 单文件）
- Priority: P3 | Phase: R10 | Milestone: v3.x | Size: L | Risk: High | Deps: 大量
- Files: `mesh/src/lib.rs`,`cli/src/lib.rs`
- Fix: 按 refine/topology/quality/io/workflow 渐进模块化（绿测约束，小步）。Tests: 全量回归。用户行为: 否。

### EM3-P3-002 — robust geometry backend（凹/holes/multipolygon/dateline/pole）
- Priority: P3 | Phase: R10 | Milestone: v3.x | Size: L | Risk: High | Deps: EM3-P1-001
- Files: `earthmesh_geometry`
- Fix: Weiler-Atherton/Vatti 或评估 `geo` 库（许可/体量）。Tests: 凹/洞/dateline/pole 套件。用户行为: 否。

### EM3-P3-003 — RefinementCriterion plugin + 多目标优化
- Priority: P3 | Phase: R10 | Milestone: v3.x | Size: L | Risk: High | Deps: EM3-P1-011
- Fix: plugin 注册表 + Pareto 多目标（[09](./09_score_based_refinement_design.md)）。Tests: planner 全套。用户行为: 是（高级）。

### EM3-P3-004 — earthmesh_quality 物理保真全量
- Priority: P3 | Phase: R10 | Milestone: v3.x | Size: L | Risk: Med | Deps: EM3-P1-009, 参考数据
- Fix: land/ocean/hydro/coupling/atmos 保真指标（[08](./08_mesh_quality_metrics_design.md) Q5）。Tests: 保真套件。用户行为: 否。

### EM3-P3-005 — Rust 化 hydro eval/ranking/HTML，收敛 Rust/Python 双实现
- Priority: P3 | Phase: R10 | Milestone: v3.x | Size: L | Risk: High | Deps: EM3-P1-009
- Files: `cli`/新 crate（替代 `util/hydro_mesh/*` eval/sweep/map）
- Fix: Rust-native eval/ranking/HTML；`colm_coupling` 单一真相源（[02](./02_workflow_consistency_audit.md) W3/W4）。Tests: eval/ranking。用户行为: 是（去 Python 依赖）。

### EM3-P3-006 — GUI 完整工作流平台
- Priority: P3 | Phase: R10 | Milestone: v3.x | Size: L | Risk: Med | Deps: EM3-P2-004/006/008
- Fix: 完整 10 步向导 + 仪表盘 + Expert（[10](./10_gui_redesign_proposal.md) 全量）。Tests: GUI 工作流套件。用户行为: 是。

---

## 统计

| 优先级 | 条目 | ID 数 | 目标里程碑 |
|--------|------|-------|-----------|
| P0 | 2 | 2 | alpha2 |
| P1 | 13 | 13 | alpha2–beta1 |
| P2 | 10 | 11（P2-006/007 合并一条目） | alpha2–beta1 |
| P3 | 6 | 6 | v3.x |
| 合计 | 31 | 32 | — |

依赖根：P2-003(`earthmesh_project`) 与 P1-009(`earthmesh_quality`) 是多数 P1/P2 的前置。
