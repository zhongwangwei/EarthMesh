# HARP-DV 自然长度弹性调整：A/B 修订报告

日期：2026-08-14  
计划：`.omx/plans/harp-natural-length-ab.md`  
状态：**复核完成；旧版“OFF 胜出 / No-Go”裁定作废**

## 0. 结论先行

自然长度候选应当保留，当前没有依据关闭它。

旧报告把一个未完成细化、严重失衡的 NXP21 状态当作生产质量优化起点，并把后来被整轮回滚的临时事务计入 `committed`，所以 “OFF 胜出” 不成立。修正这两处后：

| NXP | OFF | FALLBACK（启用、不优先） | CURRENT（生产，前 2 eta pass 优先） |
|---:|---:|---:|---:|
| 12 窗外角 | 62 | 64 | **56** |
| 21 窗外角 | 114 | **89** | 99 |

两个规模上至少一个 natural 臂优于 OFF。NXP21 的 CURRENT 虽不是违规总数最少，但它的最坏偏差、最小角和 `eta_min` 最好：

- OFF：最坏偏差 `10.4792°`，`eta_min=0.858362`；
- FALLBACK：最坏偏差 `10.6355°`，`eta_min=0.859815`；
- CURRENT：最坏偏差 **`10.2484°`**，`eta_min=`**`0.862006`**，且 `<40°` 为 **0**。

因此当前证据支持：

1. natural 候选本身有用；
2. “优先两轮”影响违规总数与最坏尾部之间的 Pareto 取舍；
3. 生产 CURRENT 暂不改；
4. 未经更高规模的同口径验证，不关闭候选、不把优先轮数改为 0。

## 1. 本轮修复

### 1.1 审计随 pass 回滚

旧实现的候选事务一旦局部提交就累加 `*_committed`；如果随后的全局 pass 守卫拒绝整轮，网格会恢复，审计不会恢复。因此旧 `committed` 实际是“曾短暂提交”，不是“最终保留”。

现在：

- `generated` 和 `line_search_attempts` 仍累计所有实际工作；
- `natural/eta/window_committed` 只计通过 pass 守卫、留在最终网格里的移动；
- pass 回滚同时恢复三类 retained 计数；
- low-degree repair 单独计入 `low_degree_committed`；
- 结束时强制断言：

```text
natural_retained + eta_retained + window_retained + low_degree_retained == quality_optimiser_moves
```

新增回归测试锁定“被拒 pass 不得声称 retained move”。

### 1.2 生产语义 checkpoint

旧 A/B 使用 `stalled_by_insertion_alone`，它刻意绕开生产 r-adaptation。旧 NXP21 起点为：

```text
pending=516, unbalanced=4208, worst_scale_ratio=20.89,
min_angle=0.3898°, max_angle=177.8151°
```

这不是生产会交给质量优化器的状态。

现在测试只在 `run_cycles` 的真实边界捕获 checkpoint：生产 refinement、fallback、degree relief、balance relief、multi-ring recovery 全部结束之后，调用 `optimise_mesh_quality` 之前。测试强制要求：

```text
validate == ok
open_edges == 0
max_degree <= 7
pending == 0
unbalanced == 0
```

三臂 clone 同一个 checkpoint，并使用同一个输入网格背景尺度和同一组 criteria。

### 1.3 每轮可解释日志

每个 eta/window pass 现在输出：

- retained / rejected 决策；
- tentative natural/eta/window 提交数；
- 窗外角 `before -> after`；
- worst deviation `before -> after`；
- `eta_min before -> after`；
- `margin_min`、`eta_p1`、`triangles_below_eta_0_89`。

这直接显示一个既有语义：eta 阶段只守 eta、物理需求和尺度，不守 `[40°,80°]` 窗口。因此 eta pass 可以增加窗外角并仍被保留。例如 NXP21 CURRENT：

```text
eta pass 10: outside 137 -> 141
eta pass 14: outside 123 -> 128
```

这不是审计 bug，而是当前目标分阶段设计；后续若调整调度，应针对这条机制，不应再怀疑自然长度公式符号。

## 2. 阶段 A：公式核对的准确结论

在锁定的 `sphere(6)` 六度星、小步长场景上：

- 拉伸、压缩两向的相对长度误差平方和均下降；
- 最差边均向目标长度移动；
- 缺口加权平均边长方向正确；
- 球面半径保持。

准确表述是：

> 在锁定 fixture 上未观察到符号、单位或球面投影缺陷；小步严格降低被测相对长度误差。

这不是对所有网格和所有步长的数学证明。未加权平均入射边长不作为方向断言，因为固定半径球面投影会移除均匀径向分量。

## 3. 共同起点

