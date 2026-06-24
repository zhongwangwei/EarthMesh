# 引擎侧设计：DataLayerConfig + 质量门禁输入（设计 + 风险评估）

> 聚焦两件事如何**真正落到引擎**：① 多图层数据输入 `DataLayerConfig`（替代单一 `landtype_file`）；② 可配置的**质量门禁输入**（当前 `QualityThresholds` 硬编码）。
> 基于真实代码流，不是纯愿景。对齐并细化已有提案 [03_config_schema_audit](../reviews/v3_mesh_audit/03_config_schema_audit.md)（ProjectConfig 蓝图）、[08_mesh_quality_metrics_design](../reviews/v3_mesh_audit/08_mesh_quality_metrics_design.md)。
> 证据级别：本文每条"现状"结论都有 `file:line`；设计部分为提案。

---

## 0. 结论先读

1. **好消息**：引擎的 namelist 解析器**宽容**——未知块/键静默忽略（`from_mkgrd_namelist` 末尾 `_ => {}`，`core/lib.rs:906`；`from_mkrefine` 同 `:1515`）。所以新增 `&datalayers` / `&quality` 块**天然向后兼容**，旧 `.nml` 和 `mkgrd.x` 不受影响。

2. **关键发现**：引擎其实**已经在读多个数据图层**——计算细化（`refine_cal`）路径从 `threshold_dir` 读 8 个独立 NetCDF（LAI/slope/k_s/SST/SSH/EKE/sea_slope/typhoon，`cli/lib.rs:7399`），`landtype_file` 只提供海陆+地类（`cli/lib.rs:17137`）。所以 `DataLayerConfig` **不是从零造数据通道，而是给已有的隐式文件流加一层声明式、强类型的前端**。这把风险从"高（改数据管线）"降到"中（加一层映射）"。

3. **质量门禁最易落地**：`compute(input, &QualityThresholds)`（`quality/lib.rs:258`）**已经接受阈值参数**，只是两个调用点都传 `::default()`（`cli/main.rs:84`、GUI `poll_run`）。把阈值从配置构造出来传进去，就完成 80%。

4. **唯一的全局抉择**：引擎当前**零 serde、全手写解析**（core/quality/cli 三个 Cargo.toml 都没有 serde）。新配置走 **(A) 扩展 namelist 块（无新依赖、与项目哲学一致）** 还是 **(B) serde + 独立 `earthmesh_project` crate（03 的推荐，YAML/JSON 友好）**——这是本设计最大的分叉点，§2 给出推荐：**分两步走，先 A 后 B**。

---

## 0.5 实施状态（已落地 vs 待办）

> 本节随实现更新。E1（可配质量门禁）与 E2（DataLayerConfig + 落位）已端到端打通；其余列为后续。

### ✅ 已落地

| 项 | 落点 | 内容 |
|----|------|------|
| **E1 质量门禁** core | `earthmesh_core`（`QualityNamelist`） | 7 阈值 + `on_violation` 的 `&quality` 块 `from/to_quality_namelist`（宽容、向后兼容）；`Default` 对齐 `QualityThresholds::default`；往返/缺块测试 |
| **E1** gui | `earthmesh_gui` | `quality_namelist()`/`apply_quality_namelist()`；保存/运行写 `&quality`；载入解析回门禁；门禁往返测试 |
| **E1** cli | `earthmesh_cli`（`--mesh-quality … [quality.nml]`） | 读 `&quality` → `QualityThresholds`；`on_violation=block` 且 Fail → **非零退出**（CI 门禁）；旧两参调用行为不变 |
| **E2 类型** core | `earthmesh_core` | `ThresholdVar`（`file_stem` 按附录 A 契约 + `is_two_layer`/`from_stem`）、`DataLayerRole`、`DataLayerConfig`、`DataLayersNamelist`（`&datalayers` 块） |
| **E2 lower** core | `earthmesh_core` | `ThresholdVar::switch_slot()`（索引映射）+ `DataLayersNamelist::lower_into(&mut EarthmeshConfig, &mut RefineConfig)`：设 `landtype_file`、翻 `refine_*` mean/std 开关、开 `refine_cal`、路由 `mask_refine_spc_fprefix`；`LowerReport`（含 stem 不符告警）；lower/告警测试 |
| **E2** gui | `earthmesh_gui` | `gui_layer_role`（landcover/lai/merit/cama 干净映射）、`data_layers_namelist()`、`apply_data_layers_namelist()`；run 时 `lower_into` 驱动 mkgrd/refine；保存/运行写 `&datalayers`；载入恢复 |
| **E2 落位** gui | `earthmesh_gui`（`stage_threshold_layers`） | `start_run` 时把启用的 threshold 图层源文件拷到 `<stage>/threshold/<stem>.nc` 并把 `refine.threshold_dir` 指过去 → 引擎读得到；落位测试 |

