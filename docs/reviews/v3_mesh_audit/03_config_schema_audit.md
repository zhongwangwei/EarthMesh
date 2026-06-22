# 03 — Config System & Project Schema Audit (EarthMesh v3)

> Phase P5/P6 衔接（提案阶段，可提 patch，不落地）· 未修改任何 `src/rust` 代码
> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md) · 上游：[01_build_and_crate_audit.md](./01_build_and_crate_audit.md) · [02_workflow_consistency_audit.md](./02_workflow_consistency_audit.md)
> 审查对象：EarthMesh v3，分支 `v3.0.0-alpha1`，仅当前项目，不引用任何旧版本。
> 证据：`earthmesh_core/src/lib.rs:735`(EarthmeshConfig, 25 字段)、`:1236`(RefineConfig, 39 字段)、`earthmesh_gui/src/main.rs`(GUI state)、`earthmesh_cli/src/main.rs`(CLI flags)、`examples/`、`delivery_manifest.json`。
> 日期：2026-06-22 · 证据等级见 AUDIT_PRINCIPLES §5。

---

## 0. 核心结论（先读）

当前配置 = **64 个扁平的、Fortran 命名的、引擎级字段**（`EarthmeshConfig` 25 + `RefineConfig` 39），直接从 namelist 1:1 迁移。它**没有"项目 / 意图"层**：用户被迫直接设定 `nxp`、`beta`、`relax`、`niter`、`halo[10]`、`max_transition_row[10]`、`spring_global_type`、`vertex_pretect_layers`、`num_rc`，并用**平行数组 + 布尔开关**（`th_onelayer_lnd[4]` / `refine_onelayer_lnd[4]`，索引语义隐式）表达细化判据。

> 三个核心问题的回答：
> **(1) 是否过于底层？** 是（A 级）——配置即引擎内部状态，无抽象层。
> **(2) GUI 是否暴露太多 engine 参数？** 是（A 级）——HALO/transition/spring/vertex_pretect_layers/num_rc 全在 GUI Advanced tab 直接暴露（见 [02](./02_workflow_consistency_audit.md) GUI 走查）。
> **(3) 用户是否必须理解 NXP/HALO/transition/smoothing/spring？** 当前是（必须）；**本提案的目标是让普通用户不必懂**——通过 intent→preset→自动推导 engine 参数。
>
> 同时回答 (4)(5)(6)(7)：**需要**引入 12 个高层 config（§2/§3）；**应**支持 YAML/JSON project 文件（§4/§5）；**应**分四层（friendly / expert / execution-plan / reproducibility，§2.2）；通过 preset + plugin criteria **能**表达全部 11 种 mesh 意图（§2.3）。

---

## 1. Current Config Risks

| # | 风险 | 证据 | 严重度 | 违背原则 |
|---|------|------|--------|----------|
| C1 | 64 字段全扁平、无层次、无"项目"概念；experiment_name/base_dir 与 nxp/spring 同级 | `core/lib.rs:735-759,1236-1276` | High | 5 |
| C2 | 意图参数与引擎参数混杂：`mesh_type/domain` 与 `beta/relax/niter/halo/spring_*` 同层 | 同上 | High | 5 |
| C3 | 阈值用平行数组 + 布尔开关，index→变量映射隐式，极易错配 | `th_onelayer_lnd[4]`,`th_twolayer_lnd[10][2]`,`refine_onelayer_lnd[4]` `core/lib.rs:1259-1274` | High | 1,2 |
| C4 | 字符串当 enum，无类型安全（拼写错=运行期错误） | `mode_grid/mask_refine_spc_type/refine_setting/output_format` String | Med | — |
| C5 | 用哨兵值代替 Option：`"/tmp"` 表"未设"、`999.0` 表"阈值禁用" | `core/lib.rs:1281-1306` | Med | — |
| C6 | Default 不安全/跨平台：`base_dir:" /tmp"`(前导空格)、`/tmp` POSIX-only | `core/lib.rs:766-790`（见 [01](./01_build_and_crate_audit.md)#4） | Med | 4 |
| C7 | GUI 暴露几乎全部 64 字段（含 halo/transition/spring/num_rc/vertex_pretect_layers） | `gui/main.rs:3711-3861` | High | 5 |
| C8 | 无 ProjectConfig：case = `.nml` + 旁挂 `*_or_manifest.json` + `delivery_manifest.json`，多文件无统一入口 | `examples/merit_hydro/*/` | High | 5 |
| C9 | 无 ReproducibilityManifest：输入数据版本/hash、工具版本、seed、阈值快照均未记录；manifest 仅列输出路径 | `gba/delivery_manifest.json` | High | 3,4 |
| C10 | 无 QualityConstraintConfig：质量门禁/约束无法声明（质量只产出 NetCDF，不可配） | 见 [02](./02_workflow_consistency_audit.md)§10 | High | 4 |
| C11 | 意图无法表达：hydrology/carbon/snow land mesh 只能手工切阈值开关，无"目标"概念 | RefineConfig 仅有原始开关 | High | 1,2 |
| C12 | 配置真相源分裂：CLI 互斥 flags（`--run-mask-restart-*`）+ namelist 内容隐式派发并存 | `cli/main.rs:75-93`,`lib.rs:16436` | Med | — |
| C13 | hydro-coast / coupling 配置散落多文件多 schema（recipe.json / case.nml / manifest） | 见 [02](./02_workflow_consistency_audit.md)§0 | Med | 5 |

