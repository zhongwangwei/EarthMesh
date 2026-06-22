# FINAL — EarthMesh v3 Mesh Audit Report

> 综合报告 · 汇总 [01](./01_build_and_crate_audit.md)–[10](./10_gui_redesign_proposal.md) 全部审查与设计 · 未修改任何 `src/rust` 代码
> 审查对象：EarthMesh v3，分支 `v3.0.0-alpha1`，作为独立 Rust-native + GUI + GIS 项目（不引用旧版本）
> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md)（总原则五条）· 进度：[TASK_STATUS.md](./TASK_STATUS.md)
> 日期：2026-06-22 · 证据等级：A=已证实(带 file:line) / B=强推断 / C=疑似
> 源报告：01 构建 · 02 workflow · 03 配置 · 04 物理细化 · 05 耦合 · 06 hydro · 07 geometry · 08 质量 · 09 细化引擎 · 10 GUI

---

## 1. Executive Summary

EarthMesh v3 已是一个**能跑通、有真实球面网格生成内核、有 GUI 与多模式输出**的项目（core/geometry/mesh 测试基本绿、`mkgrd.x` 可构建、GUI cancel/progress 真实）。但作为"更通用、更可靠、更高网格质量、更符合物理过程"的平台目标，它有**三条系统性短板**：

1. **球面网格 + 平面 GIS 的割裂**：网格生成是 3D 球面，但决定细化与耦合的 GIS/mask/area/overlay 层几乎全是 lon/lat 平面（[07](./07_geometry_gis_audit.md)）→ 高纬失真、跨 180°/极区裸奔、面积不守恒。
2. **细化是"布尔阈值"而非"物理感知的评分"**：每变量超阈值即细化，无 score / 预算 / 质量反馈 / 收益-成本（[04](./04_physical_refinement_audit.md)/[09](./09_score_based_refinement_design.md)）→ 违背总原则 1/2/3。
3. **陆海耦合是"二分类占位"而非"守恒耦合"**：点采样 land/ocean，CSV 关键列全占位，`overlay_cell` 已实现却未用（[05](./05_coupled_mesh_audit.md)）→ 违背原则 4。

此外：v3 实为 **Rust + Python 混合**（hydro eval/ranking/HTML 在 `util/`，[02](./02_workflow_consistency_audit.md)）；配置是 **64 个扁平 engine 字段**无项目层（[03](./03_config_schema_audit.md)）；GUI 是**参数表单**无质量反馈（[10](./10_gui_redesign_proposal.md)）；质量度量**仅角度+边长**（[08](./08_mesh_quality_metrics_design.md)）。

### Top 10 最严重问题

| # | 问题 | 类别 | 影响 | 源 | 级别 |
|---|------|------|------|----|------|
| 1 | 陆海耦合点采样非守恒 + CSV 关键列占位（river/coast/fraction/area=false/0） | 物理+质量 | 网格质量·下游误读 | [05](./05_coupled_mesh_audit.md) W1 | Blocker |
| 2 | GIS/mask/overlay 全平面 lon/lat（无球面/投影/守恒面积） | 架构+物理 | 网格质量·高纬失真 | [07](./07_geometry_gis_audit.md) | High |
| 3 | 细化=纯布尔阈值（无 score/预算/质量反馈/收益-成本） | 物理+科学 | 网格质量·可用性 | [04](./04_physical_refinement_audit.md)/[09](./09_score_based_refinement_design.md) | High |
| 4 | `olam_delaunay_mesh` 4 测试红（半径容差过紧，平台敏感） | bug | 数值稳定性·CI | [01](./01_build_and_crate_audit.md)#1 | High |
| 5 | v3 实为 Rust+Python 混合（hydro eval/ranking/HTML 在 util/；colm_coupling 双实现） | 架构 | 可复现性·一致性 | [02](./02_workflow_consistency_audit.md) W3/W4 | High |
| 6 | 配置 64 扁平 engine 字段无项目层；GUI 暴露全部 | 架构+GUI | 可用性 | [03](./03_config_schema_audit.md)/[10](./10_gui_redesign_proposal.md) | High |
| 7 | 质量度量仅角度+边长（连面积统计都无），无门禁/无 GUI 展示 | 质量+GUI | 网格质量·可用性 | [08](./08_mesh_quality_metrics_design.md) | High |
| 8 | 跨 180° tile 选择不支持；几何 buffer/simplify 全 degree（高纬失真/窄河道破坏） | bug+几何 | 网格质量 | [06](./06_merit_hydro_hydro_coast_audit.md) H-B1/H-G2 | High |
| 9 | 无 workspace 根 Cargo.toml（5 份 target≈27G）；`make fmt` 红 | 架构+构建 | 可维护性·CI | [01](./01_build_and_crate_audit.md)#2/#3 | Med |
| 10 | 无 river-mouth/estuary/coastline/orphan 概念（CaMa is_estuary 已读未用） | 物理 | 网格质量 | [05](./05_coupled_mesh_audit.md)/[06](./06_merit_hydro_hydro_coast_audit.md) | Med |

