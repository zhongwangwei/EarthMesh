# 输入层 / 算法 / 输出网格分层：现状审计与迁移设计（2026-09-25）

## 1. 目标与判据

**目标：** 输入层、细化算法、输出网格各自独立；换一个算法（Method-C / Red-Green / CMRC）
不改变输入接受什么、也不改变输出长什么样。

**可检验的判据：**

1. 同一份项目配置，除了 `refinement.backend` 以外不需要改动，就能在任一后端上运行；
   后端做不到的事情由后端报告“未满足”，而不是让输入层拒绝配置。
2. 输出层（gridfile、FVCOM/MPAS/ICON/CoLM 交付、质量报告）只读取一种与算法无关的结果
   结构，源码里不出现任何算法名。
3. 每个算法都交付同样的结果字段；字段缺失必须是显式的、带原因的，而不是依赖于“走了哪条
   路”。
4. 上述边界由 crate 依赖和 `make check-architecture` 机械地守住，而不是靠约定。

## 2. 已经分离的部分

| 方面 | 现状 |
|---|---|
| 算法实现 | `earthmesh_refine_method_c`、`earthmesh_refine_redgreen`、`earthmesh_refine_certified` 是独立 crate，不依赖 CLI，不做 I/O |
| 基础层 | `core` / `geometry` / `mesh` / `boundary` / `hfield` 不依赖任何算法 |
| 质量检查 | `earthmesh_quality` 独立 crate，只依赖基础层 |
| 项目配置 | `earthmesh_project` 独立 crate（schema、校验、降级为 namelist） |
| 需求层雏形 | `earthmesh_refine`（`api`、`criteria`、`demand`、`hfield`）的定位就是“项目需求与后端之间的那一层”，已有 `RefinementDemand` / `RefinementCause`；但 CLI 仍在 `refinement_demand`、`hfield_refine` 里各做一套，后端也没有统一从它取需求 |
| 网格写出 | 写出器接收统一的 `UnstructuredMesh`；任何后端写出的文件格式相同 |

## 3. 耦合清单

### A. 输入 → 算法：需求描述不统一，算法决定了输入能否被接受

各后端接受的细化需求形式不同，于是 `earthmesh_project` 的校验按后端拒绝配置
（`rust/earthmesh_project/src/validation/mod.rs:303-338`，以及 `:54-89` 的若干后端专属选项）：

| 需求来源 | Method-C | Method-C (LEPP) | Red-Green | CMRC |
|---|---|---|---|---|
| h-field（梯度限制的目标层级场） | ✅ | ❌ | ❌ | ❌ |
| `adaptive`（判据 → 点+半径圆） | ✅ | ✅ | ✅ | ❌ |
| 命名区域（圆/框/多边形） | ✅ | ✅ | ✅ | ✅（阈值/命名需求） |
| `quality_policy = domain_export` | ❌ | ❌ | ❌ | ✅ |
| `lepp_post_quality` | ✅（仅全球闭合域） | 与 AdaptiveHybrid 互斥 | ❌ | ❌ |

- Red-Green 在运行时再次拒绝 h-field、Cartesian-XY、原生地表扩展
  （`rust/earthmesh_cli/src/refine_pipeline/global_source.rs:697-722`），以及没有
  `adaptive` 时的计算判据（`:561-572`）。
- “圆”本是 Red-Green 路线的内部简化，却成了输入层对外的概念（`refinement_demand` 模块把
  判据归约为圆，`adaptive_demand_circles_for_level_windows_at_radius`）。
- 实例：Case9（全球海岸线阈值）在默认 Method-C 上失败，改走 Red-Green 必须同时关掉
  h-field，也就是说**换算法强迫用户改输入配置**。

### B. 算法渗入“前置”阶段

- 所有后端的初始网格都由 Method-C 的函数构造：`method_c_delaunay_mesh_from_unstructured_gridfile`
  （`global_source.rs:604`），并携带 Method-C 的元数据切片。
- （已于 6b 处理，见 §8）CMRC 在读取源网格之前就被分派到另一条完整流水线 `run_certified_pipeline`
  （`global_source.rs:274`），分派 `match` 里对应分支是 `unreachable!`（`:875`）。
- 弹簧平滑的迭代数按后端换算（`effective_refinement_spring_iterations`，`:5467`），
  CMRC 会丢弃用户设定的弹簧参数。

### C. 算法 → 输出：算法内部概念泄漏到结果结构和写出器

- 统一结果结构 `RefinedGrid`（`global_source.rs:3880`）带着算法专属字段：
  `method_c_metadata`、`hfield_context`、`lepp_adaptive_hybrid`、`lepp_post_quality`、
  `transition_faces`（Method-C 的过渡行概念）、`pentagon_indices`、`state`
  （Method-C 的 Voronoi 状态，Red-Green 为 `None`）。
- 写出函数名 `write_unstructured_mesh_netcdf_with_method_c_metadata`，元数据切片类型
  `MethodCMetadataSlices` / `MethodCGridfileMetadataSlices`
  （`rust/earthmesh_cli/src/refine_pipeline/outputs.rs:52`、`:149`）。
  *（第 2a 步已改名为 `write_unstructured_mesh_netcdf_with_metadata` /
  `GridfileMetadataSlices`；`MethodCMetadataSlices` 只剩 Method-C 独有的原始层级、`ngr` 与谱系。）*
- **每单元细化层级、谱系、`ngr` 只有 Method-C 提供。** Red-Green 的层级在生产路径上被丢弃
  （`rust/earthmesh_refine_redgreen/src/refine_loop/mod.rs:730`；
  `rust/earthmesh_cli/src/redgreen_bridge.rs:359`），导致质量报告对 Red-Green 网格无法做
  目标/实际层级对账（1a8d266f 只能把它标成“未测量”）。
- MPAS 宽度上下文取决于是哪一种需求生成方式在运行（`outputs.rs:100`，
  `match (hfield, adaptive, lepp)`）。

### D. 质量与 AutoRefine 随后端变化

- CMRC 自己负责质量修复（`RefinementBackend::owns_quality_repair`，
  `rust/earthmesh_project/src/schema/mod.rs:182`），AutoRefine 流程因此分叉
  （`rust/earthmesh_cli/src/cli_mkgrd_run/run.rs:213`、`:294`）。
- 质量报告挂哪种自适应诊断，取决于 gridfile 旁边有没有 `adaptive_refinement.json`、
  gridfile 里有没有 h-field 上下文（`grid_quality_inputs/adaptive.rs`、`hfield.rs`）。

### E. 交付细节依赖具体代码路径