> 完整链路（以 LAI 为例）：GUI lai 图层(路径) → 启用 → `start_run` 落位到 `threshold/lai.nc` → `run_namelist` 的 `lower_into` 翻 `refine_lai_m/s`+开 `refine_cal`+设 `threshold_dir` → 引擎读 `threshold_dir/lai.nc`。

### ⏳ 待办

| 项 | 说明 |
|----|------|
| serde `ProjectConfig`（E5/B） | 独立 `earthmesh_project` crate + intent preset + criteria 插件 + YAML/JSON（03 蓝图）。现走 namelist 块（A 路线）已够 GUI 驱动 |
| 更多 threshold 层映射 | `gui_layer_role` 目前只映射 lai；slope_avg/sst/ssh/eke/sea_slope/soil(5 个二层场)/typhoon 待 GUI 暴露并加映射（`ThresholdVar` 与 `switch_slot` 已就绪，加一行即可） |
| skew / coupling 门禁 | `QualityThresholds` 暂无 skew/coupling 字段；需引擎新增对应 metric 才能真判定（GUI 那两个门禁现仅展示） |
| coupling 门禁（QualityConstraintConfig） | 03 的 per-metric 分类 gate（geometric/topological/numerical/coupling）尚未做；现用真实 7 字段 |
| km / ° → NXP 换算 | 引擎无公式；分辨率控件 km/° 仍为预留，仅 NXP 直驱 `mkgrd.nxp` |
| 纯 CLI 读 `&datalayers` | `mkgrd.x` 主流程目前**不** lower `&datalayers`（lower 在 GUI 侧，写出的 `.nml` 里 `&mkrefine` 已含翻好的开关）。纯命令行用户若要用数据图层声明，需在 cli 主流程加一步 `lower_into`（可复用 core 的 `DataLayersNamelist::lower_into`） |

---

## 1. 现状基线（grounded）

| 维度 | 现状 | 证据 |
|------|------|------|
| 引擎配置 | `EarthmeshConfig`(25 字段) + `RefineConfig`(~39 字段)，扁平 Fortran 风格 | `core/lib.rs:740` / `:1241` |
| 序列化 | 手写 `to_mkgrd_namelist`/`to_mkrefine_namelist` → `&mkgrd`/`&mkrefine` Fortran 块 | `:917` / `:1533` |
| 解析 | 手写 `from_*_namelist`，逐行 `NL%key = val`，**未知键静默忽略** | `:847`(`_=>{}`@906) / `:1332`(@1515) |
| 单一数据输入 | `landtype_file`：NetCDF，变量 `landtype`(i8)，维度 `360·gridnum × 180·gridnum`；供海陆掩膜+地类计数 | `cli/lib.rs:17137` |
| 判据数据（计算细化） | `threshold_dir` 下**每判据一个 NetCDF**（LAI/slope/k_s/SST/SSH/EKE/sea_slope/typhoon），由 `getref` 工具预算 | `RefineConfig:1247`, `cli/lib.rs:7399` |
| 判据数据（指定细化） | `mask_refine_spc_type`+`fprefix` → 直接读 NetCDF 掩膜 | — |
| 质量阈值 | `QualityThresholds`(7 字段，全 `pub`，`derive(Clone,Copy,Debug)`，**无 serde**) | `quality/lib.rs:154` |
| 质量计算 | `compute(&QualityMeshInput, &QualityThresholds) -> MeshQualityReport` | `quality/lib.rs:258` |
| 质量调用 | CLI + GUI 都传 `::default()`（**门禁不可配**） | `cli/main.rs:84` |
| 质量输出 | `io::write_all` 手写 JSON/CSV/GeoJSON/MD | `quality/io.rs:409` |
| serde | core/quality/cli **均无** serde 依赖 | 三处 `Cargo.toml` |
| 运行入口 | `run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(nml, ...) -> Report` | `cli/lib.rs:16830` |
| 测试 | core 41（含 `tests/namelist_roundtrip.rs` 6 个）、cli **630**、quality 6 | — |

