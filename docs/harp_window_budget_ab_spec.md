# HARP-DV window 工作预算 A/B：PR20d 微规格

日期：2026-08-28
状态：实施前微规格
范围：只做固定 S3 的 window-pass 工作预算实验；默认生产行为不变

## 1. 目的与非目标

PR20d 只回答一个问题：

> 当前真实 IGBP NXP80 的主要残差，是不是首先被固定 window optimiser 工作预算截断，而不是已经在现有操作集合内平台化？

PR20d 不改变默认网格决策，不调整 candidate ladder、off-centre、eta、retirement、degree-4 策略、patch/cavity 或 StrictWindow。它也不设计公共 solver API；实验只通过显式启用的诊断入口运行，合并后默认关闭。

禁止把本 PR 写成“解决 28,161 个最终越界角”。PR20d 只给下一步分流证据。

## 2. 基线证据

当前 PR20c 证据口径：

- 真实 IGBP NXP80 最终 HARP 网格：203,426 个三角形，610,278 个可测角；
- 最终越界：28,161 个，占 4.614%；
- 其中 99.04% 位于 degree 5-7，不是 degree<=4 强制问题；
- 三类主要 `CandidateSource` 的归一化越界率接近，不支持优先改候选排序；
- 高 raw/frozen target 梯度 cohort 只承载小部分最终残差，不支持优先平滑尺寸场；
- S3 `post_eta` 越界约 54,408；当前 W32 到 S4 `post_window` 后为 28,319；第 32 个 window pass 仍有明显净改善。

因此 PR20d 的最短施工路线是固定 window 输入，比较 W32/W64/W96，而不是先做 cavity 或生成算法。

## 3. 固定输入契约

实验只采用单进程 S3 分叉：一次真实 HARP-DV 运行完成 initial low-degree 和 eta 后，在第一轮 window pass 之前冻结同一份 S3 `post_eta` mesh、frozen gradated target field、guard/quality 状态和 S3 violation `AngleKey` cohort，再 clone 为三个 arm。

三个 arm：

| arm | window pass 上限 | 说明 |
|---|---:|---|
| W32 | 32 | 当前生产基线，必须逐 bit 重现既有 S4 |
| W64 | 64 | 只增加 window 工作预算 |
| W96 | 96 | 检查 W64 后是否平台化 |

不接受三个 CLI 进程各自重放 eta，也不接受三套常量 build 作为主证据。它们最多只能证明输入等价，不能证明三个 arm 来自同一份 S3。

核心 crate 只增加诊断所需的内部切点：私有 window-budget helper 和隐藏的诊断入口。该入口只返回 compact summaries，不返回可交付的替代网格，也不成为新的公共 solver 配置面。

真实 CLI 实验同时设置 `EARTHMESH_HARP_TRACE_JSONL=<absolute-path>` 和 `EARTHMESH_HARP_WINDOW_BUDGET_AB=1`。后者只增加诊断工作，不选择 W64/W96 作为交付网格；JSONL schema 随新增记录升级为 v4。

为保持 S6 单变量口径，window-budget audit 与扩展的 `EARTHMESH_HARP_LEAF_RETIREMENT` 互斥；同时启用时在 refinement 前 fail closed。

生产默认仍是 W32。PR20d 不改变未启用实验时的 trace-off、trace-on、gridfile、raw refined grid、gridinit 和 conservative remap 输出。

## 4. 工作预算口径

报告必须把“pass 数”与真实工作量分开：

- `window_pass_limit`：32/64/96；
- `per_pass_site_budget`：当前每 pass 最多处理的质量站点数；
- `processed_sites`：本 pass 实际处理站点数；
- `unique_sites_seen`：截至本 pass 的累计唯一站点数；
- `candidate_count`：生成候选数；
- `line_search_attempt_count`：线搜索/候选尝试数；
- `retained_move_count`：提交移动数；
- `completed_breadth_sweep`：是否完成一次完整 breadth sweep；
- `stop_reason`：`pass_limit`、`no_retained_moves`、`completed_no_improvement_sweep` 等。

文案统一称为“window optimiser 固定工作预算”，不要只归因为某一个常数。

## 5. 每-pass compact telemetry

每个 window pass 输出一条紧凑记录，不输出完整逐角 JSON。

建议字段：

```text
record_type = "window_budget_pass_summary"
arm = "W32" | "W64" | "W96"
pass_index
window_pass_limit
per_pass_site_budget
processed_sites
unique_sites_seen
candidate_count
line_search_attempt_count
retained_move_count
below_40_count
above_80_count
total_violation_count
new_s3_cohort_key_count
resolved_s3_cohort_key_count
persisted_s3_cohort_key_count
new_global_angle_key_count
worst_window_deviation_deg
window_penalty
eta_min
eta_p1
physical_demands_remaining
balance_demands_remaining
unbalanced_pairs_remaining
wall_time_ms
stop_reason_if_terminal
```

