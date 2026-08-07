# HARP-DV Phase 1：crate 骨架

日期：2026-08-07
对应：实施规格 §38 任务 B/C/D/E、§35 Phase 1

## 新增文件

```
rust/earthmesh_refine_harp_dv/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── api.rs          HarpDvRequest / HarpDvOutcome / refine_harp_dv
    ├── config.rs       HarpDvConfig + validate
    ├── error.rs        HarpDvError（10 个变体）+ Result
    ├── criteria/mod.rs CriterionSemantics / EvidenceStopReason / DemandEvidence
    ├── report/mod.rs   StopReason / HarpDvRunReport
    ├── state/
    │   ├── mod.rs      SiteId / SiteMobility / AdaptiveSite / AdaptiveMesh
    │   └── id_allocator.rs
    └── tests.rs
docs/HARP_DV_REUSE_MAP.md
docs/HARP_DV_PHASE1_REPORT.md
```

## 修改文件

`Cargo.toml`：workspace members 加入 `rust/earthmesh_refine_harp_dv`。仅此一处。

## 公共 API

`refine_harp_dv`、`HarpDvRequest`、`HarpDvOutcome`、`HarpDvConfig`、`HarpDvError`、
`Result`、`SiteId`、`SiteIdAllocator`、`AdaptiveSite`、`SiteMobility`、`AdaptiveMesh`、
`CriterionSemantics`、`EvidenceStopReason`、`DemandEvidence`、`StopReason`、
`HarpDvRunReport`。每一项带 rustdoc。

## 测试（9 项，全通过）

配置逐字段拒绝并指名字段、patch 预算不得大于网格预算、site id 单调且不重发、
包装网格给每个 M 点唯一身份、无 site 的网格被拒、空请求原样返回网格、两次空运行结果相同、
配置在碰网格之前就被校验、已满足的证据不产生工作。

## 门禁

`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 零警告；
`cargo test --workspace --release` **1597 通过 / 0 失败**（Phase 1 前为 1588，新增 9）。
Method-C 与 red-green 的结果未变。

## 三个刻意的决定

**`refine_harp_dv` 不是 `todo!()`。** 它校验配置，然后如实报告"无事可做"并原样返回网格。
规格禁止生产路径留 `todo!()`；一条会 panic 的路径比一条诚实说自己什么也没做的路径更糟。

**`HarpDvRequest` 里没有 criteria 字段。** 审计发现 `earthmesh_refine_planner`
**已经有**一个 `RefinementCriterion` trait 和 `CriterionContext`，语义与规格 §7 要的不同
（现有产出离散 `TargetLevelMap`，规格要连续 `requested_scale_m` 加四种停止语义）。在这件事
定下来之前声明第二个同名 trait，本身就是混淆源。这是 Phase 2 开工前必须决的一件事。

**`AdaptiveMesh` 包 `TriangularMesh`。** 因为仓库只有这一个网格类型，而它是 Method-C 的
（`mrow`/`ngr`/`mrlw`/`impent` 都是）。这个包装就是后端中立网格类型该在的位置。

## 未解决项，以及 Phase 2 的前置条件

审计（`HARP_DV_REUSE_MAP.md`）的结论是规格 §11 的**八项低层能力一项都不存在**，其下的
增量球面 Delaunay 内核与鲁棒球面谓词也不存在。规格把 Phase 2 写作"实现或整理"，实际是
从零建内核。

因此 Phase 2 建议拆为：

```
2a  球面鲁棒谓词（定向、in-circle、自适应回退、AmbiguousGeometry）
2b  后端中立网格状态（同时是 Method-C 拆分的前提）
2c  增量球面 Delaunay（点定位、cavity、插点、legalization）
2d  patch（提取、局地 Voronoi、验证、替换）
```

以及一个待决问题：**判据 trait 是扩展 `earthmesh_refine_planner` 现有的，还是另起一套。**