---

## 2. Proposed v3 Project Schema

### 2.1 总体结构（单一 `project.yaml`/`project.json` 入口）

```
ProjectConfig                      # 顶层：一个 project = 一次可复现的网格生产
├── metadata        ProjectMetadata        # name / version / authors / created
├── domain          DomainConfig           # global | regional(bbox/circle/close/lambert)
├── targets       [ MeshTargetConfig ]      # land / ocean / atmosphere / coupled，每个带 intent preset
├── data_layers   [ DataLayerConfig ]       # landtype / MERIT-Hydro / SST / LAI ... 声明式数据源
├── refinement      RefinementRecipe        # criteria 组合 + 全局预算（cell budget / min edge）
│     └── criteria [ CriterionConfig ]      # 每条引用一个 plugin RefinementCriterion
├── hydro_coast?    HydroCoastConfig        # MERIT-Hydro/CaMa 河网海岸（可选）
├── coupling?       CoupledMeshConfig       # land-ocean 守恒耦合（可选）
├── quality         QualityConstraintConfig # 几何/拓扑/数值/耦合门禁（可声明、可阻断）
├── output          OutputConfig            # 目标模式格式 + 报告 + 打包
├── gui_session?    GuiSessionConfig        # GUI 视图/向导进度（不影响结果）
├── expert?         ExpertOverrides         # ★ engine 旋钮（nxp/halo/spring...）显式可选覆盖
└── reproducibility ReproducibilityManifest # 自动生成：输入 hash/工具版本/seed/lowered plan
```

### 2.2 四层分离（回答核心问题 6）

| 层 | 名称 | 谁写 | 内容 | 落点 |
|----|------|------|------|------|
| L1 | **User-friendly (intent)** | 普通用户/GUI 向导 | domain + target preset + 数据 + 质量目标 | `ProjectConfig` 主体 |
| L2 | **Expert (engine overrides)** | 高级用户 | 显式覆盖 nxp/halo/transition/spring 等 | `ProjectConfig.expert`（全 `Option`，默认空） |
| L3 | **Engine execution plan** | 引擎自动推导（只读产物） | 由 L1+L2 lower 出的 `EarthmeshConfig`+`RefineConfig`+namelist | `EngineExecutionPlan`（不手写） |
| L4 | **Reproducibility manifest** | 引擎自动生成 | 输入 hash、工具版本、seed、L3 快照、质量结果 | `reproducibility` + `delivery_manifest` 扩展 |

> 关键：用户只写 L1（+ 可选 L2）。L3/L4 由引擎产出，**保证"友好"与"可复现/专家可控"不互斥**。L3 直接复用现有 `EarthmeshConfig`/`RefineConfig`，故**零迁移**（§9）。

### 2.3 11 种 mesh 意图 → preset 映射（回答核心问题 7）