**这意味着**：`landtype_file` 和 `threshold_dir` 合起来，已经是一个"隐式的多图层输入"——只是没有类型、没有 id、没有覆盖校验、没有"哪个判据用哪个文件"的显式声明。`DataLayerConfig` 要做的就是把这层隐式契约**显式化、类型化、可校验化**，并 lower 回这两个现有入口。

---

## 2. 两个前置决策

### 决策 A：配置载体 —— namelist 块 vs serde 新 crate

| 方案 | 做法 | 优点 | 代价/风险 |
|------|------|------|-----------|
| **A. 扩展 namelist** | 在现有 `.nml` 加 `&datalayers` / `&quality` 块，手写 parse/emit（照 `from_mkgrd_namelist` 模式） | 零新依赖；与项目哲学一致；宽容解析=自动向后兼容；走现有执行通道 | 块内多层结构（每图层一行）表达力弱；手写 parser 有维护成本；不适合深嵌套（criteria 树） |
| **B. serde + `earthmesh_project`** | 新 crate，`ProjectConfig` 强类型 + serde JSON/YAML，`lower()` 成现有 config+namelist（03 的 PATCH-S1/S3） | 表达力强（嵌套/enum/Option）；GUI/CLI 共享；03 已设计好 | 引入 serde+serde_json/serde_yaml 依赖（编译时间↑）；新 crate；与现有 namelist 双真相源需 `lower()` 统一 |

**推荐：分两步，先 A 后 B。**
- **第一步（A）**：用 namelist 块把 `DataLayerConfig`（扁平版）和 `QualityThresholds`（直接 7 字段）接进引擎——**这是"让 GUI 真正驱动引擎"的最短路径，零新依赖、零迁移**。GUI 已有的图层管理器/门禁字段直接 lower 成这两个块。
- **第二步（B）**：当需要 03 的完整 `ProjectConfig`（intent preset、criteria 插件、嵌套预算、YAML）时，再上 `earthmesh_project` + serde，把第一步的 namelist 块作为 `lower()` 的输出目标之一。第一步不会白做（它就是 L3 的一部分）。

> 理由：A 能在**不碰 `earthmesh_core` 数据结构、不加依赖**的前提下，先把"数据图层→引擎、门禁→判定"打通并回归验证；B 是更大的产品化投入，值得单独立项。下面的设计**两步都给**，但风险评估聚焦第一步（近期可落地）。

### 决策 B：边界

严守 audit 原则——**不改网格生成核心 / refinement 内核 / production geometry**。所有改动是**加法**：新增 parse/emit、新增 lower 映射、新增 CLI flag、改两个 `::default()` 调用点。引擎算法零改动 → 可逐字节回归。

---

## 3. Part I — DataLayerConfig 设计

### 3.1 类型（对齐 03 §3，但绑定真实数据流）