### 分类汇总

- **Bug（代码缺陷）**：#4 olam 半径容差、#8 跨 180°、composite 去重键、`intersection_area` 静默回退、overlay 无 Σ=1 校验、stride 漏河、`Default` 的 `" /tmp"` 前导空格、`make fmt` 红、FVCOM regional 静默跳过。
- **架构问题**：#2 平面 GIS、#5 Rust+Python 混合/双实现、#6 无项目层/超大 lib.rs（mesh 22k/cli 36k 单文件）、#9 无 workspace。
- **物理/科学问题**：#1 非守恒耦合、#3 布尔阈值无物理感知、#10 缺 river-mouth/estuary/coastline、refine 阈值无物理过程绑定、无收益/成本与预算。
- **GUI/product 问题**：#6 参数表单、#7 无质量反馈、hydro/MERIT 不可在 GUI 触发、区域不能地图画 polygon、native OLAM 靠粘贴文本。
- **影响网格质量**：#1/#2/#3/#7/#8/#10（守恒、几何精度、细化合理性、质量门禁）。
- **影响可用性**：#3/#5/#6/#7/#9（盲跑、Python 依赖、专家门槛、无质量看板、构建臃肿）。

---

## 2. Bug Table

| Priority | File | Function | Line | Problem | Why it matters | Trigger | Fix | Test |
|----------|------|----------|------|---------|----------------|---------|-----|------|
| P0 | `mesh/tests/olam_delaunay_mesh.rs` | radius assert | 31,246,271,336 | 半径容差 ±1e-6 m（相对~1.5e-13）过紧 | 4 测试红、CI 不可靠、几何基线不稳 | 任意平台 FP 差异 | 相对容差 `1e-6·R` 或强制 renormalize | `cargo test mesh --test olam_delaunay_mesh` |
| P0 | `cli/lib.rs` | `write_colm_coupling_csv_from_mesh` | 28992-29021 | 点采样二分类 + river/coast/fraction/area 写占位 false/0 | 下游把"无河流/无海岸"误读为真值；非守恒 | 任意 LOCmesh CoLM 导出 | 接 `overlay_cell` 求守恒 fraction，填实 area | `coupled_overlay_fraction_sums_to_one` |
| P0 | `cli/lib.rs` | `merit_bbox_intersects` | 4401 | 经度线性比较，不支持跨 180° | 太平洋/白令海漏选 tile | bbox 跨 ±180° | 复用 `shift_longitudes_for_dateline_crossing` | `merit_bbox_crosses_antimeridian` |
| P1 | `geometry/lib.rs` | `intersection_area` | 115-120 | 三角化失败静默回退凸裁剪 | 复杂/自交多边形面积错且无 flag | 自交/带洞 mask | 报 flag，不静默；robust clip | `intersection_self_intersecting_flagged` |
| P1 | `geometry/lib.rs` | `overlay_cell` | 198 | 无 Σ fraction=1 校验 | 守恒失败不可见 | 任意 overlay | 加 `UnresolvedFractionSumError` | `overlay_fraction_sum_flags` |
| P1 | `cli/lib.rs` | `indices_between_inclusive` | 746 | stride 抽样跳过窄河道 | 小河/窄道丢失，河网断裂 | stride>1 | 抽样改聚合(max upa/河道命中) | `merit_stride_preserves_narrow_river` |
| P1 | `cli/lib.rs` | `apply_composite_refine_degree_cap` | 1453 | 去重键仅 refine_degree（无几何） | 跨组件同区域重复细化 | composite 多组件 | 同 degree 重叠几何 union 再 cap | `composite_dedup_geometric_overlap` |
| P2 | `core/lib.rs` | `EarthmeshConfig::default` | 766-790 | `base_dir:" /tmp"`/`mode_file:" /tmp"` 前导空格 + POSIX-only | 路径错误、跨平台失败 | 用 Default 配置 | 去空格 + `temp_dir()` | `core --all-targets` |
| P2 | `gui/main.rs` | `produce_outputs` | 1303-1305 | FVCOM regional 开边界静默跳过 | 用户拿不到 .2dm 无告警 | regional+open boundary | 显式告警/状态提示 | `gui` 集成测试 |
| P2 | mesh/cli/gui | — | — | `cargo fmt --check` 失败(1560/516/22 行) | `make fmt` CI 红 | CI fmt 门禁 | 固定 rustfmt + 一次性 fmt | `make fmt` |

