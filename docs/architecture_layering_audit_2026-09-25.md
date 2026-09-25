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
- CMRC 在读取源网格之前就被分派到另一条完整流水线 `run_certified_pipeline`
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
FVCOM 交付都失败）。“交付需要什么”没有统一契约，而是散落在各生成路径里。

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

### HARP-DV

`earthmesh_refine_harp_dv` 已从工作区移除，仓库中不再有 HARP-DV 的实现或被他处调用的函数。
仅保留显式退役防护（core 的 namelist 解析、CLI 的 `&harp_dv` 段与后端名、GUI 的后端选择）及其
测试：删掉它们会让旧 namelist 的 `&harp_dv` 段被当作未知段静默忽略、运行照常进行，属于
指南 11.1 所列的静默失败类。
