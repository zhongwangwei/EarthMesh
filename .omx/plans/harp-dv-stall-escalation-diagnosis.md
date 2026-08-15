# HARP-DV 停滞升级与终止判据诊断书

日期：2026-08-15
性质：诊断，不含改动建议的实施细节
状态：**已完结（2026-08-15）。最小实验见 §5.1，§4 已按实测改写；§7 第 1 条的补救已实现并被 A/B 否决，见 §6.6**
触发：NXP80 生产规模运行的 pending 审计与细化周期日志

## 1. 结论

`run_cycles` 有**五个**机制共用同一个触发条件 `accepted_this_cycle == 0`——即"这一周期
全球范围内没有任何事务被接受"。

NXP80 的 100 个周期里，**该条件一次都没有成立**，因此五个机制全部没有运行，包括全部的
终止判据。运行只能跑满周期上限退出。

这不是门太严。第一线的度数补救是每周期无条件跑的。问题在于：**一个全局标量守卫着一组
局部补救与终止判断，而全局标量在大网格上恒不为零。**

## 2. 证据

### 2.1 条件恒不成立

`.omx/logs/harp-nxp80-pending-audit.log`，100 个周期的插入数分布：

| 每周期接受的插入数 | 周期数 |
|---|---:|
| **0** | **0** |
| 1–5 | 72 |
| 6–25 | 20 |
| >25 | 8 |

`grep -c "^harp_dv cycle [0-9]+/100: 0 insertions"` 返回 **0**。

七成以上的周期只接受 1–5 次插入——在 70,686 个格子里，这是噪声级别的产出，却足以让
`accepted_this_cycle == 0` 永远为假。

### 2.2 产出与代价严重不成比例

| 区间 | 周期数 | 新增格数 |
|---|---:|---:|
| cycle 1–6 | 6 | +5,604（91%）|
| cycle 7–100 | 94 | +550（9%）|

后 94 个周期每个都付一次全网格 `evaluate`，平均换来 5.9 个格。`unresolved` 在这段里不是
收敛而是震荡上行：cycle 20 为 1，cycle 40 为 3，cycle 60 为 35，cycle 100 为 52。

### 2.3 残留全部卡在度数门

NXP80 pending 审计摘要：

```
ordinary_resolved=0  fallback_resolved=28  blocked=26
refusals=RejectionTally { degree: 676, pentagon: 0, not_insertable: 0,
                          topology: 0, sliver: 0, no_improvement: 0, unmeasurable: 0 }
```

**676 次拒绝全部是度数拒绝，sliver 为 0**（角度门默认已关闭）。

读数注意：**676 是候选尝试被拒的次数，不是 676 个格子**——受审计的是 54 个 pending 格
（28 个被 fallback 解决 + 26 个 blocked），676 是它们合计尝试的失败次数。

另外 `topology: 0` 是一个更强的信息：`tally_refusals`（`cycle/mod.rs:404-407`）把
`PatchTooLarge`、`SurfaceOpened`、`TopologyInvalid`、`CouldNotLegalize` **四种拒绝合并
记入 `topology`**。该字段为 0，说明 `max_patch_triangles = 10,000` 在 NXP80 上一次都没
触发过——它确实没有在限制任何东西。

所以卡住的确实只有 `max_vertex_degree = 7` 这一个门——而它是 gridfile `ItabW` 的
`[i32; 7]` 决定的写出器上限，不是可调偏好。

## 3. 机制

### 3.1 五个共用同一条件的位置

| 行 | 机制 |
|---|---|
| `cycle/mod.rs:3356` | 停滞需求的 fallback 重试 |
| `cycle/mod.rs:3417` | `use_pair_relief`（成对移动缓解） |
| `cycle/mod.rs:3597` | `recover_stalled_regions`——多环 r-adaptation 升级 |
| `cycle/mod.rs:3631` | 位移之后对 `retry_demands` 的批量重试 |
| `cycle/mod.rs:3709` | **全部终止判据** |

### 3.2 哪些跑了、哪些没跑（重要区分）

**每周期无条件运行的：**

- 主需求循环与 `collect_blockers`（`3322`）——所以 `degree_blocked_sites` 的记账是活的；
- **第一线度数补救**（`3449`）：对每个 `degree_blocked_sites` 站点，若它可移动且度数已达
  上限，就沿 `degree_relief_destinations` 尝试移动，直到某条入射边消失。注释写明这是
  "removes one hard writer blocker"；
