# HARP-DV window 工作预算 A/B：PR20d 真实证据

日期：2026-08-28

代码：`4b5feeeb`

输入：真实 IGBP，NXP80
结论：全局越界数量仍受现有 window 工作预算和调度控制流限制，但最坏角在 W64 后不再改善；按预注册规则属于分裂结论，而不是可以直接把默认预算提高到 W96。

## 1. 实验闭合

- 三个 arm 从同一进程、同一份 S3 `post_eta` mesh、frozen target、guard state 和 54,408 个稳定 `AngleKey` 分叉。
- W32/W64/W96 分别产生 32/64/96 条 compact pass summary，均以 `pass_limit` 停止。
- 所有 arm 的 `resolved + persisted = S3 cohort`、`persisted + new = global violations` 均闭合。
- W32 arm 逐状态 fingerprint 重现实际交付路径的 S4 和 S6。
- W64@32、W96@32 与 W32 S4，以及 W96@64 与 W64 S4 均通过逐状态共享前缀 fingerprint 守卫。
- 正式 trace 为 schema v4，共 276,191 行、221,894,226 bytes（211.61 MiB）；新增预算诊断只有 192 条 pass summary 和 3 条 arm summary。
- 正式 `harp_run_end`：`all_satisfied`，29 cycles，physical/balance/unbalanced/unresolved 均为 0。

七阶段的全局越界数为：

| stage | violations |
|---|---:|
| S0 input | 0 |
| S1 post_refinement | 68,407 |
| S2 post_initial_low_degree | 68,351 |
| S3 post_eta | 54,408 |
| S4 post_window（交付 W32） | 28,319 |
| S5 post_final_low_degree | 28,341 |
| S6 final（交付 W32） | 28,161 |

## 2. S4 主结果

| arm | <40 deg | >80 deg | total | 相对前一 arm | worst deviation | penalty | eta min / p1 | S3 resolved | new global keys | guards | arm wall time |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| W32 | 11,017 | 17,302 | 28,319 | baseline | 28.113875 deg | 599,502.666 | 0.702859 / 0.831490 | 33,538 | 7,449 | 0 / 0 / 0 | 198.690 s |
| W64 | 10,418 | 16,536 | 26,954 | -1,365 (-4.820%) | 19.457635 deg | 485,855.336 | 0.731493 / 0.837030 | 34,914 | 7,460 | 0 / 0 / 0 | 400.384 s |
| W96 | 9,346 | 15,146 | 24,492 | -2,462 (-9.134%) | 19.457635 deg | 378,489.116 | 0.736460 / 0.844430 | 37,229 | 7,313 | 0 / 0 / 0 | 593.895 s |

W96 相对 W32 共减少 3,827 个 S4 越界角（13.514%）。三臂均未完成一次完整 breadth sweep；最后一 pass 仍分别保留 796、474、265 次移动，并净减少 612、345、149 个越界角。

这里的 W64/W96 不是在同质调度下简单重复更多 pass。当前 pass 1-32 使用原有 breadth/exclusion 行为；pass 33 起清空 exclusion，首次进入生产 W32 不可达的 post-breadth 调度。实验因此证明的是“继续执行现有 window 控制流仍有收益”，同时也把 scheduler 公平性和站点覆盖暴露为下一项独立问题。

### 2.1 工作量与覆盖

| arm | final pass found / eligible | unique sites seen | processed-site slots | slots / unique site | line-search attempts | S4 found sites | S4 found but never processed |
|---|---:|---:|---:|---:|---:|---:|---:|
| W32 | 33,004 / 27,444 | 25,191 | 32,768 | 1.301 | 176,954 | 32,781 | 12,122 (36.98%) |
| W64 | 32,521 / 31,497 | 25,736 | 65,536 | 2.546 | 641,184 | 32,407 | 11,538 (35.60%) |
| W96 | 31,238 / 30,214 | 27,565 | 98,304 | 3.566 | 1,088,612 | 31,152 | 9,579 (30.75%) |

从 W32 到 W96，processed-site slots 增至 3 倍，但累计唯一站点只增加 2,374 个（9.42%）；W96 的 S4 仍有 9,579 个 optimiser problem site 在整个 arm 中从未被处理。额外工作大量花在重复访问少数站点，而不是完成一次公平 breadth epoch。这使下一步应优先比较 exclusion epoch、site cap 与 aging/fairness，而不是继续增加固定 pass 常量。

## 3. S6 次结果

| arm | <40 deg | >80 deg | total | 相对前一 arm | worst deviation | penalty | eta min / p1 | final low-degree moves | retirements |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| W32 | 10,956 | 17,205 | 28,161 | baseline | 28.113875 deg | 571,692.641 | 0.702859 / 0.832536 | 69 | 45 |
| W64 | 10,356 | 16,449 | 26,805 | -1,356 (-4.815%) | 18.503454 deg | 462,985.685 | 0.731493 / 0.838047 | 36 | 44 |
| W96 | 9,287 | 15,059 | 24,346 | -2,459 (-9.174%) | 18.503454 deg | 355,685.118 | 0.738693 / 0.845358 | 38 | 44 |

W96 相对 W32 共减少 3,815 个 S6 越界角（13.547%）。三个 arm 的 physical、balance 和 unbalanced residual 均保持为 0。

## 4. 预注册裁定

### 未满足“直接扩预算” Go

W64 相对 W32 的 S4 改善为 4.820%，低于预注册的 10% 门槛。因此本实验不支持直接把默认生产预算从 W32 提高到 W64/W96。

### 未出现全局计数平台

W96 相对 W64 又下降 9.134%，明显高于 1% 平台阈值；pass 96 仍净减少 149 个越界角。现有 optimiser 的全局计数仍被工作预算截断。

### 最坏角已分裂平台

S4 worst deviation 在 pass 34 到 pass 96 一直停留在 19.457635 deg；S6 worst deviation 在 W64 和 W96 同为 18.503454 deg。继续全局 pass 能减少中等尾部，却没有继续触及最坏残差。

因此下一步应分开：

1. 对全局计数，研究基于 breadth/exclusion epoch 的自适应终止与工作调度，不把固定 W96 直接设为默认值；
2. 对最坏角，先记录 persistent `AngleKey` 的选择、候选生成和拒绝历史；未被选中属于调度覆盖问题，单点候选确实不可达时才进入多点 patch 或 connectivity/cavity POC；
3. 暂不扩大 degree-3/4 retirement、不调整现有 CandidateSource 排序、不优先平滑尺寸场。

## 5. 交付物字节一致

PR20d 实验运行仍只交付 W32。四项产物与 PR20c baseline 的 SHA-256 完全一致：

| artifact | SHA-256 |
|---|---|
| gridinit | `023622dab86e12929e06359730905fb04b1d4b96d8efe2c72b144d48646c864a` |
| raw refined grid | `6d5fd69aee11fa031c6a21ce15972b6e8ef63a36d416b5fcc691aa8423180cdc` |
| final gridfile | `5f93bb854fc9497b00f2a4eaa087255fc08ea475ec28afec95bfb4e7d1c0dd97` |
| conservative remap | `0041a1058b5e4c7a2eb3b1c17f88fe793455aafefe5d590849c3ec312aec5e78` |

整次 opt-in A/B 运行墙钟时间为 1,667.77 s。该时间包含三条诊断 arm、实际交付 W32 和全部 trace I/O，只是单次生产证据，不是性能保证。