```rust
// 第一步(A)：扁平、可 namelist 化。放 earthmesh_core 或新 earthmesh_project。
pub struct DataLayerConfig {
    pub id: String,                 // "landcover" / "dem" / "merit" ... 被判据引用
    pub role: DataLayerRole,        // 决定 lower 到 landtype_file 还是 threshold_dir/某文件
    pub path: String,               // NetCDF / 目录
    pub var: Option<String>,        // NetCDF 变量名（默认按 role 推断，如 landcover→"landtype"）
    pub enabled: bool,
    pub required: bool,
}

pub enum DataLayerRole {           // ★ 关键：role 决定它喂引擎的哪个入口
    LandType,                       // → EarthmeshConfig.landtype_file（海陆+地类）
    ThresholdField(ThresholdVar),   // → threshold_dir/<name>.nc（计算细化判据数据）
    SpecifiedMask,                  // → mask_refine_spc_fprefix（指定细化掩膜）
    MeritHydroRoot,                 // → hydro 工作流（已有 CLI 通道）
    CamaReach,                      // → CaMa（已有）
}
pub enum ThresholdVar { Lai, Slope, Ks, KSolids, Tkdry, Tksatf, Tksatu, Sst, Ssh, Eke, SeaSlope, Typhoon }
```

> 第二步(B) 再升级到 03 的 `CriterionDataSource`(NetcdfVar/MeritHydroRoot/CamaReach/GeoJson/Constant) + criteria 引用 `data_layer_ids`。第一步用 `role` 枚举把"图层→引擎入口"显式化，足以驱动现有引擎。

### 3.2 lower：DataLayerConfig → 现有引擎入口（这是核心，零内核改动）

```
[DataLayerConfig...] --lower()-->  EarthmeshConfig.landtype_file   (role=LandType 的那个)
                                +  threshold_dir 布局 / 各 *.nc 路径 (role=ThresholdField)
                                +  mask_refine_spc_fprefix          (role=SpecifiedMask)
                                +  RefineConfig.refine_*[i] 开关     (按启用的 ThresholdField 置位)
```

要点：
- **landtype**：`role=LandType` 的图层 `path` → `EarthmeshConfig.landtype_file`（GUI 已做了这一步的雏形：地表覆盖行→`mkgrd.landtype_file`）。
- **threshold 判据**：每个启用的 `ThresholdField(var)` → 在 `threshold_dir` 期望一个对应文件（如 `Lai`→`threshold_dir/lai.nc`），并把 `RefineConfig` 里对应的 `refine_onelayer_lnd[i]`/`refine_onelayer_ocn[i]` 开关置 true。**这正是引擎已有的读取契约**（`cli/lib.rs:7399`）——我们只是用 `DataLayerConfig` 把"放哪个文件、开哪个判据"声明出来，而不再让用户手摆 `threshold_dir` + 手拨平行数组开关。
- **覆盖/单位校验**（V2，03 §8）：lower 前校验文件存在、维度匹配（`360·gridnum × 180·gridnum`，照 `cli/lib.rs:17165` 的检查）、变量存在；缺失则 `required` 图层报错、可选图层禁用对应判据并告警（**不静默细化**，对齐 GUI 原型的覆盖徽章）。

### 3.3 namelist 块（第一步 A 的落地形态）

新增 `&datalayers` 块（宽容解析自动兼容旧文件）：

```
&datalayers
  NL%LAYER(1)%ID = 'landcover'
  NL%LAYER(1)%ROLE = 'landtype'
  NL%LAYER(1)%PATH = './input/landtype_usgs.nc'
  NL%LAYER(1)%VAR  = 'landtype'
  NL%LAYER(1)%ENABLED = .true.
  NL%LAYER(2)%ID = 'lai'
  NL%LAYER(2)%ROLE = 'threshold:lai'
  NL%LAYER(2)%PATH = './threshold/lai.nc'
  NL%LAYER(2)%ENABLED = .true.
/
```

emit/parse 照 `to_/from_mkgrd_namelist` 同模式手写；roundtrip 测试扩到 `core/tests/namelist_roundtrip.rs`。

### 3.4 GUI 接线