> 说明：#1/#4 olam 相关测试目前在工作树有外部未提交修改（见 §末注），落地前需先定版。

---

## 3. Workflow Consistency Table

| Workflow | Input | Intermediate files | Output | Current risk | Missing validation | Recommended redesign |
|----------|-------|--------------------|--------|--------------|--------------------|----------------------|
| GUI run | UI 状态 | `earthmesh_gui_run.nml`, mask*.nc4 | gridfile+MPAS/FVCOM/CoLM | 配置散落、无质量反馈 | namelist 合法性/资源存在性预检 | 向导+ProjectConfig+质量卡片([10](./10_gui_redesign_proposal.md)) |
| gridinit | namelist | gridfile_NXP*_01 | gridfile+quality.nc4 | olam 半径红 | 几何质量门禁 | score 框架 + 门禁 |
| specified refine | refine_spc+mask | mask nc4/nml | 细化 gridfile | 区域越界无校验 | region 合法性/平滑 | RegionCriterion([09](./09_score_based_refinement_design.md)) |
| threshold/calc refine | refine_cal+landtype | 阈值场 | 细化 gridfile | 阈值无物理依据/无预算 | 异质性→过程映射/预算 | composite score+budget([04](./04_physical_refinement_audit.md)/[09](./09_score_based_refinement_design.md)) |
| MERIT-Hydro mask | MERIT root+bbox | window/geojson | river/coast geojson | 仅 bbox/跨180°/硬编码 IGBP | tile 覆盖/分类物理 | polygon+阈值外置([06](./06_merit_hydro_hydro_coast_audit.md)) |
| hydro-close recipe | geojson | recipe.json | namelist overrides | 内嵌 python3 调用 | recipe schema | Rust 化 refine_mask_export |
| hydro-close/composite mask | geojson/recipe | — | nml 集 | 数量爆炸/无几何去重 | 数量/分离度健全 | km buffer+几何去重([06](./06_merit_hydro_hydro_coast_audit.md)) |
| land-ocean coupling | gridfile+landtype | coupling.csv(占位) | coupling/restart/forcing nc4 | 非守恒/占位/无 coast-river | 守恒/和=1/coast | overlay 守恒+QA([05](./05_coupled_mesh_audit.md)) |
| 模式输出(CoLM/MPAS/FVCOM/OLAM/native) | gridfile | — | nc4/2dm/graph/csv | FVCOM 静默跳过 | 输出 round-trip 校验 | 单一真相源+读回校验 |
| 报告(GeoJSON/NetCDF/CSV/HTML) | 各产物 | — | geojson/nc/csv；HTML 仅 Python | eval/ranking/HTML 依赖 Python | Rust 侧无 HTML/eval | Rust-native 报告([08](./08_mesh_quality_metrics_design.md)) |