FVCOM 需要的开边界上下文 `earthmesh_fvcom_obc_order` 只由三条路径写入：
`mask_postproc_domain/runners.rs:361`、`regional_gridfile_writers/clean_ocean.rs:225`、
`regional_gridfile_writers/landtype.rs:202`（最后这条是 7a04c8a1 补的，此前所有全球海洋
FVCOM 交付都失败）。“交付需要什么”没有统一契约，而是散落在各生成路径里。*（第 3 步：写入留在切割步骤，读取收拢为一处，见第 8 节。）*

### F. 编排集中，边界无人把守

- 输入读取、算法分派、输出写出都在 `earthmesh_cli`（约 7.8 万行，156 个顶层模块）；
  `refine_pipeline/global_source.rs` 一个文件 6,860 行，三层混在一起。
- `make check-architecture` 只检查三条命名规范（通配符重导出、废弃兼容层、来源命名），
  没有任何分层规则。

## 4. 目标架构：三个契约

### 4.1 输入 → 算法：`RefinementRequest`

输入层（数据读取、判据评估、项目配置解释）只产出一种与算法无关的需求：

- **目标尺寸场**：每个位置期望的单元尺寸（或等价的目标层级），已做梯度限制。h-field 就是
  现成的通用形式；点+半径圆、命名区域都能先光栅化/求值成这个场。
- **硬性区域**：必须达到某层级的区域（命名区域、硬需求）。
- **计算域与掩膜**：全球 / 区域边界、陆海掩膜、开边界来源。

每个算法从这一种需求出发：Method-C 按层级取样生成嵌套掩膜；Red-Green 对“目标尺寸小于
当前单元”的三角形打标记（圆降为 Red-Green 的内部实现）；CMRC 用目标尺寸驱动反向粗化。
**算法做不到的部分，由算法以“未满足需求”报告，而不是由输入层拒绝配置。**

### 4.2 算法 → 输出：`RefinedMesh`

每个算法都交付：

- 网格（顶点、单元、邻接），即现在的 `UnstructuredMesh`；
- **每单元实际细化层级**（必填；做不到时显式为 `Unrecorded { reason }`）；
- 每单元的需求满足情况（目标层级 vs 实际层级，或未满足面积）；
- 一份不透明的算法诊断（`serde_json::Value` 或 trait object），输出层只原样存档、不解读。

算法专属的中间状态（Voronoi `state`、`transition_faces`、LEPP 报告、`ngr`/谱系）留在算法
crate 内部，或作为诊断存档，不出现在结果结构的类型签名里。

### 4.3 输出层：`Delivery`

gridfile 写出、掩膜后处理、模型格式交付（FVCOM/MPAS/ICON/CoLM）、质量报告只读取
`RefinedMesh` + `RefinementRequest`（用于目标/实际对账）+ 交付配置。交付所需上下文
（如 FVCOM 开边界）由输出层根据计算域统一推导，而不是依赖生成路径是否写过某个属性。

### 4.4 crate 划分与门禁

```
earthmesh_core / geometry / mesh / boundary / hfield     基础层
earthmesh_refine    (扩展) 输入层：数据读取、判据、目标尺寸场     → 只依赖基础层
                          （已有 api/criteria/demand/hfield，在它上面扩展，而不是另建 crate）
earthmesh_refine_*        算法层：消费 RefinementRequest，产出 RefinedMesh → 只依赖基础层
earthmesh_delivery  (新)  输出层：写出、交付、质量对账             → 只依赖基础层 + quality
earthmesh_cli             编排：解析配置、选择算法、串接三层
```

`check-architecture` 增加机械规则：`earthmesh_delivery` 与 `earthmesh_refine` 的
`Cargo.toml` 不得依赖任何 `earthmesh_refine_*`；两者源码中不得出现 `method_c`、
`redgreen`、`certified`、`lepp` 等标识。

## 5. 迁移步骤

每一步都单独提交，并用第 6 节的 A/B 精确输出回归证明现有输出不变（除非该步的目的就是改变
某个输出，届时在提交里写明）。

| 步骤 | 内容 | 验证 | 风险 |
|---|---|---|---|
| 1 | 引入 `RefinedMesh` 与必填的每单元层级；Method-C 从现有元数据填充；Red-Green 在细化循环中维护面深度（四分、过渡二分、LOP 翻边时继承父层级） | A/B 回归逐变量一致；Red-Green 质量报告的层级对账从“未测量”变为有数 | Red-Green 核心是 Fortran 移植代码，需逐例对照 |
| 2 | 输出层只读 `RefinedMesh`：`RefinedGrid` 的算法专属字段移入诊断存档；写出函数去掉 `method_c` 命名；MPAS 宽度上下文改由 `RefinementRequest` + 实际层级推导 | A/B 回归；MPAS 交付逐字节比对 | MPAS 上下文三种来源需逐一核对等价 |
| 3 | 交付契约：开边界上下文由输出层按计算域推导，删除散落在生成路径里的写入点 | FVCOM 交付：全球/区域/清洁海洋三类样本 | 区域开边界分类逻辑需整体搬迁 |
| 4 | `RefinementRequest`：目标尺寸场作为唯一需求形式；圆与命名区域先求值成场；Red-Green 改为按场打标记 | Red-Green 在 h-field 配置下可运行；原有 `adaptive` 样本输出变化须逐项解释 | 会改变 Red-Green 的输出（打标记方式变了），需单独评审 |
| 5 | 把输入层收拢进 `earthmesh_refine`、拆出 `earthmesh_delivery` crate，编排瘦身；加门禁规则 | `make check-architecture` 新规则通过；全部门禁 | 纯搬迁，风险低但改动面大 |
| 6 | 初始网格与 CMRC 流水线：初始网格由基础层构造；CMRC 在同一编排下接收 `RefinementRequest`、交付 `RefinedMesh`，质量修复作为算法内部行为 | CMRC 冻结快照与生产验收测试 | CMRC 证书语义需保持 |

建议顺序为 1 → 2 → 3 → 5 → 4 → 6：先把“输出不依赖算法”做实（收益直接、可逐变量验证），
再做会改变 Red-Green 输出的第 4 步。

## 6. 验证方法

- **A/B 精确输出回归**（本轮 2026-09-25 已用于按块容错）：同一批样本分别用改动前后的二进制
  运行，逐变量比对最终 gridfile 的全部变量与全局属性，并比对 `refine_*` 统计行。当前样本集：
  `examples/projects/*.yaml`（3 个），Case9 的四个 `g=0.05` 全球变体（海洋/陆地 ×
  Tri/Hex，真实 15 角秒数据），以及 `make regression`。每一步可按需增加 Red-Green、CMRC
  与区域海洋样本。
- 各步同时跑 CI 三个任务的本地等价门禁（`make fmt`/`clippy`/`test-fast`，
  `fmt-gui`/`clippy-gui`/`test-gui`，CLI `clippy` 与全量测试）。

## 7. 不在本设计范围内

