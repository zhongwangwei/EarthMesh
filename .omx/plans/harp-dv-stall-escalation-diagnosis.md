# HARP-DV 停滞升级与终止判据诊断书

日期：2026-08-15
性质：诊断，不含改动建议的实施细节
状态：**2026-08-16 更新。§6.6 的否决已被 §6.8 推翻——补救在 NXP80 上清光了全部残留。下一步见 §6.8 末。**
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

### 6.6.1 单位成本已降 23.5%，仍差两个数量级（2026-08-16）

`region_score` 的第一个循环为收集 `measured_sites` 走了每个区域站点的 fan，第二个循环又
走一遍。改成走一遍后重测（同 fixture、同两臂）：

| | 改前 | 改后 |
|---|---:|---:|
| LOCAL-2/16 臂（43 个种子） | 577.3 s | **441.6 s** |
| 每种子 | 13.3 s | **10.3 s** |

行为完全一致：同样 43 个种子、8 个升级周期、pending 0 → 0、17,815 sites、同样的角度分布。

对照 OFF 臂的 4.2 s，**仍然慢 105 倍**。要让局部升级可以每周期运行，每种子需要约 0.1 s，
还差约 100 倍。

### 剩余差距的形状：O(区域²)

`recover_one_region` 的成本是：

- 区域 = `RECOVERY_MOVABLE_RINGS + 1` = 4 环 ≈ 61 个可动站点；
  `measured_sites`（区域 + 各自邻居）≈ 91；
- 每个站点：1 次 `before` 打分 + 每个 `balance_destinations` / `degree_relief_destinations`
  各 1 次 + 每个 `degree_relief_pairs` 各 1 次 ≈ 13 次**全区域**打分；
- `MAXIMUM_RECOVERY_SWEEPS = 4` 轮。

即 4 × 61 × 13 × 91 ≈ **每区域 43 万次元胞构造**。打分是 O(区域)，候选数也是 O(区域)，
所以每区域是 **O(区域²)**。

把 `region_score` 改成增量式（只重算候选移动实际改变的元胞，与 §已完成的 `GuardCells`
同一形状）会把它降到 O(区域)，约 **91 倍**——正好是缺的那个量级。

### 为什么不现在做这个重写

价值链是悬空的：

1. `recover_stalled_regions` 在生产中触发 **0 次**，优化它对当前生产零影响；
2. 它的价值完全取决于"局部升级能否清掉 NXP80 的 26 个 blocked 格子"，而这从未验证——
   NXP40 上收益为零（那里本来 pending 就是 0），代价是网格改变（少 107 个站点）；
3. `RegionScore` 含极值（`worst_ratio`、`negated_min_angle_deg`），要支持删除就需要可删的
   多重集，不是小改动。

**先验证价值再决定重写**：在 NXP80 上跑一次 LOCAL 臂，看 26 个 blocked 是否下降。按每种子
10.3 s 估算需 2–6 小时，是一次性可判定支出，优于先重写数小时再发现实验本身无价值。

### 6.6.2 单位成本再降 5.7 倍，价值实验因此跑得起来（2026-08-16）

`region_score` 的元胞现在跨候选缓存：事务已经知道一个候选扰动了哪些站点
（`sites_touching` 的结果），那些现算不入缓存，其余取缓存，提交则清空。

途中两个自造的坑，都由采样发现：

1. 第一版**零收益**——收集 `measured_sites` 的循环在缓存被查询之前就走遍了每个区域站点
   的 fan。改成一次查找同时服务收集与扫掠。
2. 改对后仍不快——从缓存克隆 6 元素的 `Vec` 与走一遍环代价相当。条目改用 `Rc` 后兑现。

| | 每种子 | LOCAL 臂 |
|---|---:|---:|
| 起点 | 13.3 s | 577.3 s |
| 修双走环 | 10.3 s | 441.6 s |
| **加区域元胞缓存** | **1.82 s** | **78.1 s** |

累计 **7.4 倍**，每一步结果都逐项一致。采样显示 `region_score` 仍占 75%，其中约一半是每个
候选**必须**重测的 2 环脏站点——正确性下限。所以 1.82 s 已接近该设计的底，再往下要减少
候选数或缩小区域，都是行为变更。

## 6.8 价值实验：局部升级在 NXP80 上清光了全部残留

7.4 倍把这个实验从数小时变成十几分钟，于是它跑得起来了。NXP80，checkpoint 路径，两臂同
fixture：

| arm | sites | pending | unbalanced | below40 | above80 | min | max | 周期 | 耗时 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| OFF | 70,686 | **54** | **532** | 5,926 | 6,563 | 0.0025 | 174.3087 | **100** | 341.9 s |
| LOCAL-2/16 | 70,147 | **0** | **0** | 5,677 | 6,180 | 0.0025 | **158.5132** | **18** | 513.9 s |

