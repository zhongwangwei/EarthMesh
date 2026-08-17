# HARP-DV 自然长度弹性调整：缺陷核对与单变量 A/B 计划

日期：2026-08-14  
状态：待 Claude Code 执行  
性质：先诊断、后决定；未经 Go 判定不改生产默认

## 1. 目标

回答两个彼此独立的问题：

1. 当前 `natural_length_destination` 的方向、单位和投影是否正确；
2. 在 HARP-DV 最小角事务门默认关闭（`0°`）后，自然长度候选对最终 `[40°,80°]` 角度质量究竟是有益、无效，还是因贪心调度而有害。

本计划不重新接入旧的区域 Laplacian 弹簧，也不先调参数。先锁公式，再做同输入、单变量对照。

## 2. 已知事实

### 2.1 当前自然长度调整已经在生产 HARP 路径中运行

- `rust/earthmesh_refine_harp_dv/src/cycle/mod.rs:1155-1215`
  - 从 `CellCriterion::target_scale_m_at` 构造目标尺度场；
  - 在当前 HARP 邻接图上按 `TARGET_SCALE_GRADIENT=0.3` 做梯度限制。
- `rust/earthmesh_refine_harp_dv/src/cycle/mod.rs:1853-1885`
  - `natural_length_destination` 计算带自然长度的弹性目标点；
  - `desired = 1.9046256 * (h_i+h_j)/2`；
  - `delta = (desired-length)/length`，`weight=delta²`。
- `rust/earthmesh_refine_harp_dv/src/cycle/mod.rs:2492-2512`
  - 目标场在优化开始时计算一次并冻结。
- `rust/earthmesh_refine_harp_dv/src/cycle/mod.rs:2600-2625`
  - 前两个 eta pass 将自然长度候选排第一；
  - 后续 eta pass 排在 eta-ascent 后；
  - window pass 排在 window/eta-ascent 后。
- `rust/earthmesh_refine_harp_dv/src/cycle/mod.rs:2712-2722`
  - `NATURAL_LENGTH_PASSES=2`，线搜索为五档。

因此 NXP6/12/21/30/40 的现有角度结果已经包含自然长度调整，不能把它当作尚未启用的新功能。

### 2.2 旧弹簧的负结果不能直接归因到当前实现

- `rust/earthmesh_mesh/src/spring_regional_dynamics/mod.rs:5-10,54-85`
  - 旧区域弹簧是零自然长度 Laplacian：每轮直接移到邻居均值。
- `docs/mesh_construction_technical_guide.md:1327-1412`
  - 旧接线先因 `ngr=1` 实际不移动；
  - 放开后又把目标取自当前网格，质量恶化；
  - 改成判据目标和梯度受限场后曾显著改善。
- `docs/mesh_construction_technical_guide.md:2765-2783`
  - 当前自然长度候选在 NXP41 上有正收益，但单独收益有限；主要收益来自后续 eta/window ascent。

### 2.3 当前测试缺口

- `rust/earthmesh_refine_harp_dv/src/cycle/tests.rs:1615-1631`
  - 只验证弧长已经是米，以及目标函数返回 `Some`；
  - 没验证拉伸/压缩方向；
  - 没验证小步后边长误差下降；
  - 没隔离自然长度候选对最终结果的净贡献。

## 3. 不做事项

1. 不调用 `spring_dynamics_regional_one_based`；它没有自然长度，与渐变尺寸场不匹配。
2. 不改 `TARGET_SCALE_GRADIENT=0.3`；已有冻结目标场诊断表明它不是当前首要变量。
3. 不先改 `CELL_SCALE_TO_EDGE_LENGTH`、线搜索步长或 `NATURAL_LENGTH_PASSES`。
4. 不新增 GUI/Project/Namelist 开关。
5. 不修改 0° 默认、不恢复 25° 硬门。
6. 不用 NXP40 做第一轮诊断；先用 NXP12/21，只有 Go 后才跑高成本规模。

## 4. 阶段 A：锁定公式方向与单位

### A1. 扩充现有单测

文件：`rust/earthmesh_refine_harp_dv/src/cycle/tests.rs`

保留现有 `target_field_and_natural_length_use_metres_once`，增加两个独立断言场景：

#### 拉伸场景

1. 使用 `sphere(6)` 和一个可移动的普通六度站点；避免基础五边形。
2. 计算该站点全部入射边的平均弧长 `mean_edge_m`。
3. 构造常量目标尺度：
   `h_target = 1.10 * mean_edge_m / CELL_SCALE_TO_EDGE_LENGTH`。
4. 调用 `natural_length_destination`。
5. 用现有 `projected_step(here, destination, 0.03125)` 走一个小步。
6. 断言：
   - 平均入射边长增加；
   - `sum(((length-desired)/desired)^2)` 严格下降；
   - 点仍在原球面半径上。

#### 压缩场景

同上，但使用：
`h_target = 0.90 * mean_edge_m / CELL_SCALE_TO_EDGE_LENGTH`。

断言：

- 平均入射边长减少；
- 相对长度误差平方和严格下降；
- 点仍在球面上。