- 不解决 Method-C 过渡模板的形状局限（见技术指南 11.70）；分层只保证“换算法不必改输入”，
  不保证每个算法都能满足每种需求。
- 不改变任何算法的数值行为，第 4 步除外，且该步需要单独评审。

## 8. 进展记录

### 2026-09-25 第 1 步（部分）：每单元细化层级成为与后端无关的字段

- `RefinedGrid` 新增 `cell_levels: Option<CellRefineLevels>`（每个 M 行、W 行的深度，基础网格为 0）；
  Method-C 从自身元数据填写，Red-Green 三角形模式从面深度填写（W 行取周围面的最大值），
  LEPP 与 Red-Green 经典过渡行路径暂为 `None`。
- 写出层只在没有 Method-C 元数据时使用它，Method-C 的写出路径不变。
- Red-Green 的最终 Lawson 抛光不再清空面深度：被翻边的面取它所由切出的旧面中最深者
  （`redgreen_bridge::flipped_refinement_levels`）。
- 验证：Method-C 的 A/B（3 个例子项目 + Case9 海洋 Tri 全球变体）逐变量一致；Red-Green 全球
  海岸案例只多出 `earthmesh_{m,w}_refine_level` 两个变量，其余逐变量一致。按层级分组的三角形
  面积中位数之比为 4.03 / 4.03 / 4.02，与每层面积缩小到 1/4 一致。
- 质量报告对 Red-Green 首次有了层级对账：该案例 428,629 个单元比目标细、32,829 个比目标粗，
  并出现“比目标差一层以上”与 194 个孤立细化单元两条新 warn——这是 Red-Green 的真实行为
  （大面积过度细化；halo 外取消的标记造成不足），此前无从测量。
- 待做：Red-Green 经典过渡行路径（Hex）的面深度跟踪；把 `method_c_metadata` 中的层级改为只从
  `cell_levels` 流向写出层（第 2 步）。

### 2026-09-25 第 2 步（部分）：写出层不再以算法命名、层级只走中立通道

- 2a：`write_unstructured_mesh_netcdf_with_method_c_metadata` → `write_unstructured_mesh_netcdf_with_metadata`，
  `MethodCGridfileMetadataSlices` → `GridfileMetadataSlices`（26 个文件、103 处，纯改名）。
- 2b：`MethodCMetadataSlices` 去掉每单元层级；写出层的 `earthmesh_{m,w}_refine_level` 只来自
  `cell_levels`。Method-C 的 `cell_levels` 与原元数据同源，输出不变。
- 验证：A/B 回归 6 个样本（3 个例子项目；Method-C 海洋 Tri、陆地 Hex 全球；Red-Green 全球海岸）
  最终网格逐变量一致、`refine_*` 统计一致；CLI clippy 与全量测试 1,159 通过。
- 2c：MPAS 宽度上下文改由需求层的 `refinement_demand::width::NominalDemandWidth` 提供。三种来源
  （h-field、`adaptive` 逐层区域、LEPP 已解析目标）本来就都是“名义需求宽度在最终 W 点上的取样”，
  区别只在读哪种需求；现在由编排从“哪个生产者运行了”构造一次，写出层只调用 `mpas_context`，
  不再接收 `adaptive` 与 LEPP 报告、也不再三选一。仍然是名义需求而不是实际层级——MPAS 交付要的是
  “被要求的宽度”，这是原设计的有意选择，没有改。
- 2d：`RefinedGrid` 分成三部分：中立结果（`output_mesh`、`cell_levels`、`pentagon_indices`）、
  `RefinedDemandRecord`（h-field 上下文、`adaptive` 记录、LEPP 硬区域——交付读取它做 MPAS 宽度、
  切割保护与目标/实际对账）、`BackendDiagnostics`（Voronoi `state`、Method-C 元数据、过渡面数、
  弹簧轮数、h-field 诊断、LEPP 报告与后处理——只报告或存档，不决定网格的样子）。
  `realized_max_level` 改为从 `cell_levels` 读取：Method-C 与原来同源，结果不变；只有命名区域、
  没有逐层判据的 Red-Green 运行过去报 0，现在报实际深度。
- 验证：A/B 回归 5 个样本（3 个 MPAS 例子项目；Method-C 海洋 Tri、陆地 Hex 全球）逐变量一致，
  `refine_*` 统计一致。
- 仍在编排里、留给第 5 步的：`method_c_metadata` 的谱系/`ngr` 仍作为额外变量写进 gridfile；
  LEPP 报告同时是诊断和需求宽度来源；`write_refined_outputs` 仍在 `refine_pipeline` 模块内。

### 2026-09-25 输出契约：三角形内角 35°–85° 强制窗口（d1f6633a）

全三角形网格的每个内角必须在 [35°, 85°] 内，否则质量结论为 Fail；项目与 namelist 都不能放宽
（指南 11.72）。这让“输出长什么样”对三角形网格有了一条与后端无关的硬契约：换后端不会换来
另一种质量的三角形网格——达不到窗口的后端被拦下，而不是交付。现状：Method-C 全球 Tri 通过；
Red-Green 的 green 闭合必然产生约 30°/90° 的过渡三角形，原本失败；现在三角形模式在 Lawson 抛光后
做一次与后端无关的角度窗口修复（`earthmesh_mesh::repair_triangle_angle_window`：度数翻边、删除度数
3/4 的加密顶点、按最差余量移动顶点），Case9 全球海岸为 35.21°–84.86°，通过（指南 11.73）。修复放在
基础层而不是 Red-Green crate 里，任何产出三角形网格的后端都可以调用。Hex 单元不受此窗口约束。

### HARP-DV

`earthmesh_refine_harp_dv` 早已从工作区移除；2026-09-25（e8d028df）连同退役防护一并删除——用户
只有一人，没有需要保护的旧配置。HARP-DV 的名称不再有任何特殊处理，未知后端名按通用规则拒绝。

### 2026-09-25 第 3 步：FVCOM 只从 gridfile 自带的开边界上下文导出

- 核查后的实际情况比第 3 节 E 写的具体：三个写入点（区域切割分类器 `mask_postproc_domain/runners.rs`、
  清洁区域海洋 `clean_ocean.rs`、全球陆地切割 `landtype.rs`）都在切割步骤里，切割本身已是输出层、与后端
  无关；开边界分类要知道哪些边界边是计算域切口、哪些是海岸线，只有切割时知道，所以由切割记录、写进
  gridfile 是合理的位置。真正分散的是**读取**：FVCOM `.2dm` 有 8 个写出点，开边界列表来自 4 种来源
  ——gridfile 属性、`obc.nc4` 旁路文件、内存中的列表、以及硬编码的 `&[]`。