- **残留全部清空**：pending 54 → 0，unbalanced 532 → 0。这正是 §1 结论指认的那批格子。
- **周期 100 → 18**：需求真的归零，循环走 `demands.is_empty()` 的正常出口，不再跑满上限。
- checkpoint 处角度同向改善：below40 −249、above80 −383、最大角 −15.8°。
- 站点少了 539 个，最小角 0.0025 未变（那个退化薄片仍在，见 §6.7）。
- 代价 +172 秒（+50%），但那是 18 个周期对 100 个周期。

**这推翻了 §6.6 的裁定。** 那条裁定基于 NXP40，而 NXP40 上它收益为零的原因是**那里本来就
没有残留**（pending 已是 0）。NXP80 是唯一有残留的规模，而它把残留清光了。

### 修正后的结论

诊断的主结论成立且比初稿更强：升级手段确实有价值，它只是**同时被两件事挡住**——

1. 触发条件是全局标量，在任何非平凡网格上永不成立（§4）；
2. 即使触发，单位成本也高到不能每周期运行（§6.6）。

两条现在都已解决：第 2 条已降 7.4 倍并证明可跑；第 1 条的替代触发（有界局部调度）已实现
并证明有效。

### 尚未解决的：小规模上的代价

NXP40 上开启它是**净负面**：清不掉任何东西（本来就是 0），却把 4.2 s 变成 78.1 s，还让
below40 从 1756 涨到 1792。所以**不能无条件开启**。

触发条件需要再收一层：现在只要有站点连续两轮 degree-blocked 就升级，而 NXP40 上那种站点
一直存在、需求却是可满足的。合理的加强是要求**运行确实卡住**（例如需求连续若干轮没有净
减少），而不只是"有站点被度数挡住"。

这是下一步，且是**行为变更**，须按 §9 走独立的质量 A/B——覆盖 NXP12/21/40/80，判据为
pending/unbalanced 清零、角度不退化、耗时代价可接受。

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

## 6.7 那个"碰不到"的三角形：归因完成，说法要改

§8 曾把 NXP80 的 `worst_deviation` 钉死记为"优化器碰不到那个三角形"，并注明归因未完成。
诊断（`who_made_the_triangle_the_optimiser_cannot_touch`，只读，checkpoint 路径）做完了，
**那个说法是错的**。

checkpoint 处最差三角形 141207：角度 5.30° / 0.391° / 174.31°，eta 0.0118。

| 顶点 | 度数 | 生成周期 | 深度 | 可移动 | 累计位移 |
|---|---:|---:|---:|---|---:|
| 70606 | **7** | 1 | 4 | 是 | 322.0 m |
| 70595 | **7** | 1 | 3 | 是 | 352.1 m |
| 70650 | **3** | 1 | 5 | 是 | **0.0 m** |

**优化器看得见它，而且排第一。** window pass 里三个顶点的排名是 0、1、2；eta pass 里是
4、9、10。三个顶点全部 `Interior`、全部通过 `can_move_site`。

真正的原因是**度数**：一个度数 3 的顶点楔在两个度数 7 的顶点之间。`max_vertex_degree = 7`
是 gridfile `ItabW` 行宽的硬上限，任何能缓解这个薄片的翻转都会把某个顶点推过 7，于是被
`DegreeOverBudget` 拒绝。看得见、排第一、每趟都试、每趟都被门拒。

顶点 70650 的累计位移是 **0.0 米**——从生成起一次都没动过。三个顶点的 `birth_cycle` 都是
**1**：这是第一个细化周期就造出来的缺陷，此后 99 个周期加 48 趟优化都没修掉。

### 轨迹的补正

`margin_min` 在 checkpoint 处是 **−94.308685**，优化器开始时是 **−92.577117**。两者之间只有
一次 `repair_low_degree_stars`——**低度数修复确实动了它**，之后 48 趟 pass 再无改善。所以
"改善量恰好为零"只对优化器成立，对低度数修复不成立。

### 这条与本诊断的关系

它和 §1 的主结论是**同一个瓶颈的两面**：残留的 26 个 blocked 格子卡在度数门，这个退化三角形
也卡在度数门。度数门本身不能放宽（写出器上限），所以两者的出路都是**让插入不要把度数推到
7**，或者**在度数已经饱和之后仍能重新分配它**——而后者正是 `recover_stalled_regions` 想做
的事，也正是它因为触发条件永不成立而从未做成的事。

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

  归因已于 §6.7 完成，结论与此处初稿相反：优化器**看得见**那个三角形并把它排在第一，
  拒绝它的是度数门。它与关闭角度门无关。
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