### A2. 失败时的诊断输出

单测失败信息必须打印：

- `site`；
- `mean_edge_before/after`；
- `desired`；
- `error_before/after`；
- 目标位移与实际投影步长。

### A3. A 阶段验收

- 两个方向测试均通过；
- `cargo test -p earthmesh_refine_harp_dv --release target_field_and_natural_length -- --nocapture` 通过；
- `cargo fmt --all --check` 通过。

若任一方向失败：停止，不做 A/B。先修 `natural_length_destination` 的方向/投影/单位，再重新开始本计划。

## 5. 阶段 B：增加只供内部测试使用的自然长度模式

### B1. 保持生产入口不变

文件：`rust/earthmesh_refine_harp_dv/src/cycle/mod.rs`

保留现有生产签名：

```rust
fn optimise_mesh_quality(...) -> Result<(usize, AngleWindowSurvey)>
```

让它调用一个内部实现：

```rust
fn optimise_mesh_quality_with_natural_length(
    ...,
    natural_length_enabled: bool,
    natural_length_priority_passes: usize,
) -> Result<(usize, AngleWindowSurvey, QualityMoveAudit)>
```

生产包装器固定传：

```rust
natural_length_enabled = true
natural_length_priority_passes = NATURAL_LENGTH_PASSES // 当前为 2
```

不得把这两个值暴露为用户配置。

### B2. 三个实验臂

定义三臂，其他代码与输入完全一致：

| 实验臂 | natural 候选 | 优先轮数 | 用途 |
|---|---|---:|---|
| OFF | 完全不生成 | — | 判断没有弹性候选时的结果 |
| FALLBACK | 生成 | 0 | 判断候选本身是否有价值，避免抢占 eta-ascent |
| CURRENT | 生成 | 2 | 当前生产语义 |

注意：仅把 `NATURAL_LENGTH_PASSES` 改成 0 **不等于关闭**，因为现有代码仍会把 natural 放在 eta-ascent 后。OFF 必须让 `natural=None`，在所有 phase 中完全移除。

### B3. 最小审计字段

新增私有、非序列化的 `QualityMoveAudit`，仅用于测试和 stderr 诊断：

```text
natural_generated
natural_line_search_attempts
natural_committed
eta_generated / eta_line_search_attempts / eta_committed
window_generated / window_line_search_attempts / window_committed
```

实现要求：

- 候选数组保留来源标签，不靠数组下标在测试侧猜来源；
- “generated”是目标点为 `Some` 的次数；
- “line_search_attempts”是实际进入 `propose_move_cached` 的次数；
- “committed”只统计 `Acceptance::Committed`；
- 生产报告 schema 不变；
- 不增加新依赖。

## 6. 阶段 C：同起点、单变量 A/B

### C1. 测试构造

文件：`rust/earthmesh_refine_harp_dv/src/cycle/tests.rs`

新增忽略测试，例如：

```rust
#[test]
#[ignore = "bounded natural-length A/B; run explicitly"]
fn natural_length_ab_on_the_nxp_proxy()
```

测试必须：

1. 读取 `EARTHMESH_TEST_NXP`，默认 12；只接受 `>=6` 的整数。
2. 创建一次 `sphere(nxp)`。
3. 创建一次 `steep_target(&mesh)`。
4. 使用现有 `stalled_by_insertion_alone` 得到同一个“细化完成、质量优化未运行”的起点。
5. 从该起点 clone 三份，分别运行 OFF/FALLBACK/CURRENT。
6. 三臂使用相同：
   - `HardGates::default()`（当前默认角门 0°）；
   - `limits(40, 200_000)`；
   - 冻结背景尺度；
   - eta/window pass 数；
   - 线搜索；
   - 候选顺序（除 natural 的启用和优先轮数外）。

### C2. 每臂输出

打印单行表格：

```text
nxp arm sites moves natural_commits eta_commits window_commits
pending unbalanced worst_scale_ratio below40 above80 outside
min_angle max_angle worst_deviation eta_min eta_p1 margin_min runtime_s
```

### C3. 每臂硬断言

每臂均必须满足：

- `mesh.state().validate()` 成功；
- `open_edge_count == 0`；
- `max_degree <= 7`；
- `pending_after <= pending_before`；
- `unbalanced_after <= unbalanced_before`；
- 输出无不可测三角形；
- 相同实验臂重复运行得到逐位相同 `MeshState` 和相同 audit。

### C4. 运行顺序

先运行：

```bash
EARTHMESH_TEST_NXP=12 \
cargo test -p earthmesh_refine_harp_dv --release \
  natural_length_ab_on_the_nxp_proxy -- --ignored --nocapture
```

通过后运行：

```bash
EARTHMESH_TEST_NXP=21 \
cargo test -p earthmesh_refine_harp_dv --release \
  natural_length_ab_on_the_nxp_proxy -- --ignored --nocapture
```

## 7. 预注册判读规则

主指标：

```text
outside = angles_below_40_deg + angles_above_80_deg
worst_deviation = max(40-min_angle, max_angle-80, 0)
```

### 结果 1：CURRENT 胜出

满足全部条件：