- 改动：CMRC 的两条路线、gridinit 的区域路线、`write_clean_regional_ocean_fvcom` 全部改为调用
  `write_fvcom_from_final_gridfile`，它只读 gridfile 自带的上下文并逐项校验（必须是边界顶点、相邻两点
  必须是边界边、有边界而无上下文则拒绝）。`write_fvcom_2dm_from_carved` 改为私有，不再有调用方能传入
  旁路列表或假定的空列表；无调用方的 `write_standard_fvcom_from_gridfile` 删除。
- 修掉的一个静默缺陷：CMRC 全球陆地切割路线原来直接以 `&[]` 写 FVCOM，也就是把任何边界都当成墙。现在
  它读切割写下的上下文——闭合球面切出来的只有海岸线，上下文就是空；若输入本身有边界、切割无法判断，
  上下文缺失，导出被拒绝而不是写成全墙。
- 旁路 `obc.nc4` 仍作为辅助产物交付（测试断言它与嵌入上下文一致），但不再是任何导出的输入。
  `fvcom_mesh_writer::write_fvcom_mesh_save_outputs`（Fortran `FVCOM_Mesh_Save` 的文件级移植）只有测试
  调用，保留。
- 验证：CLI clippy 与全量测试 1,159 通过，其中 `certified_close_ocean_publishes_regional_fvcom_after_global_certificate`
  逐字节比较流水线产出的 `.2dm` 与 `write_fvcom_from_final_gridfile` 的产出；CMRC 全球海洋 FVCOM、
  gridinit 区域海洋、清洁海洋窗口各有集成测试覆盖。gridfile 本身不变，这一步只改 `.2dm` 的来源。

### 2026-09-25 第 5 步（第一部分）：先立门禁，再搬 crate

整体拆出 `earthmesh_delivery` 是几万行、牵动 `UnstructuredMesh` 与全部 NetCDF 读写的搬迁；先把
“输入/输出代码不得依赖算法”变成机械规则，让后续逐块搬迁每一步都有门禁把守。

- 需求层清理：`refinement_demand/nest.rs` 里的 Method-C 逐层嵌套驱动（`spawn_nest_adaptive*`、
  `AdaptiveNestSpring`、超大分组拆分、判据驱动暂停说明）搬到 `method_c_adaptive_nest.rs`，与
  `redgreen_bridge.rs` 对称；需求层只留各后端共用的逐层需求、报告与目标层级函数。
- MPAS 宽度的第三种来源不再以 LEPP 的报告类型出现：需求层定义 `ResolvedTargetWidths` 接口（目标边长
  取样、目标边长列表、最深层级），LEPP 报告在 `refine_pipeline/lepp_targets.rs` 实现它；
  `MpasGridfileContext::from_lepp_resolved_demand` 改名为 `from_resolved_target_demand`，写进文件的
  来源字符串不变。
- 写出层清理：`outputs.rs` 中构造 Red-Green 宽环的测试移到 `global_source.rs` 的测试里，写出层源码
  不再出现后端 crate。
- 门禁（`scripts/check_architecture.py`，由 `make check-architecture` 调用，CI `fast` 任务运行）：
  1. 基础层与需求层 crate（core、geometry、boundary、mesh、hfield、quality、project、refine、
     refine_planner）的 `Cargo.toml` 不得依赖任何后端 crate，源码（去掉注释后）不得出现后端 crate 名；
  2. CLI 中只有 `CLI_BACKEND_ADAPTERS` 列出的编排与适配模块（`refine_pipeline/global_source.rs`、
     `cmrc_local_updates.rs`、`lepp_targets.rs`、`redgreen_bridge.rs`、`method_c_adaptive_nest.rs`、
     `certified_options.rs`、运行记录 `mkgrd_run_types/refine.rs`）可以引用后端 crate。新文件引用后端
     会被拒绝，除非它是适配层并加入名单。
  3 条自测试覆盖：写出层引用后端被拒、适配层与注释不算、基础/需求 crate 依赖后端被拒。
- 仍待做：把 CLI 内的输入层模块收进 `earthmesh_refine`、输出层拆成 `earthmesh_delivery` crate；
  门禁的“算法名标识符”规则（`method_c`、`lepp` 等出现在输出层函数名里，如
  `write_method_c_mesh_with_optional_domain_and_metadata`）留到搬迁时一起处理。

### 2026-09-26 第 4 步拆分，4a + 4b：统一的目标层级查询，Red-Green 读 h-field

第 4 步原写法（圆与命名区域先“求值成场”）会因栅格化与梯度限制改变 Red-Green 输出，但那是表示方式
的选择，不是分层本身的要求。拆成三步：4a 统一接口、输出不变；4b 新能力；4c 有意改输出（11.71 的
green 下限与需求嵌套），单独决定。

- 4a：需求层 `earthmesh_refine::target_level` 定义 `TargetLevelField`（“这个点是否要求至少第 L 层”，
  以及“是否有任何点要求第 L 层”）。`RegionTargets` 用标记一直在用的精确包含判定，不做栅格化；
  `HfieldTargets` 按 h-field 量化后的目标层级回答。Red-Green 的标记只经由这个接口读取需求
  （`redgreen_bridge::redgreen_marking`）。Method-C 由区域形状（种子、周界）构造，继续直接读形状。
- 4b：球面 h-field 的合成（区域与阈值栅格组合、水文目标、裁到计算域）从 Method-C 函数中提出为
  `hfield_refine::compose_spherical_hfield`，两个后端共用。Red-Green 遇到 h-field 配置时按场的目标
  层级标记，命名区域已合成进场里（与 Method-C 的 h-field 路线一致），并把 h-field 记入结果，
  MPAS 宽度与质量对账照常可用。解除了运行时、项目校验与 GUI 三处对“Red-Green + h-field”的拒绝；
  没有 `&adaptive` 时，带阈值来源的 h-field 也能承接 Red-Green 的计算判据。Case9 不再需要为换
  Red-Green 而关掉 h-field。
- 验证：A/B 回归 4 个样本（3 个例子项目、Case9 Red-Green h-field 关）逐变量一致；原“Red-Green 拒绝
  h-field”的测试改为断言它能按场细化并记录 h-field；fast（1,619）、CLI 全量（1,159）、GUI 四项门禁、
  架构门禁通过。
- 已知缺口：Red-Green 经典过渡行路径（Hex 输出）仍不记录每单元深度，所以 h-field 驱动的 Hex 运行
  `realized_max_level` 报 0，质量对账为“未测量”。

### 2026-09-26 4c：Red-Green + h-field 的 green 下限固定为 20°

用户决定按建议实施（数据见指南 11.71 补充）。只作用于天然嵌套的需求（h-field）与三角形输出；判据圆
路线与 Hex 输出不变。GUI 中 Red-Green 默认需求表达为 h-field。这是有意改变 Red-Green + h-field 的
输出，该路线在 a8083510 才开放，没有依赖它的旧项目。

