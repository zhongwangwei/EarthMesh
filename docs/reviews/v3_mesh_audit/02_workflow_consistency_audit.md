# 02 — Workflow Consistency Audit (EarthMesh v3)

> Phase P1/P3 衔接 · 只读阶段（未修改任何 `src/rust` 代码）
> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md) · 计划：[PROJECT_AUDIT_PLAN.md](./PROJECT_AUDIT_PLAN.md) · 上游：[01_build_and_crate_audit.md](./01_build_and_crate_audit.md)
> 审查对象：EarthMesh v3，分支 `v3.0.0-alpha1`，仅当前项目，不引用任何旧版本。
> 证据：函数名/行号取自 `rust/earthmesh_gui/src/main.rs`、`rust/earthmesh_cli/src/lib.rs`、`rust/earthmesh_cli/src/main.rs`、`rust/earthmesh_mesh/src/lib.rs`、`util/` 与 `examples/merit_hydro/*/delivery_manifest.json`。
> 日期：2026-06-22 · 证据等级见 AUDIT_PRINCIPLES §5。

---

## 0. 最重要发现（先读）

1. **v3 实为 Rust + Python 混合，而非纯 Rust-native**（A）。hydro-coast 的**评估 / ranking / HTML 地图 / QA 门禁 / CaMa 河口 / 守恒交集 / CoLM 耦合**主要住在 Python `util/`（40+ `.py`：`util/v3_core/*`、`util/hydro_mesh/*`）。Rust 的 hydro-close recipe **生成一条调用 Python 的命令** `python3 -m util.hydro_mesh.refine_mask_export`（`cli/src/lib.rs:1070-1072`），且 `colm_coupling` 在 Rust（`lib.rs:28965+`）与 Python（`util/hydro_mesh/colm_coupling.py`）**重复实现**。
2. **land-ocean coupling 当前是占位骨架**（A）：cell 用**单点最近邻采样**做 land/ocean 二分类（`lib.rs:28992-29009`），CSV 的 river/coast/fraction/area 列全是占位符（`false/none/0.0`，`lib.rs:29001-29021`）；**无守恒面积分数、无 coastline/river-mouth 识别、无耦合质量校验**。`earthmesh_geometry` 已实现 `overlay_cell()`/`intersection_area()`（`geometry/src/lib.rs:114-210`）但 **coupling 完全没调用**。
3. **质量报告分裂**（A）：mesh 生成有 3 阶段 quality NetCDF（`gridfile_quality.nc4`/`quality_before_spring.nc4`/`quality_after_spring.nc4`，`lib.rs:16049/16090/16128`），但**无 HTML、无几何质量阈值门禁**；GUI **完全不展示任何质量指标**；hydro-coast 的 eval/ranking/HTML 只在 Python `util/` + delivery package 里。
4. **GUI 仅暴露 mesh 生成主干**（A），MERIT-Hydro / hydro-close / composite / CaMa / coupling-package 等 workflow **不可在 GUI 触发**，必须走 CLI + Python。

---

## 1. Mermaid Flowchart — GUI → Config → Engine → Output → Quality Report

```mermaid
flowchart TD
    subgraph GUI["GUI (earthmesh_gui/main.rs)"]
        U1["选 mesh_type / model / grid(hex,tri)<br/>main.rs:3155-3193"]
        U2["选 domain: global / regional<br/>bbox·circle·close·lambert<br/>main.rs:3198-3258"]
        U3["配置 refinement<br/>specified + calculated criteria<br/>main.rs:3500-3742"]
        U4["选 landtype / initial mesh<br/>main.rs:3278-3293,3749-3758"]
        CFG["run_namelist + combined_namelist<br/>→ earthmesh_gui_run.nml + mask*.nc4<br/>main.rs:1457-1497,1715-1889"]
    end
    subgraph ENGINE["Engine (链接 earthmesh_cli 库, 非 shell)"]
        RUN["thread::spawn → run_mkgrd_top_level_namelist_<br/>with_default_restart_refine_handoff<br/>main.rs:1969-2019"]
        PROG["progress::set 回调 (phase,done,total)<br/>cancel = Arc&lt;AtomicBool&gt; 真实可取消<br/>main.rs:1970-1972,2132-2137"]
    end
    subgraph OUT["Output (produce_outputs main.rs:1200-1349)"]
        O1["write_landtype_masked_gridfile / write_regional_gridfile"]
        O2["MPAS: write_standard/regional_mpas_from_gridfile"]
        O3["FVCOM: write_standard_fvcom / clean_regional_ocean_fvcom"]
        O4["CoLM: write_colm_coupling_csv → netcdf_from_csv"]
    end
    subgraph PREVIEW["Result preview"]
        P1["read_gridfile_mesh_points + walkers 地图<br/>MeshOverlay main.rs:698-996"]
        QR["质量报告: 仅显示 cell/vertex 计数<br/>★无几何质量指标 main.rs:2969-2983"]
    end
    U1-->U2-->U3-->U4-->CFG-->RUN
    RUN<-->PROG
    RUN-->O1-->O2 & O3 & O4
    O1-->P1-->QR
    QR-.->|缺口: 无 aspect/skew/min-angle/before-after|X1["★ Quality report 缺失"]
    style X1 fill:#fdd,stroke:#c00
    style QR fill:#ffd,stroke:#aa0
```