GUI 已有的 8 图层管理器（`step_data_layers`，启用/路径/角色/覆盖）**正是 `DataLayerConfig` 的可视化**。接线：GUI 状态 `Vec<DataLayer>` → 构造 `Vec<DataLayerConfig>` → lower → 写进 run namelist（GUI 的 `run_namelist()` 多拼一个 `&datalayers` 块即可）。覆盖徽章直接复用 lower 的 V2 校验结果。

---

## 4. Part II — 质量门禁输入设计

### 4.1 现状最短路径

`compute()` 已接受 `&QualityThresholds`。要让门禁"可配 + 真生效"，分两层：

| 层 | 改动 | 影响 |
|----|------|------|
| **L-a 阈值可配** | 让 `QualityThresholds` 从配置/GUI 构造，替换两处 `::default()`（`cli/main.rs:84`、GUI `poll_run`） | 判定 verdict（Pass/Warn/Fail）随用户阈值变化。GUI 侧已做了 min_angle/area_ratio 的雏形 |
| **L-b 违规策略生效** | 新增 `on_violation: Warn\|Block`；`Block` 时 verdict=Fail → CLI 非零退出 / GUI 高亮阻断 | 让"严格模式"真正能拦住坏网格 |

### 4.2 类型

```rust
// 第一步：直接给真实的 QualityThresholds 加序列化能力 + 策略包装
pub struct QualityConfig {
    pub thresholds: QualityThresholds,   // 复用现有 7 字段
    pub on_violation: ViolationPolicy,   // Warn | Block
}
pub enum ViolationPolicy { Warn, Block }
```

> 第二步(B) 升级到 03 的 `QualityConstraintConfig`（按 geometric/topological/numerical/coupling 分类的 `Vec<QualityGate{metric,min,max}>`），与 [08](../reviews/v3_mesh_audit/08_mesh_quality_metrics_design.md) 的 `QualityMetricId` 对齐。第一步先用真实的 7 字段，最小且立刻可用。

### 4.3 序列化抉择（quality 侧）

`QualityThresholds` 无 serde。两个选项：
- **手写**：加 `&quality` namelist 块 + 手写 parse（与决策 A 一致，零依赖）。
- **serde**：给 `QualityThresholds` 加 `#[derive(Serialize, Deserialize)]`（需在 `earthmesh_quality/Cargo.toml` 加 serde）。**注意**：它当前是 `Copy` 的小结构体，加 serde derive 风险极低（纯加法，不改字段/语义）。

**推荐**：第一步随决策 A 走 `&quality` namelist 块（统一载体）；若团队倾向给 quality 单独出 JSON（便于 CI/报告），则给 `QualityThresholds` + `QualityConfig` 加 serde（局部、低风险）。

### 4.4 CLI / GUI 接线

- **CLI**：`mkgrd.x` 读 namelist 时一并解析 `&quality` 块 → 构造 `QualityConfig` → 传给 `compute()`；`Block` 且 Fail → 进程非零退出（CI 友好，对齐 03 §8 V4）。新增可选 `earthmesh validate`/`--quality-config` 入口（03 §7）。
- **GUI**：质量门禁那步（`step_quality`，已有 mode/gates/on_violation 状态）→ 构造 `QualityConfig` → 写进 run namelist 的 `&quality` 块；`poll_run` 里用它替换 `::default()`（GUI 已有雏形，补上 `&quality` 块的往返即闭环）。

---

## 5. 向后兼容

| 机制 | 保障 |
|------|------|
| 宽容解析 | 旧 `.nml`（无 `&datalayers`/`&quality`）照常解析，新块缺失=走默认（`landtype_file` 单图层 + `QualityThresholds::default()`）。`mkgrd.x` 行为零变化 |
| 加法而非替换 | `EarthmeshConfig`/`RefineConfig`/`QualityThresholds` 字段**不动**；新结构体/块是叠加 |
| lower 唯一真相源 | 项目模式下 namelist 由 lower 生成，消除双真相源（03 C12）；但直接喂旧 namelist 的老路径保留 |
| 逐字节回归 | 不启用新块时，lower 产物 == 现有 namelist；可 diff 比对（630 个 cli 测试是安全网） |