### 2026-09-26 输出契约：三角形角度在输出层统一执行

三角形角度契约不再由各后端各自满足：编排尾部对任何后端的最终三角形网格执行一次修复与优化
（`refine_pipeline/angle_contract.rs`，指南 11.74），按行记录随修复跟随。这让“输出长什么样”对三角形
网格有了与算法无关的实现，而不只是判定。

### 2026-09-26 第 5 步（第二部分）：拆出 `earthmesh_delivery` crate 的第一块

- 先量依赖闭包：从“gridfile 网格类型 + NetCDF 读写”出发，经 `crate::` 引用可达 109 个模块、约 6 万行
  （CLI 的四分之三）。拉进来的只有两条边：MPAS 宽度上下文里“由需求推导宽度”的三个构造函数
  （→ 需求层 → h-field 合成、阈值读取……），以及杂物模块 `mesh_conversion_support` 里被网格读写借用的
  三个小函数（→ 掩膜后处理类型）。
- 切断：三个构造函数改为需求层 `refinement_demand::width` 的普通函数
  （`mpas_context_from_hfield` / `_from_region_passes` / `_from_resolved_targets`），MPAS 上下文模块只留
  数据结构与读写；`lon_values`/`lat_values` 移到坐标类型模块，`require_len` 移到 NetCDF 模块。闭包降到
  7 个模块、2,541 行。
- 拆出：这 7 个模块（坐标类型、NetCDF 读写、文件辅助、网格类型与拓扑检查、网格 NetCDF 读写、h-field 与
  MPAS 的 gridfile 记录）用 `git mv` 移入新 crate `rust/earthmesh_delivery`，依赖只有 geometry、hfield、
  quality 与 netcdf。CLI 以 `pub use earthmesh_delivery::模块` 重新导出，外部路径 `earthmesh_cli::…`
  不变。
- 门禁：`earthmesh_delivery` 加入 `check_architecture.py` 的中立 crate 名单（不得依赖、不得命名任何后端）。
  它需要 NetCDF，所以和 CLI 一样不进 `fast`，由 `heavy` 任务 clippy 与测试（ci.yml 已改），`make test` /
  `make clippy-full` 也包含它；CLAUDE.md 的计数改为 14 / 12 / 14。
- 仍待做：把更多写出与交付模块（FVCOM/MPAS/ICON/CoLM 写出器、掩膜后处理、质量写出）逐块移入
  `earthmesh_delivery`，每块先量闭包、切边再搬；输入层模块收进 `earthmesh_refine`。

### 2026-09-26 第 5 步（第三部分）：写出器第二批移入 `earthmesh_delivery`

- 第一批拆出后重算：多数写出模块的闭包已只剩几百到一千多行。共同底座是 5 个互相依赖的模块
  （`contain_io`、`mask_postproc_types`、`mask_postproc_writers`、`mesh_conversion_support`、
  `obc_boundary_io`，1,485 行），搬走它就能带走 FVCOM、质量全局写出、网格度量写出、MPAS 一组、CoLM
  一组、`json_support`、`atomic_output`——共 26 个模块、4,752 行，闭包封闭，外部依赖只多了 core、mesh、
  serde、serde_json。
- 用脚本搬迁（`git mv`、`pub(crate)`→`pub`、在 delivery 根上补回被搬代码通过 `crate::` 引用的名字、
  CLI 里 `mod X;` 改为 `use earthmesh_delivery::X;`），CLI 根上只剩被搬代码才用的导入由 `cargo fix`
  清掉；delivery 沿用 CLI 的 clippy 允许项（算法式下标循环等）。
- 验证：3 个例子项目（MPAS）与 Case9 海洋 Tri FVCOM 的 A/B 逐变量一致；fmt、clippy、clippy-full、
  check-architecture、`make test`（2,782）通过。上一批的 CI heavy 日志已确认跑了 delivery 的测试。
- 现状：`earthmesh_delivery` 33 个模块、约 7,300 行；CLI 约 7.2 万行。

### 2026-09-26 第 5 步（第四部分）：ICON、MPAS 其余写出与掩膜后处理移入 `earthmesh_delivery`

- 阻碍点是转发门面 `grid_quality_pipeline`（20 行）与两个宿主模块里的子文件：MPAS 构建器和 ICON 写出
  经门面取函数，而门面把质量输入、需求层一起拉进来。做法：把纯 gridfile 读取的
  `grid_quality_inputs/gridfile.rs`、区域 gridfile 写出下的 `lineage.rs`（谱系校验）与 `levels.rs`
  （层级读取）抽成 delivery 的独立模块 `gridfile_quality_input` / `gridfile_lineage` / `gridfile_levels`，
  宿主模块以 `use earthmesh_delivery::新名 as 原名;` 接回，旧路径不变；ICON、MPAS 写出、掩膜布局改为
  直接引用。
- 随后移入 14 个模块：`grid_production_adapters`、MPAS 行/简版/拓扑/完整写出/构建器/gridfile 写出、
  `icon_writer`、`gridfile_output_writers`、掩膜后处理的 ocean/patchtypes/layout、`close_mesh_io`、
  `boundary_model`。delivery 新增依赖 `earthmesh_boundary`、`earthmesh_project`（均为中立 crate）。
- 验证：3 个例子项目（MPAS）、LEPP 全球 Tri（经 ICON 交付）、Case9 海洋 Tri（FVCOM、掩膜后处理）A/B
  逐变量一致；fmt、clippy、clippy-full、check-architecture、`make test`（2,782）通过。
- 现状：`earthmesh_delivery` 约 1.5 万行，CLI 约 6.5 万行。仍在 CLI 的输出侧模块：
  `mask_postproc_domain`、`mask_postproc_atmos`（依赖项目交付/质量编排）、`regional_gridfile_writers`
  其余部分（依赖 `area_judge` 等输入侧读取）、`grid_quality_inputs` 的对账部分（依赖需求层）——
  这些要等输入层的归属（交接文档 3.2 的 A/B 选择）定了再拆。

### 2026-09-26 第 5 步（第五部分）：输入层的归属与第一批 `earthmesh_inputs`

- 归属决定（用户两次说“继续”未另选，按推荐的方案 A 执行，可再议）：输入层**不**放进
  `earthmesh_refine`，新建 `earthmesh_inputs`。原因：数据读取大量使用 NetCDF，而 `earthmesh_refine`
  被 Method-C 依赖；放进去会让所有算法 crate 间接依赖 NetCDF，`fast` 任务就测不了它们。
  `earthmesh_refine` 保持为与数据格式无关的需求定义（`TargetLevelField` 等）。
- `earthmesh_inputs` 依赖 `earthmesh_delivery` 的 gridfile/NetCDF/JSON 辅助（两者都是中立 crate，门禁
  允许）；若以后需要更干净的“公共底座”，可把这些辅助再拆成单独 crate。
