# 让 `fast` 门重新变快：集成级测试的瘦身计划

日期：2026-08-16
状态：待执行
性质：只改测试的 fixture 规模，不改生产代码，不改任何断言

## 1. 问题

CI 的 `fast` 任务（`make test-fast` = `cargo test --workspace --exclude earthmesh_cli
--all-targets`，**debug 模式**）本该是快速反馈门，现在跑不完。

| 提交 | `fast` 耗时 | 结果 |
|---|---:|---|
| `ebe8283`（8-11） | **2 分钟** | success |
| `12078c4`（8-14，基线 HEAD） | 15 分钟 | **超时取消** |
| `harp-dv-seeded-objective` | 35 分钟 | **超时取消** |
| 同上 + 超时调到 50 | 约 40 分钟 | 预期通过 |

三天内从 2 分钟长到 40 分钟。超时已经从 15 调到 50，**这是止血不是治疗**：套件还会继续长，
下次仍会撞墙。

## 2. 证据：钱花在哪

`earthmesh_refine_harp_dv` 里在 CI 上单个超过 60 秒的测试有 **12 个**，全部是"跑完整细化 +
质量优化器"的集成级测试，住在一个本该做单元验证的门里：

```
degree_relief_moves_reduce_the_wall_without_breaking_quality_gates
frontal_placement_ab_on_the_nxp6_proxy
protected_segments_make_a_quality_target_terminate
scale_balance_and_r_adaptation_close_the_gap
target_scale_optimizer_improves_eta_tail_without_spending_harp_gates
the_cycle_limit_is_reported_as_the_cycle_limit
the_degree_budget_saturates
the_refusals_are_counted_by_kind
the_report_separates_balance_from_what_was_asked_for
the_surviving_residue_is_attributed_to_refinement_or_to_optimisation
the_wall_behind_degree_is_the_pentagons
without_balance_the_same_target_breaks_the_bound
```

本机 debug 单线程实测（CI 比本机慢约 5–8 倍）：

| 测试 | 耗时 |
|---|---:|
| `the_cycle_limit_is_reported_as_the_cycle_limit` | **172 s** |
| `the_degree_budget_saturates` | **104 s** |
| `protected_segments_make_a_quality_target_terminate` | **73 s** |
| `target_scale_optimizer_improves_eta_tail_without_spending_harp_gates` | 54 s |
| `the_surviving_residue_is_attributed_to_refinement_or_to_optimisation` | 53 s |
| `the_refusals_are_counted_by_kind` | 30 s |

CI 上 harp_dv 的测试 03:48:31 开始、04:22 被杀，**33 分钟只差最后一个**
（`protected_segments_make_a_quality_target_terminate`），其余未完成项都是 `#[ignore]` 的。

## 3. 已经验证可行的做法

`harp-dv-seeded-objective` 分支上有两个同类测试被瘦身，**断言一条未改**：

| 测试 | 做法 | 前 → 后 |
|---|---|---:|
| `the_kept_cell_survey_never_drifts_from_a_fresh_sweep` | 细化 40 周期 → 3 周期；逐 pass 校验 48 趟 → 前 8 趟 | **54 → 35 s** |
| `the_natural_length_arms_differ_only_in_the_natural_candidate` | 保留周期数（需要收敛的 checkpoint），改用半径 500 km / 0.4 倍尺度的更小细化区域 | **55 → 13 s** |

两条经验：

1. **能减周期就减周期**——多数测试不需要收敛，只需要"有东西被细化过"；
2. **需要收敛的就缩区域**——`steep_target` 的 1200 km / 0.3 倍是为实验设计的，不是每个
   断言都需要那么大的网格。

## 4. 每个测试的处理流程

逐个来，不批量改。对每一个：

1. **先读它断言什么**，写下一句话；
2. **判断它需要多大的网格**——需要收敛的 checkpoint？需要特定的失败模式（度数饱和、
   五边形墙、平衡失败）？还是只需要"跑起来过"？
3. **缩小 fixture**（周期数、区域半径、目标尺度、nxp），断言一条不动；
4. **证明它没被掏空**——加非空/非平凡断言，例如"至少提交过一次移动"、"候选列表长度 > 1"、
   "该失败模式确实出现过"。这一条是硬要求：一个缩小到什么都碰不到的测试比原来更糟；
5. **记录前后耗时**。

若某个测试无论如何都需要大网格，**移到 `#[ignore]`** 并在 `heavy` 里跑，而不是留在 `fast`
里拖时间。仓库已有 9 个 `#[ignore]` 测试，这是既有惯例。

## 5. 非目标

1. **不改 `make test-fast` 为 release 模式。** 那会永久失去 `debug_assert!`（工作区有 4 处）
   和整数溢出检查，是用覆盖面换时间；提速的由头不该顺手做覆盖面降级。
2. **不再调高超时。** 已经从 15 调到 50，再调就是承认这个门不再是快速门。
3. **不改任何断言的语义**，不删测试。
4. **不改生产代码。**

## 6. 验收

- `make test-fast` 在本机 debug 下的总耗时下降到当前的一半以下；
- CI 的 `fast` 稳定在 **20 分钟以内**，随后把 `timeout-minutes` 从 50 调回 **30**；
- 上述 12 个测试全部仍然通过，且每个都带有非空断言；
- 每个测试改动附前后耗时；
- 生产代码零改动（`git diff` 只涉及 `tests.rs` 与 `ci.yml`）。

## 7. 给执行者的提醒

CI 比本机慢 5–8 倍，所以**本机看着还行的测试在 CI 上可能是大头**。判断优先级用本机相对
耗时排序即可，但验收要以 CI 的实际 `fast` 时长为准。

另外，本机 `--release` 跑测试比 debug 快 5–10 倍，容易让人误以为套件很快——`fast` 跑的是
debug，两者不是一回事。这个坑在 `harp-dv-seeded-objective` 的开发中踩过一次：本地
`--release` 全绿，CI 的 debug 门超时。