---

## 2. Mermaid Flowchart — Land Feature Refinement

```mermaid
flowchart TD
    A["namelist: NL%refine=.TRUE.<br/>RL%refine_cal / refine_spc"]-->B["EarthmeshConfig + RefineConfig<br/>from_*_namelist (cli lib.rs:16826,9002)"]
    B-->C["gridinit 初始网格<br/>icosahedron→Delaunay→spring→voronoi<br/>mesh.rs:1209,1590,1796,253"]
    C-->D{"refine 模式分流<br/>olam_direct_refine_dispatch_requested<br/>lib.rs:16436"}
    D-->|specified|E1["read_olam_specified_refinement_regions<br/>lib.rs:9066 (bbox/circle/close)"]
    D-->|calculated|E2["read_olam_calculated_refinement_regions<br/>lib.rs:9073"]
    subgraph CRIT["land criteria 阈值 (GUI main.rs:3590-3658)"]
        T1["num_landtypes / area_mainland"]
        T2["lai_m·s / slope_m·s"]
        T3["soil: ks/ksol/tkdry/tksatf/tksatu m·s (2-layer)"]
    end
    E2-->CRIT
    E1-->F["spawn_nest_* 嵌套细化<br/>lib.rs:9150-9279"]
    CRIT-->F
    F-->G["transition zone: HALO + max_transition_row<br/>boundary_rows → transition_faces lib.rs:9281"]
    G-->H["refine loop: judge b/c/e/g → Delaunay LOP<br/>→ array_length → ngr_renew<br/>mesh.rs:21567,22039"]
    H-->I["spring smoothing<br/>springjustment_global/regional mesh.rs:16238,16500"]
    I-->J["quality check before/after spring<br/>lib.rs:16049,16090,16128"]
    J-->K["output gridfile_*.nc4"]
    K-.->|原则2/3 校验缺口|Z["★ 无证据: 阈值是否对应物理过程<br/>无收益/成本(cell 数)上限"]
    style Z fill:#fdd,stroke:#c00
```

---

## 3. Mermaid Flowchart — Ocean / Coast Refinement

```mermaid
flowchart TD
    A["mesh_type=oceanmesh(tri) / LOCmesh<br/>mask_sea_ratio (case.nml)"]-->B["gridinit + ocean criteria"]
    subgraph OC["ocean criteria 阈值 (GUI main.rs:3660-3688)"]
        O1["sea_ratio (pair threshold)"]
        O2["sst_m·s / ssh_m·s / eke_m·s / seaslope_m·s"]
    end
    B-->OC-->C["spawn_nest 细化 + spring"]
    A2["coast: MERIT-Hydro classify<br/>COAST_OCEAN/COAST_LAND 邻接判定<br/>lib.rs:4437-4470,889-946"]-->A3["merit_coast_masks.geojson<br/>lib.rs:949-1023"]
    A3-->A4["hydro_close recipe/nml: class→close mask<br/>RL%mask_refine_spc_type='close'<br/>lib.rs:1035,1154"]
    A4-->C
    C-->D["mask postproc 海岸/海洋整形:<br/>remove_isolated_ocean / widen_narrow_waterway<br/>bdy_connection_closed_curve mesh.rs:18701"]
    D-->E["FVCOM .2dm / OBC boundary nc<br/>lib.rs:18993,28816"]
    D-->F["clean_regional_ocean_fvcom (GUI)<br/>main.rs:1354-1398"]
    E-.->|缺口|Z1["★ coastline 单元未在 mesh 内显式标注<br/>coast 分类来自外部 GeoJSON"]
    E-.->|缺口|Z2["★ ocean refine 无质量门禁/eval"]
    style Z1 fill:#fdd,stroke:#c00
    style Z2 fill:#fdd,stroke:#c00
```