- 切断两条把 CLI 运行记录拉进来的边：`MkgrdRestartAreaJudgeOptions` 的定义移到使用它的
  `global_source_axes`，恢复类型模块改为重新导出；`LandtypeDataPreprocessReport` 所在文件移入
  inputs（`landtype_preprocess_report`）。
- 移入 33 个模块（约 1.3 万行）：`area_judge_*` 全系、CaMa 二进制读取与河段清单、MERIT Hydro 读取与
  瓦片选择、bbox/circle/close/Lambert 掩膜文件读写、Get_Contain 几何与类型、海岸带读取、阈值输入、
  区域来源、地表类型预处理、`v3_data_source_io`、`namelist_reader`、`mask_counts` 等。
- 注意：`cargo fix` 只看非测试构建，会删掉“只有 CLI 测试用到的根导入”；另外被移模块里带
  `#[cfg(test)]` 的再导出在 CLI 测试里不可见（条件属于新 crate），需去掉条件。
- `earthmesh_inputs` 进入中立 crate 名单；需要 NetCDF，由 `heavy` 任务与 `make test`/`clippy-full`
  覆盖（ci.yml、Makefile 已改）；CLAUDE.md 计数 15 / 12 / 15。

### 2026-09-26 第 5 步（第七部分）：拆开 CLI 的核心纠缠，需求构造移入 `earthmesh_inputs`

- 用 Tarjan 算强连通分量（不计测试代码）：CLI 里有一个 15 个模块、2.87 万行的互相依赖团（细化流水线、
  h-field 合成、需求构造、质量对账、域掩膜后处理、区域 gridfile 写出、mkgrd 分派/恢复）。团内有几条
  “下层回调上层”的边：
  1. `hydro_refinement_adapter` 的后半（写 namelist 再跑流水线的四个 `run_*`）调用
     `run_refine_pipeline_namelist`——拆到 CLI 的新模块 `hydro_refinement_runs`（编排），前半（目标场）
     留在原模块；
  2. `hfield_refine` 借用恢复模块里的 `landtype_file_is_real` / `namelist_sets_landtype_file`——移到
     输入层的地表类型模块。
  两刀之后大团分成三个小团：{h-field 合成、水文适配、需求构造}（1.13 万行，输入侧）、
  {mkgrd 恢复/分派三件}（编排）、{域掩膜后处理、区域 gridfile 写出}（输出侧）。
- 第一个小团移入 `earthmesh_inputs`：`hfield_refine`、`hydro_refinement_adapter`、`refinement_demand`，
  以及它们从水文交付工作流借用的 `feature_table.rs`（→ `hydro_cell_features`）。`earthmesh_inputs`
  新增依赖 `earthmesh_refine`、`earthmesh_refine_planner`、`earthmesh_quality`、`earthmesh_hfield`、
  `rayon`。
- 孤儿规则：`ResolvedTargetWidths` 现在属于 inputs，LEPP 报告属于 method_c，CLI 不能直接为它实现；改为
  CLI 里的包装类型 `LeppResolvedTargets`（适配层 `refine_pipeline/lepp_targets.rs`）。
- 过程教训：辅助脚本里 `open(p, "w").write(open(p).read() + …)` 会先截断再读，把 inputs 的 `lib.rs`
  清空了一次；已从 HEAD 恢复并逐项重放本轮改动（56 个模块声明与目录一一对应），脚本已修。
- 验证：3 个例子项目、LEPP、Case9 Method-C h-field、Case9 Red-Green + h-field 的 A/B；fmt、clippy、
  clippy-full、check-architecture、`make test`（2,782）。
- 剩下的跨层模块：质量对账（读需求 + 网格）、耦合质量（读 CaMa）、项目交付/质量、域/大气掩膜后处理、
  水文交付工作流——它们本身就是“读输入、调算法、写输出”的串接，按第 4.4 节属于编排层 CLI；要让输出层
  只经 `earthmesh_refine` 读需求（而不是读 inputs 的具体类型），是后续的接口工作。

### 2026-09-27 第 6 步（6a）：初始网格的名字归位，CMRC 流水线独立成适配模块

- 初始网格：原 `method_c_delaunay_mesh_from_unstructured_gridfile` 并不依赖 Method-C——它读 gridfile 表，
  调基础层 `TriangularMesh::from_voronoi_gridfile_tables_with_metadata`，所有后端共用。改名为
  `initial_triangulation_from_gridfile`；基础层 `MethodCGridfileMetadata` 改为 `GridfileCellMetadata`；
  `write_method_c_mesh_with_optional_domain_and_metadata` 改为 `write_mesh_with_optional_domain_and_metadata`。
- CMRC：它自己的整条流水线（约 2,700 行：母网格家族、渐变包络、Voronoi 重映射、需求计划、认证构造、
  产物组装与原子发布、区域/MPAS 发布）从 `global_source.rs` 移到 `refine_pipeline/certified_pipeline.rs`，
  加入 `CLI_BACKEND_ADAPTERS`；它的 4 个测试随之移动。`global_source.rs` 从约 6,900 行降到 4,493 行。
  行为不变：A/B（3 个例子项目、LEPP）逐变量一致，`make test` 2,782 通过，CMRC 由其集成测试覆盖。
- 6b 待定（需要用户决定）：让 CMRC 经共享结果交付不是机械搬迁。与共享流程的三处根本差异：
  1. 初始网格必须是 CMRC 已认证的母网格家族（证书建立在它上面），不能用 gridinit 读入的网格；
  2. 需求计划带可认证的统计界，比 `TargetLevelField` 的按点层级多出证书需要的信息；
  3. 交付除 gridfile 外还有重映射表与证书，且整体原子发布。
  可行方向：共享结果结构为“证书、重映射表、附加产物”留出位置，原子发布做成共享能力，CMRC 只替换
  “构造”这一段；或接受 CMRC 为独立流水线，只要求它满足同样的输出契约（角度窗口、MPAS 宽度、FVCOM
  开边界），由门禁与测试保证。


### 2026-09-27 第 6 步（6b，用户选择“并入共享流程”）：CMRC 走同一分派、同一尾部

分三次提交，每次都与上一版二进制做 A/B：

- M1+M2（`9837c916`）：`run_refine_pipeline_in_workspace` 拆成 `refine_from_shared_source`（源网格准备 +
  后端分派 → `RefinedGrid`、`TailInputs`）与 `finish_refined`（角度契约、实测、写出、报告、运行记录）。
  行为不变：3 个例子项目、两个 Case9 样例、3 个 CMRC 样例与 LEPP 一致（CMRC、LEPP 比较全部产物）。
