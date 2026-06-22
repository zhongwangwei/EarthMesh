# R0 — Fix Queue 建立报告

> 阶段：R0（建立修复队列与规则）· 未修改任何 `src/rust` 代码
> 日期：2026-06-22 · 配套：[FIX_QUEUE.md](../FIX_QUEUE.md) · [FIX_PLAN.md](../FIX_PLAN.md) · [RELEASE_TRACKER.md](../RELEASE_TRACKER.md)

## 改了什么
- 在 `docs/reviews/v3_mesh_audit/` 下新增：`FIX_QUEUE.md`、`FIX_PLAN.md`、`RELEASE_TRACKER.md`、本报告。
- 把 10 份审查/设计报告（01–10）+ FINAL 的发现，转为 **31 个修复条目 / 32 个 ID**（P0×2 / P1×13 / P2×10（11 ID，P2-006/007 合并）/ P3×6）。
- 定义 R0–R10 阶段（允许修改/禁止/验收）与 alpha2/alpha3/alpha4/beta1 版本计划（含 release blockers）。

## 为什么
- 审查产出是"诊断 + 设计"，需要一份**可执行、可追踪、单线落地**的队列，避免直接跳进大重构（工程原则 2）。
- 明确优先级与阶段边界，保证"先修会崩/会错/会损坏数据的问题"，再补验证器，最后才架构（工程原则 1/2）。

## 如何验证（本阶段 = 文档，无代码测试）
- `ls docs/reviews/v3_mesh_audit/` 含 4 个新文件 + `fix_reports/`。
- FIX_QUEUE 每 item 含 14 字段（ID/Title/Priority/Files/Problem/Why/Fix/Tests/Risk/Size/Deps/Phase/Milestone）。
- FIX_PLAN 含 R0–R10；RELEASE_TRACKER 含 alpha2–beta1 + blockers。
- `git status`：仅 `docs/reviews/` 变更（无 `src/rust` 改动）。

## 是否改变用户行为
- 否。纯文档/规划。

---

## Top 10 fixes（来自 FIX_QUEUE，按优先级+影响）

| 排名 | ID | 修复 | 优先级 | 阶段 |
|------|----|------|--------|------|
| 1 | EM3-P0-001 | olam_delaunay 半径容差（mesh 测试红） | P0 | R1 |
| 2 | EM3-P0-002 | coupling CSV 占位列诚实化（防静默错误数据） | P0 | R1/R5 |
| 3 | EM3-P1-004 | overlay Σ fraction=1 守恒校验 | P1 | R2 |
| 4 | EM3-P1-009 | 质量度量+门禁（earthmesh_quality） | P1 | R2 |
| 5 | EM3-P1-001 | 几何球面/等面积面积（修平面失真） | P1 | R3 |
| 6 | EM3-P1-002 | MERIT 跨 180° tile 选择 | P1 | R4 |
| 7 | EM3-P1-008 | hydro buffer/simplify 改 km/投影 | P1 | R4 |
| 8 | EM3-P1-010 | river-mouth/estuary/coastline/orphan + CaMa | P1 | R5 |
| 9 | EM3-P1-011 | score-based + budget + repair（替代纯布尔阈值） | P1 | R7 |
| 10 | EM3-P2-003 | ProjectConfig 项目配置层（零迁移） | P2 | R6 |

## 第一批应该修的 3 个 patch（R1，低风险、隔离、即时收益）

> 前置：**先合并工作树中的 OLAM 外部改动并定版**（否则 mesh 基线漂移）。

1. **EM3-P0-001 olam 半径容差** — 改 ≤4 行测试（或 1 处 renormalize）。验证 `cargo test mesh --test olam_delaunay_mesh`。让 `make test` 转绿，恢复 CI 基线。
2. **EM3-P1-007 default " /tmp" + EM3-P2-002 fmt** — 去前导空格+temp_dir（`core/lib.rs:766-790`）；固定 rust-toolchain + 一次性 `cargo fmt`（独立纯格式提交）。验证 `cargo test core` + `make fmt`。
3. **EM3-P2-001 workspace 根 Cargo.toml** — 加 `[workspace] members`，共享 target。验证 `cargo build --workspace` + `make`。

> 这 3 个都不触碰 refine/geometry/coupling 内核算法，互不冲突，可单线快速落地。

## 不应该现在修的大改动（推迟到 R10/v3.x）
- 一次性重写 mesh(22k)/cli(36k) 超大 lib.rs（EM3-P3-001）——应渐进拆分，绿测约束。
- 引入重型第三方几何库做 robust clipping（EM3-P3-002）——先做局地投影+球面面积(R3)，库选型后评估许可/体量。
- RefinementCriterion plugin + 多目标优化全量（EM3-P3-003）——R7 先做基础 planner，多目标推迟。
- Rust 化全部 hydro eval/ranking/HTML、收敛 Rust/Python 双实现（EM3-P3-005）——需先定 Python `util/` 去留。
- GUI 完整重写（EM3-P3-006）——R8/R9 增量，R10 收尾；不一次性爆改。
- 在 OLAM 外部改动未定版前，落地任何 mesh 内核 patch（基线漂移）。

## 未完成 / 待决策（移交后续）
- OLAM 工作树外部改动需用户合并定版（R1 前置）。
- 是否引入 `earthmesh_project`/`earthmesh_quality`/`earthmesh_refine_planner` 三个新 crate（架构决策）。
- Python `util/` 长期去留（决定 R10 范围）。
- buffer 单位 degree→km 变更对既有算例的兼容策略。