---

## 4. Mermaid Flowchart — Land + Ocean Coupled Mesh

```mermaid
flowchart TD
    A["coupled mesh: gridfile_*.nc4 + landtype_file"]-->B["write_colm_coupling_csv_from_mesh<br/>lib.rs:28965"]
    B-->C["遍历 cell 中心 → sample(lon,lat) 单点最近邻<br/>lib.rs:28992-28996"]
    C-->D{"classify_area_judge_landtype<br/>lib.rs:20263 (二分类)"}
    D-->|landtype==0|E1["OCEAN"]
    D-->|landtype!=0|E2["LAND"]
    E1 & E2-->F["coupling.csv: surface_class 填好<br/>★ has_river/river_class/fraction/area = 占位符 false/none/0.0<br/>★ has_coast/coastal_fraction = 占位符<br/>★ normalized_cell_area_m2 = 0.0 未填<br/>lib.rs:29001-29021"]
    F-->G["write_colm_coupling_netcdf_from_csv lib.rs:29038"]
    F-->H["restart_template: land_fraction=1/0/(1-coastal)<br/>lib.rs:29226"]
    F-->I["forcing_template: area*fraction (基数=0 ⇒ 无意义)<br/>lib.rs:29311"]
    G & H & I-->J["write_colm_package_delivery_manifest<br/>lib.rs:29387"]
    subgraph MISSING["★ 守恒/质量缺口 (overlay_cell 已实现但未用 geometry/lib.rs:155)"]
        M1["无面积守恒 land/ocean fraction"]
        M2["无 coastline / river-mouth 识别"]
        M3["无 fraction 和=1 / 互补 / 守恒校验"]
        M4["CaMa estuary(is_estuary) 未接入 lib.rs:4571"]
    end
    C-.->M1
    F-.->M2 & M3
    style F fill:#ffd,stroke:#aa0
    style MISSING fill:#fdd,stroke:#c00
```

> 注：真正的守恒 fraction / coast / river-mouth / QA 逻辑当前在 Python `util/hydro_mesh/earthmesh_intersection.py`、`colm_coupling.py`、`qa_gates.py`、`coastal_band.py`、`cama_*.py`，**不在 Rust crate 内**。

---

## 5. Mermaid Flowchart — MERIT-Hydro → Hydro/Coast Masks → Refinement → Quality Report

```mermaid
flowchart TD
    A["MERIT-Hydro root (5°×5° tiles n##e###.nc)<br/>merit_tile_bounds_from_name lib.rs:804"]-->B["select_merit_hydro_tiles(bbox)<br/>★仅 bbox 不支持 polygon lib.rs:864"]
    B-->C["read_merit_hydro_window: dir/upa/elv/wth/landtype_igbp<br/>+ stride 子采样 lib.rs:731"]
    C-->D["classify_merit_hydro_window<br/>R3(wth≥300|upa≥5e4) R2(wth≥50|upa≥5e3)<br/>COAST_* 8邻接 OCEAN/LAND lib.rs:449-464,889"]
    D-->E["write_merit_hydro_mask_geojson_layers<br/>river/coast/surface .geojson + summary.json lib.rs:949"]
    E-->F["write_hydro_close_refinement_recipe_json<br/>class_refine {R2:1,R3:2} lib.rs:1026,1035"]
    F-->G["★ close_mask_command = python3 -m util.hydro_mesh.refine_mask_export<br/>lib.rs:1070-1072 (Rust→Python 接缝)"]
    F-->H["write_hydro_close_mask_nmls / composite<br/>refine_spc_*_d#_###.nml lib.rs:1154,1284"]
    H-->I["mesh refine: RL%mask_refine_spc_type='close'<br/>RL%refine_spc=.TRUE. (喂回 §2/§3 pipeline)"]
    I-->J["gridfile + 3 阶段 quality.nc4 (mesh 几何质量)"]
    subgraph PY["★ delivery package 由 Python util 生成 (非 Rust)"]
        Q1["refinement_eval.py → eval_json"]
        Q2["refinement_sweep.py → ranking_json"]
        Q3["geojson_map.py → leaflet .html"]
        Q4["refinement_package.py → delivery_manifest.json<br/>metrics: coast/river_overlap_cells, retained_triangles"]
    end
    J-->Q1
    I-->Q1
    Q1-->Q2-->Q3-->Q4
    style G fill:#ffd,stroke:#aa0
    style PY fill:#eef,stroke:#55a
```