- M3a（`380ca667`）：所有后端在同一个 `match` 里分派。CMRC 分支是 `refine_with_certified_as_grid`，
  它把发布网格与逐单元层级交给 `RefinedGrid`，构造记录放在 `BackendDiagnostics::certified`。
  独立入口 `run_certified_pipeline` 已删除；交付直接接收解析好的 `file_dir` 与层级，不再重算。
  行为不变，比较同上。
- M3b：certified 的分流点下移到实测之后。共享尾部对 CMRC 跳过角度修复（证书覆盖其几何，不允许改动），
  但照常测量实际分辨率；运行记录由共用的 `refined_runtime_state` 组装。**这是一次有意的报告变化**：
  CMRC 原来把 `finest_cell_km`、`coarsest_cell_km`、`realized_region_halvings` 写成 0（打印时被当作“未测”
  而省略），现在与其他后端一样是实测值；`realized_max_level` 仍与原值相同。网格与全部产物逐字节不变。
  `certified_hidden_cli` 增加了断言，旧代码会让它失败。

剩下仍属于 CMRC 自己的：构造（认证母网格家族、需求计划、反向粗化）与交付（重映射表、证书、清单、
原子发布）。上文 6b 的三处根本差异依旧成立，它们现在是分派与尾部里的两个明确分支，而不是一条
平行的流水线。`refine_from_shared_source` 的 `match` 里 CMRC 分支仍是 `unreachable!`，因为 CMRC
不需要共享源网格准备；要去掉它需把源网格准备按后端能力拆开，收益很小，暂不做。

### 2026-09-27 质量对账只经 `earthmesh_refine` 读需求

- 原来的对账各有一套取目标层级的办法：h-field 路线直接调 `HField::level_at`，自适应圆路线在 CLI 里
  自带一个圆的分桶索引。现在两者都只经 `TargetLevelField` 读取：
  - trait 新增 `target_level(point)`（该点要求的最深层级）。默认实现逐级问 `demands`；`HfieldTargets`
    直接返回 `try_level_at`（与原 `level_at` 同值，只是无效场由 panic 改为报错）。
  - 圆的分桶索引移入 `earthmesh_refine`，成为 `CircleTargets` / `LeveledCircle`。它用的是自适应路线记录的
    大圆距离判据，与 `RegionTargets` 的规范包含判据不是同一个公式，所以单独成类型而不是复用
    `RegionTargets`；“索引与全扫描逐点一致”的对照测试随之移动。
  - CLI 只剩一个取样函数 `target_levels_for_quality_cells(mesh, kind, HexSample, &dyn TargetLevelField)`；
    Hex 单元取角点最大（h-field）还是取中心（圆，硬边界）由 `HexSample` 显式指定。
- 需求从哪里来（重建 h-field、读 `adaptive_refinement.json`）仍是输入侧的事，这次不动。
- 验证：3 个例子项目、两个 Case9 样例比较**全部产物**（质量报告 JSON 包含对账结果）一致；
  `make test` 2,783 通过。`scripts/ab_compare.py` 现在也规范化嵌套临时目录名
  （`earthmesh-lowered-<pid>-<时间戳>-N`），此前它让 quickstart 的 `namelist.save` 误报不同。

### 2026-09-28 第 1 步收尾：所有后端都记录每单元层级（0f623331）

第 1 步最后两个空缺补上（技术指南 11.77）：
- Red-Green 经典（Hex、过渡行）路线按血缘传递面深度：四分子面 = 旧面 + 1；过渡对分子面继承旧面；
  Lawson 翻边删除两行、在新行写出另一对角线的一对，按两者张成的同一四边形精确配对，取较深者。
- LEPP 没有细分血缘，按其自身需求定义（L 层要求最长边 h0/2^L）反推实测深度。
至此 Method-C、Red-Green（三角与 Hex）、LEPP、CMRC、Stretch 全部写出 `earthmesh_{m,w}_refine_level`，
质量对账不再有“未测量”的后端。

### 2026-09-28 第 4 步：判据圆逐层嵌套（5d08657e）

11.71 待办的后半（技术指南 11.78）：各层判据圆先全部规划，第 L 层打标记时并入更深层的圆，并按中间
各轮 halo 外扩半径；`RegionTargets` 对所有区域按 `>= level` 作答，天然嵌套，Red-Green 三角形因此对
判据圆、命名区域统一使用 20° green 下限。Case9：比目标粗 33,249 → 6,591，halo 取消 18,203/45,670 →
687/2,718。命名区域的 halo 外扩随后在三角形路线补上（11.81）。

### 2026-09-28 新后端 Stretch：ICON 的加密方式（92fe43de）

闭合球面上 ICON（每顶点至多 6 边）容不下插点式加密（技术指南 11.76）。新增 `Stretch` 后端（11.79）：
Schmidt 保角变换，拓扑不变。它按本设计的接口接入：需求只经 `TargetLevelField` 读取（命名区域、嵌套
判据圆、h-field），结果以 `RefinedGrid` 交给共享尾部，每单元层级实测；项目、namelist、CLI 分派、GUI
一并登记。说明接口已经足够通用——一个与既有三种都不同的算法无需改动输入层或输出层即可接入。

### 2026-09-28 输入层的两处提速（8020bbd0、e70d92e8）

不改变任何输出（技术指南 11.80）：地表类型按 HField 列分组并行分箱；海岸线需求按窗口缓存（非零字）。

### 现状小结（2026-09-28）

| 步骤 | 状态 |
|---|---|
| 1 每单元层级中立 | 完成：全部后端记录 |
| 2 输出层只读中立结果 | 完成 |
| 3 FVCOM 开边界只取自 gridfile | 完成 |
| 4 需求统一为目标层级查询 | 完成于 Red-Green、Stretch、质量对账；Method-C 按设计继续读区域形状（它按形状构造嵌套） |
| 5 crate 拆分与门禁 | 完成：`earthmesh_inputs` 约 2.9 万行、`earthmesh_delivery` 约 1.5 万行、CLI 约 3.6 万行（原 7.9 万） |
| 6 初始网格归位、CMRC 并入共享流程 | 完成；共享源网格准备里 CMRC 分支的 `unreachable!` 有意保留 |

剩余：命名区域的 halo 外扩已在 Red-Green 三角形与 Hex 路线对全部区域形状完成（技术指南 11.81、11.82）；CLI 中的质量编排、耦合质量、掩膜后处理
按设计属于编排层，不再拆。

### 2026-10-03 复查：其余后端是否按“输入—算法—输出”编排

用户在 CMRC 合并判据（技术指南 11.111）落地后要求复查各后端。两份只读审计（crate 级 IO 边界、各后端从
namelist 到写出的调用链）的结论：**还没有都按这个形状编排**。按耦合程度：

1. 没有统一的需求类型：各后端拿到的输入不同；CMRC 栅格路线用自己的配方再合成一遍 h-field。（改输出）
2. 输入层带着 Method-C 的假设：无 h-field 时命名区域对所有后端加 Method-C 父级光环；h-field 区域裙带按
   Method-C 的过渡行数限梯度。（改输出）