---

## 6. 分阶段实施（每步独立可回归）

| 阶段 | 内容 | 依赖 | 风险 |
|------|------|------|------|
| **E1** | `QualityConfig`(7 阈值+policy) + `&quality` 块 parse/emit；两处 `::default()` 改为读配置；`Block`→非零退出 | — | 低 |
| **E2** | `DataLayerConfig`(扁平+role) + `&datalayers` 块 + `lower()` 到 `landtype_file`/`threshold_dir`/refine 开关 + V2 校验 | — | 中 |
| **E3** | GUI 接线：图层管理器→`&datalayers`、门禁步→`&quality`（GUI 的 `run_namelist()` 多拼两块） | E1,E2 | 中 |
| **E4** | 扩 roundtrip 测试 + 注入越界/缺文件用例；CLI `validate` 子命令 | E1,E2 | 低 |
| **E5（可选，B）** | `earthmesh_project` crate + serde + `ProjectConfig.lower()`，把 E1/E2 的块作为 lower 目标；intent preset + criteria 插件（03 S1-S3） | E1-E4 | 高 |

> E1→E2→E3→E4 即可让"GUI 的数据图层 + 质量门禁真正驱动引擎"，全程不碰内核、不加依赖。E5 是产品化升级，单独立项。

---

## 7. 测试计划

| 测试 | 目的 | 落点 |
|------|------|------|
| `quality_block_roundtrip` | `&quality` parse→emit→parse 一致 | core/tests/namelist_roundtrip.rs |
| `datalayers_block_roundtrip` | `&datalayers` 往返一致（多图层、转义） | 同上 |
| `unknown_block_ignored` | 含 `&datalayers` 的新 nml 被旧解析路径无害忽略（兼容证明） | core 测试 |
| `lower_landtype_maps_to_config` | `role=LandType` → `landtype_file` 正确 | 新 lower 测试 |
| `lower_threshold_enables_refine_switch` | 启用 `ThresholdField(Lai)` → 对应 `refine_*[i]`=true 且期望 `threshold_dir/lai.nc` | 新 |
| `missing_required_layer_errors` | required 图层缺文件→报错并指明 id | 新 |
| `quality_block_drives_verdict` | 收紧 `min_angle_warn` 使同一网格从 Pass→Warn | quality/tests |
| `on_violation_block_nonzero_exit` | Block+Fail → CLI 退出码非零 | cli 测试 |
| 回归 | 不带新块时 lower 产物逐字节==旧 namelist | 对 examples/ 比对 |

---

## 8. 风险评估（核心）

| 改动 | 爆炸半径 | 向后兼容 | 测试负担 | 触碰 audit 边界? | 缓解 |
|------|----------|----------|----------|------------------|------|
| `&quality` 块 + 阈值可配（E1） | 小：2 个 `::default()` 调用点 + 1 个新块 | 安全（宽容解析） | 低（quality 仅 6 测试，易扩） | 否（不改算法，只改判定阈值来源） | 默认值=现有 `::default()`，缺块行为不变；Block 仅在显式开启时生效 |
| `&datalayers` 块 + lower（E2） | 中：新 parse/emit + lower 映射到既有入口 | 安全（缺块=单 landtype 旧行为） | **中高**：触及 cli 数据加载预期（630 测试） | 否（复用 `landtype_file`/`threshold_dir` 既有读取，不改读取逻辑本身） | lower 只是"摆文件+拨开关"，不改 NetCDF 读取代码；V2 校验前置拦截坏输入；逐字节回归 |
| 给 `QualityThresholds` 加 serde（若选 serde 路线） | 小：纯 derive | 安全 | 低 | 否 | 不改字段；可先不加，走 namelist |
| serde + `earthmesh_project`（E5/B） | **大**：新 crate + 新依赖 + lower 全链路 + GUI 重接 | 需 `lower()` 保证等价 | 高 | 否（新增层，内核不动） | 单独立项；E1-E4 已交付价值，E5 非阻塞 |
| `threshold_dir` 文件命名契约 | 中：lower 期望的文件名 vs `getref` 实际产出 | — | 中 | 否 | **✅ 已解（附录 A）**：命名常量 + `{name}.nc` + `{name}_l1/_l2` 已逐条确认，producer/consumer 共用 `area_judge_threshold_path` |
| 平行数组开关映射 | 中：`ThresholdVar`→`refine_*[i]` 索引必须与 `RefineConfig` 平行数组语义一致 | — | 中 | 否（03 C3 指出索引语义隐式易错） | 在 lower 里集中维护一张 `ThresholdVar↔index` 映射表 + 单测每条；避免分散硬编索引 |