每种意图 = 一个 `MeshIntentPreset`：选定主物理过程、默认启用的 criteria（plugin）、主数据层、质量侧重。

| Intent preset | 目标域 | 默认 criteria（plugin id） | 主数据层 | 质量侧重 |
|---------------|--------|---------------------------|----------|----------|
| `hydrology_land` | land | river_network, slope, drainage_density | MERIT-Hydro, DEM | 河网连通、min-edge |
| `carbon_land` | land | lai_heterogeneity, landtype_diversity, soil_carbon | LAI, IGBP, soil | landtype 边界保真 |
| `snow_permafrost_land` | land | elevation_band, slope, soil_thermal(tkdry/tksat) | DEM, soil | 高程带过渡平滑 |
| `urban_land` | land | landtype_urban_fraction, impervious | IGBP/urban | 城市边界锐度 |
| `coastal_ocean` | ocean | coastline_proximity, bathymetry_gradient(seaslope) | coastline, bathy | 岸线贴合、CFL |
| `estuary` | ocean/coupled | river_mouth(CaMa is_estuary), coastline, salinity_front | CaMa, coastline | 河口分辨率 |
| `river_network` | land | river_network(R2/R3), drainage | MERIT-Hydro | 河道宽度匹配 |
| `merit_hydro_coast` | coupled | river_network + coastline(MERIT classify) | MERIT-Hydro | 见 [02](./02_workflow_consistency_audit.md)§5 |
| `land_ocean_coupled` | coupled | land_ocean_fraction(守恒), coastline | landtype+bathy | **守恒 fraction 校验** |
| `atmosphere_typhoon_precip` | atmosphere | typhoon_track, precip_gradient | typhoon clim, precip | 路径带细化、平滑过渡 |
| `multi_objective_balanced` | any | 多 criteria 加权（`CriterionWeight`） | 多源 | 多目标 Pareto + 预算 |

> 表达力来自 **plugin criteria + 加权**：新意图 = 新 preset（一组 criteria + 权重 + 质量约束），无需改引擎内核。

---

## 3. Rust Type Sketches

> 以下为 sketch（提案，未落地）。设计目标：强类型 enum 替代字符串、`Option` 替代哨兵、强类型阈值（值+单位+方向）、criteria 插件化、L1↔L3 可 lower。放在新库 crate `earthmesh_project`（依赖 `earthmesh_core`，被 cli/gui 共用），避免污染现有 64 字段。

