# HARP-DV Phase 0：复用盘点

日期：2026-08-07
基线：`ocean-carve-topology-and-spring-defaults`
对应：`EarthMesh_HARP_DV_Claude_Code_implementation_spec` §38 任务 A、Phase 0

规格说"以仓库事实为准，并在交付物中记下差异"。下面每一条都来自代码。

## 判定摘要

架构是对的，依赖方向是对的，分阶段纪律是对的。**但 Phase 2 被严重低估**：规格写的是
"实现或整理 `earthmesh_mesh` 局地 patch API"，实际情况是**这套机制一件都不存在**，而且
它依赖的增量球面 Delaunay 内核本身也不存在。Phase 2 不是整理，是从零建一个内核。

## §11 的八项低层需求，逐项核对

| 规格要求 | 仓库现状 | 结论 |
|---|---|---|
| `extract_mesh_patch` | 无 `MeshPatch` 类型，无 patch 概念 | **缺失** |
| `locate_spherical_face` | 无点定位 | **缺失** |
| `build_spherical_delaunay_cavity` | 无。全仓 `cavity` 五处命中全是 `concavity`（凹口）拼写巧合 | **缺失** |
| `insert_site_into_cavity` | 无任何插点 | **缺失** |
| `legalize_spherical_delaunay_edges` | `earthmesh_mesh` 无。翻边只存在于 `earthmesh_refine_redgreen::refine_edge_flip`，而按依赖规则 harp_dv 不得依赖它 | **缺失** |
| `rebuild_local_voronoi` | 只有 `voronoi_grid_from_triangular_mesh`，**全局**重建，不接受 patch | **缺失（只有全局版）** |
| `validate_mesh_patch` | 有 `validate_topology`，**全局** | **缺失（只有全局版）** |
| `replace_mesh_patch` | 无 | **缺失** |

八项里零项可直接复用，两项有全局版可作参照。

## 更根本的一条：没有增量 Delaunay

`grep -niE "incremental|bowyer|watson|delaunay_insert"` 在 `earthmesh_mesh` 里**零命中**。

网格不是插出来的：`TriangularMesh::from_icosahedron` 由二十面体细分而来，再经弹簧松弛，
Voronoi 由三角网整体求对偶。整条链是**构造式**的，没有一处把一个点插进已有三角剖分。

HARP-DV 的全部前提是局地增量插点。这不是缺一个函数，是缺一个内核。

## 鲁棒谓词也不在

规格 §12 明令拓扑判定不得只靠固定 epsilon。仓库现有：

- `robust_spherical_area_unit`（面积，不是定向）
- `earthmesh_geometry` 里一个平面 `is_point_in_circle`

**没有球面定向谓词、没有 in-circle 谓词、没有自适应精度回退。** §12 要求的
"快速 f64 → 接近退化时高精度 → 仍不确定则报 AmbiguousGeometry" 三段式需要新建。

## `earthmesh_refine_planner` 是什么，以及它与 §7 重叠

1462 行，单个 `lib.rs`，只有 `earthmesh_cli` 依赖它，且只从 hydro 交付流程调用。

它**不碰网格**。它做的是逐单元打分与预算分配：

```
CellFeatureTable      每单元的特征值表
RefinementCriterion   判据 trait（已存在！）
CriterionContext      判据上下文（已存在！）
CellScore / CombineRule / CompositeScoreConfig
RefinementBudget / QualityConstraint
TargetLevelMap        每单元目标层级（已存在！）
RefinementDecision / RefinementReport
```

具体判据：`land_cover_entropy_criterion`、`distance_to_river_criterion`、
`distance_to_coast_criterion`、`SpecifiedRegionCriterion`。

**规格 §7 要新建的 `RefinementCriterion` + `CriterionContext`，以及 §6 的目标尺度概念，
这里已经有一份了**，语义相近但不同：现有的产出是离散 `TargetLevelMap`，规格要的是连续
`requested_scale_m` 加四种停止语义（TargetScale / ErrorTolerance / FeatureCoverage /
MeshQuality）。规格没有提到这个 crate 的存在。

**建议**：不要平行再造一个同名 trait。要么扩展现有的（加语义枚举与 `DemandEvidence`），
要么明确写下为什么另起一套。两个 `RefinementCriterion` 共存会是长期的混淆源。

## 为什么 Method-C 至今没有独立出来

不是没人动手，是**没有可切的缝**。

`TriangularMesh` 是三个后端共同消费的类型，而它**定义在 `method_c_mesh/mod.rs` 里**，
字段就是 Method-C 的数据模型：

```rust
pub struct TriangularMesh {
    pub impent: [usize; 12],                       // 二十面体五边形
    pub(crate) m_metadata: Vec<...>,               // mrlm / mrlm_orig / ngr
    pub w_faces: Vec<IcosahedronWFace>,            // mrlw / mrow / ngr
    pub(crate) boundary_rows: Vec<usize>,          // 过渡行
    pub(crate) w_lineage / m_lineage,              // Method-C 谱系
    ...
}
```

`mrow`、`ngr`、`mrlw` 是 Method-C 的过渡行与代号；红绿并不消费它们，而是从这个类型**桥
出去**（`redgreen_mesh_from_triangular`）再桥回来。

体量：`earthmesh_mesh` 下 `method_c_*` 共 **54 个模块目录、17324 行**，非 Method-C 部分
**8772 行**。也就是说这个 crate 的三分之二是 Method-C，而剩下三分之一里最核心的那个类型
也是 Method-C 的。

所以"把 Method-C 拆出去"的第一步不是搬文件，是**先定义一个后端中立的网格类型**——而这正
是 HARP-DV 需要的那个 `MeshState`。两件事是同一件事。

## 对分阶段计划的修正建议

规格的 Phase 顺序是：Phase 1 建 crate → Phase 2 建 patch API → Phase 3 单轮原型。

按上面的事实，Phase 2 应拆开并前置一项：

```
Phase 2a  球面鲁棒谓词（定向、in-circle、自适应回退、AmbiguousGeometry）
Phase 2b  后端中立网格状态 MeshState（这同时是 Method-C 拆分的前提）
Phase 2c  增量球面 Delaunay：点定位、cavity、插点、legalization
Phase 2d  patch：提取、局地 Voronoi、验证、替换
```

2b 是与"Method-C 独立出来"共用的那块地基。先做它，两条路线都受益；跳过它，HARP-DV 会
被迫直接消费 Method-C 的类型，而那正是规格 §2.3 想避免的耦合。

## Phase 1 可以照常开工

Phase 1 只要求 crate 骨架、稳定 ID、配置校验、空运行与报告，**不依赖上面任何缺口**。
按规格 §38 任务 B/C/D 执行。