> 证据：`examples/merit_hydro/gba/delivery_manifest.json` 含 `eval_json`/`ranking_json`/`html_map`/`metrics{coast_overlap_cells,river_overlap_cells,retained_triangles}`，`kind=earthmesh_hydro_coast_delivery_package`——这些产物由 Python `util/hydro_mesh/*` 生成，Rust 内无对应函数（grep 确认）。

---

## 6. Workflow Consistency Table

| Workflow | Input | Intermediate files | Output | Current risk | Missing validation | Recommended redesign |
|----------|-------|--------------------|--------|--------------|--------------------|----------------------|
| **GUI run** | UI 状态 (mesh_type/grid/domain/refine) | `earthmesh_gui_run.nml`, `mask_*.nc4`(临时) | gridfile + MPAS/FVCOM/CoLM | 配置散落 UI，无 dry-run 校验；无质量反馈 | 无 namelist 合法性/资源存在性预检；无质量门禁 | 引入"config 校验+预览+质量卡片"步骤；project template 化 |
| **Gridinit (普通 mesh)** | namelist (NXP/niter/beta/relax) | gridfile_NXP*_01_*.nc4 | gridfile_*.nc4 + quality.nc4 | olam 半径单测红(见 01#1) | 无几何质量阈值门禁（仅写 nc 不判定） | 质量指标→阈值门禁，失败可阻断 |
| **Specified-region refine** | refine_spc + bbox/circle/close mask | mask nc4 / nml | 细化 gridfile | 区域重叠/越界无校验 | 无 region 合法性、无 refine ratio 平滑检查 | 区域校验 + transition 平滑度报告 |
| **Threshold/calculated refine** | refine_cal + threshold_dir/landtype | 阈值场 | 细化 gridfile | 阈值物理依据不明(原则2)、无成本上限(原则3) | 无"异质性→物理过程"映射、无 cell 预算 | 阈值→物理过程登记表 + cell 预算约束 |
| **MERIT-Hydro mask gen** | MERIT root + bbox + thresholds | window, *.geojson, summary.json | river/coast/surface geojson | **仅 bbox 不支持 polygon**；ocean landtype 硬编码(0/17) | tile 覆盖完整性、分类阈值未对照物理 | 支持 polygon/shapefile；阈值参数外置 |
| **Hydro-close recipe** | river/coast geojson + class_refine | recipe.json | recipe + namelist overrides | **recipe 内嵌 python3 调用**(Rust→Python) | 无 recipe schema 校验 | 把 refine_mask_export 真正 Rust 化，去 Python 依赖 |
| **Hydro-close mask NML** | geojson + options | — | `refine_spc_*_d#_###.nml` | cumulative_refine 可致 nml 数量爆炸 | 无 nml 数量/分离度健全性检查 | 数量上限告警 + 干运行统计 |
| **Hydro-composite close mask** | recipe(components[]) | — | 合成 nml + summary.json | 合成优先级=数组顺序(隐式)；去重靠 (feature,ring) | 无组件冲突/覆盖报告 | 显式优先级 + 冲突可视化 |
| **Land-ocean coupling** | gridfile + landtype | coupling.csv (占位列) | coupling/restart/forcing nc4 + manifest | **点采样非守恒；river/coast/area 全占位** | **无守恒 fraction、无和=1、无 coast/river-mouth** | 接 overlay_cell 做守恒 fraction + QA 校验 |
| **CoLM/MPAS/FVCOM/OLAM/native output** | gridfile | — | nc4/2dm/graph.info/csv | FVCOM regional 开边界静默跳过(main.rs:1303) | 输出后无结构校验/读回验证 | 输出 round-trip 读回校验 |
| **GeoJSON/NetCDF/CSV/HTML report** | 各 workflow 产物 | — | geojson/nc/csv；**HTML 仅 Python** | HTML/eval/ranking 依赖 Python util | Rust 侧无 HTML/eval/ranking | 决策：Rust 化报告 or 正式承认 Python 层 |

---

## 7. Top 20 Workflow Risks

| # | 风险 | 严重度 | 类别 | 证据 |
|---|------|--------|------|------|
| 1 | coupling 用点采样非守恒，land/ocean 分类网格方向敏感、漏小岛/碎岸 | Blocker | Physical | `lib.rs:28992-29009` |
| 2 | coupling CSV 的 river/coast/fraction/area 全为占位符，下游误读为"无河流/无海岸" | Blocker | Data | `lib.rs:29001-29021` |
| 3 | 无守恒 fraction：`overlay_cell`/`intersection_area` 已实现却未接入 coupling | High | Physical | `geometry/lib.rs:114-210` 未被调用 |
| 4 | hydro-coast eval/ranking/HTML/QA 在 Python util，与"Rust-native"定位冲突，可复现性割裂 | High | Arch | `util/hydro_mesh/*`, `lib.rs:1070-1072` |
| 5 | `colm_coupling` Rust 与 Python 双实现，易行为漂移 | High | Arch | `lib.rs:28965` vs `util/hydro_mesh/colm_coupling.py` |
| 6 | recipe 生成内嵌 `python3 -m util...` 命令，部署需 Python 环境 | High | Build/Deploy | `lib.rs:1070-1072` |
| 7 | 无 coastline / river-mouth 显式识别（CaMa `is_estuary` 未接入） | High | Physical | `lib.rs:4571`, doc `28960-28966` |
| 8 | calculated refine 阈值无"物理过程相关性"依据（违原则2） | High | Physical | `case.nml` RL%refine_* + GUI criteria |
| 9 | refine 无 cell 预算/成本上限（违原则3，可无限细化） | High | Physical | 无相关参数 |
| 10 | 无几何质量门禁：quality 只写 NetCDF 不判定/不阻断 | High | Quality | `lib.rs:16049-16128` |
| 11 | GUI 不展示任何质量指标，用户盲跑（违原则5） | High | GUI | `main.rs:2969-2983` |
| 12 | MERIT tile 选择仅 bbox 不支持 polygon，复杂流域需手工 | Med | Workflow | `lib.rs:864-886` |
| 13 | ocean/coast landtype 硬编码 IGBP 0/17，数据集变更即失效 | Med | Robustness | `lib.rs:449-464,4437` |
| 14 | composite 合成优先级=组件数组顺序（隐式），无冲突报告 | Med | Workflow | `lib.rs:1403-1490` |
| 15 | cumulative_refine 默认生成 1..N 全层 mask，nml 数量可爆炸 | Med | Scale | `lib.rs:1543`, options |
| 16 | FVCOM regional 开边界静默跳过，用户拿不到 .2dm 无告警 | Med | UX/Data | `main.rs:1303-1305` |
| 17 | olam_delaunay 半径单测红（数值稳定性），细化前几何基线不稳 | Med | Numerical | 见 [01](./01_build_and_crate_audit.md)#1 |
| 18 | CLI 派发靠 namelist 内容隐式推断模式，难以预测走哪条分支 | Med | Workflow | `lib.rs:16436-16514` |
| 19 | 无输出 round-trip 校验（写出的 MPAS/FVCOM/nc 未读回验证） | Med | Quality | output 段无校验 |
| 20 | GUI 无法触发 hydro/MERIT/composite/coupling-package，能力割裂 | Med | GUI | GUI 仅 §1 主干 |

---

## 8. 哪些 Workflow 适合在 GUI 暴露

| Workflow | 现状 | 建议 | 理由 |
|----------|------|------|------|
| Gridinit / specified refine / 基础 output | 已暴露 | 保持 + 加质量卡片 | 高频、参数有界、可视化收益大 |
| Calculated refine 阈值 | 已暴露(参数) | 增加"阈值→物理过程→预期收益"解释面板 | 满足原则5 |
| MERIT-Hydro mask 生成 | 未暴露 | **应暴露**：选 MERIT root + bbox/polygon + 阈值 → 预览 river/coast 图层 | 空间交互天然适合 GUI |
| Hydro-close recipe / mask / composite | 未暴露 | **应暴露**（向导式）+ 预览 close mask 叠加 | 参数多、易错，需可视化 |
| Land-ocean coupling 预览 | 部分(CoLM 输出) | 暴露 land/ocean/coast fraction 热力图 + QA 卡片 | 守恒/分类需可视核验 |
| 输出 round-trip 校验报告 | 无 | 暴露"输出健康检查"面板 | 即时反馈 |
| Native OLAM 嵌套网格 | 仅文本粘贴(main.rs:3814) | 暂不深做 GUI 构建器（低频、专家用） | 成本高、用户少 |

---

## 9. 哪些 Workflow 应先改造成 Project Template

> "project template" = 一份可复制、自带默认 + schema + 校验的算例骨架（参考现有 `examples/merit_hydro/*/case.nml` + `*_or_manifest.json`）。

| 优先级 | Workflow | 模板化内容 | 理由 |
|--------|----------|-----------|------|
| P0 | **MERIT-Hydro 区域 hydro-close 算例** | case.nml + thresholds + recipe + delivery manifest schema | 已有雏形(gba/yangtze)，最复杂、最需复用 |
| P0 | **Land-ocean coupled (CoLM) 算例** | coupling 配置 + landtype + QA 期望值 | 占位实现需模板锁定契约 |
| P1 | **全球 atmosphere/land/ocean hex 基础算例** | 已有 `examples/default/*.nml`，补 schema/校验 | 入门模板 |
| P1 | **specified-region refine 算例** | bbox/circle/close 区域模板 + 质量期望 | 常用、参数有界 |
| P2 | **composite close mask 多源算例** | components[] 模板 + 优先级说明 | 高级用法，需规范 |

建议模板统一带：`schema_version`、输入清单、默认阈值、**期望质量指标/QA 门禁**、可选 `delivery_manifest`。

---

## 10. 哪些 Workflow 缺少质量报告

| Workflow | 质量报告现状 | 缺口 |
|----------|--------------|------|
| Mesh 生成(几何) | ✅ 3 阶段 quality NetCDF（before/after spring） | ★ 无阈值门禁、无 HTML、无 GUI 展示、指标含义未文档化 |
| GUI run | ❌ 仅 cell/vertex 计数 | ★ 无任何几何/拓扑质量指标（违原则5） |
| Specified/calculated refine | ❌ 无细化专属报告 | ★ 无 transition 平滑度、refine ratio、收益/成本报告 |
| MERIT-Hydro mask | ⚠ 仅 `summary.json`(计数) | ★ 无分类质量/覆盖率评估 |
| Hydro-close / composite | ⚠ Python `refinement_eval.py`/`refinement_sweep.py` | ★ **Rust 侧完全无** eval/ranking |
| Land-ocean coupling | ❌ 无 | ★ **无守恒校验、无 fraction 和=1、无陆海互补检查** |
| 模式输出(MPAS/FVCOM/CoLM/OLAM) | ❌ 无 | ★ 无输出结构/round-trip 校验报告 |
| HTML 可视报告 | ⚠ 仅 Python leaflet(`geojson_map.py`) | ★ Rust 侧无 HTML 报告能力 |

**总结**：除 mesh 几何质量有 NetCDF 输出外，**refine 收益评估、coupling 守恒校验、输出健康检查、统一 HTML 报告**四类质量报告在 Rust 侧均缺失；hydro-coast 的 eval/ranking/HTML 依赖 Python `util/`。

---

## 关键证据索引（file:line）

- GUI 编排：`gui/main.rs:1457-1497`(config), `1896-2019`(start_run), `2040-2139`(poll/cancel), `1200-1349`(produce_outputs), `698-996`(地图), `2969-2983`(仅计数无质量)
- CLI 派发：`cli/main.rs:158-234`; 调度 `cli/lib.rs:16436-16514`
- mesh pipeline：`mesh.rs:1209/1590/1796/253`(gridinit), `21567/22039`(refine LOP/length), `16238/16500`(spring), `18168/18701`(postproc); quality `cli/lib.rs:16049/16090/16128`
- hydro：`cli/lib.rs:731/804/864/889/949/1026/1035/1070-1072/1154/1284`
- coupling：`cli/lib.rs:28965/28992-29021/29038/29226/29311/29387`; geometry `geometry/lib.rs:114-210`; CaMa `cli/lib.rs:4571`
- Python 接缝：`util/v3_core/*`, `util/hydro_mesh/*`; delivery `examples/merit_hydro/gba/delivery_manifest.json`

*本报告所有结论基于实际源码与示例 manifest；未修改任何 `src/rust` 代码。Mermaid 图中红色=缺口、黄色=占位/弱、蓝色=Python 外部依赖。*