```rust
// ---------- 顶层 ----------
pub struct ProjectConfig {
    pub schema_version: SemVer,            // 例 "3.0.0" —— 兼容性锚点
    pub metadata: ProjectMetadata,
    pub domain: DomainConfig,
    pub targets: Vec<MeshTargetConfig>,
    pub data_layers: Vec<DataLayerConfig>,
    pub refinement: RefinementRecipe,
    pub hydro_coast: Option<HydroCoastConfig>,
    pub coupling: Option<CoupledMeshConfig>,
    pub quality: QualityConstraintConfig,
    pub output: OutputConfig,
    pub gui_session: Option<GuiSessionConfig>,
    pub expert: ExpertOverrides,           // 全 Option，默认空 = 全自动
    // reproducibility 不手写，由引擎在 lower 后填充
}

pub struct ProjectMetadata { pub name: String, pub authors: Vec<String>, pub description: String, pub created_utc: Option<String> }

// ---------- Domain ----------
pub enum DomainConfig {
    Global,
    Regional { shape: RegionShape, sea_ratio: Option<f64> },
}
pub enum RegionShape {
    Bbox { w: f64, e: f64, n: f64, s: f64 },
    Circle { lon: f64, lat: f64, radius_km: f64 },
    Close { ring: Vec<LonLat> },
    Lambert { /* ... */ },
}

// ---------- Mesh target (含 intent preset) ----------
pub struct MeshTargetConfig {
    pub kind: MeshDomainKind,              // Land | Ocean | Atmosphere | Coupled
    pub cell: MeshCellKind,                // Hex | Tri  (替代 mode_grid 字符串)
    pub intent: MeshIntentPreset,          // §2.3 的 11 种之一 + Custom
    pub base_resolution: ResolutionSpec,   // 用 km 或 NXP；NXP 仅 expert
    pub model_format: ModelFormat,         // CoLM | MPAS | MPASSimple | FVCOM | OLAM | Native
}
pub enum ResolutionSpec { ApproxKm(f64), Nxp(i32) }   // 友好层用 km，引擎 lower 成 NXP
pub enum MeshIntentPreset {
    HydrologyLand, CarbonLand, SnowPermafrostLand, UrbanLand,
    CoastalOcean, Estuary, RiverNetwork, MeritHydroCoast,
    LandOceanCoupled, AtmosphereTyphoonPrecip, MultiObjectiveBalanced,
    Custom,
}

// ---------- Data layer（声明式数据源） ----------
pub struct DataLayerConfig {
    pub id: String,                        // 被 CriterionConfig 引用
    pub source: CriterionDataSource,
    pub required: bool,
}

// ---------- Refinement recipe + 预算（原则3） ----------
pub struct RefinementRecipe {
    pub criteria: Vec<CriterionConfig>,
    pub max_passes: u8,
    pub budget: RefinementBudget,          // ★ 成本上限：当前缺失
    pub transition: TransitionPolicy,      // 替代 halo[10]/max_transition_row[10]
}
pub struct RefinementBudget {
    pub max_cells: Option<u64>,
    pub min_edge_km: Option<f64>,          // CFL/时间步约束
    pub max_refine_ratio: Option<f64>,     // 相邻层级平滑过渡
}
pub struct TransitionPolicy { pub auto: bool, pub halo_override: Option<Vec<u8>> }

// ---------- 每条 criterion 的实例配置 ----------
pub struct CriterionConfig {
    pub criterion_id: String,              // 对应注册的 plugin
    pub enabled: bool,
    pub weight: CriterionWeight,
    pub threshold: CriterionThreshold,
    pub data_layer_ids: Vec<String>,       // 引用 data_layers
}

// ================= Plugin-style criteria =================
pub trait RefinementCriterion: Send + Sync {
    fn metadata(&self) -> CriterionMetadata;
    fn required_data(&self) -> Vec<CriterionDataSource>;
    /// 给定一个 cell/邻域窗口的输入，返回归一化细化需求 + 可解释原因（原则5）
    fn score(&self, input: &CriterionInput, th: &CriterionThreshold) -> CriterionScore;
    /// GUI 如何呈现本判据（控件/单位/帮助）—— 满足原则5
    fn gui_spec(&self) -> CriterionGuiSpec;
    /// 细化本判据预计对哪些质量指标产生何种影响（用于收益评估，原则3）
    fn quality_contribution(&self) -> CriterionQualityContribution;
}

pub struct CriterionMetadata {
    pub id: String,                        // "river_network"
    pub display_name: String,
    pub physical_process: String,          // "river routing / 汇流" —— 强制声明（原则2）
    pub applicable_domains: Vec<MeshDomainKind>,
    pub version: SemVer,
}
pub struct CriterionInput<'a> {
    pub cell_center: LonLat,
    pub cell_area_m2: f64,
    pub neighbors: &'a [LonLat],
    pub samples: &'a FieldSamples,         // 来自 data_layers 的多字段采样/窗口
}
pub struct CriterionScore {
    pub demand: f64,                       // 0..1 细化需求强度
    pub confidence: f64,                   // 0..1 数据可信度
    pub reason: String,                    // 人类可读："wth=420m≥R3" —— 原则5 可追溯
}
pub enum CriterionWeight { Off, Fixed(f64), Adaptive { base: f64, cap: f64 } }
pub struct CriterionThreshold {
    pub value: f64,
    pub unit: String,                      // "m" / "km2" / "fraction"
    pub aggregation: Aggregation,          // Mean | Std | Gradient | Quantile(f64)
    pub direction: Compare,                // GreaterEq | LessEq
}
pub enum CriterionDataSource {
    NetcdfVar { path: PathBuf, var: String, stride: Option<u32> },
    MeritHydroRoot { path: PathBuf, stride: Option<u32> },
    CamaReach { path: PathBuf },
    GeoJson { path: PathBuf },
    Constant(f64),
}
pub struct CriterionGuiSpec {
    pub label: String, pub help: String, pub unit: String,
    pub range: (f64, f64), pub default: f64, pub advanced: bool,
}
pub struct CriterionQualityContribution {
    pub improves: Vec<QualityMetricId>,    // 如 RiverConnectivity, CoastFidelity
    pub may_degrade: Vec<QualityMetricId>, // 如 MinEdge (变小→CFL 风险)
}

// ---------- hydro / coupling / quality / output ----------
pub struct HydroCoastConfig {              // 见 02 §5
    pub merit_root: PathBuf,
    pub selection: TileSelection,          // Bbox | Polygon ★(当前仅 Bbox)
    pub thresholds: MeritMaskThresholds,   // 复用现有
    pub composite: Option<CompositeRecipe>,
}
pub struct CoupledMeshConfig {             // 见 02 §4 缺口
    pub fraction_method: FractionMethod,   // PointSample | ConservativeOverlay ★(默认应 Conservative)
    pub identify_coastline: bool,
    pub identify_river_mouth: bool,        // 接 CaMa is_estuary
    pub colm: Option<ColmCouplingExport>,
}
pub enum FractionMethod { PointSample, ConservativeOverlay }  // 后者用 geometry::overlay_cell
pub struct QualityConstraintConfig {
    pub geometric: Vec<QualityGate>,       // 长宽比/最小角/well-centered
    pub topological: Vec<QualityGate>,     // 闭合/邻接一致(Method-C)
    pub numerical: Vec<QualityGate>,       // min-edge / refine ratio
    pub coupling: Vec<QualityGate>,        // fraction 和=1 / 互补
    pub on_violation: ViolationPolicy,     // Warn | Block
}
pub struct QualityGate { pub metric: QualityMetricId, pub min: Option<f64>, pub max: Option<f64> }
pub enum ViolationPolicy { Warn, Block }
pub struct OutputConfig {
    pub formats: Vec<ModelFormat>,
    pub reports: ReportConfig,             // 是否出 quality NetCDF / HTML / eval / ranking
    pub package: bool,                     // 生成 delivery manifest
}
pub struct ReportConfig { pub quality_netcdf: bool, pub html: bool, pub eval: bool, pub ranking: bool }

// ---------- expert overrides（L2，全 Option） ----------
pub struct ExpertOverrides {
    pub nxp: Option<i32>,
    pub niter: Option<i32>,
    pub beta: Option<f32>,
    pub relax: Option<f32>,
    pub spring_global_type: Option<i32>,
    pub spring_regional_type: Option<i32>,
    pub vertex_pretect_layers: Option<i32>,
    pub num_rc: Option<i32>,
    pub halo: Option<Vec<u8>>,
    pub max_transition_row: Option<Vec<u8>>,
}

// ---------- reproducibility（L4，自动生成） ----------
pub struct ReproducibilityManifest {
    pub tool_version: String,              // earthmesh + crate 版本
    pub schema_version: SemVer,
    pub inputs: Vec<InputFingerprint>,     // path + sha256 + bytes
    pub random_seed: Option<u64>,
    pub lowered_plan: EngineExecutionPlan, // L3 快照（namelist 等价物）
    pub quality_results: Vec<QualityResult>,
}
pub struct InputFingerprint { pub path: PathBuf, pub sha256: String, pub bytes: u64 }

// ---------- L1+L2 → L3 lowering ----------
pub struct EngineExecutionPlan {
    pub mkgrd: earthmesh_core::EarthmeshConfig,   // ★ 复用现有 → 零迁移
    pub refine: earthmesh_core::RefineConfig,
    pub generated_namelist: String,
    pub generated_masks: Vec<PathBuf>,
}
impl ProjectConfig {
    pub fn lower(&self) -> Result<EngineExecutionPlan, ConfigError> { /* preset→criteria→阈值数组+namelist */ }
}
```