- 尺度平衡阶段。

日志里每周期 4–28 次的 `r-adaptations` 就是这些在跑。**所以"补救机制完全没运行"是错的。**

**从未运行的：**

- 上述五项全部。其中最相关的是 `recover_stalled_regions`（`3597`），它的种子集合正是
  ：

```rust
let mut recovery_seeds = persistently_stalled.clone();
recovery_seeds.extend(degree_blocked_sites.iter().copied());
recovery_seeds.extend(balance_blocked_sites.iter().copied());
```

  即**专门针对度数阻塞站点的升级手段**，做的是"widen what may move rather than what may
  be inserted"，失败后还会再放宽一环（`RECOVERY_MOVABLE_RINGS + 1`）。

准确表述是：**第一线补救每周期都跑且不足以解决；专为它准备的升级从未被触发。**

### 3.3 终止判据也在同一个开关下

```rust
if accepted_this_cycle == 0 {
    if adapted_this_cycle == 0 {
        stop_reason = ... QualityConstraintReached | NoAcceptedTransactions;
        break;
    }
    // NoProductiveAdaptation 的 adaptation_probe 比较
} else {
    adaptation_probe = None;      // ← 一次插入就把停滞探针清零
}
```

唯一独立于此的正常出口是 `3271` 的 `demands.is_empty()`（→ `AllSatisfied` /
`MinimumScaleReached` / `SourceResolutionReached`）。NXP80 的需求从未清空（残留 54 个
平衡需求），于是只能落到 `MaximumCyclesReached`。

对照：NXP40 在 cycle 11 之后正常停止，`pending = 0`，走的正是 `demands.is_empty()` 这条
出口。**所以这套逻辑在需求能被全部满足时是好的；它缺的是"需求满足不了"时的收敛判断。**

## 4. 为什么只在大规模暴露（已按实测改写）

**初稿的解释是错的。** 初稿说"小网格上全局标量是个不错的代理，随规模退化"。实测证明
退化的不是触发条件，而是第一线补救的充分性。

`accepted_this_cycle` 是一个**全局标量**，守卫着一组**局部补救**。实测显示它在
**两个规模上都从未成立**（NXP40 0 次 / 11 周期，NXP80 0 次 / 100 周期）——只要网格上任何
一处还能接受一次插入，它就为假。因此那五个机制在实践中接近死代码，**与规模无关**。

真正随规模变化的是**第一线度数补救够不够用**：

| | NXP40 | NXP80 |
|---|---:|---:|
| 周期数 | 11 | 100 |
| 有待升级对象的周期 | 10 | **100** |
| 升级实际触发 | **0** | **0** |
| degree-blocked 站点·周期累计 | 442 | 2,755 |
| tier-1 单点移动 | 234 | 756 |
| tier-1 成对移动 | 0 | 0 |
| **每个 blocked 站点·周期的清除率** | **0.53** | **0.27** |
| 最终结果 | pending 0，blocked 0 | pending 54，blocked 26 |

所以准确的说法是：**升级手段在任何规模上都拿不到；小规模上这不要紧，因为 tier-1 一个人
就把活干完了。到 NXP80，tier-1 的单位清除率掉了一半，剩下的没有任何后备手段接手。**

成对缓解（`use_pair_relief`）在两个规模上都是 0 次——它也挂在同一个条件上。

## 5. 可证伪的预测

若本诊断成立，以下应当同时观察到：

1. NXP80 的 100 个周期中，`recover_stalled_regions` 的调用次数为 **0**；
2. 同期 `degree_blocked_sites` 在多数周期**非空**——即"有待升级的对象，但升级从不触发"；
3. `adaptation_probe` 从未累积到可比较的两个周期（每周期都被 `else` 分支清零）；
4. NXP40 上第 1 条同样为 0，但第 2 条为空——因为它的需求最终被满足，属于健康路径。

若第 2 条为空（升级从不触发**且**没有待升级对象），则本诊断不成立，问题在别处。

### 5.1 实测结果（2026-08-15）