**最高风险点**：
1. ~~`threshold_dir` 文件命名/变量约定~~ → **✅ 已解，见附录 A**（`{name}.nc`，单层 var=`{name}`、双层 `{name}_l1/_l2`；判据名常量 `cli/lib.rs:20724-26`）。
2. ~~`RefineConfig` 平行数组索引↔变量~~ → **✅ 已逐条确认，见附录 A 映射表**（`core/lib.rs:1404-1514`）。残留：双层 `th_twolayer_lnd:[[f64;2];10]` 的层(l1/l2)语义隐式（03 C3）→ 用集中映射表 + 单测覆盖（风险 中→低）。
3. 计算细化对 `gridnum_perdegree` 维度的硬性要求（`360·gridnum × 180·gridnum`）——图层维度不符会在引擎内部报错，V2 校验必须前置覆盖。（仍需在 lower 前做覆盖/维度校验。）

---

## 9. 待你拍板的决策点

1. **载体路线**：先走 namelist 块（A，零依赖、最快落地）？还是直接上 serde + `earthmesh_project`（B，产品化但投入大）？（推荐 A→B）
2. **serde 是否引入**：即便走 A，是否愿意给 `QualityThresholds` 加 serde 以便单独出 JSON 质量配置？（低风险，看是否需要）
3. ~~**`getref` 输出契约**~~ → **✅ 已确认并写入附录 A**（不再是未决项）。
4. **门禁范围**：第一步质量门禁先用真实的 7 个 `QualityThresholds` 字段，还是直接做 03/08 的完整 `QualityConstraintConfig`（per-metric 分类 gate）？（推荐先 7 字段）
5. **Block 行为**：`on_violation=Block` 时，是 CLI 非零退出 + GUI 阻断生成，还是仅标红不阻断？

---

## 附录 A — threshold_dir 数据契约（已确认，解掉 §8 最高风险点）

> 已通过源码调查精确定位消费端（`read_area_judge_threshold_inputs_fortran_indexed` 系列），且 producer（`getref`）用同一套命名函数 → 天然一致。这是 `DataLayerConfig.lower()` 必须满足的契约。

**判据名常量（权威命名源）**
- 陆面单层：`AREA_JUDGE_LAND_ONELAYER_NAMES = ["lai", "slope_avg"]`（`cli/lib.rs:20724`）
- 陆面双层：`AREA_JUDGE_LAND_TWOLAYER_NAMES = ["k_s", "k_solids", "tkdry", "tksatf", "tksatu"]`（`:20725`）
- 海洋单层：`AREA_JUDGE_OCEAN_ONELAYER_NAMES = ["sst", "ssh", "eke", "sea_slope"]`（`:20726`）
- 大气单层：`"typhoon"`

**文件 & 变量命名规则**（`area_judge_threshold_path`，`cli/lib.rs:20734`：`threshold_dir.join(format!("{name}.nc"))`）
- 路径：`{threshold_dir}/{name}.nc`
- 变量：单层 = `{name}`；双层 = `{name}_l1` / `{name}_l2`（`:20917` / `:20923`）
- 数据：f64、二维 (lon, lat)、跨 `360·gridnum × 180·gridnum`、按细化区裁剪读取
- 按 mesh_type 读：landmesh→陆面单层+双层；oceanmesh→海洋单层；atmos→typhoon；LOCmesh/earthmesh→全部