---

## 4. JSON Example（hydrology-focused MERIT-Hydro land-ocean coupled project）

```json
{
  "schema_version": "3.0.0",
  "metadata": { "name": "gba_hydro_coupled", "authors": ["SYSU"], "description": "GBA 河网+海岸 陆海耦合网格" },
  "domain": { "type": "Regional", "shape": { "Bbox": { "w": 112.0, "e": 115.5, "n": 23.5, "s": 21.5 } }, "sea_ratio": 0.5 },
  "targets": [
    { "kind": "Coupled", "cell": "Hex", "intent": "MeritHydroCoast",
      "base_resolution": { "ApproxKm": 12.0 }, "model_format": "CoLM" }
  ],
  "data_layers": [
    { "id": "merit", "source": { "MeritHydroRoot": { "path": "/data/merit_hydro", "stride": 5 } }, "required": true },
    { "id": "landtype", "source": { "NetcdfVar": { "path": "./input/landtype_usgs.nc", "var": "landtype" } }, "required": true }
  ],
  "refinement": {
    "max_passes": 3,
    "budget": { "max_cells": 200000, "min_edge_km": 3.0, "max_refine_ratio": 2.0 },
    "transition": { "auto": true, "halo_override": null },
    "criteria": [
      { "criterion_id": "river_network", "enabled": true, "weight": { "Fixed": 1.0 },
        "threshold": { "value": 300.0, "unit": "m", "aggregation": "Mean", "direction": "GreaterEq" },
        "data_layer_ids": ["merit"] },
      { "criterion_id": "coastline_proximity", "enabled": true, "weight": { "Fixed": 0.8 },
        "threshold": { "value": 0.0, "unit": "fraction", "aggregation": "Mean", "direction": "GreaterEq" },
        "data_layer_ids": ["merit", "landtype"] }
    ]
  },
  "hydro_coast": { "merit_root": "/data/merit_hydro", "selection": { "Bbox": {} },
    "thresholds": { "r3_width_m": 300, "r2_width_m": 50, "r3_upa_km2": 50000, "r2_upa_km2": 5000 } },
  "coupling": { "fraction_method": "ConservativeOverlay", "identify_coastline": true, "identify_river_mouth": true,
    "colm": { "restart_template": true, "forcing_template": true } },
  "quality": {
    "geometric": [ { "metric": "MinAngleDeg", "min": 25.0, "max": null } ],
    "numerical": [ { "metric": "MinEdgeKm", "min": 3.0, "max": null } ],
    "coupling": [ { "metric": "FractionSumError", "min": null, "max": 1e-6 } ],
    "on_violation": "Block"
  },
  "output": { "formats": ["CoLM"], "reports": { "quality_netcdf": true, "html": true, "eval": true, "ranking": false }, "package": true },
  "expert": {}
}
```