3. CMRC 交付另有一套输出路径：陆海裁剪不保护需求单元、海洋网格强制只留最大连通海域；MPAS 宽度用交付
   层级（交付层按生产者名字字符串分支）；MPAS/FVCOM 自己写。（改输出）
4. 算法驱动仍在 CLI：`global_source.rs` 里约 2500 行（Red-Green 层级循环、Method-C 重试、Stretch），
   CMRC 的构造在 `certified_pipeline.rs`；ICON 嵌套规划器在 `earthmesh_mesh`，门禁看不到。（不改输出）
5. ICON 嵌套的规划与写文件在输出尾部混在一个函数里。（不改输出）
6. 算法层做 IO：Method-C 在设环境变量时自己写调试文件。（不改输出）
7. 放错层的代码：输入 crate 里有写输出产品的模块；交付 crate 里有区域类型、close 掩膜读取、namelist 行
   解析与边界模型构造。（不改输出）
8. 零碎：CLI 里第二个后端枚举；输入 crate 把仅测试用的 `earthmesh_quality` 当正式依赖；根 `Cargo.toml`
   说 Red-Green “生产引擎不依赖”。（不改输出）

用户选择先做不改输出的 4–8，会改输出的 1–3 逐项对比后再定。本批（第 5–8 项与第 7 项）：

- 一个后端枚举（`earthmesh_refine::RefinementBackend`）；Red-Green、Stretch、ICON 嵌套共用的点+半径判据
  结构改为中性名 `PointRadiusCriteria`；`earthmesh_quality` 改为输入 crate 的开发依赖；清单注释改正。
- Method-C 的 h-field 遍记录交给调用方装的接收器（`set_pass_sink`），算法层不再读环境变量、不写文件；
  CLI 的 Method-C 适配函数在 `EARTHMESH_METHOD_C_DUMP_DIR` 下写出，文件与回放测试不变。
- 水文交付产品（单元掩膜、河网交叉表、加密评估、扫描排名）移入 `earthmesh_delivery`；扫描配方仍在输入层。
- 区域类型（`GridRegion`、`LonLatPoint` 等）移入 `earthmesh_geometry`，输入层直接从几何层取；close 掩膜
  读写与 namelist 行解析移入输入层；边界模型构造移入 `earthmesh_mesh`（输入、输出都用；放
  `earthmesh_boundary` 会与 `earthmesh_mesh` 成环）。
- ICON 嵌套拆成“规划”（无 IO，带角度与服务统计）与“写出”两步，尾部先规划后写出；规划仍须在角度契约
  之后（嵌套角点要等于父网格最终顶点）。

每步与改动前的二进制做 A/B：12 个交接用例与 3 个示例工程，逐产物比对全部一致。`scripts/ab_compare.py`
加了 `--reuse-base`（基准只跑一次）与只记录基准的 `-` 模式。

第 4 项（不改输出）的进度，每步同样逐产物 A/B 一致：

- ICON 嵌套规划器成为独立后端 crate `earthmesh_refine_icon_nest`，交付层改收中性的嵌套网格类型
  （`earthmesh_mesh::nested_grid`）。
- Stretch 的规划与施加成为后端 crate `earthmesh_refine_stretch`，CLI 只读判据、写网格。
- CMRC 的构造（层级、区域发布、混合物化与局部更新的取舍）移入 `earthmesh_refine_certified::construction`；
  局部更新由调用方以闭包注入（读 JSON 是 CLI 的事）。`certified_pipeline.rs` 从 4654 行减到 2937 行。
- Red-Green：逐层驱动（`refine_levels`）、收尾（`finish_levels`：角度安全的 Lawson、角度窗口、六边形回滚），
  以及原 CLI `redgreen_bridge` 里的标记、每层设置、打磨与角度窗口，移入 `earthmesh_refine_redgreen::driver`。
  判据圆的规划（读栅格）、转成 gridfile 表、海绵平滑与运行记录留在 CLI 适配层；六边形环是否折叠要在将要写出的
  网格上数，由调用方以闭包传入。`LevelCircles`、`NestPassReport`、`AdaptiveNestReport`、`CriteriaEvidence`
  与 `nested_criteria_regions`、`widened_region` 从输入层与 CLI 移到中性的 `earthmesh_refine::nest`。
- Method-C：h-field 的重试（陡的场嵌不进父网格或丢了块时，按 `max_mrows` 放缓梯度重新合成，留下丢面更少的
  那个）、点+半径逐层分组加密、LEPP-Delaunay 的加密与修复（放宽门限插入、修不回 7 度以内时在严格门限下重做）
  移入 `earthmesh_refine_method_c::driver`。h-field 的合成与每层判据圆的规划（读栅格）由调用方以闭包传入；
  逐组记录 `refinement_groups.jsonl` 改由调用方的接收器写出，算法层不再写文件。`LevelledHfield` 移到
  `earthmesh_refine::hfield`。`refine_with_method_c` 其余分支（原生大气/地表嵌套、命名区域的各个
  `spawn_nest` 变体、Cartesian-XY 的 h-field）是按配置直接调用 `MethodCMesh` 的方法，留在适配层。

至此第 4 项完成：`global_source.rs` 里不再有哪个后端自己的层级循环、重试或修复；剩下的是读配置、规划需求、
调用后端、转成发布网格、各后端共用的发布网格后处理（区域海绵平滑及其回退检查）与写记录。

第 2 项（改输出）：用户定为"Red-Green 是自己的，跟 Method-C 没有太大关系"。命名区域的 Method-C 父层只给 Method-C 后端
（原版与 LEPP；LEPP 在轮数上限内要靠父层才到得了深层）；h-field 区域外缘的过渡行改由调用方传入，只有 Method-C 传它的
`max_mrows`。Red-Green 命名圆的对账从 597 个低于目标降到 9、结论 warn→pass；ICON 第 1 层嵌套 1,506→854 单元；Red-Green
区域 h-field 交付几何不变、中间整球网格减半；Method-C 与 LEPP 逐字节不变。测量与做法见技术指南 11.113。

第 3 项（用户选：裁剪对齐共用规则；MPAS 宽度与写出只整理结构）。裁剪部分已做：CMRC 把需求区域作为硬需求传给裁剪、最大连通
海域按 `isolated_ocean`、关闭时孤立块报警告而不拒绝（技术指南 11.114）。**更正**：上面第 3 条"陆海裁剪不保护需求单元"是照
`retain_edge_connected_components_with_hard_demand_one_based` 过时的文档写的；共用规则早已只留最大连通块，需求只决定顶点处
留哪个扇区、并报告丢掉的需求单元，所以这一条的实际差别只有这两点。