允许在 arm 结束时输出一条 `window_budget_arm_summary`。`wall_time_ms` 是非确定诊断字段，不参与字节一致性断言。禁止每 pass 重复写全量 `angle_violation` 或逐角 transition；大规模逐角记录会把 trace 膨胀成不可用的实验产物。

## 6. 固定 S3 AngleKey cohort

在 S3 分叉点冻结：

```text
s3_violation_cohort = all violating AngleKey at S3
```

每个 pass 和每个 arm summary 至少报告：

- `s3_keys_resolved`；
- `s3_keys_persisted`；
- `s3_keys_kind_changed`；
- `new_global_angle_keys_created`；
- `global_net_change`。

动态 component 可作为诊断字段，但不能作为主分母，因为 component 会随移动、翻边和局部重构变化。

## 7. 主比较点与次比较点

主比较点：S4 `post_window`。

比较：

- W32 S4 vs W64 S4；
- W64 S4 vs W96 S4；
- 三个 arm 相对同一 S3 的净变化。

主指标：

- 全局越界总数；
- `<40` 与 `>80` 分项；
- worst window deviation；
- window penalty；
- S3 fixed cohort resolved/persisted；
- new global AngleKey；
- physical/balance/unbalanced residual；
- wall time 与每 1,000 个净修复的成本。

次比较点：每个 arm 再执行完全相同的 final low-degree repair 和 leaf retirement 后的 S6 `final`。S6 只用于确认交付影响，不替代 S4 主结论。

## 8. Go / plateau 分流

预注册判断，避免跑完后改口径。

### Budget extension 有效

同时满足时，下一步优先做自适应 window 收敛终止，而不是 cavity：

- W64 相对 W32 的 S4 全局越界再下降至少 10%；
- physical/balance/unbalanced 不退化；
- eta、worst deviation、window penalty 不退化；
- W64 后部 pass 仍有稳定净改善，且不是只在制造新 AngleKey 的同时消除旧 AngleKey。

### 开始平台化

满足以下情况时，下一步转向 persistent AngleKey component 的 local patch/connectivity/cavity POC：

- W64 相对 W32 改善不足 2%；或
- W96 相对 W64 再改善不足 1%；或
- 完整 breadth sweep 没有提交移动；或
- 后部 pass 只有极少净改善且 worst deviation 长期不动。

### 分裂结论

若总数下降但最坏角不动，下一步不是全局加 pass，而是对持久 S3 AngleKey cohort 做局部拓扑归因。

## 9. 真实 IGBP NXP80 验证与 artifact parity

PR20d 必须跑真实 IGBP NXP80 A/B，并保存：

- S3 固定输入身份/校验信息；
- W32/W64/W96 的 pass summary；
- S4 主结果表；
- S6 次结果表；
- wall time 与 trace 大小；
- Go/plateau 判据结果。

Artifact parity 要求：

1. 默认未启用实验时，PR20d 与 PR20c baseline 的 gridinit、raw refined grid、final gridfile、conservative remap 字节一致；
2. W32 实验 arm 必须重现当前生产 S4/S6 统计；
3. W64/W96 是诊断 artifact，不得覆盖或冒充默认生产 gridfile；
4. trace/telemetry 失败时沿用 fail-closed：不交付实验输出为成功证据。

## 10. 最小测试

1. 默认关闭：不改变 W32 生产路径，不新增输出；
2. W32 arm：从固定 S3 运行后重现既有 S4；
3. S3 固定断言：若三 arm 的分叉输入不一致，实验失败；
4. pass telemetry：每 pass 只有 compact summary，没有逐角 per-pass JSON；
5. S3 cohort：resolved/persisted/new 的计数闭合；
6. terminal reason：pass-limit 和 no-move/no-improvement 可区分；
7. failure path：telemetry 写入或发布失败不产生正式实验 artifact；
8. real IGBP NXP80：完成 W32/W64/W96，输出主/次表和 artifact parity 记录。

## 11. 合并边界

PR20d 合并条件：

- 默认生产 W32 行为不变；
- 实验只读/诊断输出与生产交付隔离；
- 固定 S3 分母成立；
- compact telemetry 足以判断预算继续有效还是平台化；
- 不新增公共 solver API；
- 不把 PR20d 结论表述为最终原因，只表述为下一步算法分流证据。