| 预测 | 结果 |
|---|---|
| 1. NXP80 上 `recover_stalled_regions` 调用 0 次 | **证实**：fired 0 / 100 周期 |
| 2. 同期 `degree_blocked_sites` 多数周期非空 | **证实且更强**：100/100 周期非空，累计 2,755 站点·周期 |
| 3. `adaptation_probe` 从未累积 | **证实**（fired=0 蕴含每周期都走 `else` 清零） |
| 4. NXP40 上第 1 条为 0，**第 2 条为空** | **前半证实、后半证伪**：NXP40 也是 fired 0，但 eligible 10/11 周期、累计 442 站点·周期 |

第 4 条的证伪是本次实验最有价值的产出：它推翻了"小规模上没有停滞所以健康"的假设，
迫使 §4 改写。NXP40 之所以健康，不是因为没有卡住的站点，而是因为 **tier-1 把它们全清了**
（234 次移动，最终 pending 0、refusals 全零）。

## 6. 最小验证实验

**只加只读计数，不改任何行为。** 在 `run_cycles` 内累计并在结束时打印一行：

- `escalation_eligible`：`degree_blocked_sites` 非空的周期数；
- `escalation_fired`：`accepted_this_cycle == 0` 成立的周期数；
- `degree_blocked_total`：各周期 `degree_blocked_sites.len()` 之和；
- `tier1_degree_moves`：第一线度数补救（`3449`）实际提交的移动数。

在 NXP40 与 NXP80 各跑一次。预期：NXP80 上 `escalation_fired = 0` 而
`escalation_eligible` 远大于 0；NXP40 上两者都接近 0。

成本：NXP40 约 7 秒，NXP80 约 7 分钟（checkpoint 路径即可，不必跑完整路径）。

## 6.5 关键前情：局部化调度已经被试过并失败

`cycle/mod.rs:3347-3355` 记录了这件事，就在被锁住的第一个机制正上方：

> **Tried and rejected**: offering the broader ladder to every demand that was also refused
> last cycle, regardless of what the rest of the globe managed. **It is the right shape --
> one neighbourhood's stall is not evidence about another's** -- but the early cycles have
> thousands of demands that stall twice and then resolve on their own, and giving each of
> them the twenty-candidate ladder took the crate's own test suite **from 32 seconds past
> 30 minutes without finishing**. **Bounding it by persistence is not enough; it would need
> a bound on how much broadening a cycle may buy.**

这段注释同时给出三个信息：

1. **本诊断的核心判断被独立确认**——"one neighbourhood's stall is not evidence about
   another's"，全局标量守卫局部补救确实是错的形状；
2. **单纯把触发条件改成局部会爆炸**——早期周期有成千上万个"卡两轮然后自己解决"的需求，
   给它们每个都上二十档候选梯子，测试套件从 32 秒涨到 30 分钟仍不结束；
3. **缺的那味药已经被点名**——"a bound on how much broadening a cycle may buy"。按持久性
   过滤是不够的，必须限制**每周期总共能买多少扩展**。

因此任何"把 `accepted_this_cycle == 0` 改成局部条件"的方案，若不带每周期总量上限，
就是在重走这条已知会失败的路。

## 6.6 有界局部调度已实测：不可行

§7 第 1 条那个"有界局部停滞调度"已经实现并做了 A/B（`local_recovery_ab_on_the_nxp_proxy`，
私有 `LocalRecoveryPolicy`，生产默认 `OFF`）。

NXP40，checkpoint 路径，两臂同起点：

| arm | sites | pending | unbal | cycles | 局部升级 | runtime |
|---|---:|---:|---:|---:|---|---:|
| OFF | 17,922 | 0 | 0 | 11 | 0 周期 / 0 种子 | **4.4 s** |
| LOCAL-2/16 | 17,815 | 0 | 0 | 16 | 8 周期 / 43 种子 | **577.3 s** |

（`LOCAL-2/16` = 连续阻塞 ≥2 周期入队，每周期最多 16 个种子。）

**慢 131 倍，pending 没有任何改善，而且改变了网格**——少了 107 个站点，角度分布也不同
（min 5.2550 → 3.3304，max 143.5226 → 138.7563）。43 个种子吃掉 573 秒，**每种子约 13.3 秒**。

这正是 §6.5 那条注释警告的形状，只是换了机制：注释针对的是 fallback 候选梯子，这次是多环
恢复。**同一个教训在第二个机制上被经验证实。**