---

## 4. Physical Consistency Table

| Mesh type | Criterion | Physical meaning | Current limitation | Better method | Required data | GUI recommendation |
|-----------|-----------|------------------|--------------------|---------------|---------------|--------------------|
| Land | landcover heterogeneity | 地表通量异质 | 仅 num_landtypes 计数 | 熵+纯度 score | landcover | "Land cover diversity" 滑杆 |
| Land | LAI / slope | 蒸散/能量/汇流 | mean/std 阈值开关 | 归一化 score | LAI/DEM | 滑杆+热力图 |
| Land | elevation/TWI/river-distance | 地形/水文 | **缺**(slope 有，elev/TWI 无) | 接 MERIT(已读) | MERIT-DEM | "Terrain/Wetness/Rivers" |
| Land | soil hydraulic/thermal | 土壤水热 | mean/std 阈值 | 归一化 score | soil | 折叠区 |
| Land | snow/permafrost/urban/crop | 雪冻/城市/农业 | **全缺** | 新 criterion | snow/permafrost/urban | preset 暴露 |
| Ocean | SST/SSH/EKE | 中尺度动力 | mean/std 阈值 | 归一化梯度 score | OISST/AVISO | 滑杆 |
| Ocean | bathy slope | 地形动力 | 仅 seaslope，无深度 | +深度+shelf break | GEBCO | 折叠区 |
| Ocean | coastline/distance-to-coast | 陆海交互 | **缺**(仅 MERIT 分类) | exp(-d/Lc) km | coastline | "Coastal proximity" |
| Ocean | estuary/river-mouth | 盐淡水混合 | CaMa 有数据未接 | 接 is_estuary | CaMa | "Estuaries" |
| Atmosphere | typhoon/TC | 强对流路径 | **唯一支持** | 泛化 track density | IBTrACS | 滑杆 |
| Atmosphere | topo grad/orographic precip | 地形抬升降水 | **缺** | norm 梯度 score | DEM/precip | preset |
| Coupled | land/ocean fraction | 通量分配 | 二值非守恒 | overlay 守恒 fraction | landcover+bathy | fraction 热力图 |

> 通则缺失：所有 criterion **无 `physical_process` 声明、无预算、无 quality_contribution**（[04](./04_physical_refinement_audit.md)/[09](./09_score_based_refinement_design.md)）。覆盖率：Land 6/24·Ocean 5/21·Atmos 1/17。

---

## 5. MERIT-Hydro / Hydro-Coast Review

**当前流程**（[06](./06_merit_hydro_hydro_coast_audit.md)）：MERIT root（可配 `:1190`）→ `select_merit_hydro_tiles`（仅 bbox `:864`）→ `read_merit_hydro_window`（stride 抽样 `:746`，nodata `clean_merit_fill` `:4393`）→ `classify_merit_hydro_window`（R2/R3/COAST/LAND/OCEAN，`:4412`）→ geojson 图层 → recipe（内嵌 `python3 -m util...` `:1070`）→ close/composite nml（degree buffer/simplify `:2734,2911`）→ mesh refine。eval/ranking/HTML 在 Python `util/`。

