# 细化层的 crate 结构：现状与剩余差距

日期：2026-08-07

目标结构（用户给定）：

```text
rust/
├── earthmesh_mesh/
├── earthmesh_boundary/
├── earthmesh_refine/
│   ├── api/
│   ├── hfield/
│   ├── criteria/
│   └── demand/
├── earthmesh_refine_method_c/
├── earthmesh_refine_redgreen/
└── earthmesh_refine_harp_dv/
```

## 已经到位

| 目标 | 状态 |
|---|---|
| `earthmesh_boundary/` | **新建**。中立边界拓扑：`BoundaryRole`（6 种角色）、`LoopType`、`SphericalBoundaryModel`，逐项不变量校验（孤儿 hole、hole 套 hole、捏合环、退化环），`topology_counts` 给出细化必须保持的那对数。6 个测试 |
| `earthmesh_refine/api/` | **新建**。`RefinementBackend` 三选一 + namelist 名字往返 + `serves_criteria_directly`（Method-C 为 false，这是实测结论） |
| `earthmesh_refine/criteria/` | **新建**。`CriterionSemantics` 四种停止语义、`EvidenceStopReason`、`DemandEvidence` |
| `earthmesh_refine/demand/` | **新建**。`RefinementCause`（物理因与簿记因分开计数）、`RefinementDemand`（取最细尺度、最强违反给 witness）、`order_demands`（硬→优先级→id，末项保证跨机一致） |
| `earthmesh_refine/hfield/` | **就位**，以 re-export 形式。h 场是自带测试与自带调用方的数值内核，搬文件是独立一刀 |
| `earthmesh_refine_redgreen/` | 早已存在 |
| `earthmesh_refine_harp_dv/` | 上一步新建，现已改为消费 `earthmesh_refine` 的判据词汇，不再自带一份 |

依赖方向：`mesh`/`boundary` → `refine` → 三个后端。`earthmesh_refine` **不依赖任何后端**，
这条边是三者并列而非成链的保证。

## 关于两个 `RefinementCriterion`

`earthmesh_refine_planner` 里也有一个。**不该合并**：

- planner 的是 `score(ctx, cell_index) -> CellScore`，对预先算好的特征表按索引打分，再做预算分配；
- 规格要的是 `evaluate(cell_geometry, context) -> DemandEvidence`，量一个单元的几何、带停止语义。

"按索引查表"和"量这块多边形"是两件事，一个 trait 覆盖两者会两头不讨好。原本的问题不是重复，
是**同名、异义、分处两个 crate、且没有一句话说明**。现在它们是邻居，`criteria/mod.rs` 顶部
就是那句说明。

## 还没到位：`earthmesh_refine_method_c/`

这是唯一没动的一格，原因在 `HARP_DV_REUSE_MAP.md`：

`TriangularMesh` 定义在 `earthmesh_mesh/src/method_c_mesh/mod.rs`，字段就是 Method-C 的
数据模型（`impent`、`mrlm`、`mrow`、`mrlw`、`ngr`、`boundary_rows`）。三个后端都消费这个
类型，红绿是桥出去再桥回来。`method_c_*` 共 54 个模块 17324 行，非 Method-C 部分 8772 行。

**所以拆分的第一步不是搬文件，是先有一个后端中立的网格类型**——而那正是 HARP-DV 的
`MeshState` 需要的同一个东西。两件事是同一块地基，做一次两边都受益。

现在搬会得到一个 `earthmesh_refine_method_c` 反过来被 `earthmesh_mesh` 依赖（因为核心类型
在里面），依赖方向立刻反转——比不搬更糟。

## 建议顺序

```
1. 后端中立网格状态（HARP-DV Phase 2b 与 Method-C 拆分的共同前提）
2. earthmesh_refine_method_c 搬出（此时是纯文件移动 + 依赖调整）
3. HARP-DV Phase 2a/2c/2d：球面谓词、增量 Delaunay、patch
```

第 1 步是唯一的瓶颈，两条路线都堵在它后面。