**判据 → 引擎索引映射**（核对 `core/lib.rs:1404-1514`）

| 判据（均值/标准差） | 文件 | NetCDF 变量 | 阈值数组[索引] | 开关数组[索引] |
|------|------|-------------|----------------|----------------|
| LAI | `lai.nc` | `lai` | th_onelayer_lnd[0]/[1] | refine_onelayer_lnd[0]/[1] |
| 坡度 | `slope_avg.nc` | `slope_avg` | th_onelayer_lnd[2]/[3] | refine_onelayer_lnd[2]/[3] |
| k_s | `k_s.nc` | `k_s_l1`,`k_s_l2` | th_twolayer_lnd[0]/[1] | refine_twolayer_lnd[0]/[1] |
| k_solids | `k_solids.nc` | `k_solids_l1/_l2` | th_twolayer_lnd[2]/[3] | refine_twolayer_lnd[2]/[3] |
| tkdry | `tkdry.nc` | `tkdry_l1/_l2` | th_twolayer_lnd[4]/[5] | refine_twolayer_lnd[4]/[5] |
| tksatf | `tksatf.nc` | `tksatf_l1/_l2` | th_twolayer_lnd[6]/[7] | refine_twolayer_lnd[6]/[7] |
| tksatu | `tksatu.nc` | `tksatu_l1/_l2` | th_twolayer_lnd[8]/[9] | refine_twolayer_lnd[8]/[9] |
| SST | `sst.nc` | `sst` | th_onelayer_ocn[0]/[1] | refine_onelayer_ocn[0]/[1] |
| SSH | `ssh.nc` | `ssh` | th_onelayer_ocn[2]/[3] | refine_onelayer_ocn[2]/[3] |
| EKE | `eke.nc` | `eke` | th_onelayer_ocn[4]/[5] | refine_onelayer_ocn[4]/[5] |
| 海底坡度 | `sea_slope.nc` | `sea_slope` | th_onelayer_ocn[6]/[7] | refine_onelayer_ocn[6]/[7] |
| 台风 | `typhoon.nc` | `typhoon` | th_onelayer_atmos[0]/[1] | refine_onelayer_atmos[0]/[1] |

**开关启用逻辑**：`area_judge_refine_flag_pair_enabled(flags, i) = flags[2i] || flags[2i+1]`（`cli/lib.rs:20729`）——某判据文件只要 mean 或 std 开关之一为真就读。lower 时：启用某 `ThresholdField(var)` → 置位对应 mean/std 开关。

**双层阈值结构（残留小风险，03 C3）**：`th_twolayer_lnd: [[f64; 2]; 10]`（`core/lib.rs:1267`，默认 `[[999.0;2];10]`@1311）——10 个 (变量,统计量) 条目，每条目 `[f64;2]` 是**两个土壤层 (l1, l2) 的阈值**；开关 `refine_twolayer_lnd: [bool;10]` 是扁平的 (变量×{均值,标准差})。索引语义隐式，**lower 里务必用一张集中映射表 + 逐条单测**，不要分散硬编。

**不在 threshold_dir 的三项**：`th_num_landtypes` / `th_area_mainland` / `th_sea_ratio` 从 `&mkrefine` namelist 直接读（`core/lib.rs:1456-1457,1502-1503`），数据来自 `landtype_global`（即 `landtype_file`），**不生成 threshold 文件**。

> **风险降级**：§8 "最高风险点 #1（命名契约）"——**已解**，照本附录实现即可。残留仅双层阈值索引语义（中→低，集中映射表+单测覆盖）。§9 决策点 #3——**已关闭**。

---

*本文为引擎侧设计提案；所有"现状"结论基于实际源码 file:line（附录 A 锚点已二次核验）。未修改任何 `src/rust` 代码。配套：GUI 侧已落地的数据图层管理器/质量门禁/分辨率单位见本目录 `GUI_REDESIGN_SUGGESTIONS.md` 与 `prototype.html`。*