| NXP | checkpoint sites | pending | unbalanced | below 40 | above 80 |
|---:|---:|---:|---:|---:|---:|
| 12 | 1710 | 0 | 0 | 288 | 290 |
| 21 | 5051 | 0 | 0 | 591 | 712 |

注：质量优化器先运行 low-degree repair，因此其正文日志中的优化起始窗外角可能与 checkpoint 原始计数略有差异；三臂执行完全相同的 low-degree repair。

## 4. 修订后的 A/B 结果

### 4.1 NXP12

| arm | moves | natural retained | eta retained | window retained | low-degree | below 40 | above 80 | outside | worst deviation | eta_min |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| OFF | 2574 | 0 | 2535 | 21 | 18 | 2 | 60 | 62 | 11.1072° | 0.852228 |
| FALLBACK | 3163 | 489 | 2628 | 28 | 18 | 1 | 63 | 64 | 11.1266° | 0.855510 |
| CURRENT | 3225 | 879 | 2317 | 11 | 18 | 3 | 53 | **56** | **10.2876°** | **0.861748** |

CURRENT 同时赢主指标、最坏偏差和 `eta_min`。

### 4.2 NXP21

| arm | moves | natural retained | eta retained | window retained | low-degree | below 40 | above 80 | outside | worst deviation | eta_min |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| OFF | 5366 | 0 | 5315 | 23 | 28 | 1 | 113 | 114 | 10.4792° | 0.858362 |
| FALLBACK | 6407 | 1016 | 5348 | 13 | 30 | 3 | 86 | **89** | 10.6355° | 0.859815 |
| CURRENT | 6907 | 2008 | 4843 | 28 | 28 | **0** | 99 | 99 | **10.2484°** | **0.862006** |

FALLBACK 的违规总数最好；CURRENT 的深尾和最小角最好。两者都在关键轴上优于 OFF。

### 4.3 硬门

六个运行全部满足：

- `MeshState::validate()`；
- 闭合球面；
- `max_degree <= 7`；
- 无不可测三角形；
- `pending=0`；
- `unbalanced=0`；
- retained audit 与 `quality_optimiser_moves` 精确对账。

## 5. 裁定

旧版“结果 3：OFF 胜出”撤销。

当前裁定：**自然长度机制 Go；优先级未形成跨规模的单一最优答案。**

- 关闭 natural 会让 NXP12 的 outside 从 56 增至 62，让 NXP21 从 89/99 增至 114；不应关闭。
- FALLBACK 并未稳定支配 CURRENT：它在 NXP21 的计数更好，但在 NXP12 的计数和两个规模的最坏尾部均更差。
- CURRENT 也未稳定支配 FALLBACK：NXP21 违规总数多 10。
- 所以生产维持 CURRENT 是最保守且有证据的选择；本轮不改默认。

## 6. 下一步

下一项实验不应再改公式，而应只测调度：

1. 保持候选公式、目标场、线搜索不变；
2. 在生产 checkpoint 上扫描优先轮数 `{0, 1, 2}`；
3. 预先声明双指标：outside 优先，worst deviation 为硬尾部守卫；
4. 先跑 NXP12/21；只有同一个优先轮数在两者上不被支配，才跑 NXP30/40；
5. 若仍为 Pareto 交叉，保留当前 2，不继续调参。

本轮没有修改生产优先轮数。

## 7. 证据

完整日志：

- `.omx/logs/harp-natural-length-ab-nxp12-revised.log`
- `.omx/logs/harp-natural-length-ab-nxp21-revised.log`

复现：

```bash
EARTHMESH_TEST_NXP=12 \
  cargo test -p earthmesh_refine_harp_dv --release \
  natural_length_ab_on_the_nxp_proxy -- --ignored --nocapture

EARTHMESH_TEST_NXP=21 EARTHMESH_TEST_SKIP_REPEAT=1 \
  cargo test -p earthmesh_refine_harp_dv --release \
  natural_length_ab_on_the_nxp_proxy -- --ignored --nocapture
```

NXP12 包含逐臂确定性重复；NXP21 为控制成本跳过重复，三臂本身完整执行。

## 8. 最终门禁

| 命令 | 结果 |
|---|---|
| `cargo test -p earthmesh_refine_harp_dv --release` | `103 passed, 0 failed, 4 ignored`；e2e `3 passed` |
| NXP12 修订 A/B | `1 passed`，含三臂确定性重复 |
| NXP21 修订 A/B | `1 passed`，三臂完整、跳过重复 |
| `cargo test -p earthmesh_project --lib` | `91 passed, 0 failed` |
| `cargo clippy -p earthmesh_core -p earthmesh_project -p earthmesh_refine_harp_dv --all-targets -- -D warnings` | 通过 |
| `cargo fmt --all --check` | 通过 |
| `git diff --check` | 通过 |