---

## 5. YAML Example（atmosphere typhoon/precipitation，展示不同 preset + expert 覆盖）

```yaml
schema_version: "3.0.0"
metadata:
  name: wnp_typhoon_atmos
  authors: [SYSU]
  description: 西北太平洋 台风路径 大气网格
domain:
  type: Global
targets:
  - kind: Atmosphere
    cell: Hex
    intent: AtmosphereTyphoonPrecip
    base_resolution: { ApproxKm: 50.0 }   # 友好层用 km；引擎 lower 成 NXP
    model_format: MPAS
data_layers:
  - id: typhoon_clim
    source: { NetcdfVar: { path: /data/typhoon_track_density.nc, var: track_density } }
    required: true
refinement:
  max_passes: 2
  budget: { max_cells: 500000, min_edge_km: 15.0, max_refine_ratio: 2.0 }
  transition: { auto: true }
  criteria:
    - criterion_id: typhoon_track
      enabled: true
      weight: { Adaptive: { base: 1.0, cap: 2.0 } }
      threshold: { value: 0.2, unit: fraction, aggregation: Quantile(0.9), direction: GreaterEq }
      data_layer_ids: [typhoon_clim]
quality:
  geometric: [ { metric: MinAngleDeg, min: 25.0 } ]
  numerical: [ { metric: MaxRefineRatio, max: 2.0 } ]
  on_violation: Warn
output:
  formats: [MPAS]
  reports: { quality_netcdf: true, html: false, eval: true, ranking: false }
  package: true
expert:                        # L2：仅当需要时显式覆盖引擎旋钮
  niter: 500
  spring_global_type: 1
  halo: [4, 4, 3]
```

---