- NXP12、NXP21 上 `CURRENT.outside <= OFF.outside`；
- 至少一个规模严格减少；
- 两个规模上 `CURRENT.worst_deviation <= OFF.worst_deviation`；
- pending/unbalanced 不增加；
- `natural_committed > 0`。

裁定：当前自然长度机制有效，进入阶段 D。

### 结果 2：FALLBACK 胜出但 CURRENT 变差

条件：

- FALLBACK 优于 OFF；
- CURRENT 劣于 FALLBACK。

裁定：公式大概率无 bug，问题在候选优先级/贪心路径。生产候选应保留，但 `NATURAL_LENGTH_PASSES=2` 需重新评估；进入阶段 D，只扫优先轮数。

### 结果 3：OFF 胜出

条件：OFF 在任一规模上同时拥有更少 outside 和不差的 worst deviation，且无硬门差异。

裁定：停止调参。逐站点记录首个 natural 提交造成的后续路径差异；检查接受目标是否允许“局部 eta 改善但全局窗口更差”。不要接旧弹簧。

### 结果 4：三臂逐位相同

裁定：检查 audit：

- `natural_generated == 0`：目标场或站点筛选未覆盖，属于接线问题；
- generated > 0、attempts == 0：投影/步长生成问题；
- attempts > 0、committed == 0：候选被当前目标完全支配；
- committed > 0 但最终逐位相同：检查回滚/后续覆盖，不得凭最终摘要判断“弹簧生效”。

## 8. 阶段 D：只在 Go 后扫优先轮数

只比较：

```text
natural_length_priority_passes = 0, 2, 4
```

规则：

- natural 始终启用；
- 只改变前置优先轮数；
- NXP12、NXP21 两个规模；
- 不同时改步长、梯度或 pass 总数。

采用条件：某一档在两个规模上 outside 均不高于当前 2，至少一个严格降低，同时 worst deviation、pending、unbalanced 均不退化。否则保留 2。

## 9. 阶段 E：高 NXP 验证

只有阶段 C/D 得出明确 Go 后才运行 NXP30；NXP30 也通过后才运行 NXP40。

当前生产语义（CURRENT、角门 0°）基线：

| NXP | below40 | above80 | outside | min | max | pending |
|---:|---:|---:|---:|---:|---:|---:|
| 12 | 3 | 53 | 56 | 39.6151° | 90.2876° | 0 |
| 21 | 0 | 99 | 99 | 40.9036° | 90.2484° | 0 |
| 30 | 6 | 154 | 160 | 37.3855° | 91.0665° | 0 |
| 40 | 22 | 221 | 243 | 36.7917° | 92.7105° | 0 |

高 NXP 验证必须与这些基线使用同一 fixture、同一 release 构建和同一统计口径。

注意运行成本：现有三档角门对照中，NXP30 约 617 秒，NXP40 约 1693 秒。不要在未通过低成本门前重复高成本实验。

## 10. 最终生产改动门

只有同时满足以下条件才能改生产常数或候选顺序：

1. 方向/误差单测通过；
2. NXP12、21、30 均通过预注册收益门；
3. NXP40 不出现方向相反的回归；
4. pending/unbalanced/degree/closure 全部不退化；
5. 确定性测试逐位一致；
6. 总耗时增加不超过 CURRENT 的 20%，或有相应的 outside 降幅证明成本合理。

若只有 NXP12 改善，不能改生产默认。

## 11. 完整验证命令

```bash
cargo test -p earthmesh_refine_harp_dv --release
cargo test -p earthmesh_project --lib
cargo clippy -p earthmesh_core -p earthmesh_project \
  -p earthmesh_refine_harp_dv --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

若生产代码净行为未改，仍需跑上述测试，因为内部包装器和候选标签位于生产模块。

## 12. 交付报告格式

Claude Code 最终应提交：

1. 改动文件列表；
2. A 阶段拉伸/压缩断言结果；
3. NXP12/21 三臂表；
4. 若 Go，再附 NXP30/40 表；
5. 每臂候选 generated/attempted/committed；
6. 明确裁定：公式 bug、调度问题、有效、无效或证据不足；
7. 未经 Go 不修改生产默认；
8. 若实验代码只用于诊断，保留一个小型回归测试，删除一次性打印与临时开关。

## 13. Claude Code 可直接使用的任务说明

```text
按 .omx/plans/harp-natural-length-ab.md 执行。

先做阶段 A 的方向/误差回归测试；失败立即停在根因修复，不做参数实验。
然后实现私有的 natural-length OFF/FALLBACK/CURRENT 三臂，不暴露用户配置，保持生产包装器为 CURRENT。
从同一个 refinement-only MeshState clone 三臂，先跑 NXP12，再跑 NXP21，并按计划中的字段输出和断言。
严格使用预注册判读；未达到 Go 不跑 NXP30/40、不调梯度、不改步长、不接旧 Laplacian。
若 Go，再按 0/2/4 扫自然长度优先轮数，最后按门控运行 NXP30/40。
完成后运行完整 test/clippy/fmt/diff-check，并用表格报告结果、候选提交计数和最终裁定。
```

