# HARP-DV 主残差归因：PR20c 微规格

日期：2026-08-28
状态：实施规格
范围：只补稳定分母、refinement-boundary 口径和 raw/frozen 尺度梯度证据；不改变网格决策

## 1. 要回答的问题

PR20c 只回答：

1. 每个 `CandidateSource`、refinement depth 和 birth cycle 的越界角是“数量多”，还是在自身稳定分母下“发生率高”；
2. 越界角是否集中在可复现定义的 refinement boundary；
3. 越界角是否集中在陡峭的 raw criterion target 或 optimiser frozen gradated target 梯度；
4. 上述集中在哪个 S0-S6 阶段首次出现或被保留。

PR20c 不调整 candidate ladder、off-centre、quality optimiser、retirement、patch/cavity 或
StrictWindow。PR20b 的 degree-4 audit 保持独立。

## 2. 分母与归因单位

角度越界的基本样本是一个 measurable triangle corner。CandidateSource 归因到该 corner 的站点，
因此 source rate 的主分母不是“候选尝试次数”，而是该 cohort 的：

- `active_site_count`；
- `sites_with_violation_count`；
- `measurable_angle_count`；
- `below_40_count`；
- `above_80_count`。

每个 active site 恰好进入一个 lineage cohort；每个 measurable corner 也恰好进入一个 lineage
cohort。lineage cohort key 为：

```text
birth_source_class + refinement_depth + birth_cycle
```

`birth_source_class` 冻结为：

- `inherited`：`birth_cycle == 0 && depth == 0 && birth_candidate_source == None`；
- 六个现有 `CandidateSource` 值；
- `unknown`：站点存在，但 provenance 字段不满足 inherited/production-insert 不变量，不能伪装成
  inherited。active triangle 缺少稳定 `SiteId` 不是 unknown cohort；它继续使 trace fail-closed，
  因为这种 corner 无法参与 stable `AngleKey` transition。

消费者可按 source、depth 或 cycle 对同一张表求和，不再维护三套容易漂移的分母。

## 3. refinement boundary 的操作定义

“refinement boundary”不作为未经定义的自然语言标签。每个 measurable triangle 使用两个原始、
可复算的量：

1. `lineage_depth_span = max(vertex.depth) - min(vertex.depth)`；
2. `raw_target_coverage_count`：三个顶点中可由 raw criterion 得到有效 target 的数量（0-3）。

派生的 `refinement_boundary_class` 冻结为：

- `neither`：`lineage_depth_span == 0` 且 raw coverage 不是 partial；
- `lineage_only`：`lineage_depth_span > 0` 且 raw coverage 不是 partial；
- `raw_criterion_only`：depth span 为 0 且 raw coverage 为 1 或 2；
- `both`：depth span > 0 且 raw coverage 为 1 或 2；
- `unknown`：内部闭合用 sentinel；任一顶点无法映射到 stable `AdaptiveSite` 时产生，并使 trace
  fail-closed。成功 trace 不得包含该 class。

这里的 lineage boundary 是谱系深度混合三角形，不宣称等同于物理海陆边界、protected segment
或任意网格层级边界。raw criterion boundary 只表示 target availability 在三角形内部发生变化。

## 4. raw 与 frozen 梯度

对一个三角形的三条边计算：

```text
edge_gradient = abs(target_a - target_b) / spherical_edge_length_m
normalized_gradient = max(edge_gradient) / 0.3
```

`0.3` 是生产 `target_cell_scales` 使用的 `TARGET_SCALE_GRADIENT`。ratio 为 1 表示等于该梯度限制。

- raw：在每个 stage 通过 `CellView::centre()` 表示的当前 site/vertex 位置重新采样 criterion；
  只有三个顶点 target 全部有效时可测。现有
  `realized_to_raw_criterion_target_scale_ratio` 使用同一位置语义；
- frozen：质量优化器在 S1 后构造并做图梯度限制的 `target_cell_scales`；值随 Site/vertex slot
  冻结，后续在当前活动三角形和当前位置上读取；
