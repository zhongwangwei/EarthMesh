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

## 地基已经落成：`MeshState`

`earthmesh_mesh::MeshState`（`mesh_state/`）是后端中立的三角剖分：**顶点、三角形、每条边
对面的三角形**。它**不带** `mrlm`、`mrow`、`ngr`、`impent`、`boundary_rows`——那些是
Method-C 的嵌套簿记，红绿和 HARP-DV 既用不上也维护不了。

- `from_triangular_mesh` 取 Method-C 网格的中立部分（M 点即 site，W 面的 `im` 即三角形），
  其余留在原处；
- 邻接是**导出**的而不是接收的，边由"两端点排序"作键，同一条边从两侧看是同一个键；
- 构造即校验：越界顶点、退化三角形（角点重复）、非流形边（三个三角形抢一条边）一次全报；
- `validate` 另查邻接对称性——单向指认的邻接走不动；
- `open_edge_count` 是闭合性的度量，闭合球面为零。

索引沿用 canonical 一基、0/1 保留。中立类型是个丢掉这条约定的诱人位置，丢掉它只会把 bug
搬进转换层：这个类型周围每一个读者、写者和测试都从 1 数起。

测试 7 项，含一条 **Euler 校验**（V − E + F = 2）：它保证转换保住的是拓扑，不只是数字。
基础网格与细化后的网格都通过。

**HARP-DV 的 `AdaptiveMesh` 已改为包 `MeshState`**，不再包 `TriangularMesh`。这是中立类型
成立的证明，也是让 HARP-DV 成为并列后端而不是建在某个后端之上的那一步。

## Phase 2a 已完成：球面鲁棒谓词

`earthmesh_mesh::mesh_predicates`。规格 §12 明令拓扑判定不得只靠固定 epsilon，实现的是它
要求的三段式：

```
f64 快路 + 严格误差界（Shewchuk 静态过滤）
  → 行列式离零足够远就直接返回
  → 否则以双双精度重算同一个行列式
  → 仍分不开零就返回 Ambiguous，绝不选分支
```

`orient3d`、`orientation_on_sphere`、`in_circle_on_sphere`。球面上三点的外接圆就是过它们
那个平面与球面的交，所以"在圆内"即"在该平面背离球心的一侧"——与 `orient3d` 是同一个行列式，
按三角形自身绕向去读。

**精确为零与无法判定是两个答案，都会返回。** 四点真共面是关于输入的事实；分不开是关于算术
的事实。`Ambiguous` 刻意不可转成符号：走到这里的调用方必须去做点什么（扰动候选点、扩大
patch、拒绝事务），而不是靠掷硬币继续。

乘积误差用 Dekker 拆分而不是 `mul_add`：后者只在真正融合时给出同样的答案，而是否融合取决于
目标平台——一个在两台机器上判得不一样的谓词，正是这个模块要防的东西。

**第二层是被实测触发过的**：2 万个近退化输入（第三点几乎落在前两点连线上）里，快路只settle
了少数，其余全部走扩展精度并全部决断，零 ambiguous。正确性用一条算术伪造不了的不变量校验
——**交换行列式两行必须变号**，累加里任何一处符号错都会被它抓到。9 个测试。

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
1. 后端中立网格状态                                    ✅ 已完成
2. earthmesh_refine_method_c 搬出（现在是纯文件移动 + 依赖调整）
3. HARP-DV Phase 2a：球面谓词                          ✅ 已完成
4. HARP-DV Phase 2c/2d：增量 Delaunay、patch
```

第 1 步曾是唯一的瓶颈，现已落成，3 与 4 都不再被它堵着。

## 第 2 步不是文件移动（2026-08-07 更正）

上面那句「剩余工作不再是设计而是搬迁」是错的，写它的时候没有量。量过之后：

**阻碍是孤儿规则，不是模块数。** Method-C 是 `TriangularMesh` 上 33 个 `impl` 块里的 97 个
方法。Rust 不允许在别的 crate 里给外来类型写固有 impl，所以在这些方法挂到一个 Method-C
自己拥有的类型上之前，搬文件搬不动任何东西。

**`TriangularMesh` 不是 Method-C 的类型。** 这一点也量反了。看起来私有的字段里，`mrlm`、
`mrlm_orig`、`ngr` 在 nesting 模块之外只被读一次——`voronoi_grid` 填 `ItabW`，因为 gridfile
本身带这三个字段；两个 lineage 在外面一次都没被读。真正只属于 Method-C 的字段只有
`boundary_rows`（mrow 过渡行），写它的只有 `method_c_perimeter_mrows`，生产代码里读它的只有
CLI 的一个计数。所以类型留在 `earthmesh_mesh`，Method-C 需要的是一个包住它的自有类型。

**`method_c_` 前缀不等于归属。** 92 个方法按 Method-C 词汇（thirdm / rad3 / mrow / perim /
nest / spawn）扫描，21 个一个词都不含，其中包括 `spring_global` 与
`spring_global_with_controls`——而中性的 `from_icosahedron` 直接调用后者。所以拆分要逐方法
判断，不能按目录名批处理。

**外部调用面很小。** `method_c_*` 之外的生产代码只调到 18 个方法名：11 个 `spawn_nest*`
重载加 `boundary_rows` 是 Method-C 的，其余 6 个（`from_icosahedron`、`from_relaxed_icosahedron`、
`from_voronoi_gridfile_tables_with_metadata`、`from_cart_hex`、`validate_topology`、
`expand_by_factor`、两个 lineage 访问器）是任何后端都要的。

### 修正后的顺序

```
2a. TriangularMesh 移出 method_c_mesh/，进 primal_dual_mesh/     ✅ 已完成
2b. 逐方法划分 92 个方法：通用的留下，Method-C 的归拢
2c. 新建 MethodCMesh 包住 primal-dual 网格，boundary_rows 挪进去，
    33 个 impl 块改挂 MethodCMesh（Deref 让字段访问不用逐处改写）
2d. 把 method_c_* 模块与 MethodCMesh 一起移入 earthmesh_refine_method_c，
    pub(crate) 提升为 pub，修 CLI / project / GUI 的依赖
```

2c 是唯一有真实风险的一刀，也是唯一必须一次做完的：33 个 impl 块换宿主之后，中间状态编译
不过。CLI 与 GUI 的门禁必须全绿。