收紧上界救不回来：即使降到每周期 1 个种子，NXP80 的 100 个周期也要多付约 1300 秒，而收益
未知——NXP40 上收益为零，因为那里本来就没有残留。

### 结论修正

问题**不在触发条件的粒度**，而在 `recover_stalled_regions` 的单位成本：它把种子分成连通
分量，每个分量在 4 环范围内做最多 `MAXIMUM_RECOVERY_SWEEPS` 轮扫掠，且每次候选移动都要
给整个区域打分（`region_score`）。在这个成本降下来之前，任何放宽触发条件的方案都不可行。

因此 §7 的方向排序作废，改为：

1. **先降 `recover_stalled_regions` 的单位成本**（区域打分改增量、减少扫掠轮数），
   使"每种子 13.3 秒"降到可以每周期跑几个的量级。这是所有后续方案的前置。
2. 之后再重测有界局部调度。
3. 终止判据独立于升级判据——这一条与成本无关，仍然成立且独立可做。

实验装置保留在 `LocalRecoveryPolicy` 里，生产默认 `OFF`，随时可重测。

## 7. 若诊断成立，可能的方向（已被 §6.6 部分作废）

按侵入性从低到高：

1. **有界的局部停滞调度**——不是简单地把条件改成局部（§6.5 已记录那样会失败），而是
   带上注释点名的那个总量上限。可测的形状：

   - 站点须连续阻塞若干轮才进入队列（沿用现有 `persistently_stalled` 的思路）；
   - **每周期最多处理固定数量**的持久 blocked 站点——这就是"a bound on how much
     broadening a cycle may buy"；
   - 排序确定性：按阻塞轮数、degree 拒绝次数、站点 id；
   - 每个区域每轮只允许一次 pair / multi-ring 尝试；
   - 验收仍由现有的 degree ≤ 7、物理、尺度、拓扑门统一把关，一个都不放宽；
   - 判据：blocked 是否从 26 下降，以及额外耗时是多少。
2. **终止判据独立于升级判据**——现在两者绑在同一个 `if` 上，但它们回答的是不同问题
   （"要不要再努力一次"与"还有没有可测量的进展"）。
3. **让插入策略不把度数推上去**——候选梯子的选点方式。工作量最大，且会改变输出。

三条都会改变输出，都需要各自的质量 A/B，**不在本诊断的范围内**。

## 8. 明确不做的事

- **不放宽 `max_vertex_degree = 7`。** 它是 gridfile `ItabW` 行宽 `[i32; 7]` 的物理限制，
  度数 8 的顶点写不出去。

  但要区分清楚：它是**提交时**的门，不是**每一步**的门。`check()` 读的是改动后、提交前的
  状态，所以复合事务（如现有的 `propose_pair_move_cached`）内部完全可以暂时超过 7，只要
  统一提交前回到 ≤7。这条路不削弱门，值得在上面第 1 条的设计里利用。
- **不重开 `min_triangle_angle_deg`。** 它已经是 0（关闭），并且今天的 A/B 已确认残留不是
  sliver 造成的（`sliver: 0`）。

  归因上要克制：NXP80 上 `worst_deviation` 被钉在 92.577117 只能证明**当前优化器碰不到
  那个三角形**。它既不能单独证明"先放后修"这条路线整体失败，也不能证明那个三角形是关闭
  角度门造成的。要完成归因，必须追踪该三角形的**生成周期**——它是哪一轮插入产生的、当时
  是否有别的合法选择。在此之前不应把它当作任何一方的论据。
- **不修改 `require_closed_surface` 或 `max_patch_triangles`。** 前者是回滚机制的正确性
  前提，后者是内存上界且在两环 patch（约 20 个三角形）面前离 10,000 差三个数量级，
  根本没有在限制任何东西。
- **不把 `max_cycles` 当性能手段。** 降低它会改变输出与 `stop_reason`，属于改预算而非
  修缺陷。

## 9. 与其他工作的关系

- `.omx/plans/harp-dv-voronoi-performance-plan.md`（已执行）：性能，与本诊断无关，但其
  §3.2 已把"新增 stagnation/no-progress 退出"列为非目标，正是指本诊断第 7 节第 2 条。
- `.omx/plans/harp-dv-seeded-objective-plan.md`（待执行）：性能，同样无关。
- 本诊断若立项，属于**行为变更**，须走独立的质量 A/B，不得与上述性能工作合并提交。