- S0/S1 或质量优化器未建立 frozen field 时，frozen gradient 为 unavailable；
- retirement 只删除站点，不重排 frozen vector；S6 对仍 active 的 slot 继续使用同一冻结值；
- frozen 当前边梯度不宣称仍满足构造时的 `<= 1`，因为站点位置和连边可在优化中改变。

trace-on 路径在生产 `target_cell_scales` 成功构造后，把同一个 vector 只读缓存在
`TraceEmitter`，供 S2-S6 snapshot 使用；质量优化器仍使用原 vector。trace-off 不克隆、不缓存该
field，也不增加扫描。若质量优化器在构造前跳过，则 S2-S6 的 frozen gradient 都是 unavailable。

固定 bin 使用 normalized ratio：

- `unavailable`；
- `le_0_25`；
- `gt_0_25_le_0_5`；
- `gt_0_5_le_1`；
- `gt_1_le_2`；
- `gt_2`。

## 5. trace 记录

JSONL schema 从 2 升到 3。仍只有七条 `stage_summary`；不新增逐角全量记录。

每条 `stage_summary` 增加两个稳定排序数组：

### `lineage_angle_exposure`

每行包含 lineage cohort key，以及第 2 节的五个计数。

### `triangle_context_angle_exposure`

key 为：

```text
refinement_boundary_class
+ raw_criterion_target_gradient_bin
+ frozen_gradated_target_gradient_bin
```

每行包含 `measurable_angle_count / below_40_count / above_80_count`。同一 measurable triangle 的
三个 corner 进入同一个 context cohort。

现有 `angle_violation` 增加：

- `lineage_depth_span`；
- `raw_target_coverage_count`；
- `refinement_boundary_class`；
- `raw_criterion_target_gradient_to_limit_ratio` + measurable flag；
- `frozen_gradated_target_gradient_to_limit_ratio` + measurable flag。

这允许消费者按 stable `AngleKey` 离线计算跨阶段 transition，同时用 stage exposure 作为分母。

## 6. 闭合约束

每个成功 stage 必须满足：

- lineage `active_site_count` 总和等于 stage `vertex_count`；
- lineage `measurable_angle_count` 总和等于 stage `measurable_angle_count`；
- lineage below/above 总和分别等于 stage below/above；
- context measurable/below/above 总和分别等于 stage 对应全局计数；
- 每个 exposure 行 `below + above <= measurable`；
- 所有数组按 typed key 稳定排序；
- 非有限 gradient 不进入 JSON number，而进入 `unavailable` bin / `null + measurable=false`。

任一闭合失败时显式启用的 trace fail-closed，不能发布正式 trace，也不能交付最终 gridfile。
exposure 聚合和闭合验证属于 HARP core certifier/typed-record 层；CLI 只做 DTO 映射、有限浮点
转换和既有的 stage/file 发布完整性检查，不重新实现统计。

## 7. 最小验证

1. inherited tetrahedron：lineage site/angle 分母和全局统计闭合；
2. mixed-depth fixture：boundary class 与 depth span 正确；
3. constant raw + nonuniform frozen fixture：raw/frozen gradient 可区分且 bin 正确；
4. S0/S1 frozen unavailable，S2-S6 在成功构造 field 后 available；
5. JSON schema=3、数组排序稳定、非有限值不进入 JSON number；
6. callback/serialization/closure 错误仍 fail-closed；
7. trace 关闭时不克隆 frozen field，不增加 certifier 扫描，不改变默认输出；
8. real IGBP NXP80 的 source/context 分母闭合，并与 PR20b 的 gridinit/raw/final/remap SHA-256 一致。

## 8. Go / No-Go

PR20c 本身不选择算法。真实 IGBP 完成后：

- source rate 必须用 `violations / measurable_angle_count` 和
  `sites_with_violation / active_site_count`，禁止只按 violation 构成下结论；
- boundary/gradient cohort 同时报告分母、越界率和对全局残差的贡献；
- 只有一个或多个 cohort 同时表现为高越界率和显著残差贡献，才进入对应算法 POC；
- 若 marginals 不集中，先做联合 cohort 离线分析，不直接扩大 retirement 或发布新求解器 API。