## 6. GUI Mapping（三档渐进式 + criteria 自描述）

| GUI 模式 | 暴露内容 | 对应 schema 层 | 谁用 |
|----------|----------|----------------|------|
| **Guided（向导）** | 选 intent preset → 选 domain → 选数据 → 选质量目标；engine 全自动 | L1（preset + domain + data + quality） | 普通用户 |
| **Standard** | 上 + 逐 criterion 开关/阈值（由 `CriterionGuiSpec` 自动渲染：label/单位/范围/帮助） | L1 criteria | 领域用户 |
| **Expert** | 上 + `ExpertOverrides`（nxp/halo/transition/spring/num_rc...）折叠区，明确标注"高级，可不填" | L2 | 专家 |

要点（回应核心问题 2/3）：
- **engine 参数从默认界面消失**，移入 Expert 折叠区且全部可空 → 普通用户**不必理解** NXP/HALO/transition/spring。
- 每条 criterion 的 UI **由 `CriterionGuiSpec` 自动生成**（不再手写 64 个控件）→ 新增 criterion 即自动获得 GUI，且自带 `physical_process` 说明与 `quality_contribution`（满足原则 5："为什么细化"+"细化后质量影响"）。
- 质量结果以 before/after 卡片展示（接 §3 `QualityResult`），补上 [02](./02_workflow_consistency_audit.md)§10 的 GUI 质量报告缺口。

---

## 7. CLI Mapping

| 命令（提案） | 作用 | 说明 |
|--------------|------|------|
| `earthmesh run project.yaml` | 跑完整 project | L1/L2 → lower → 执行 |
| `earthmesh plan project.yaml` | 只 lower，输出 `EngineExecutionPlan`（namelist 等价物），不执行 | dry-run / 审计 |
| `earthmesh explain project.yaml` | 打印每条 criterion 的 `metadata.physical_process` 与预期质量贡献 | 满足原则 2/5 |
| `earthmesh validate project.yaml` | 仅校验（§8） | CI 友好 |
| `earthmesh import legacy.nml -o project.yaml` | 把现有 `.nml` 反向导入为 L2/expert | 兼容（§9） |
| `mkgrd.x <legacy.nml>` | **保持不变** | 旧路径继续可用 |

> 与现状的关系：现有 namelist 派发（`cli/main.rs:75-93`）**保留**为底层执行通道；新 CLI 在其上加一层 project→plan。消除 C12（双真相源）的做法：project 模式下 namelist 由 `lower()` 唯一生成，不再手写。

---

## 8. Validation Strategy

分四级，逐级阻断（对应 AUDIT_PRINCIPLES 五原则）：

| 级别 | 校验 | 例子 | 失败动作 |
|------|------|------|----------|
| V1 结构 | schema 反序列化 + `schema_version` 兼容 | 字段缺失/类型错/未知 enum | 拒绝加载 |
| V2 语义/引用完整性 | criterion 引用的 `data_layer_id` 存在；数据文件可读；preset↔domain 相容 | river_network 用在 Atmosphere | 报错并指明字段 |
| V3 物理一致性（原则 1-3） | 每条启用 criterion 必须有 `physical_process` + 关联 data layer；预算非空 | 启用细化但无 `budget.max_cells` | 警告/可配为阻断 |
| V4 质量门禁（原则 4） | 运行后用 `QualityConstraintConfig` 判定，`on_violation=Block` 则失败退出 | 耦合 fraction 和≠1 超容差 | Block |

补充：criterion 插件可自带 `self-check`（`required_data` 与 `threshold.unit` 一致性）；`validate` 子命令在 CI 跑，零数据也能过 V1/V2。

---

## 9. Migration-free Compatibility Strategy（v3 内部，无破坏）

> 目标：引入 ProjectConfig **不动现有 64 字段、不动现有 308 个 cli pub fn、不动 GUI 现状**，新旧并存。

