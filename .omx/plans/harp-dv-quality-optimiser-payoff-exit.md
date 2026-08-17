# 质量优化器:按收益退出,而不是按提交数退出

> **2026-08-17 修正**:本文档最初把优化器的开销归因于"跑满 48 遍"。采样
> 证明那不是主因。在 NXP=21 的 CLI 算例上,**90.9% 的样本落在
> `MeshState::triangle_count()` 里面**——`triangle_fan_from` 用它算循环上界,
> 而它是一次 O(F) 全槽位扫描,为一个约 6 个三角形的扇形付整张网格的代价。
> 同样的写法在 `mesh_insertion` 和 `mesh_flip` 的循环上界里各还有一处。
>
> 真正的修复是把这三处上界换成 O(1) 的 `triangles().len()`(见
> `harp-dv-triangle-count-loop-bound.md`)。那是纯性能修复,不改输出。
>
> 本文档描述的收益退出**仍然成立且独立**:NXP80 的后 13 个 window 遍确实
> 只买到 0.72% 的收益。但它是"遍数"层面的改进,量级远小于单遍成本的三个
> 数量级,且**会改变交付网格**。两者必须分开落地,先做不改行为的那个。

## 1. 现状

`optimise_mesh_quality_with_natural_length` 跑固定的 48 遍
(`ETA_QUALITY_PASSES = 16` + `WINDOW_QUALITY_PASSES = 32`),唯一的提前退出
条件是

```rust
if committed_this_pass == 0 { ... break }
```

也就是**一遍一次提交都没有**才停。只要每遍还能挤出哪怕一次提交,它就把
48 遍走满。这个判据度量的是"还能不能动",不是"动了还值不值"。

## 2. 证据

NXP80 全路径日志(`nxp80-fullpath-after3.log`)的 window 阶段,32 遍逐遍收益,
`outside` 是落在 40–80 度窗口外的角度数:

| 遍 | outside 前 → 后 | 收益 | 占残余 |
|---|---|---|---|
| 1–11 | 7509 → 2327 | 5182 | 7%–14% 每遍 |
| 12–16 | 2327 → 1715 | 612 | 4%–9% 每遍 |
| 17–19 | 1715 → 1603 | 112 | 0.8%–4.5% |
| **20–32** | **1603 → 1560** | **43** | **0.0%–1.3%** |

总收益 5949。**最后 13 遍买到 43 个角度,占 0.72%**,期间仍提交了 199 次移动
——所以 `committed_this_pass == 0` 从未触发,48 遍走满。

拐点在第 19–20 遍之间,清晰可辨。

## 3. 改动

在 window 阶段增加一个收益判据,与现有的零提交判据并存:

```rust
const WINDOW_MIN_RELATIVE_GAIN: f64 = 0.01;   // 残余的 1%
const WINDOW_GAIN_PATIENCE: usize = 2;        // 连续两遍不达标才停
```

一遍被"保留"(未被 `regression_from` 否决)之后,若

```
outside_before - outside_after  <  WINDOW_MIN_RELATIVE_GAIN * outside_before
```

则计一次不达标;连续 `WINDOW_GAIN_PATIENCE` 次即 break。任何一遍达标就清零。

**为什么要 patience 而不是一遍即停**:曲线是抖的,不是单调的。第 17 遍
0.816% 之后,第 18 遍回到 4.527%。patience=1 会在第 17 遍停,放弃 155 个角度
(2.6%);patience=2 在第 23 遍停,放弃 9 个(0.15%)。抖动是真实的,不能当噪声。

**为什么用相对量而不是绝对量**:同一个绝对阈值在 NXP12 和 NXP80 上意义完全
不同。残余的百分比在规模之间可比。

按此规则回放 NXP80 的曲线:

```
遍 17: 0.816% → 记 1
遍 18: 4.527% → 清零
遍 19: 1.293% → 清零
遍 20: 0.062% → 记 1
遍 21: 1.311% → 清零
遍 22: 0.316% → 记 1
遍 23: 0.444% → 记 2 → 停
```

停在第 23 遍,省掉 24–32 共 9 遍,放弃 9 个角度 = 总收益的 0.15%。

## 4. 预登记验收条件

这是一个**会改变交付网格**的改动,所以先立标准,再改,再比。
判据在改之前写死,不允许事后调整。

同一个算例改前改后各跑一次,以下全部成立才算通过:

- **A1 硬门不退**:`min_triangle_angle_deg`、`max_vertex_degree ≤ 7`、
  `require_closed_surface`、物理需求覆盖,四项的判定结果必须与改前一致。
  任何一项由通过变为不通过,直接否决。
- **A2 窗口残余不显著变差**:最终 `outside` 相对改前增加不超过 **1%**。
  NXP80 改前是 1560,则改后不得超过 1575。
- **A3 报告诚实**:`quality_optimiser_moves > 0` 仍然成立,
  `triangle_eta_min`、`triangle_eta_p1` 仍为有限正数。
- **A4 确有加速**:window 阶段墙钟时间下降不少于 **20%**,否则这个改动
  不值得它带来的行为变化,应当回退。

A2 的 1% 是这样定的:被放弃的收益按曲线是 0.15%,留 6 倍余量容纳
平台浮点差异与算例差异。

## 5. 不做什么

- **不碰 eta 阶段**。曲线只在 window 阶段量过;eta 阶段的收益结构未测量,
  没有依据就不动。
- **不降 `MAXIMUM_QUALITY_PASSES`**。上限仍是 48,收益判据只在它之内提前
  停止。一个真正需要 32 遍 window 的算例仍然跑得到。
- **不为了让 CI 变绿而缩测试算例**。CLI 的
  `harp_dv_output_passes_the_mesh_quality_gate` 断言的是"输出通过质量门",
  缩掉算例就是把断言掏空——与
  `the_wall_behind_degree_is_the_pentagons` 那次的教训同类。

## 6. 与 CI heavy 的关系

`heavy` 自 `12078c4` 起从 9m52s 涨到 2 小时以上被杀,挂住的四个测试里两个是
HARP-DV 的。`12078c4` 正是引入质量优化器的提交(`cycle/mod.rs` +2762 行),
其提交信息自己写着 "the complete CLI static-netcdf suite which remained
CPU-active past 48 minutes and was stopped"。

本改动是否足以让 heavy 回到预算内,需要实测,不预设。若不够,再单独诊断,
**不以缩小测试算例的方式凑数**。