| 风险类 | 内容 | 证据 |
|--------|------|------|
| **Bug** | 跨 180° 不支持(`:4401`)；composite 无几何去重(`:1453`)；buffer 自交未必修复(`:2911`) | [06](./06_merit_hydro_hydro_coast_audit.md) H-B1/B2/B3 |
| **物理** | 无 river-mouth/estuary/delta 类(CaMa is_estuary 未接)；river>coast 掩盖河口；阈值无物理依据；Rust 无 eval 证明更优 | H-P1/P3/P4/P6 |
| **几何** | buffer/simplify/距离全 degree → 高纬失真、窄河道破坏；无等面积投影 | H-G1/G2/G3 |
| **数据** | stride 抽样漏窄河(`:746`)；ocean landtype 硬编码 IGBP 0/17；无覆盖率/单位校验 | H-D1/D3/D4 |

- **更好的 score**：`score_hydro_coast`（14 项，km 距离归一化，含 upa/wth/order/river-coast distance/river-mouth/estuary/delta/connectivity/basin/coastline/fraction-error/coupling-error，[06 §6](./06_merit_hydro_hydro_coast_audit.md#6-proposed-hydro-coast-score)）。
- **更好的 GUI workflow**：13 步（选 root→画 polygon→选目标→图层→feature→score 热力图→预算分配→生成 mask→refine→拓扑→质量→导出→可视化，[06 §9](./06_merit_hydro_hydro_coast_audit.md#9-gui-workflow13-步)）。
- **应新增 quality metrics**：river length/width/upstream-area captured、connectivity、coastline Hausdorff、small-river loss、benefit/cost Pareto、before/after fraction error（[06 §8](./06_merit_hydro_hydro_coast_audit.md#8-better-eval--ranking-metrics)）。

---

## 6. Mesh Quality Metric Proposal

> 完整 ~80 指标见 [08](./08_mesh_quality_metrics_design.md)；现状仅算角度+边长。下表为代表性门禁项。

| Metric | Definition | Applies to | Pass / Warn / Fail | Output file | GUI display layer |
|--------|-----------|------------|--------------------|-------------|-------------------|
| min angle | 最小内角 | 几何(全) | tri≥30/20-30/<20°；hex≥100/90-100/<90° | quality.nc4+csv | 几何卡片+worst cells |
| aspect ratio | 最长/最短边 | 几何 | <2 / 2-4 / >4 | quality.nc4 | 几何卡片 |
| compactness | 4πA/P² | 几何 | >0.7 / 0.5-0.7 / <0.5 | quality.nc4 | 几何卡片 |
| cell area CV | std/mean 面积 | 几何 | <0.5 / 0.5-1 / >1 | quality.nc4 | 直方图 |
| neighbor reciprocity | a↔b 对称 | 拓扑 | 0 / – / ≥1 fail | quality.nc4 | 拓扑卡片 |
| orphan cell | 孤立单元 | 拓扑 | 0 / – / ≥1 | geojson worst | 地图高亮 |
| Euler characteristic | V-E+F | 拓扑 | =2 / – / ≠2 | csv | 拓扑卡片 |
| max adjacent res ratio | 相邻分辨率比 | 数值 | ≤2 / 2-3 / >3 | quality.nc4 | 过渡图 |
| CFL min edge | 最小边长 | 数值 | ≥L_CFL / 0.8-1 / <0.8 | csv | 数值卡片 |
| mass conservation residual | \|1-Σfraction\| | 耦合 | <1e-9 / <1e-6 / ≥1e-6 | coupling nc4 | 耦合卡片 |
| land/ocean fraction error | vs 参考 | 耦合 | <0.5% / 0.5-1% / >1% | coupling nc4 | 耦合卡片 |
| coastline Hausdorff | 网格 vs 真实岸线 | 物理(ocean) | <0.5cell / 0.5-1 / >1 | geojson | 地图 |
| river connectivity | 连通河段比 | 物理(hydro) | =1 / ≥0.95 / <0.95 | csv | hydro 卡片 |
| LAI/elev variance retained | 子格方差下降 | 物理(land) | ≥X% / / <X% | csv | 物理卡片 |

聚合 verdict：任一 Fail→Fail；`Block` 阻断运行（原则 4）。

---

## 7. GUI Redesign Proposal

> 详见 [10](./10_gui_redesign_proposal.md)。把 GUI 从"参数表单"升级为"工作流平台"。

- **Workflow wizard（10 步）**：New Project → Select Target → Select Domain → Add Data Layers → Refinement Strategy → Quality Constraints → **Preview Scores** → Generate → **Inspect Quality** → Export。
- **Tabs（8）**：Project / Target / Domain / Data / Strategy / Quality / Preview / Run-Results（Expert 追加 raw config/CLI 预览/生成文件/log）。
- **Presets（12 模板）**：global atmos / regional land / regional ocean / coupled / MERIT-Hydro / estuary / hydrology land / urban / orographic atmos / CoLM land / MPAS atmos / FVCOM ocean。
- **Data layer manager（17 图层）**：landcover/DEM/LAI/soil/river/MERIT/CaMa/coastline/bathy/SST/SSH/EKE/precip/typhoon/urban/cropland/snow-permafrost；含覆盖率/单位/缺失校验（缺则 criterion 灰显告警）。
- **Score preview**：composite score 热力图 + 点击 cell 看 reason（"wth=420m≥R3"）→ 原则 5。
- **Quality dashboard**：verdict 徽章 + 几何/拓扑/物理/耦合 P/W/F + worst cells 地图 + recommended fixes + 导出（[08](./08_mesh_quality_metrics_design.md)）→ 原则 4。
- **Expert mode**：保留现有 64 字段全部能力（nxp/HALO/spring/manual mask/plugin），Normal 用户不必接触。
- **Project save/load**：单一 `project.yaml`/`.json`（[03](./03_config_schema_audit.md) `ProjectConfig`），旧 `.nml` 可导入。
- **推荐 i18n keys**（沿用 `(key,en,zh)`）：`wizard.*`/`target.*`/`layer.*`/`strategy.*`/`preview.*`/`quality.*`/`expert.*`/`project.*`/`mode.*`（清单见 [10 §11](./10_gui_redesign_proposal.md#11-i18n-key-suggestions)）。

---

## 8. Refactoring Roadmap

### Immediate fixes（1–2 周，低风险，多为 bug/卫生）
- 修 olam 半径容差（P0 bug，#4）；先合并工作树中的 OLAM 外部改动并定版。
- 修 `EarthmeshConfig::default` 的 `" /tmp"` 前导空格 + 跨平台 temp（#P2）。
- 固定 `rust-toolchain.toml` + 一次性 `cargo fmt` + CI `make fmt` 门禁（#9）。
- 加根 `[workspace] Cargo.toml`（共享 target，省 ~27G 重复，#9）。
- 文档统一 cli 测试 feature（消除动态/静态 netcdf 缓存抖动，[01](./01_build_and_crate_audit.md)）。
- 跨 180° tile 选择（接现有 `shift_longitudes_for_dateline_crossing`，#8）。

### Medium-term（1–2 月，新增能力，不破坏现状）
- `earthmesh_project` crate（ProjectConfig 四层 + lower→现有 config，[03](./03_config_schema_audit.md) S1-S3）。
- `earthmesh_quality` crate（几何/拓扑/数值 metrics + 门禁，[08](./08_mesh_quality_metrics_design.md) Q1-Q6）。
- `GeometryQualityFlag`(11) + Σfraction 守恒校验 + 球面/等面积面积（[07](./07_geometry_gis_audit.md) G1-G3）。
- coupling 守恒分类（接 `overlay_cell`）+ CaMa estuary/river-mouth + 守恒门禁（[05](./05_coupled_mesh_audit.md) C1-C6）。
- hydro buffer/simplify 改 km + 局地投影（[06](./06_merit_hydro_hydro_coast_audit.md) H2）。
- `RefinementCriterion` plugin + composite score + 预算（[09](./09_score_based_refinement_design.md) R1-R5，先把布尔阈值包成 criterion 保回归）。

### Long-term v3.x architecture（3–6 月，系统性）
- 拆分超大 `lib.rs`（mesh 22k / cli 36k）为模块化 crate（按 refine/topology/quality/io/workflow）。
- robust geometry backend（凹/holes/multipolygon/dateline/pole，[07](./07_geometry_gis_audit.md) G4-G7）。
- repair loop（质量驱动自适应，[09](./09_score_based_refinement_design.md) R7）+ 物理 criteria 全量（[04](./04_physical_refinement_audit.md)）。
- GUI 工作流平台（10 步向导 + 仪表盘，[10](./10_gui_redesign_proposal.md) G-1~G-10）。
- Rust 化 hydro eval/ranking/HTML，收敛 Rust/Python 双实现（[02](./02_workflow_consistency_audit.md) W3/W4）。

---

## 9. Patch Plan

### A. 可直接改的小 patch（低风险、隔离、即时收益）
| Patch | 内容 | 测试 |
|-------|------|------|
| fix-olam-tol | olam 半径相对容差 | `mesh --test olam_delaunay_mesh` |
| fix-default-tmp | 去 `" /tmp"` 空格+temp_dir | `core --all-targets` |
| fix-fmt | rust-toolchain + cargo fmt + CI 门禁 | `make fmt` |
| add-workspace | 根 `[workspace]` | `cargo build --workspace` |
| fix-fvcom-warn | regional 跳过显式告警 | gui 集成 |
| geom-flags | `GeometryQualityFlag`(11)+Σ=1 校验 | `overlay_fraction_sum_flags` |

### B. 需讨论的大改动（架构级，需决策）
| Patch | 决策点 | 测试 |
|-------|--------|------|
| project-crate | 是否引入 `earthmesh_project` + YAML/JSON | `project roundtrip` |
| quality-crate | 是否引入 `earthmesh_quality` + 门禁 | quality 全套 |
| refine-planner | 是否引入 score/预算/repair planner | planner 全套 |
| coupling-conservative | overlay 守恒 + CaMa 接入 | `coupled_*` |
| geometry-backend | 球面/投影/robust clipping(自研 vs 库) | geometry 全套 |
| hydro-rustify | Rust 化 eval/ranking/HTML，去 Python | hydro eval |
| gui-platform | 10 步向导 + 仪表盘 | gui 工作流 |

### C. 不建议现在做
- 一次性重写 mesh/cli 超大 lib.rs（应渐进拆分，绿测约束下分步）。
- 引入重型几何依赖前未评估许可/体量（先做局地投影+守恒，robust clipping 库后评估）。
- native OLAM 嵌套网格的完整 GUI 构建器（低频专家用，成本高）。
- 在 OLAM 外部改动未定版前落地任何 mesh 内核 patch（基线漂移风险）。

> 每个 patch 的详细测试见各源报告的 Tests/Patch Plan 节。统一依赖根：B 类全部指向 `earthmesh_project`(S1) 与 `earthmesh_quality`。

---

## 附注：审查期间的外部代码改动

审查全程**未修改任何 `src/rust`**（仅写 `docs/reviews/v3_mesh_audit/`）。但工作树中出现**外部未提交改动**（非本审查所为）：`mesh/src/lib.rs` + `tests/olam_delaunay_mesh.rs`/`olam_spawn_nest.rs`/`voronoi_grid_state.rs`（OLAM 多区域细化修复，疑似针对 #4）。**落地任何 mesh patch 前应先合并/定版这些改动**，否则审查基线（尤其 #4 olam 半径、§2 Bug Table）会漂移。

---

*本终版报告综合 10 份源报告；所有现状结论基于实际源码与 grep/测试实证（A 级带 file:line）。设计提案为建议，落地需用户批准（P8）。未修改任何 `src/rust` 代码。*