1. **新增不替换**：ProjectConfig 放新 crate `earthmesh_project`；`EarthmeshConfig`/`RefineConfig` 保持原样，作为 **L3 lowering 目标**（`EngineExecutionPlan.mkgrd/refine` 直接是它们）。
2. **单向 lower + 反向 import**：`ProjectConfig::lower()` → 现有 config + namelist（复用 `to_mkgrd_namelist`/`to_mkrefine_namelist`）；`import` 把旧 `.nml` 读成 L2/expert，便于老算例无损接入。
3. **执行通道不变**：lower 出的 namelist 喂给现有 `run_mkgrd_top_level_namelist*`，引擎内核零改动 → 行为可逐字节比对（回归安全）。
4. **GUI 渐进**：GUI 先在现有界面旁加 "Guided" 入口（产出 ProjectConfig→lower→复用现有 `start_run`）；旧 tab 保留为 Expert。
5. **schema 版本化**：`schema_version` 做前向兼容；未知字段保留（serde `#[serde(flatten)] extra`）以免旧工具读新文件即崩。
6. **Python util 桥接**（见 [02](./02_workflow_consistency_audit.md)）：`OutputConfig.reports.{html,eval,ranking}` 暂可委托现有 `util/hydro_mesh/*`，待 Rust 化后切换实现而不改 schema。

> 结论：**v3 内部零迁移**——旧 `.nml` 与 `mkgrd.x` 继续可用；ProjectConfig 是叠加的高层入口，不是替换。

---

## 10. Patch Plan（提案，待 P8 批准）

| Patch ID | 关联风险 | 目标 | 改动摘要 | 验证 | 风险 |
|----------|----------|------|----------|------|------|
| PATCH-S1 | C1,C8 | 新 crate `earthmesh_project` | 定义 ProjectConfig + 子结构（§3），serde JSON/YAML | `cargo test earthmesh_project` | 低（纯新增） |
| PATCH-S2 | C11 | `RefinementCriterion` trait + 注册表 | trait + 3 个样板 criterion（river_network/coastline/lai） | 单测 score/gui_spec | 中 |
| PATCH-S3 | C1,C12 | `ProjectConfig::lower()` → 现有 config + namelist | 复用 `to_*_namelist`，round-trip 测 | 与现有 namelist 比对 | 中（须逐字节回归） |
| PATCH-S4 | C9 | `ReproducibilityManifest` + 输入 sha256 | 扩展 delivery manifest | 校验 manifest 字段 | 低 |
| PATCH-S5 | C10 | `QualityConstraintConfig` + V4 门禁 | 接质量 NetCDF 结果做判定 | 注入越界用例→Block | 中 |
| PATCH-S6 | C7,C2,C3 | GUI Guided/Standard/Expert 三档 | criteria 由 `CriterionGuiSpec` 渲染；engine 旋钮入 Expert | GUI 内联测试 | 中（改 GUI，归 P6） |
| PATCH-S7 | C4,C5,C6 | enum/Option 化（先在 project 层，不动 core） | project 层全强类型；lower 时映射回字符串/哨兵 | 类型测 | 低 |
| PATCH-S8 | C13 | hydro/coupling 配置并入 ProjectConfig | HydroCoastConfig/CoupledMeshConfig 统一入口 | import 现有 recipe.json | 中 |

> 实施顺序建议：S1→S2→S3（打通 L1→L3）→ S4/S5（可复现+质量门禁）→ S6（GUI）→ S7/S8（强类型+整合）。每步独立可回归，符合 surgical change。

---

## 关键证据索引（file:line）

- `EarthmeshConfig`：`core/lib.rs:735-759`（25 字段）；`Default`：`:763-792`（`/tmp`/前导空格）
- `RefineConfig`：`core/lib.rs:1236-1276`（39 字段，平行数组 `th_*`/`refine_*`）；`Default`：`:1278-1316`（`999.0`/`/tmp` 哨兵）
- GUI engine 参数暴露：`gui/main.rs:3711-3861`（halo/transition/spring/native OLAM 文本粘贴 3814）
- CLI 互斥 flags：`cli/main.rs:75-93`；namelist 隐式派发：`cli/lib.rs:16436`
- examples / manifest：`examples/default/*.nml`、`examples/merit_hydro/gba/{case.nml,delivery_manifest.json}`、`yangtze_delta/case_or_manifest.json`

*本报告为设计提案；所有现状结论基于实际源码字段。未修改任何 `src/rust` 代码。*
