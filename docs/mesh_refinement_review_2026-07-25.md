# EarthMesh 网格细化方案独立评审（只读）

日期：2026-07-25
评审对象：`docs/mesh_refinement_method_research_2026-07-02.md`，重点为其中「2026-07-25 实施状态复核：当前问题与下一步」一节
评审方式：只读核查该节引用的全部 Rust 生产路径与测试；所有结论均标注 `文件:行` 证据
评审时快照：分支 `v3.0.0-alpha2`，HEAD `ebddef6a`，工作树 85 条状态记录 / 78 文件 / `+11165 −1474`（`git diff --shortstat` 实测）

> **当前权威冻结点以
> `target/mesh-refinement-m0-formal-1785055022/freeze.json` 为准**
> （同时锁定 `measurement_manifest.json`、`measurements.json`、`cargo-test.log` 和
> `baseline_summary.json` 的 sha256；manifest 含 git HEAD、`git status --porcelain=v2`、
> 源码树 sha256、rustc/cargo/platform、测试可执行文件及输入文件 sha256）。
> 上面一行只是本评审执行当时的非正式快照，**不是** M0 的冻结定义；工作树此后继续变动，
> 请勿用它比对后续运行。
>
> 本文件为定格的评审记录：结论与建议按评审时状态写就，除标注 `[状态更新]` 的段落外不随实施进度改写。
> 最近一次状态更新：2026-07-27（见 §5、§7 与 §9）。

> 本文件为独立评审记录，不修改被审文档、不修改任何代码。

---

## 1. 执行摘要

文档的方向判断（统一 HField 契约、后端按拓扑类别实现）是正确的，问题 B/C/G/H 证据充分。但它在**三处关键事实上不准确**，导致其推荐的 M0→M1→M2 顺序把一个存在未覆盖风险、且收益上限受结构约束的步骤排在了最前面：

1. **M1 不是 drop-in。** 生产弹簧在每个 pass 后调用
   `spring_nest_with_radius_projection(..., move_interior=false, ...)`
   （`rust/earthmesh_mesh/src/method_c_spawn_hfield/mod.rs:1048`）。其可动节点集由
   `method_c_nest_movable_m_points`（`rust/earthmesh_mesh/src/method_c_nest_spring/mod.rs:303-331`）
   定义为「本 pass 新建（`ngr == grid_number`）且邻接 `mrow != 0` 过渡面」的 M 点。也就是说，
   被弹簧移动的节点**全部**位于过渡行。而
   `MethodCNestSpringScratch::with_edge_target_lengths` 把 `target_mrow_multiplier`
   硬置为 `vec![1.0; edge_count]`
   （`rust/earthmesh_mesh/src/method_c_nest_spring_iteration/mod.rs:174`），
   **丢弃了 Method-C 的 mrow 过渡整形**（compat 路径为 `11/12 … 10/6`，同文件 `:4-15`）。
   注意作用域差异：**可动性判定在点上**（点的任一邻接面 `mrow != 0`，`:320-327`），
   **乘子判定在边上**（该边两侧面的 mrow 对，`method_c_nest_spring_iteration/mod.rs:97-99`），
   且乘子表只对 5 个特定 pair 返回非 1.0、其余全部落入 `_ => 1.0`（同文件 `:8-13`）。
   因此 shaped 边在可动集中**占比非零但小于 100%，具体比例必须实测**——这正是 §5 的测量项之一。
   风险论证不依赖这个比例：`rust/earthmesh_mesh/src/tests/method_c_hfield_spring.rs:100-124`
   同时断言 `unshaped > 0` 与 `shaped > 0`，且**只在 `multiplier == 1.0` 的边上做位级断言**，
   即 shaped 子集在该原语上**完全没有 parity 断言覆盖**。按文档 M1 直接接入，
   会在这个非零且未充分测试的子集上改用仅由连续 HField 表达的整形；是否退化尚未被证明，
   必须通过保留/去除 mrow 乘子的 A/B 对照确认。

2. **文档对 compat 弹簧的描述「近似固定自然长度」不准确。** 实际目标是
   `dist00/1.2 / 2^(mrlu-1) × angle_ratio × mrow_multiplier`，其中
   `angle_ratio = clamp(twocosphi3 + twocosphi4, 0.15, 1.2)`
   （`method_c_nest_spring_iteration/mod.rs:286-291`）——已经是分层、分过渡行、角度自适应的。
   HField 相对它**唯一新增的信息是「层内连续尺寸」**。加上可动集只有过渡行，
   M1 的可得收益受显著限制，具体幅度必须实测，不能预先声称相差一个数量级。

3. **文档未发现的机制性风险：两个后端对同一个 h 场用了不同的量化器。**
   球面走 `topology_level_at`（**floor** + 双线性 stencil 取 max，
   `rust/earthmesh_hfield/src/lib.rs:704,745-754`），
   Cartesian 走 `cartesian_hfield_level_at`（解析锥 + **ceil**，
   `rust/earthmesh_cli/src/hfield_refine/mod.rs:471`）。
   同一个 h 场在两个后端会得到不同层级：ceil 倾向物化更多渐变裙带，floor 倾向只保留硬核。
   **[实测 2026-07-29]** 同一解析 circle、`base=1600 km`、level 2、`g=0.2` 下，
   两端在硬需求中心均返回 level 2；距中心 600 km 的过渡点上，球面 floor 返回 level 1，
   Cartesian ceil 返回 level 2。该差异已由
   `spherical_and_cartesian_quantizers_match_hard_demand_but_not_transition_level`
   锁定；它证明 R4 的语义差异存在，但不是任何既有 case 失败的单独归因。

**主要矛盾不是 HField，也不是弹簧，而是：缺少可比较的度量闭环 + 拓扑闭包成本没有归因。**
因此建议把顺序改为 **M0 → M2a(归因，行为不变) → M1 → M2b → M3 → M4(归档)**，
并在 M2a 归因完成后设置一个明确的暂停点。

> **[状态更新 2026-07-26]** NXP=81 的三案例扫描已经证明：现有逐-pass 弹簧是
> **必要但不充分**的。`niter=0` 时三个案例均为 quality fail；50 次迭代消除了
> G-CIRCLE/G-FRAGMENT 的自相交并把三个案例提升到 warn，500 次时为
> warn/warn/pass。固定测得 5000 次后，G-CIRCLE 继续改善但仍为 warn，
> G-FRAGMENT 的 edge-CV max 反而由 `0.4866` 回升到 `0.5129`，R-BBOX 保持 pass。
> 因而数据不支持「继续增加迭代就是通解」，也不支持删除逐-pass 弹簧。
>
> **[状态更新 2026-07-26]** 官方 MPAS 对标已扩展为 2 张准均匀网格和 4 张变分辨率网格。
> 六张网格全部拓扑合法、无自相交/非法多边形；两张准均匀网格通过现有门，四张变分辨率网格
> 均触发至少一个 provisional absolute-max 门。该矩阵足以否定 absolute-max 门作为变分辨率
> hex 普适有效性门，但只有可接受样本、没有区域产品、拒绝对照或模拟误差，仍不足以定义正式
> 分布阈值。因此 M1 生产接入、checkpoint/rollback 与 M3 保持关闭，现有生产阈值暂不修改。
>
> **[路径限定：`case9_projected_hfield_20260728`]** 使用最新共享 phase-support 遍历和
> Ocean 产品支撑口径，Case 9 已完成三层 Method-C：`116/116` active hard bins 全覆盖，
> `target_above_actual=0`，自交、非法 polygon、orphan 与非流形均为 0。
> 最终网格为 `210048` 个三角形；相对旧 `197152` 基线增加 `12896`
>（`+6.54%`）。同一网格上的 triangular-primal P2 负对照为 `passes=0`、
> `changed=false`，因此原 7-bin 缺口及其 P2 生产立项依据均已消失。
> P2 只保留为 research-only 能力证据，不接 dispatcher；详见 §5.0.1 与 §9.1。
>
> **[当前原生路径：`case9_native_15arcsec_20260729`]** 使用全部
> `86400×43200` 源像元并禁止 coarse HField projection。跨层父支撑已闭合，但 pass 2
> 仍以同层 `TransitionPatch` 失败；不得用上述 projected 路径的成功结论替代原生 15″
> 生产闭环。

---

## 2. 当前根因排序

| 序 | 根因 | 判定 | 证据 |
|---|---|---|---|
| R1 | **度量闭环原先缺失，现已补齐本阶段所需字段**（不是「真实引擎测试缺失」） | 已处理 | 真实引擎测试是存在的（`rust/earthmesh_cli/tests/refine_pipeline.rs:114-162` 跑 `run_refine_pipeline_namelist` 全链；`rust/earthmesh_cli/tests/refine_end_to_end_topology.rs:31` 验 χ=2）。质量报告现已同时记录 edge-CV 与 aspect 的 P95/P99、活动 warn/fail 阈值、超阈值数量和占比；`gate_calibration` 另行标注当前 cell view 的校准覆盖、参考集与触发的 provisional 极值门。旧冻结产物保持不变，新校准产物见 §5 |
| R2 | **拓扑闭包成本无归因，且闭包是扩张偏置的** | 主要矛盾 | `spawn_nest_pass_method_c_repairing`（`rust/earthmesh_mesh/src/method_c_spawn_pass/mod.rs:70-193`）：先 `close_method_c_concavities`，再 64 轮循环，修复顺序为 shrink-once（且被 `method_c_repair_candidate_preserves_coverage` 过滤）→ fill-specific-M → fill-perimeter-boundary → grow-perimeter。**四条修复路径中三条只会加单元**。这是 `actual_above_target` 的结构性来源，且当前无任何字段记录是哪一条触发的 |
| R3 | **弹簧目标不一致 + 可动集受限** | 必要但不充分（决定 M1 天花板，不是通解） | NXP=81 扫描中弹簧消除了自相交并显著降低 aspect ratio，但 G-CIRCLE/G-FRAGMENT 到 500 次仍为 warn；见 §1 第 1、2 点 |
| R4 | **跨后端层级量化与过渡参数语义未形成统一契约** | 次要但机制性风险 | `topology_level_at` floor+max（`earthmesh_hfield/src/lib.rs:704`）vs `cartesian_hfield_level_at` ceil（`earthmesh_cli/src/hfield_refine/mod.rs:471`）；同一 circle 配对实测中，硬需求中心均为 level 2，但距中心 600 km 的过渡点为球面 level 1、Cartesian level 2。另有第三条语义：hydro 走 `earthmesh_refine_planner` 的 cell-graph 平滑（`rust/earthmesh_refine_planner/src/lib.rs:763`）再降为 HField（`rust/earthmesh_cli/src/hydro_refinement_adapter.rs:504-527`）。球面 HField 的 `topology_g` cap 还读取区域路径的 `halo/max_transition_row`，而真实 child spawn 只消费固定 atmos/surface `max_mrows`；rows=8 在 G-CIRCLE 上增加 4812 cells，却未改变可生成 g 边界，属于共享参数但语义不共享的同族问题。**[数据点 2026-07-26]** G-FRAGMENT、production cap、`global_niter=5000` 的 pass 2 中 demand-tail-only 为 `1530/5193=29.46%`，是 connectivity-bridge-only 的 10 倍；它是合法覆盖成本，但表明顶点+边中点采样对碎片化阈值场的边细尾需求欠分辨。当前只记录，不启动量化或采样算法改动 |
| R5 | **质量口径已有全球 hex 外部矩阵和一个区域 tri 运行产品正样本，但两类都仍是 provisional；`actual_above_target` 口径仍偏松** | 当前第一优先级 | **[更新 2026-07-29]** 两张准均匀和四张 4×/6× 变分辨率 MPAS 官方网格证明当前 absolute-max 门不能作为变分辨率 hex 的普适门；新增 NOAA NGOFS2/FVCOM 区域 tri 正样本含 569405 个三角形，拓扑合法、负面积/自相交/非法多边形均为 0，edge-CV max/p99 为 0.253/0.173，aspect max/p99 为 1.878/1.515。tri 仍缺负样本、稳定性和跨产品矩阵，不能据单样本修改阈值。另有目标口径用 `topology_level_at` 的 **max-over-stencil**，会在 bin 边界抬高目标，从而**低报**实际过量 |
| R6 | **HField 本身** | 最不像瓶颈 | 梯度限制有强单元测试：极区球面 Lipschitz（`earthmesh_hfield/src/lib.rs:942-1019`）、跨极子午线闭合（`:596-614`）、幂等与确定性（`:932-938,1226-1248`）。文档把它排在问题 A 首位是合理的（消费链确实只完成一半），但把它当「当前主要矛盾」会误导 |

### 2.1 R4 最小共享契约（2026-07-29）

球面与 Cartesian 不统一量化实现，也不要求过渡层级图逐点一致。共同契约只包含：

1. 源区域内部的硬需求必须达到请求层级，且不得超过配置的 `max_level`；
2. regularized gradation apron 是后端局部的拓扑提示，不得回写成 immutable hard demand；
3. 跨后端验收比较 hard coverage、拓扑合法性和配置上限，不比较过渡区逐点层级或 gridfile hash。

`spherical_and_cartesian_quantizers_match_hard_demand_but_not_transition_level` 已锁定第 1 条及
允许的过渡差异；`composed_hfield_keeps_gradient_apron_out_of_hard_demand` 锁定第 2 条。
因此当前不把 Cartesian `ceil` 改成球面 `floor`，也不新增共享量化器。若未来需要两端生成
相同过渡拓扑，必须先用两套完整 Project 证明这是产品需求，而不是仅凭单点层级差异改算法。

---

## 3. 文档中需要修正的结论

| # | 文档原文 | 问题 | 应修正为 |
|---|---|---|---|
| C1 | 问题 A：「仍用**近似固定自然长度**的弹簧决定细化后节点位置」 | 不准确 | compat 目标已是「分层 × mrow 整形 × 角度自适应」（`method_c_nest_spring_iteration/mod.rs:88-99,286-291`）。HField 唯一新增的是**层内连续尺寸** |
| C2 | M1 步骤 2：「调用现有 `spring_nest_with_edge_targets`」 | **存在未验证的回归风险，不能直接接入** | 该函数把 mrow 乘子置为 1.0（`method_c_nest_spring_iteration/mod.rs:174`）。可动集全部取自过渡围裙，其中 shaped 边占比非零（`tests/method_c_hfield_spring.rs:117-124` 的 `shaped > 0`）但**未实测**，且恰是现有 parity 测试排除在位级断言之外的子集。M1 必须先比较「HField base + 保留 mrow」与「HField base + 去除 mrow」，不能预先断言任一方案正确 |
| C3 | M1 步骤 3：「第一版只移动当前 pass 的过渡/新增节点」 | 表述为设计选择，实为既有约束 | 这是 `method_c_nest_movable_m_points` 的既有行为（`method_c_nest_spring/mod.rs:310-328`），不是「第一版保守起见」。写成设计选择会**高估 M1 的可得收益**，并让 M1 的 null 结果无法解释 |
| C4 | 问题 E：「尚未证明真实引擎闭环」 | 过强 | 真实引擎闭环**部分存在**（见 R1 证据）。准确表述：真实引擎覆盖存在但（a）网格规模过小（NXP=6，362 cells，质量指标无意义）、（b）HField 驱动下无极区/换日线/流域/mask/hydro 用例、（c）断言是布尔式的，不产出可比较指标 |
| C5 | M0：要求「edge-CV 的 P95/P99 和超阈值单元数」 | **评审时不可直接计算，现已补齐** | 质量报告现已直接输出 edge-CV 与 aspect 的 P95/P99、活动阈值、超阈值数量和占比；JSON/CSV/Markdown 与 CLI 使用同一口径。旧 M0 冻结产物不追写，新校准产物见 §5 |
| C6 | 问题 B 列举的六类抬高原因 | 不完整，且缺「口径本身偏松」 | 应补充：(a) 目标口径 `topology_level_at` 的 max-over-stencil 会抬高目标从而**低报**过量；(b) 真实的第七类原因是 **anchor 兜底**——`method_c_spawn_hfield/mod.rs:822-868` 为未覆盖的 demand anchor 追加「最近 owner」整块 rad3 足迹 |
| C7 | 全文未提 | **遗漏** | 跨后端量化实现不同但共享语义未声明（floor+max vs ceil vs planner 平滑），且球面 HField cap 复用了不参与其 child topology 的区域 `halo/max_transition_row`，见 R4。这与问题 H「统一契约」直接相关：应统一需求覆盖、过渡宽度和参数消费契约，而不是预先要求统一算法或复用同名参数 |
| C8 | 问题 C：「弹簧只能优化现有连接关系」 | 对，但在本仓更强 | 本仓不仅不能改连接，**连节点位置也只能改一小部分**（过渡行）。因此「固定拓扑质量下限」实际上是「固定拓扑 + 固定内部/父层节点」的下限，比文档说的更低 |
| C9 | 0.6：「已运行的针对性测试…证明两个原语成立」 | 过强 | `method_c_hfield_spring` 的三项测试证明的是**与 compat 路径的等价性**（喂 level-derived targets）与确定性，**没有任何一项证明 HField 目标能改善 edge-CV**。这一点必须在文档中写明，否则会被当成 M1 的正面证据 |
| C10 | 全文未区分 tri / hex 的验收口径 | **遗漏**（新增 2026-07-26） | **在当前 Method-C 后端内**，三角形与六边形是同一张网格的 primal/dual 两视图：生成路径从不分支于 `mode_grid`（`earthmesh_mesh` 内仅见于 `mask_postproc_data/mod.rs:110`），`gridfile_m_cell_lineages`/`gridfile_w_cell_lineages`（`method_c_mesh/mod.rs:128-141`）即对偶定义。因此当前 Method-C run 的细化层级不能按视图拆开，否则会破坏其 Voronoi 对偶；**这不是长期架构上把 Tri 绑定 FVCOM、把 Hex 绑定 MPAS 的理由。** Tri/Hex 是拓扑族，FVCOM/MPAS/CoLM 只是当前 model adapters。验收口径必须先按拓扑族分开，再叠加模型专属门；当前两层都未形成：`QualityThresholds` 是单一扁平结构，`cell_view` 只是标签（`quality/src/lib.rs:417`）从不参与任何 gate 判定，而 `min_angle_warn_deg = 25.0`（`earthmesh_core/src/constants/mod.rs:11`）对理想内角 120° 的六边形几乎永不触发（该门对 hex 实际是死门），对理想 60° 的三角形却是有效的 sliver 门；三角形单元数约为六边形 2 倍、面积约 1/2，故 `nCells`、`normalized_area_cv`、`actual_above_target/nCells` **跨视图不可直接比较**。此外 `base_m` 默认 `2πR/(5·nxp)`（`global_source.rs:316`）是 M 点间距，对 hex 是中心距、对 tri 是边长，而硬需求覆盖判定用**输出单元多边形**与栅格 bin 求交（`hfield_support_coverage.rs:833`），故同一 h 场在两视图下覆盖结论不同——契约需写明 h 指哪个几何量。已做对的范式是 `angle_deviation_deg`（相对等角球面 n 边形的偏差，n-gon 感知）与仅对三角形计算的 `triangle_eta`/`triangle_nsr`：**指标定义应 n-gon 感知，而非给每种形状硬编码一套数**。这是问题 H 的具体实例 |

---

## 4. 实现状态证据表

状态取值：`production` / `implemented-but-not-wired` / `test-only` / `incomplete` / `unverified`

> **[状态更新 2026-07-26]** 新增复合取值 `production，unverified`：代码已接入生产路径并实际执行，
> 但其行为**没有任何测量或测试覆盖**。单一取值无法表达这一状态——`production` 会暗示已被验证，
> `unverified` 会暗示未接线。2026-07-26 新进入生产的两项改动（`topology_g`、M2a 归因）均属此类。

| 能力 | 状态 | 证据（文件:行） | 边界说明 |
|---|---|---|---|
| **HField 组合与梯度限制** | `production` | `HField::min_with_region:464` / `min_with_field:481` / `limit_gradient:516`（`rust/earthmesh_hfield/src/lib.rs`）；生产接线 `rust/earthmesh_cli/src/refine_pipeline/global_source.rs:319`（组合）、`:330`（hydro 叠加）、`:346`（domain 约束）、`:392`（native expansion 后重限） | 栅格分辨率要求写在文档注释里（`earthmesh_hfield/src/lib.rs:506-511`）但**无运行期校验**：`hfield_nlon/nlat` 小于局部 h 时窄走廊会丢失 |
| **桥接内孔域中的 HField 精确重叠** | `production`，真实 Project 回归已通过 | even/odd 三重相交 `polygon_triple_intersection_area_even_odd`（`rust/earthmesh_geometry/src/lib.rs:558`）→ `projected_triple_intersection_area`（`rust/earthmesh_cli/src/grid_quality_inputs/hfield_support_coverage.rs:731`）；最小回归 `hfield_support_coverage.rs:1710`；耐久入口 `scripts/run_basin_hole_regression.sh` | `ShapefilePolygonComponent::into_close_ring` 用零宽双桥编码 hole；旧简单多边形三角剖分会把该合法自接触表示算成零重叠，导致 `specified_close` hard layer 静默为空。现保留精确 positive-area source/domain/bin 交集及 hole 排除，不用 bin-center 近似，也不改变凸多边形快路径 |
| **HField 栅格间距运行期诊断** | `production`，非阻断 warning | `hfield_raster_resolution_warning`（`rust/earthmesh_cli/src/refine_pipeline/global_source.rs`）在 hydro、domain clip 和 native expansion 合成完成后检查 regularized HField；回归 `hfield_raster_resolution_warning_reports_only_underresolved_bins` | 对每个球面 HField bin 比较最大轴向中心间距与 local h；仅在 `spacing/h > 1` 时一次性报告欠分辨 bin 数和最大比值，不改变生成。案例 7 的默认 `720x360` 报 `136/259200`、最大 `2.250`，gridfile SHA256 仍逐位不变。该 warning 关闭了“静默欠分辨”，但不等于窄 corridor 已通过 |
| **`topology_level_at` 的生产消费** | `production`（仅球面） | 驱动闭包 `global_source.rs:606`；质量目标口径 `rust/earthmesh_cli/src/source_demand_artifact.rs:816` → `rust/earthmesh_cli/src/grid_quality_inputs/hfield_support_coverage.rs:833` | **Cartesian 后端不消费它**，改用 `hfield_refine/mod.rs:449-471`（ceil）。两者对同一 h 场给出不同层级 |
| **Method-C 目标层级闭包** | `production` | `spawn_nest_from_target_levels_internal`（`method_c_spawn_hfield/mod.rs`）；共享 phase-support 遍历不再用 intermediate demand 截断（`:947-950`）；回归 `method_c_hfield_spawn.rs:508`；覆盖校验 `MethodCHfieldDemandCoverage::validate` | 这是共享 Tri/Hex 选择语义，不按产品或 landcover 类别分支。`case9_projected_hfield_20260728` 为 `116/116` hard bins、`target_above_actual=0`，由 `197152` 增至 `210048` cells（`+6.54%`）；`case9_native_15arcsec_20260729` 仍在 pass 2 `TransitionPatch` 失败，两条路径不得混写 |
| **Ocean mask 后 hard-demand 产品支撑投影** | `production` | placeholder 恢复与 unsupported hard demand 排除：`mask_postproc_ocean/renewal.rs:238-245,305-313`；回归 `renewal.rs:441` | 只把不在最终 active product support 内的需求移出验收口径；不修改仍在 active support 内的 hard demand。Case 9 最新 active hard bins 为 `116`，且 `116/116` 覆盖 |
| **triangular-primal P2** | `test-only`，生产立项已归档 | `rust/earthmesh_cli/src/p2_primal_refine_research.rs`（仅 `#[cfg(test)]`）；耐久复现 `scripts/run_case9_regression.sh` | 旧 7-bin 基线上证明过构造能力，但最新共享 Method-C 基线已全覆盖；当前 `passes=0`、`changed=false`。无自然 unmet-demand 产品证据前不接 dispatcher、不抽象 backend |
| **HField edge-target spring** | `test-only`，生产接入已拒绝 | 定义 `method_c_nest_spring/mod.rs`；生产路径仍调 `spring_nest_with_radius_projection`（`method_c_spawn_hfield/mod.rs`）。M1 冻结拓扑 harness 只由 `EARTHMESH_M1_DIAGNOSTICS_PATH` 显式开启 | shaped movable edge 实测约占 10–16%，不可忽略。B/C 在 G-FRAGMENT 上发生零面积/非局部球面外心失效；精确三角形正面积 backtracking 只能延迟/转移失败。见 §5.1，当前不接入生产 |
| **transition repair** | `production` | `spawn_nest_pass_method_c_repairing`（`method_c_spawn_pass/mod.rs:70-193`），由 `method_c_spawn_hfield/mod.rs:1032` 与 `method_c_spawn_pass/mod.rs:54` 调用 | 扩张偏置：4 条修复路径中 3 条只加单元；上限 64 轮 + 掩码重复检测 |
| **`actual_above_target` 统计** | `production`（仅计数） | `compute_hfield_diagnostics`（`rust/earthmesh_quality/src/lib.rs:1400-1421`）；JSON `rust/earthmesh_quality/src/io.rs:106,122`；接线 `rust/earthmesh_cli/src/grid_quality_inputs/hfield.rs:118-130` | 口径用 regularized 场的 floor+max 量化，倾向**低报** |
| **M2a 闭包原因归因** | `production`，部分验证 | **[更新 2026-07-26]** `MethodCHfieldPassDiagnostics` 记录 `initial_seed_footprint` / `demand_tail` / `connectivity_bridge` 三个来源位，并输出 8 种 mask 组合、各来源独占计数和两两重叠计数；另有 `unexplained_selected_faces` / `seed_reconstruction_matches` | `initial_seed_footprint` 仅表示**初始 seed** 的足迹来源，不等于全部被选 seed 的原子足迹。全部被选 seed 足迹并集已由 `final_selected_faces` + `seed_reconstruction_matches` 完整证明，无需冗余字段。三 fixture 的诊断开/关 parity 已有字节级断言；尚未覆盖 `phase_halo` / `parent_closure` / `boundary_backtrack` 的逐面原因位 |
| **`topology_g` 梯度上限** | `production`，单例已校准 | **[更新 2026-07-27]** `method_c_topology_gradation_g`（`global_source.rs:39-53`）在 `max_level ≥ 2` 时把 g 压到 `1/(4·transition_rows)`；M0 已记录 `requested_g` / `effective_g`，并可独立开关 | G-CIRCLE release 扫描中，可生成边界在 `0.06434` 附近。默认 rows=4 的 cap `0.0625` 低约 `2.94%`，代价为 159 个 lbx cells（约 `0.137%`）。但 rows=5/8 时 cap 降至 `0.05/0.03125`，边界仍约 `0.06434`，裕度扩大到 `28.65%/105.90%`。代码复核确认：球面 HField 路径只在 cap 公式中读取 `halo/max_transition_row`，实际 spawn 使用固定 atmos/surface `max_mrows`，不消费这两个 expert rows。Case 9（有效 max-level=3）向下测试 `g=0.05/0.03125` 仍失败，且失败从 pass-2 valence 转为 pass-2 transition-patch / pass-1 parent-boundary，未支持“level 3 只需更小 g”假设。因此 rows 扫描证明的是 cap 参数耦合可能过保守，不是不同 transition geometry 下的系统安全裕度；跨 demand/product 的真实安全边界仍未证明，生产公式暂不改 |
| **LOP / Delaunay edge flip** | `test-only` | `checked_lop_edge_flip`（`rust/earthmesh_mesh/src/refine_edge_flip/mod.rs:11`）、`refine_delaunay_lop_one_based`（`rust/earthmesh_mesh/src/refine_lop/mod.rs:11`）；调用点仅 `rust/earthmesh_mesh/tests/refine_delaunay_lop.rs:44,88,125,160` | 在 lonlat 平面上操作 `cells_on_triangle`，旧面置 `[1,1,1]`、新面追加（`refine_lop/mod.rs:75-80`），完全不触及 Method-C 派生表 |
| **mask 后孤立分量清理** | `production` | `cleanup_masked_topology_one_based`（`rust/earthmesh_cli/src/masked_topology_cleanup.rs:108`）← `rust/earthmesh_cli/src/regional_gridfile_writers/landtype.rs:171`、`rust/earthmesh_cli/src/mask_postproc_ocean/renewal.rs:203`；`remove_isolated_ocean_one_based` ← `renewal.rs:173` | 另有一套 `remove_isolated_refined_cells`（`rust/earthmesh_quality/src/topology.rs:989`）走 refine_planner（`rust/earthmesh_refine_planner/src/lib.rs:763`），语义与前者不同 |
| **全球真实引擎测试** | `production`（规模不足） | `rust/earthmesh_cli/tests/refine_pipeline.rs:114-162`（HField 阈值源，NXP=6）、`:268`（circle spc）；`refine_end_to_end_topology.rs:31`（χ=2，`#[ignore]`，NXP=6） | NXP=6 → 362 cells；edge-CV/area-CV 在此规模下无统计意义 |
| **M0 度量探针** | `test-only`，schema 已收口 | **[更新 2026-07-27]** `rust/earthmesh_cli/tests/mesh_refinement_m0.rs`、`scripts/run_mesh_refinement_m0.sh`；输出分位数、可动集分组、质量 verdict、三个几何失败字段、请求/生效 g、M2a 原因 mask、成功/失败/跳过汇总及流式 `stage_trace.jsonl`；后续运行另记 `delaunay_proxy`；新 manifest 显式记录 `build_profile=test` | 三 fixture 的诊断开/关 parity 已有断言；迭代契约固定为 `{0,50,500,5000}`。`delaunay_proxy` 读取失败只记 `available=false`，不得让诊断改变生产成功/失败；正式冻结的 24 条旧记录未追写该字段。测量记录显式标注 `m0_diagnostics_enabled`：诊断态会额外执行 `validate_hfield_candidate` 的试生成，因此诊断态 wall-time **不能充当生产性能基线**；详细 repair 子阶段只在 `EARTHMESH_M0_REPAIR_TRACE` 显式开启时记录，正常 CLI 只保留低频轮次进度 |
| **区域真实引擎测试（HField 下）** | `production`，Atmosphere/Land/Coupled 已实测 | 区域 Atmosphere Method-C 回归 `default_regional_specified_refine_uses_method_c_and_subsets_domain`；独立 Land 使用真实 15″ IGBP + 内孔流域 Project；另有 LandOceanCoupled、真实 MERIT/CaMa Hydro、闭合流域内孔 Earth Hex | 独立 Land 为 130 Hex cells、max-level 2、24 个 hard bins、`target_above_actual=0`、`uncovered_hard_support=0`、`χ=1=expected`、质量 `pass`，SHA256 `d63a6beb08f5cc4c32289f26bd32c04b2947be1a0bafba026ddafb991a436bdb`。区域 HField 与独立 Land 的旧证据缺口已关闭；该真实 Land 数据文件仍是本机外部 fixture，不是可分发仓库资产 |
| **极区 / 日期变更线真实引擎测试** | `production`，release 回归已通过 | 耐久入口 `scripts/run_refinement_boundary_regression.sh`；Project lowering 负载仍由 `rust/earthmesh_project/tests/refinement_capability_matrix.rs:179-233` 锁定 | NXP=81、两层、5000 次 global/逐 pass spring。北极 circle 与跨日期线 bbox 均 `χ=2`、单连通、零 boundary/orphan/self-intersection/invalid polygon、`target_above_actual=0`；输出哈希两次独立运行一致 |
| **Cartesian 区域保护** | `production`，生产测试已通过 | `spawn_nest_from_cartesian_xy_target_levels_with_spring_deltax`（`method_c_spawn_hfield/mod.rs:1111`）；周期缝分类 `:94-259`；`refine_pipeline.rs:924,977,1026` 三条 release 测试分别锁定 mDomain=5、原生 XY 米制 HField、带地理 origin 的阈值采样；配对量化测试 `spherical_and_cartesian_quantizers_match_hard_demand_but_not_transition_level` | 与球面共用 pass 机器但**不共用量化器**；同一 circle 的硬需求一致，过渡点实测为球面 level 1、Cartesian level 2。生产语义未改，统一契约仍待决定 |
| **共享 Hex 输出定向** | `production`，真实 Project 回归已通过 | `orient_hex_cells`（`rust/earthmesh_cli/src/refine_pipeline/outputs.rs:53`）在所有最终 Hex writer 前执行；回归 `outputs.rs:1034` | Method-C 内部 M incidence ring 的方向允许任意，但最终 Hex 产品要求相邻共享边反向且球面环为 CCW。修复只规范输出环序，不改变 Method-C 选择、HField 或 Tri；全球极区/换日线与区域 Coupled/内孔 Hex 均通过，且各自两次运行哈希稳定 |
| **真实 MERIT-Hydro/CaMa 闭环** | `production`，real-data E2E 已通过 | 耐久入口 `scripts/run_real_hydro_e2e.sh`；测试 `rust/earthmesh_cli/tests/project_hydro_real_e2e.rs:49`；真实 MERIT 跨日期线测试 `project_coast_refinement.rs:410` | NXP=42 production parent、真实 MERIT/CaMa：`plan_applied=true`，coarse 1 cell → final 2 cells，最终网格 16307 Hex cells、level 2、拓扑/覆盖合法；coupling `pass`。质量为 `warn`，唯一触发项是尚未完成变分辨率校准的 `cell_edge_length_cv_max=0.42419`，不是几何/拓扑失败。20 km corridor / 10° bin 与 0.2° hydro polygon / 10° bin 的生产 HField 适配器压力例已通过；尚未把该 sub-bin 条件带入真实数据完整 Project |

---

## 5. 基线状态与下一步

### 5.0 [状态更新 2026-07-26] 实施进度：M0 正式矩阵已冻结

最终 schema 的正式矩阵已经完成并冻结：

- 契约：NXP=81、`global_niter=5000`、production topology-g cap
  （requested `0.2`、effective `0.0625`）、三个 fixture、固定
  `niter_refine ∈ {0,50,500,5000}`、两次重复、诊断开关 parity。
- 结果：24/24 条运行生成网格，0 failed、0 skipped；24/24 重复哈希一致；
  三个 fixture 的诊断开/关 gridfile 字节一致。
- 硬门：全部运行 `topology_issue_count=0`、`topology_fail_count=0`、
  `uncovered_hard_support_bins=0`、`target_above_actual=0`。
- 指标：P95/P99、可动集分组、质量 verdict、`self_intersection_count` /
  `invalid_polygon_count` / `aspect_ratio_max`、请求/生效 g、M2a 三来源 mask/独占/重叠、
  gridfile 字节级 parity、成功/失败/跳过汇总。
- `repair_plan_cell_count` / `repair_plan_movable_adjacent_count` 只描述受
  `repair_batch_limit=1` 限制的**本次修复计划批次**，不是全部坏单元数量。
- `target/mesh-refinement-m0-gcircle-curve-1785031313`、
  `target/mesh-refinement-m0-gfragment-curve-1785032304`、
  `target/mesh-refinement-m0-rbbox-curve-1785032512` 生成于 quality-verdict schema 之前，
  仅作 exploratory 证据，不作为正式基线。

正式质量曲线（数值为 edge-CV max）：

| case | niter=0 | niter=50 | niter=500 | niter=5000 |
|---|---:|---:|---:|---:|
| G-CIRCLE | fail / `0.6283` | warn / `0.5224` | warn / `0.4213` | warn / `0.3682` |
| G-FRAGMENT | fail / `0.6225` | warn / `0.4990` | warn / `0.4866` | warn / `0.5129` |
| R-BBOX | fail / `0.6080` | warn / `0.4118` | pass / `0.3420` | pass / `0.3234` |

这证明逐-pass 弹簧消除几何退化是必要的，但固定 5000 次不是通用生产策略：
G-CIRCLE 尚未触门，G-FRAGMENT 已出现过迭代，R-BBOX 则继续受益。M0 的
**可复现性、拓扑和覆盖门已通过**，不是三个案例的最终质量全部通过。

**正式基线前的清障项（均已关闭）：**

| 编号 | 问题 | 处理 |
|---|---|---|
| P1 | `topology_g` 与 M0 捆绑 | **已处理**：有旁路开关；正式冻结基线使用当前生产 cap，代价另做 cap-off g 校准，不再跑无产出网格的完整 cap-off 矩阵 |
| P2 | 生效 g 未进入记录 | **已处理**：每条 run 记录 `requested_g` / `effective_g` |
| P3 | 全部用例失败时 `cargo test` 仍报 `ok` | **已处理**：输出 ok/failed/skipped 汇总，全失败 panic |
| P4 | 「行为不变」无断言 | **已处理**：三个 fixture 分别比较诊断开/关；成功比较 gridfile 字节，失败比较失败类型 |
| P5 | 5000、峰值内存、诊断 fatal 路径 | **已处理到 M0 所需范围**：固定跑 `{0,50,500,5000}`；仅 cap-off 在 niter=0 失败时跳过同配置后续迭代；峰值内存不可用时显式记录 unavailable；诊断采集非致命 |

冻结产物：

- `target/mesh-refinement-m0-formal-1785055022/measurements.json`
- `target/mesh-refinement-m0-formal-1785055022/baseline_summary.json`
- `target/mesh-refinement-m0-formal-1785055022/measurement_manifest.json`
- `target/mesh-refinement-m0-formal-1785055022/freeze.json`

`topology_g` 校准仍是独立证据，不得用单一 G-CIRCLE 结果直接改生产公式。

> **[校准结果 2026-07-26；性能口径修正 2026-07-27]** `g=0.0625` 成功；
> 已知 `g=0.2` 为父边界越界失败。诊断关闭的 `g=0.08` 在
> `global_niter=5000` 下于 NXP=27/45/81 均最终失败，
> 并非无界循环；相同 NXP=27、`global_niter=0` 的控制臂成功，故不能把结果简化为
> 「g=0.08 在拓扑上绝对不可行」，它是 g 与父网格松弛后几何的耦合失效。失败臂 pass-2 的
> `selection / spawn` 分别为 `0.063 / 23.585 s`、`0.146 / 97.949 s`、
> `0.417 / 699.572 s`，但这些数字全部来自 Cargo `test` profile
> （`unoptimized + debuginfo`），只定位了 debug 路径内热点，**不能作为 production
> 性能基线或“运行时相变”证据**。当前源码的 release、诊断关闭 NXP=81 对照在
> `9.795 s` 后有界失败，故撤回生产运行时相变定性；拓扑失败与
> `global_niter` 耦合的结论不变。NXP=27 的 debug 流式记录显示 64 次外层
> repair 全部完成，selected faces 从 3070 单调增至 4377；23.544 秒 spawn 中
> fill-boundary 累计 20.531 秒，emit 仅 1.224 秒，non-triplet 1.315 秒。它是
> 「debug 下候选扫描主导、掩码单调增长、最终仍不可生成」的有界失效，不是 selection
> 或 emit 卡死。
>
> release 口径恢复后，原扫描点 `0.10/0.125/0.15/0.20` 均在 `8.3–9.8 s`
> 内失败，未发现高 g 非单调成功；随后在 `0.0625–0.065` 间夹逼。两次重复确认：
>
> - 最大观测成功 g：`0.0643359375`；
> - 最小观测失败 g：`0.06435546875`；
> - 区间宽：`1.953125e-5`；
> - cap `0.0625` 相对成功上界保留约 `2.94%` 裕度；
> - cap 网格为 116238 lbx cells / 31606 transition faces，边界成功网格为
>   116079 / 31451，即 cap 在该单例中增加 159 cells（约 `0.137%`）和
>   155 transition faces。
>
> 因而“cap 代价结构上无法测量”已撤回；在默认 rows=4 的 G-CIRCLE fixture 上，当前
> 公式接近真实可生成边界，并不明显过度保守。但 `2.94%` 不是跨配置稳定安全裕度。
> 进一步只改变第一活动层的 expert 配置，得到：
>
> | halo / max_transition_row | 公式 cap | 最大成功 / 最小失败 g | 相对裕度 | cap 额外 cells |
> |---|---:|---:|---:|---:|
> | `3 / 1`（公式下限仍按 4） | `0.0625` | `0.06433594 / 0.06437500` | `2.94%` | 159 |
> | `4 / 4` | `0.0625` | `0.06433594 / 0.06435547` | `2.94%` | 159 |
> | `5 / 5` | `0.0500` | `0.06432404 / 0.06436341` | `28.65%` | 1209 |
> | `8 / 8` | `0.03125` | `0.06434296 / 0.06438200` | `105.90%` | 4812 |
>
> 四组边界几乎不变不是“组合界稳定”的证据。球面 HField 接线把
> `halo/max_transition_row` 用于 `method_c_topology_gradation_g`，随后调用
> `spawn_nest_from_target_levels*` 时只传固定的 atmos/surface `max_mrows`；这两个 expert
> rows 不再参与 HField child topology。因此本扫描实际证明：增大 rows 只会收紧 HField g
> cap，并可能增加单元数，不会形成新的 transition geometry。沿该维度暂未看到 cap 放行
> 不可行 g 的风险，反而看到过保守风险；真正的正确性风险仍需跨 demand shape、max-level、
> spherical product 验证。以上仍是 `niter_refine=0` 的单例拓扑校准，不是最终质量对照，
> 生产公式保持不变。
>
> 记录：
> `target/mesh-refinement-m0-topology-g-calibration-1785046531/calibration_summary.json`、
> `target/mesh-refinement-m0-g008-small-1785049861/stage_summary.json`、
> `target/mesh-refinement-m0-g008-background-1785049841/stage_summary.json`、
> `target/mesh-refinement-m0-g008-fine-nxp27-1785050357/trace_summary.json`、
> `target/mesh-refinement-m0-g008-release-1785142958/run_summary.json`、
> `target/mesh-refinement-topology-g-release-sweep-1785143386/sweep_summary.json`、
> `target/mesh-refinement-topology-g-transition-sweep-1785143943/sweep_summary.json`。
>
> **[max-level=3 反证实验 2026-07-27]** Case 9 实际执行三层 HField 细化
> （日志为 `method_c-hfield-spawn-start 1/3, 2/3`）。为检验“同一 `0.0625` cap
> 在 level 3 不够保守”这一假设，使用同一 release CLI 向下测试：
>
> | g | repair 上限 | 结果 | 失败位置与类型 |
> |---:|---:|---:|---|
> | `0.05` | 1 | 退出码 2，`102.02 s` | pass 2，transition-patch；初始 selected faces `52638/276738` |
> | `0.05` | 64 | 退出码 2，`197.57 s` | pass 2，跑满 64 轮后仍为 transition-patch |
> | `0.03125` | 1 | 退出码 2，`88.16 s` | pass 1，parent-boundary；初始 selected faces `76383/131220` |
>
> 降低 g 会显著扩宽选择并改变失败类，但没有恢复 materialization；`0.03125`
> 反而更早失败，故可行性对 g **不是简单的“越小越安全”单调关系**。这组结果不支持把
> Case 9 改判为“cap 公式未随 max-level 缩放”；既有 valence/transition 闭包调查仍然必要。
> 必须同时保留三个边界：
>
> 1. 项目请求值 `0.2` 在 cap-on 下实际生效为 `0.0625`，因此本次只观测了
>    `0.03125/0.05/0.0625` 三个**有效 g 点**，跨度为 2 倍，不是 `0.03125–0.2`
>    的连续扫描；
> 2. `g=0.03125` 只允许 1 轮 repair，证明初始构型明显恶化并更早失败，不是完整
>    64 轮不可行性证明；
> 3. 没有穷举 `(0.03125,0.0625)`，不能宣称该区间为空。当前只能排除“继续减小 g
>    会单调恢复闭合”这一调参路线。
>
> `g=0.05` 从 1 轮到 64 轮时，失败由 iw6 移到 iw9、但始终属于 transition-patch，
> 说明 repair 在同一约束类内移动失败位置而没有闭合。`g=0.03125` 的 pass-1 选择率
> `76383/131220=58.2%`，高于 `g=0.05` 的 `49149/131220=37.5%`；更宽裙带与更早
> parent-boundary 失败一致，但当前只作为机制证据，不据此添加经验 lower clamp。
> 强制抬高用户请求的 g 会改变其梯度约束；正确契约应检测并报告“过渡区域膨胀后父层外部
> 不足”，而不是静默改成另一个 g。
>
> 后续跨 demand shape / max-level / product 的校准必须遵守：
>
> - 每个点同时记录 `requested_g`、`effective_g`、完整/有界 repair 口径、selected-face
>   比例、失败 pass 与失败类别；
> - 先做离散粗扫；只有观测到同一配置、同一口径下的局部单调可行/失败分界，才允许夹逼；
> - 默认输出 `observed_feasible_points` / `observed_failure_points`，不得把稀疏点补成连续边界；
> - 只有经过局部单调性验证后才输出 `feasible_intervals`；可能有多个区间。只有在预先声明
>   的搜索范围、分辨率和完整 repair 口径内均无成功点时，才允许报告“该已测范围为空”。
>
> 在没有单调可行性信号前继续盲目夹逼没有解释力，本轮停止。完整记录：
> `target/case9-topology-g-downscan-1785144967/sweep_summary.json`。
>
> **[production cap 松弛耦合对照 2026-07-26]** 固定 NXP=81、requested `g=0.2`、
> cap on（effective `g=0.0625`）、`niter_refine=0`，对 G-CIRCLE/G-FRAGMENT
> 分别比较 `global_niter=0/5000`。四条生产 spawn 的两个 pass 都在第一轮外层 attempt
> 直接 materialize，未进入 fill-boundary/shrink/grow。pass-2 spawn：
> G-CIRCLE `1.3990 → 1.3998 s`，G-FRAGMENT `0.9736 → 0.9445 s`；
> 因此全球弹簧没有在 production cap 下增加 transition repair 成本。
>
> G-FRAGMENT pass 2 的 final selected faces 为 `4878 → 5193`，但
> connectivity-bridge-only 仅 `135/4878=2.77% → 153/5193=2.95%`；
> 大部分 bridge 毛计数与 demand-tail 重叠（`2349 → 2691`），不能作为安全删除候选。
> 这组数据**不支持启动 connectivity-bridge 最小化型 M2b**；高 demand-tail/bridge
> 重叠仍应作为需求表达/量化信号观察，而不是交给 transition repair 删除。
> 本实验 `niter_refine=0`，只判定拓扑生成与运行时耦合，不作为最终质量证据。完整记录：
> `target/mesh-refinement-m0-production-cap-coupling-1785054022/comparison_summary.json`。

---

### 5.0.1 [状态更新 2026-07-27] 全球三角形 landcover 阈值细化：第一处缺陷已修复，第二层闭包仍失败

本次使用真实全球 landcover 数据执行案例 9（全球海洋三角形 + landcover 阈值），目的不是让单一
fixture 过关，而是验证 production cap 下 Method-C 对碎片化全球需求场的通用闭包能力。

**输入契约：**

| 项 | 值 |
|---|---|
| domain / target | `Global` / `Ocean` / `Tri` / `CoastalOcean` / `FVCOM` |
| 分辨率 | 100 km；Method-C 将 NXP `80 → 81` 以保持 stride-3 lattice |
| 细化 | landcover threshold `12`，AutoRefine，最多 3 层 |
| HField | requested `g=0.2`；production topology cap 生效 |
| 数据 | `input/landtype_igbp_update.nc`，sha256 `89bde86be2436f8762bd9d2b9bcfa727193e74299941e9d1545222b54e41be2a` |
| 项目 | `target/global-tri-landcover-threshold-2026-07-26/project.yaml`，sha256 `eb5b4950d6cb61797bf05e5f514afbd922d1714d9b1460ce5c190eb01d72a40c` |

#### 运行序列与结论

| 阶段 | wall-time / 退出 | 结果 |
|---|---:|---|
| 原始全尺度复现 | 1183 s / 2 | pass 1 在 `Method-C perimeter loop revisited M point 3833 before closing` 失败 |
| 第一版修复复验 | 1786 s / 2 | 证明单点候选修复不足；同一掩码内存在多个独立的 vertex-only perimeter contacts |
| 最小批量闭包复验 | 1856 s / 130（主动停止） | pass 1 成功并完成 2000 次 nest spring；pass 2 进入既有 64 轮外层 repair，串行候选扫描成本不可接受 |
| 候选并行化后的完整复验 | 16750 s / 2 | pass 1 继续成功；pass 2 跑满 64/64 仍失败；历史产物未记录 executable/profile，不能与 release 直接比较 |
| 当前 release 分段计时复验 | 217.379 s / 2 | 64/64 仍以同一 7-edge 类错误失败；shrink 与 fill-boundary 占完整 wall-time `57.65%`，占新增 63 轮 repair 成本约 `96.9%` |
| coverage-aware shrink + 稳定父空间见证 | trace-only，3 轮 | 每轮检查 6235 个 shrink 候选，`coverage_rejected=0`、`chosen_w=none`；三次变化的 child M 均映射到同一 parent U `291553` |
| seed/rad3 并集诊断 | trace-only，1–3 轮 | pass 1 原始并集有 3 个 vertex-only contacts，首个正是 M `3833`；pass 2 原始并集有 1 个 contact（M `7009`），但 7-edge 见证落在 parent U `291553`，其端点 M `89628/89629` 都只有一个 active run |

第一处缺陷的通用修复位于共享 Method-C perimeter repair，而不是案例分支：

- `fill_method_c_vertex_only_perimeter_contacts`
  （`rust/earthmesh_mesh/src/method_c_perimeter_repair_candidates/mod.rs:8-91`）
  扫描全部 M 点；只在至少两个独立 contact 存在时，对每个 cyclic ring 保留最长 inactive gap，
  填充其余最小连接面，并重新执行 concavity closure 与 parent-mrlw 验证。
- 该路径接入 `repair_method_c_non_triplet_perimeter`
  （`rust/earthmesh_mesh/src/method_c_perimeter_repair/mod.rs:17-107`）。
- `method_c_perimeter_repair_fills_multiple_vertex_only_contacts`
  （`rust/earthmesh_mesh/src/tests/method_c_perimeter.rs:299-354`）
  锁定「两个独立 vertex-only contacts 不能靠单候选修复」的回归。

这项修复在全尺度运行中确实解决了原始错误：pass 1 从 `revisited M point` 失败转为
`method_c-hfield-spawn-end 1/3`，随后完成 `method_c-nest-spring 2000/2000`。
因此它不是只让小测试通过的局部补丁；但新诊断也证明它属于**事后 repair**：
原始 seed/rad3 足迹并集在进入 repair 前已经含 3 个 vertex-only contacts。
它封住了 pass 1 的症状，尚未证明选择器按构造不会再次产生同类掩码。

#### 剩余失败不是卡死

pass 2 初始选择 `39398/244410` faces。外层 repair 每轮都完成
non-triplet 检查、emit、shrink 与 fill-boundary，并把选择面数固定增加 18：

```text
39398, 39416, 39434, ... , 40514, 40532
```

64/64 轮全部有界完成，但没有找到可生成的合法掩码。最终错误为：

```text
Method-C perimeter length invalid:
Current nested grid crosses (or is too close to) the next coarser grid boundary;
M point 141814 exceeds 7-edge Method-C ring while walking from U edge 440114
```

因此当前状态应精确表述为：

1. **已封住：** 多个独立 vertex-only perimeter contacts 导致 pass 1 周界游走重访；
   根因位于原始 seed/rad3 并集，repair 只负责事后闭合；
2. **已修正但不是本例根因：** shrink 现在先过滤 coverage、再从合法候选中选最佳，
   避免「先选唯一最佳、再过滤后放弃整个收缩方向」；单元回归已锁定。
   真实 pass 2 中 `coverage_rejected=0` 且无任何可行 shrink，说明该缺口没有造成本例单调扩张；
3. **未解决：** 碎片化 landcover 在 pass 2 的父层边界附近，现有
   `shrink → fill-specific-M → fill-boundary → grow` 贪心序列单调扩张但不收敛；
4. **不能把 pass 1 签名直接外推到 pass 2：** pass 2 虽有一个原始 vertex-only contact，
   但 7-edge 见证的稳定 parent U 两端 ring 都只有一个 active run；二者可能同属
   seed 并集合规性问题，但当前证据不支持“同一个单点 cyclic-run 规则可以同时修复”；
5. **release 失败搜索仍有浪费，但不能宣称算法获得 77× 加速：** 最新 release
   完整 64 轮为 `217.379 s`；旧 4 h 39 min 产物未记录 executable/profile，
   两者不可直接比较。当前 release 中 `125.31 s` 花在不会消除稳定见证的
   shrink/fill-boundary 扫描上，且最终仍失败；
6. **不能继续做的事：** 不增加 64 轮上限，不把 `g`、halo、transition rows 或质量阈值
   调参当作修复，不为 landcover=12 或某个 M point 增加特例。`g=0.05/0.03125`
   的诊断反证已单独记录：更小 g 只改变失败形态，没有闭合 Case 9。

`try_fill_method_c_perimeter_boundary` 与 `try_shrink_method_c_perimeter_once` 已复用现有 Rayon
并行独立候选（分别见
`method_c_perimeter_repair_candidates/mod.rs:123-190` 与
`method_c_perimeter_repair_shrink/mod.rs:8-75`），且有 1/4 线程确定性回归
`method_c_boundary_repairs_are_deterministic_across_thread_counts`
（`tests/method_c_perimeter.rs:356-402`）。并行化只降低同一搜索的耗时，不改变候选评分或结果；
完整复验仍失败，说明剩余问题是**搜索/闭包策略**，不是缺少并行。
2026-07-27 最新执行 `cargo test -p earthmesh_mesh --lib`：
145 passed、0 failed、1 ignored（共 146 项）；`cargo clippy -p earthmesh_mesh --lib -- -D warnings`
亦通过。小规模回归通过不改变上述全尺度失败结论。

#### 架构判断：停止增强 repair，回到选择侧，但不能只做单点形态学

真正的产品目标是：**选择器产出的 seed/rad3 并集在进入 emit 前已经可 materialize**，
而不是不断增加能挽救更多失败形状的 repair 类。否则案例矩阵每增加一种边界，
repair 就可能再增加一种特判，重回“万能优化器”膨胀路线。

但“在 seed 格点上消掉 vertex-only contact”本身还不是完整解法。最新 trace 将两类约束分开了：

- pass 1 是父 M 点上的多 active-run，现有 `cyclic_active_runs` 能在 emit 前直接识别；
- pass 2 是 parent U 邻域经过 canonical split 后产生的 child 7-edge ring，
  而该 U 两端父 M ring 各自都只有一个 active run。它需要一个**边过渡兼容性不变量**，
  不能由单点 majority closing/opening 推出。

因此下一步只允许以下顺序：

1. **保持行为不变地测量：** 已输出 `seed_union_vertex_only_contacts`、
   `seed_union_first_contact_m_point`，并在 repair trace 中记录稳定 parent U、端点 M、
   端点 active ring；诊断不得改变生产掩码。
2. **从 canonical split 表推导 parent-space 局部判据：** 给定 parent U 两端的有序 active ring，
   在不构建完整子网格的情况下预测 child ring 是否会超过 7。先用本例失败模式做正例，
   再用现有全球 hex、区域边界和 Cartesian 成功例做反例。判据与真实 emit 不一致就停止，
   不进入算法修改。
3. **判据成立后才改 seed 选择：** 只在 legal seed 集上枚举能消除见证的局部 seed 编辑；
   每个编辑必须同时通过硬需求 coverage、parent-mrlw、perimeter triplet 和真实 emit。
   这是选择合法化，不新增 face-level repair 类。
4. **失败契约保持诚实：** 若当前 seed 算子集找不到闭合掩码，只报告
   “在当前算子集下未闭合”，不得把单个 fail-fast 见证或局部候选穷尽写成数学不可行证明。

#### [状态更新 2026-07-27] parent-U 自环预言器已通过正反例门，但尚未改变选择

最新 trace 将 pass 2 的 `7-edge` 症状进一步定位到 canonical transition patch 的一个确定别名：

- perimeter triple `867` 的有向 `nwdiv` 模式为 `[3, 3, 4]`；
- p2 的 canonical 远端槽 `iu51` 与 p3 半边槽 `iu45` 指向同一 child U；
- `perim_fill3_method_c` 随后把 p3 midpoint 写入 `iu51`，而该边另一端已经是同一 midpoint，
  因此 parent U `291553` 的 child copy 变成 midpoint-to-itself 自环；
- M-ring walker 之后在这条自环上循环，最终以“超过 7-edge”退出。这里的 `7-edge`
  是下游症状，自环才是本例的首个确定结构错误。

`method_c_transition_self_loop_witnesses` 直接复用 `perim_fill3` 的 p2/p3 canonical 槽规则，
只读取 parent perimeter 与 suppression flags，不构建子网格，也不改变掩码。验证结果：

| 验证 | 结果 |
|---|---|
| case 9 pass 1 | `[]`，真实 emit 成功 |
| case 9 pass 2 | `[(867, 291553), (889, 293204)]`；真实 fail-fast 首见证正是 parent U `291553` |
| canonical 表级负例 | 原始 transition fixture 预测 `0`，真实 emit 成功 |
| canonical 表级正例 | 同一周界错位一个 triple phase 后预言器非空，真实 emit 以 `Valence` 失败 |
| M0 冻结成功矩阵 | G-CIRCLE / G-FRAGMENT / R-BBOX，`0/50/500/5000 × 2`，24/24 成功、全部 candidate validation 可 materialize、预测非零 `0`、重复运行位级一致 |

冻结负对照证据位于
`target/mesh-refinement-m0-self-loop-predictor-release-1785129511/measurements.json`；
真实正例位于
`target/global-tri-landcover-threshold-2026-07-26/run-self-loop-predictor.log`。

这一步完成的是**廉价且已校准的局部合法性 oracle**，不是案例 9 的最终修复。
生产选择与 repair 顺序仍未改变，案例 9 仍明确退出码 2。下一步只能用该 oracle
枚举能消除见证的局部 seed 编辑；候选仍必须通过硬需求 coverage、parent-mrlw、
perimeter triplet 与真实 emit。不得把 `[3,3,4]` 本身硬编码成禁用模式：
合法性来自有向 canonical 槽是否别名，不来自无序 `nwdiv` 计数。

#### [状态更新 2026-07-27] 局部 seed 编辑已完成最小穷举；自环消失仍不等于可 materialize

选择阶段已有的 `legal_seed`、`selected_seeds`、rad3 footprint 与 hard-demand
coverage 被保留到 M0 candidate validation，仅用于诊断，不改变返回的 `selected_faces`。
对两个已预测 parent-U 自环的端点一环，合法 seed 候选池只有 3 个；单 seed 与双 seed
全部非空组合共 6 组，因此该局部范围内没有静默截断：

| 门 | 通过数 |
|---|---:|
| 局部编辑集 | 6 |
| hard coverage | 5 |
| parent-mrlw | 5 |
| perimeter triplet | 5 |
| 已预测自环清零 | 2 |
| 真实 no-repair emit 成功 | 0 |

两个自环清零候选分别为：

- 移除 seed `90008`；
- 同时移除 seed `89611` 与 `90008`。

二者都不再触发 `method_c_transition_self_loop_witnesses`。旧版 walker 曾把真实 emit
稳定报告为 parent M `90016` 的 `Valence`；在非起点 U-edge 重访被单独分类后重跑，
两组候选均改判为 `TransitionPatch`，而 corrected valence census 只剩独立见证
`[90455]`。旧记录中的 `90016/90038` 是短周期误计，不是真实高价环。由此可得：

1. parent-U 自环预言器准确覆盖了已经证明的 p2/p3 child-U alias 类，但不是完整的
   transition compatibility 判据；
2. 清除两个已预测 alias 后仍可形成另一种 M-ring 短周期，同时还保留独立 parent-M
   价数不兼容；两类约束不能二选一处理；
3. **生产选择继续不改，案例 9 继续明确失败。** 下一步应先从 canonical 表推导
   同时覆盖 transition-patch 与 parent-M valence 的选择合法性约束，并以这两个
   oracle-clear/emit-fail 候选作正例、冻结成功运行为负例。在双向校准前，不再扩大
   seed 搜索深度，也不新增 repair 类。

证据：

- 单 seed 初筛：
  `target/case9-single-seed-edit-1785130685/diagnostics.json`
- 单/双 seed 最小穷举：
  `target/case9-pair-seed-failure-1785131337/diagnostics.json`
- 两个 oracle-clear 候选的稳定父空间失败：
  `target/case9-pair-seed-all-failures-1785131754/run.log`
- 修正重复 U-edge 分类后的候选复验：
  `target/case9-cycle-reclassification-1785161036/classification_summary.json`

实验性的 witness-local 单点/邻域 fill 曾把 valence 依次转成 non-triplet、
mask cycle 或 transition-patch，并未闭合；这些行为改动已退出生产路径。
`IcosahedronMPointNeighbors` 的 7 槽是 canonical 表示上限，不是可调质量阈值，禁止放宽。

#### [状态更新 2026-07-27] parent-M 全量价数普查已接入诊断；半径 1 已被证伪

`derive_icosahedron_m_neighbors_canonical_checked_with_prognostic` 仍保持生产 fail-fast
语义；仅在该步骤已经失败后，新增的 canonical ring census 扫描同一份已生成 connectivity，
收集全部超过 7-edge 的 child M，并通过 `imnew` 映射为稳定 parent M。它不改变选择、
repair、emit 结果或退出码。

Case 9 的实测结果：

| 掩码 | fail-fast 首见证 | 全量 parent-M 价数见证 |
|---|---:|---|
| 原始 pass-2 candidate | `TransitionPatch`，parent U `291553` | `[89628, 89633]` |
| 移除 seed `90008` | `TransitionPatch`，parent M `90016` | `[90455]` |
| 移除 seed `89611, 90008` | `TransitionPatch`，parent M `90016` | `[90455]` |

表中候选行已按重复 U-edge 修正后的实现重跑。旧普查的
`[90016,90038,90455]` 中，`90016/90038` 来自短周期误计；`90455` 仍是 corrected
census 中的真实价数见证。原始 candidate 则同时有 transition-patch 首错误和独立
`[89628,89633]` 价数见证。严格结论因此不是“先判断瞄准哪一类”，而是：
**Case 9 同时违反 transition-patch 与 valence 两类 canonical 约束；任何 pre-emit
合法化都必须联合验证，不能修一类后假定另一类保持不变。**

局部依赖也已收紧：

- `7/13` 是 `apply_method_c_perimeter_mrows` 的传播宽度；该函数在 M-neighbor/7-edge
  检查成功后才执行，因此不能用它推导本次判据半径；
- parent M `90455` 的 6 个入射 parent W 面全部未选中，且没有 perimeter triple
  直接触及其入射 U 边；仅检查 parent M 的入射面/边（半径 1 直接签名）会漏报；
- 将入射 U 的 canonical `iu[0..12]` 邻边纳入后，可看到与 `90016` 相同的两个
  transition triples。但修正分类后 `90016` 属于 `TransitionPatch`，不是真实 valence
  见证；因此该相似性只能证明两类约束存在局部耦合，不能再用来宣称一跳扩展 stencil
  已足以预测 valence。

当前 census 是失败后诊断，不是可用于大规模候选搜索的 pre-emit oracle。下一步必须在
共享 canonical split/perim-fill 语义上同时验证两类约束，并满足：

1. 对两个真实自环清零但 emit 失败的候选，同时命中 `TransitionPatch` 与 `[90455]`
   valence；
2. 对 M0 冻结的 24 个成功 candidate 保持零误报；
3. 与 canonical split 表做表级 parity；不复制一份会漂移的“近似 emit”；
4. 双向校准通过前，生产选择和 repair 继续不改，也不继续扩大 seed 组合。

新增证据：

- `target/case9-parent-m-valence-census-1785133678/diagnostics.json`
- `target/case9-parent-m-radius1-signatures-1785134155/run.log`
- `target/case9-parent-m-expanded-signatures-1785134501/run.log`

#### [状态更新 2026-07-27] 成功路径价数普查通过冻结矩阵；仍不是 pre-emit 判据

M0 candidate validation 在 no-repair emit 成功后，对已经 materialize 的 child connectivity
运行同一 canonical ring census，并记录：

- `materialized_m_valence_census_available`
- `materialized_m_valence_violation_count`

正式 release 矩阵结果：

| 检查 | 结果 |
|---|---:|
| M0 成功运行 | `24/24` |
| candidate pass 普查可用 | `48/48` |
| candidate pass 价数违规为 0 | `48/48` |
| 与上一冻结矩阵 gridfile 逐字节一致 | `24/24` |

证据位于
`target/mesh-refinement-m0-valence-census-formal-1785137121/measurements.json` 和
`valence_census_summary.json`。这一步验证的是 census 实现与成功的 canonical derive
在现有矩阵上保持一致；两者读取的是同一份已生成 child connectivity，因此它**不是**
未来 parent-space pre-emit 判据的负例验证，不能据此宣称扩展 stencil 已经充分。

失败 repair 的每轮 trace 现在直接输出该轮 error payload 中已有的
`parent_m_valence_witnesses`，没有重复执行 census，也没有改变 emit 成本或生产行为。
冻结 24 例均首轮 materialize，不会产生 repair 轨迹。Case 9 另做了 3 轮有界诊断：

- 三轮 parent-M 价数见证均为 `[89628, 89633]`；
- 每轮 fill-boundary 仍增加 `18` 个面，但选择的 parent M 分别为 `243/2613/3049`；
- 已预测 transition self-loop 每轮仍为 `[(867,291553),(889,293204)]`；
- 运行在 `83 s` 内按设定上限退出码 2。

证据位于 `target/case9-valence-trajectory-1785137807/run.log` 与
`trajectory_summary.json`。这只证明前三轮没有改善上述两个已知错误类；完整 64 轮未重跑，
不得把短轨迹外推为全程不变或不可行性证明。

后续实现保持最小组合，而不是新建一个重复全部逻辑的巨型 oracle：

1. 复用现有 coverage、parent-mrlw、perimeter triplet、vertex-only contact 与
   transition-self-loop 检查；
2. 不把 self-loop predictor 清零等同于 transition 合法；真实 no-repair emit 仍是最终
   canonical oracle；
3. 对两个真实 self-loop-clear/emit-fail candidate 同时核对 `TransitionPatch` 与
   parent-M valence，对冻结成功 candidate 做负例；
4. 全部门通过前，生产选择、repair 顺序和失败契约继续不改。

代码路径复核后，本轮在新 hard gate 之前停止：仓库内目前唯一已证明正确的 valence oracle 仍是
`selected mask -> no-repair child materialization -> canonical ring census`。一跳 canonical U
扩展只解释了 Case 9 已观察到的依赖，没有证明充分性；仓库也没有可直接复用的“只构建局部
child connectivity”接口。此时实现 parent-space 启发式会复制一份近似 emit，违反表级 parity
要求。因此本轮**没有新增 pre-emit valence hard gate**；下一阶段若继续，应先证明能从现有
canonical table 写入逻辑抽出共享的局部 connectivity plan，否则保持 exact child census。

证据文件：

- 原始失败：`target/global-tri-landcover-threshold-2026-07-26/run.log`
- pass 1 修复、pass 2 性能暴露：
  `target/global-tri-landcover-threshold-2026-07-26/run-minimal-closure.log`
- 64 轮最终结果：
  `target/global-tri-landcover-threshold-2026-07-26/run-parallel-shrink-fill.log`
- 最终状态：
  `target/global-tri-landcover-threshold-2026-07-26/run-parallel-shrink-fill-status.txt`
  （`exit_code=2`，`wall_seconds=16750`）
- seed 并集与稳定父空间见证：
  `target/global-tri-landcover-threshold-2026-07-26/seed-union-diagnostics-2.json`、
  `target/global-tri-landcover-threshold-2026-07-26/run-parent-ring-diagnostic.log`

#### [状态更新 2026-07-27] exact child materialization 已计时；暂不抽取 topology-only builder

`emit_method_c_tables` 新增了仅在 `EARTHMESH_M0_DIAGNOSTICS` 下输出的 release 分段计时。
它不进入报告 schema、不改变错误处理，也不改变生产输出。发布态对照结果：

| 运行 | exact emit 次数 | 单次已计时阶段合计 |
|---|---:|---:|
| Case 9，repair 最多 1 轮，最终失败 | 5 | `0.049–0.095 s`，中位数 `0.050 s` |
| G-CIRCLE，成功负对照 | 4 | `0.060–0.095 s`，中位数 `0.077 s` |

Case 9 的失败 emit 中，`index-plan + base-remap/full-subdivision + transition-patch +
connectivity-neighbors + M-neighbors` 合计约 `0.05 s`。因此当前 release 中一次 exact
emit 不是主要成本；旧 `4 h 39 min` 运行因缺少 executable/profile 记录，不能据此归因
或与当前 release 做加速比。1 轮与 64 轮的 release 差分另行证明，新增 repair 成本主要
来自大量候选的全掩码复制、闭包与周界评分。

但这也不等于 exact oracle 已经适合大规模候选搜索。它仍需构造全量 child U/W 表并运行
canonical 邻接推导，复杂度为全网格 `O(N)`；按本次 release 下限估算，1000 个候选仅
materialization 就约 `50 s`，6000 个候选约 `5 min`，尚未计候选生成与 coverage/周界检查。
把现有 emit 拆成共享 topology-only builder 只能省去少量坐标、lineage 和最终验证工作，
不能把全网格 oracle 变成廉价局部判据。

因此本轮作出以下停止决定：

1. 保留现有 exact materialize + canonical census 作为最终 oracle；
2. 暂不为尚未收敛的候选搜索抽取共享 topology-only builder，避免一次无明确消费者的
   中风险重构；
3. 下一步先证明候选空间可以稳定缩到很小，或从 canonical 表推导出有表级 parity 的
   增量/局部 exact plan；只有届时 exact oracle 的调用次数与收益可量化，才重新评估抽取；
4. 生产选择、repair、64 轮上限和失败契约继续不改。

计时证据：

- Case 9：
  `target/case9-emit-stage-timing-1785140559/run.log` 与
  `emit_stage_timing_summary.json`
- 成功 release 对照：
  `target/m0-success-release-emit-stage-timing-1785140979/run.log`
- 诊断开/关 gridfile sha256 均为
  `be672e51ecfb7aec6468eca380006fef9267049049d1f38cf32da37737e59fdd`，
  证据位于
  `target/m0-success-release-emit-stage-parity-1785141052/gridfile_sha256.txt`

#### [状态更新 2026-07-27] 完整 64 轮 repair 分段计时：候选扫描是主成本，但局部过滤尚不安全

在 `EARTHMESH_M0_REPAIR_TRACE` 下，外层 repair 现记录 non-triplet、emit、shrink、
fill-M、fill-boundary 与 grow 的阶段总耗时。用当前 release 静态 CLI 对 Case 9
完整执行 64 轮，未超时，结果如下：

| 阶段 | 次数 | 总耗时 | wall-time 占比 | 首次 → 末次 |
|---|---:|---:|---:|---:|
| non-triplet | 65 | `0.149 s` | `0.07%` | `0.0036 → 0.0033 s` |
| exact emit | 65 | `6.045 s` | `2.78%` | `0.0970 → 0.1442 s` |
| shrink | 64 | `42.754 s` | `19.67%` | `0.4359 → 1.3032 s` |
| fill-boundary | 64 | `82.558 s` | `37.98%` | `1.1606 → 2.2692 s` |
| 已计时阶段合计 | — | `131.507 s` | `60.50%` | — |
| 未计时固定基线 | — | `85.872 s` | `39.50%` | global/nest spring、选择、I/O 等 |
| 完整运行 | — | `217.379 s` | `100%` | 退出码 `2` |

每轮候选规模保持不变：

- shrink：`6235`，`coverage_rejected=0`，64 轮均无可行收缩；
- fill-boundary：`3222`，每轮仍选择 `added=18` 的候选；
- parent-M 价数见证 64 轮均为 `[89628,89633]`，首个 parent U 仍为 `291553`；
- 最终仍以 child M `141814` 的 7-edge ring 失败。

未计时的 `39.50%` 不是隐藏的逐轮候选成本。相同 release 二进制将 repair 限为 1 轮时，
wall-time 为 `88 s`、已计时阶段为 `1.648 s`、未计时部分为 `86.352 s`；放开到 64 轮时，
wall-time 增加 `129.379 s`，已计时阶段增加 `129.860 s`，差仅计时粒度内的 `−0.481 s`。
因此约 `86 s` 是 global/nest spring、选择与 I/O 等固定基线；新增 63 轮的成本已被当前
阶段计时完整解释。

这组数据同时修正三种过强结论：

1. 当前 release 路径中“一轮一次”的 exact emit 不是瓶颈；旧 4 h 39 min 的构建口径
   未记录，不能宣称算法性能问题已经消失或获得 77× 加速。
2. shrink + fill-boundary 是**新增 repair 轮次**的主要成本，不是完整 wall-time 的
   `100%`；二者 `125.313 s` 约占 1→64 轮 wall-time 增量的 `96.9%`。
3. 不能因此对全部候选逐一 emit。按首轮失败 emit 约 `0.063 s` 计算，
   `3222` 个 fill 候选已约 `203 s/轮`；末轮约 `0.144 s` 时约 `464 s/轮`。
   exact oracle 只有在候选已被构造性缩小到几十个后才合适。

本轮没有加入 witness-local 候选过滤器。原因不是缺少启发式，而是尚无完备性证明：
Method-C 在每个连通周界上按有序 triple phase 写 canonical transition；远处候选改变周界
顺序或长度后，可能移动见证处的 triple phase。因而“与 parent M 几何距离近”不等于
“唯一可能改变 child ring”，把搜索限制到局部邻域可能漏掉合法解。现有 concavity closure
还会迭代传播新增 face，进一步扩大依赖。

当前结论是：

- 保留全量 cheap scoring 与每轮一次 exact emit；
- 不实现近似 dependency-domain filter，也不恢复 topology-only builder；
- 后续若继续，必须先从 canonical perimeter component 与 triple phase 推导可证明完备的
  候选等价类；若该依赖最终覆盖整个 3222 点周界，则候选缩减路线关闭，问题回到
  repair 目标/移动集合，而不是性能优化。

证据：

- `target/case9-repair-stage-timing-64-1785141891/run.log`
- `target/case9-repair-stage-timing-64-1785141891/repair_stage_timing_summary.json`
- repair trace 开/关的成功 G-CIRCLE gridfile sha256 均为
  `be672e51ecfb7aec6468eca380006fef9267049049d1f38cf32da37737e59fdd`：
  `target/m0-repair-stage-timing-parity-1785142278/gridfile_sha256.txt`

#### [状态更新 2026-07-27] 见证依赖域精确候选普查：评分不是唯一问题，单步/双步/三步 fill 均无解

复用完整 64 轮 trace，而不是新增运行，首先得到：

- parent U 始终为 `291553`，parent-M 价数见证始终为 `[89628,89633]`；
- 两端 parent-M ring 不是完全不变：第 34 轮起，M `89628` 的 active pattern 从
  `[F,F,F,T,T,F]` 变为 `[F,F,T,T,T,F]`，但价数见证没有消失；
- 64 个 fill 选择均不同、每轮仍固定增加 18 faces。因而现有 repair 最终会改变见证邻域，
  但当前评分没有把这种改变与 child 合法性联系起来。

随后在 `EARTHMESH_M0_REPAIR_TRACE` 下，从稳定 parent U 的端点 ring、直接 W faces 与相邻
canonical U faces 构造一个 **24-face 观测依赖域**。它只用于缩小诊断集合，不是生产
完备性过滤器。首轮 3222 个合法 fill-boundary 候选中：

| 精确普查 | 组合数 | 成功 | valence | non-triplet |
|---|---:|---:|---:|---:|
| 改变观测依赖域的单候选 | 14 | 0 | 5 | 9 |
| 上述 14 个候选的两两组合 | 91 | 0 | 45 | 46 |
| 上述 14 个候选的三组合 | 364 | 0 | 134 | 230 |

当前贪心仍选择 `M=243`，它不改变该 24-face 域。故可得两层结论：

1. **评分缺少 child 合法性信号：** 3222 个候选中已有 14 个能改变已观察的见证依赖域，
   但 `(added,remainder,perimeter_len,im)` 仍按最小 `im` 选择无关移动；
2. **只改评分仍不能解决：** 对 14 个相关候选复用 exact materialization 后，单步、
   双步及与 Method-C triple 相位对应的三步组合均无成功项。当前
   `mark_fill_rad3 + concavity closure` 移动原语在该有界集合内没有解。

这不是全局不可行性证明：24-face 域尚无完备性证明，也没有穷举四个以上 fill、混合
shrink/fill 或选择侧 seed 配置。但继续扩大组合数会重新进入无界调试搜索，且没有新的
结构依据，因此在三组合后停止。生产评分、repair 顺序和候选过滤均未改变；一次性
pair/triple 穷举代码在取证后删除，只保留低成本的 trace 依赖域计数与单候选精确分类。

证据：

- 64 轮依赖变化摘要：
  `target/case9-repair-stage-timing-64-1785141891/witness_dependency_summary.json`
- 单候选精确分类：
  `target/case9-dependency-candidate-exact-1785146420/summary.json`
- 双候选精确分类：
  `target/case9-dependency-pair-exact-1785146659/summary.json`
- 三候选精确分类：
  `target/case9-dependency-triple-exact-1785146912/summary.json`

下一机制步骤不再是调 g、调评分权重或扩大 fill 组合，而是定义一个能同时表达
`non-triplet + transition-patch + valence` 的选择侧 canonical 合法性约束；在该约束
能复用真实 split 表并通过成功矩阵负对照前，Case 9 保持明确失败，不新增生产修复。

#### [状态更新 2026-07-27] v2 `iterC` 范式可借鉴，但原判据不能移植到 v3

`v2.0.0:src/MOD_refine.F90` 的 `iterC_judge` 确实在 materialize 前处理五/六边形周围的
细化标记、外部“射线”和七边上限，并由外层循环迭代到不动点。它证明了“选择阶段先闭合
合法性，再 materialize”是这个网格家族中的既有范式；但 v2 使用 `1→4` 细分与
`1→2` 过渡三角形，v3 使用 stride-3 seed、rad3 footprint 与 mrow transition，
因此 v2 的计数公式不能直接视为 v3 的价数判据。

为验证而非猜测，本轮临时实现了完整的 v2 `iterC` 只读探针：

- 以 v3 的 W face 作为 v2 triangle、v3 的 M point 作为 v2 polygon；
- 覆盖 pentagon fill、opposite-hex fill、adjacent rays、single-selected rays 和
  empty-ring overflow 五条 v2 分支；
- 只在诊断副本上迭代到不动点，不修改生产 selected mask；
- 闭包后调用现有 exact materialize + canonical valence census 验证，而不把探针结果
  当成成功。

Case 9 的 release 结果是否定的：

| pass | v2 首轮命中 | v2 闭包 | exact 结果 |
|---|---|---|---|
| 1 | 7 个 parent M，推荐 8 faces | 16 轮，增加 89 faces | `non_triplet_perimeter` |
| 2 | `[2759,59577,59584,59588,59590]`，推荐 8 faces | 2 轮，增加 20 faces | 仍为 `valence` |

pass 2 原始 exact 见证是 `[89628,89633]`，与 v2 首轮及闭包触发集合均不重合；
v2 闭包后的 exact 见证反而扩大为
`[8866,8867,8873,8878,59601,89628,89633]`。一个已能 exact materialize 的小型
HField 正样本上，v2 规则也会额外命中 `[105,249,285]` 并增加 4 faces，说明它还会
对 v3 合法掩码产生过度闭包。

因此当前结论是：

1. **保留 v2 的架构范式，拒绝移植 v2 的判据公式。**
2. 不把 v2 `npoly + adjusted rays > 7`、`0.5` 相邻射线合并或固定 active-run 模式
   接入生产，也不据此扩大掩码。
3. 下一步仍须从 v3 自己的 canonical split/triple-phase/rad3/mrow 表推导合法性约束；
   它必须同时覆盖 `non-triplet + transition-patch + valence`，并以现有 exact
   materialize+census 为最终 oracle。
4. 临时探针在取得结论后已删除，避免继续扩大
   `method_c_spawn_hfield/mod.rs`；生产选择、repair、输出和失败契约均未改变。

证据：

- `target/case9-v2-iterc-probe-1785149284/probe_summary.json`
- `target/case9-v2-iterc-probe-1785149284/diagnostics.json`
- release executable sha256：
  `0ba19e48bd9d083131b8d9d6cd3f67d2c55ba242c07cf4fcbbf50f3b4851bd22`
- 运行退出码 `2`，wall-time `83.93 s`；repair 被限制为 1 轮，避免重复已知的 64 轮盲搜。

#### [反证实验 2026-07-27] Case 9 不是合法的高价 M ring；禁止用放宽 7 作为修复

为区分“Tri 产品被不必要的 7-edge 上限阻塞”和“child connectivity 本身非法”，本轮只在
`/tmp` 隔离副本中把 Method-C M-neighbor / spring 临时容量从 7 提升到 12；生产源码、
选择、repair、默认 dispatch 与格式均未改。相同 project 与输入哈希下，结果为：

| 实验 | 失败点 | guard 前 walk | 唯一 U edge | wall-time |
|---|---|---|---:|---:|
| fixed 7 | M `141814` exceeds 7-edge | `440114,440120,440117,440119,440117,440119,440117,440119` | 4 | `438.94 s` |
| capacity 12 | M `141814` exceeds 12-edge | 前四条相同，之后 `440117,440119` 交替直到第 13 步 | 4 | `740.40 s` |

因此 Case 9 的该见证不是一个合法的 8–12 价多边形。walk 在
U `440117 ↔ 440119` 间循环；其中 U `440119` 已在既有 trace 中表现为
`im=[141814,141814]` 的 self-loop/alias。把容量继续扩大只会延后同一失败并增加耗时，
不能让 Tri 产品绕开它。`7` 在这个案例中首先充当非法环游走的有限保护，而不是被证明
过严的产品质量门。

三个隔离负对照
`two_level_field_spawn_nests_and_deeper_passes_stop_cleanly`、
`disconnected_hfield_demands_select_every_component` 和
`method_c_boundary_repairs_are_deterministic_across_thread_counts` 均通过（`3/3`）。
这只证明临时容量改动没有破坏这些小型正样本，不构成生产采用证据。

结论：

1. **关闭“Tri 产品直接放宽 7/扩大 `dimc`”路线。** 本案例没有显示合法高价环需求。
2. 根因仍在 materialize 前的 canonical transition/self-loop/alias 闭包；下一步应让
   选择/transition 构造不产生该循环，而不是扩大数组。
3. 若未来其他独立案例确实 materialize 出闭合、无重复且价数大于 7 的合法 ring，再单独
   评估动态容量和输出兼容性；不得用本案例作为依据。

证据：

- `/tmp/earthmesh-method-c-valence12-20260727-Cyqs8s/experiment_summary.json`
- cap-12 release executable sha256：
  `e2e8922514327af1377d217095c7cebc033a86bffb0cef5ca87367790bf8bf25`
- project sha256：
  `eb5b4950d6cb61797bf05e5f514afbd922d1714d9b1460ce5c190eb01d72a40c`

#### [状态更新 2026-07-27] 重复 U-edge 已与真实高价环分离；现有 Method-C 移动集仍不能闭合 Case 9

canonical M-ring walker 原先只在 `npoly > 7` 时退出，因此
`U_a → U_b → U_a → U_b ...` 这类不返回起点的短周期会被误报成 `Valence`。
共享实现现已在计数前检测**非起点 U-edge 重访**：

- 唯一 U-edge 数真正超过 7 的环继续报告 `Valence`；
- 非起点 U-edge 重访报告 `TransitionPatch`；
- `collect_icosahedron_m_valence_witnesses_canonical` 不再把该短周期计入真实高价环普查。

定向回归
`repeated_non_start_u_edge_is_a_transition_patch_not_valence`
锁定了这三个语义。`earthmesh_mesh --lib` 全套结果为
`146 passed / 0 failed / 1 ignored`，`cargo clippy -p earthmesh_mesh --lib -- -D warnings`
通过。该改动只修正错误分类和诊断，不改变合法网格输出。

相同 Case 9 在 release 下限制为 3 轮 repair 后，3/3 均稳定报告：

- `kind=TransitionPatch`；
- parent U=`291553`；
- exact trace 在返回起点前重访 child U，并包含 duplicate-endpoint child U。

这证明此前 fail-fast 的“7-edge”不是独立失败类，而是 transition patch 产生短周期后的
下游误报；它不否定同一候选中 corrected census 给出的真实价数见证。修正后复验还显示：

- 原始 pass-2 candidate：`TransitionPatch` + parent-M valence `[89628,89633]`；
- 两个 self-loop-predictor-clear 候选：均为 `TransitionPatch` + parent-M valence `[90455]`；
- 旧 `[90016,90038,90455]` 中只有 `90455` 保留为真实高价环。

因此目标已从“先判断 TransitionPatch 或 Valence”收紧为**联合 canonical 合法性约束**。
但准确分类**没有解决**掩码闭合。为避免再次扩张 repair，本轮只在 `/tmp` 隔离副本中
完成两个有界反证：

| 现有 Method-C 对称/移动 | 搜索范围 | 结果 |
|---|---:|---|
| 周界方向 × triple phase | 正向/反向各 3 相位，共 6 个 exact emit | `0/6` 成功；均仍形成非法 M-ring |
| 见证依赖域原始 W-face grow | 17 个未选 face 的单/双组合，共 153 个 exact emit | `0/153` 成功；55 non-triplet、3 transition、43 valence、52 其他结构失败 |
| coverage-preserving 整圈 grow | 连续 32 层；selected faces 从 `42,359` 增至 `145,806` | `0/32` 成功；转为 non-triplet 或其他 transition 失败 |

结合前述单/双/三 fill 组合均无解，可关闭以下路线：

1. 放宽 7 或扩大 `dimc`；
2. 调整周界起始相位或反转遍历方向；
3. 继续增加单点 fill 或把现有 raw-face grow 提前；
4. 用全边界 grow 代替 canonical 合法性闭包。

当前诚实状态是：**Method-C 主线仍可用于已通过的全球/区域 Hex 与既有 Tri 正样本；
Case 9 的碎片化 landcover、三层 global Tri 选择尚不能被当前 canonical transition
移动集物化。** 下一步若继续，必须在 materialize 前定义同时覆盖
`non-triplet + transition-patch + valence` 的 v3 canonical 选择约束；在该约束能用
成功矩阵作负对照、用 Case 9 作正例之前，不再新增 repair 类或扩大组合搜索。

证据：

- 准确分类后的 Case 9：
  `/tmp/earthmesh-method-c-ring-cycle-main-1785159074/stderr`
- 准确分类后的两个 self-loop-clear 候选：
  `target/case9-cycle-reclassification-1785161036/classification_summary.json`
- 六种方向/相位：
  `/tmp/earthmesh-method-c-reverse-phase-1785159624/stderr`
- 见证依赖域 raw-face 单/双组合：
  `/tmp/earthmesh-method-c-raw-face-probe-1785160295/stderr`
- 32 层 coverage-preserving grow：
  `/tmp/earthmesh-method-c-grow-probe-1785159832/stderr`

#### [后备研究 2026-07-27] 产品适配保持可分离；当前优先修复共享 Method-C

> **后续校正：** 容量 12 反证实验已经排除“Case 9 只是被 Tri 产品不需要的 7-edge
> 上限卡住”。因此本节保留为长期架构边界与失败兜底，**不再是当前下一实现**。
> 当前最小路径是继续使用熟悉的共享 Method-C，修复 canonical transition 产生
> self-loop/alias 后让 M-ring walker 循环的问题；只有该根因被修复后仍证明 Tri 产品
> 存在不可调和的合法拓扑需求，才重新启动 triangular-primal 旁路。

v2 没有给出可直接移植的 Case 9 修法，但它促使本轮重新检查了一个更上游的问题：
**案例 9 是 `Ocean + Tri + FVCOM` 产品，却仍通过面向 Method-C Delaunay/Voronoi
对偶表的共享细化后端生成。** 当前失败表面由
`IcosahedronMPointNeighbors { iu: [usize; 7], iw: [usize; 7] }`
（`rust/earthmesh_mesh/src/icosahedron_types/mod.rs:96-103`）报出，但上面的容量 12
反证实验已证明 Case 9 的具体见证是 self-loop/alias 上的重复游走，不是合法高价 ring；
EarthMesh 的 FVCOM 读取路径则使用文件提供的动态 `maxelem`
（`rust/earthmesh_cli/src/mode_file_io/fvcom.rs:21-49,93-107`）。因此：

- 该错误证明当前 **Method-C child connectivity 无法物化这个选择掩码**；
- 它不证明把内部上限从 7 放宽即可修复，也不支持直接扩大输出 `dimc`；
- 它不等价于“所需的 FVCOM 三角网格在数学或产品契约上不可行”；
- MPAS mesh 本身也使用动态 `maxEdges`，不能把 Method-C 的固定 7 误写成普适 MPAS 规范；
- 继续为 tri 案例扩张共享 Method-C repair，可能是在修一个 tri 产品本不需要承担的内部约束。

仓库已经存在适合分流而无需重写管线的边界：

1. `capability_registry.rs:291-330,447-455,538-549` 已将
   `Atmosphere→Hex→MPAS` 与 `Ocean→Tri→FVCOM` 定义为不同正式产品；这是**当前受支持的
   adapter 组合**，不是把 Tri 永久绑定 FVCOM、把 Hex 永久绑定 MPAS 的架构不变量；
2. `global_source.rs:407-442,678-697` 当前将两者都送入
   `MethodCDelaunayMesh`，共享的是生成器而不只是需求场；
3. `UnstructuredMesh` 已用固定三角 `m_to_w` 和变宽 `w_to_m`
   表达中立输出（`unstructured_mesh_support/types.rs:5-13`），现有 NetCDF、mask、
   quality 和可视化链可继续复用；
4. 连续 `HField h(x)`、hard demand、source provenance 与 product support mask
   可以继续共享。需要分开的只是 **拓扑生成器** 与 **产品质量契约**。

##### 后备架构（当前不实施）

```text
source / landcover / hydro / region
                │
                ▼
shared HField + hard demand + provenance
                │
        ┌───────┴──────────┐
        ▼                  ▼
triangular-primal      polygonal-dual
generator             generator
        │                  │
        ▼                  ▼
 TriMesh              PolygonMesh
        │                  │
  ┌─────┴─────┐      ┌─────┴─────┐
  ▼           ▼      ▼           ▼
FVCOM       UGRID   MPAS         CoLM
(current)   /other  (current)    (current)
        │                  │
        └────────┬─────────┘
                 ▼
      existing neutral writers/adapters
```

这不是要求现在增加 backend trait、factory 或配置层。**第一阶段只做独立 research
harness**：读取冻结的 HField/需求快照，输出既有 `UnstructuredMesh`，不接入
`global_source.rs` 的生产 dispatch。只有旁路实验跨案例成功，才设计最小 opt-in 接缝。

这里有三条相互独立的轴：

- **Topology family：** triangular primal、polygonal/Voronoi dual，未来也可以有 quad/mixed；
- **Domain policy：** global、regional、basin、coast、masked、coupled；
- **Model adapter：** FVCOM、MPAS、CoLM，以及未来的 UGRID、ADCIRC、SCHISM、FESOM、ICON
  或其他格式。

生成器由 topology family + domain policy 决定；writer 和最后一层验收由 model adapter
决定。不得再用 `output_format` 反向定义网格算法。

##### 候选路线与优先级

| 优先级 | 产品 | 候选 | 为什么值得测 | 当前边界 |
|---|---|---|---|---|
| 1 | Triangular primal（当前 FVCOM） | 在现有球面 primal 上做确定性的 conforming triangle bisection（longest-edge 或 newest-vertex） | 不受 Method-C 固定 7-ring 表限制；可直接检验 Case 9 的失败究竟是 backend 表示问题还是需求本身不可闭合；Rivara/NVB 有有限终止、共形闭包和形状正则性理论 | 先做 ignored test / research binary；输出先经中立 TriMesh，再由 FVCOM adapter 消费 |
| 2 | Triangular primal + coast/basin | 区域/海岸采用 constrained 或 restricted Delaunay refinement | 符合复杂岸线、岛屿、浅水/CFL 的三角网格要求；可把边界误差、最小角和尺寸渐变作为显式约束 | 工程量高于 bisection；先用外部结果作 oracle，不进主线 |
| 3 | Polygonal/Voronoi dual（当前 MPAS/CoLM） | 保持当前 Method-C 生产路径；用 spherical Delaunay/Voronoi SCVT 生成器作外部比较 | 这类网格关心正交、centroidal、well-centered 和平滑变分辨率，而不是“看起来像六边形” | 只比较，不替换已通过的 Method-C cases |
| 4 | Regional polygonal dual | 从已验证的全球 SCVT/Voronoi 网格做 subset/cull | 可避免在开边界重新发明局部对偶合法化；MPAS limited-area 是当前参考实例 | 这是域策略，不应写死为 MPAS 专属实现 |
| 停止 | shared Method-C | 为 Case 9 继续添加 repair 类、调 g/halo/轮数或扩大盲搜 | 已证明 v2 原判据、局部 seed 编辑和 bounded fill 组合均不能闭环；会重新走向单体万能优化器 | 除非新的 canonical 不变量先被证明，否则不再扩张 |

外部工具只承担 **比较器**：

- MPAS-Tools 官方以连续 `cellWidth` 驱动 JIGSAW 生成球面/平面网格，并提供 signed-distance
  与平滑 blending；这支持“共享 HField、后端分别消费”的边界：
  <https://mpas-dev.github.io/MPAS-Tools/master/mesh_creation.html>
- JIGSAW-GEO 的 Frontal-Delaunay / hill-climbing 面向球面 Delaunay/Voronoi 变分辨率网格：
  <https://arxiv.org/abs/1611.08996>
- restricted Delaunay 的 curve protection/off-centre refinement 是区域复杂边界的候选：
  <https://arxiv.org/abs/1606.01289>
- OceanMesh2D 是 FVCOM 等海岸三角网格的有用 oracle，明确处理 shoreline、bathymetry、
  wavelength、feature size、gradation 和 CFL：
  <https://gmd.copernicus.org/articles/12/1847/2019/>
- longest-edge / newest-vertex bisection 的共形闭包、有限终止与 shape regularity 可作为
  内部最小实验的理论基础：
  <https://doi.org/10.1002/nme.1620200412>，
  <https://www.nist.gov/publications/30-years-newest-vertex-bisection>

JIGSAW 的上游许可证允许研究/机构使用，但商业分发需另行安排：
<https://github.com/dengwirda/jigsaw>。因此它不得作为本轮新增依赖或随产品分发；
只允许用户本地安装后的非默认比较运行，并记录版本、输入与输出 sha256。
OceanMesh2D 为 GPL-3.0/MATLAB，同样只作为科学参考，不进入 Rust 主线。

##### 分开的验收契约

**Triangular-primal 基础门：**

- 硬门：三个不同顶点、正面积/一致方向、每条内部边恰有两个入射三角形、边界环可遍历、
  邻接互反、无重叠/自交、hard demand 覆盖 100%、按 ocean policy 清理后无非法孤立分量；
- 分布指标：最小角分位数与超限占比、radius-edge 或等价三角质量、aspect 分布、
  `edge_length / h(x)`、相邻尺寸渐变；
- 海岸域追加：岸线误差、岛屿/孔洞保持、bathymetry slope、最浅水深下 CFL 尺度；
- 不使用 hex 的 regular-polygon angle deviation 或 hex edge-CV 作为 tri 产品门。

在基础门之后再叠加 adapter 门，例如 FVCOM 的 OBC/`maxelem`/节点要素表，或其他
tri-based model 的格式、边界和物理约束；这些不属于三角生成器本身。

**Polygonal/Voronoi-dual 基础门（UI 可继续称 Hex）：**

- 硬门：primal/dual 邻接互反、cell-edge-vertex 顺序一致、Euler 与域契约一致、
  Voronoi cell 不自交、无 non-local circumcenter、hard demand 覆盖 100%；
- 分布指标：primal-dual orthogonality、centroidality、well-centeredness、
  edge-crossing midpoint offset、尺寸渐变及 edge/aspect 的 P95/P99/超限占比；
- MPAS 最终验收不能只靠几何指标，应按 mesh specification 建议增加 shallow-water
  test 2 / test 5 等模拟烟测：
  <https://www2.mmm.ucar.edu/projects/mpas/mpas_website_linked_files/mpas_meshspec.pdf>；
- MPAS-A 的目标是 centroidal Voronoi C-grid：
  <https://mpas-dev.github.io/atmosphere/atmosphere.html>，
  不能用 tri 最小角门代替其 primal/dual 数值契约。

同样，MPAS 与 CoLM 应在 polygonal-dual 基础门后分别叠加 adapter 门；未来其他
Voronoi/hex consumers 不需要复制生成器。

当前 `QualityThresholds` 仍是一套共享阈值，而 `cell_view` 只是报告标签
（`rust/earthmesh_quality/src/lib.rs:380-423,440-446,1728-1789`）。因此在生成器分流前，
质量报告也必须先形成上述 **分视图/分产品契约**；但本阶段只定义字段和证据，不拍新的
绝对阈值。

##### 后备实验（仅在 Method-C 根因修复仍不能闭合时启动）

1. 冻结 Case 9 的 HField、hard demand、product support 和输入 primal，记录 sha256；
2. A 臂运行当前 Method-C，保留已知失败作为负对照；
3. B 臂在 ignored test 或独立 research binary 中执行 tri-only conforming bisection，
   只生成 `UnstructuredMesh`，不调用 Method-C repair；
4. 可选 C 臂调用用户本地 JIGSAW/OceanMesh2D，只作外部 oracle，不加入依赖；
5. 对 A/B/C 使用同一 triangular-primal 基础门，并叠加当前 Case 9 所需的 FVCOM adapter
   门；记录 coverage、cell count、wall-time 和两次重复哈希；
6. B 只有在 Case 9、一个全球平滑阈值 tri 正样本、一个区域海岸 tri 正样本上同时
   `topology=pass + coverage=100% + deterministic`，且没有修改生产输出时，才进入
   opt-in backend 设计；否则删除实验代码并记录否定结果。

在这个门通过前：

- 不修改默认 dispatcher、Method-C repair、`topology_g`、质量默认阈值；
- 不为单一 landcover 阈值或某个 parent M/U 写特例；
- 不建立统一 backend 抽象；
- hex 主线继续用已验证的 Method-C，tri 旁路只回答“不同生成器是否解除错误约束”。

#### [最新状态 2026-07-28] Case 9 hard-demand 已由共享选择路径闭合；P2 生产路线归档

耐久复现入口：

`scripts/run_case9_regression.sh`

脚本固定 Case 9 Project 语义，校验真实 landcover 输入 sha256
`89bde86be2436f8762bd9d2b9bcfa727193e74299941e9d1545222b54e41be2a`，使用
`static-netcdf` release CLI 生成网格，再运行现有 P2 负对照；任何覆盖、拓扑、单元数或
输出哈希变化都会失败。最新完整复验清单位于
`target/case9-regression-1785248896/case9-regression.json`。

结果：

| 项 | 旧基线 | 最新共享 Method-C |
|---|---:|---:|
| triangle cells | `197152` | `210048`（`+12896`，`+6.54%`） |
| active / adequately covered hard bins | `114 / 107` | `116 / 116` |
| `target_above_actual` | `22` | `0` |
| 自交 / 非法 polygon / orphan / 非流形 | `0 / 0 / 0 / 0` | `0 / 0 / 0 / 0` |
| final verdict | `fail` | `warn`（仅角偏差 provisional warn） |

最终 gridfile sha256：
`ecfd5366d9087df9c9208913aa27851e976f71184e4a0a5da76fc332eca79ef2`。
该哈希与此前 `/private/tmp/earthmesh-case9-shared-phase-rerun-1785246312` 的独立运行一致。
变化来自共享 phase-support 遍历完成和最新 Ocean product-support 口径，不是
`mode_grid`、FVCOM、landcover 类别或具体 parent id 的特例。它确实改变了全局选择并增加
单元数，故必须作为生产行为变更记录，不能只写成覆盖门修复。

同一最终 gridfile 上运行既有 triangular-primal P2 负对照：

| 指标 | 结果 |
|---|---:|
| active / adequately covered hard bins | `116 / 116` |
| passes | `0` |
| changed | `false` |
| cells / boundary edges / max primal valence | `210048 / 13560 / 7` |
| Euler χ | `-61` |

因此旧 P2 A/B 仍证明“若共享 Tri base 存在 unmet demand，共形 shared-edge refinement
能够补齐”的研究能力，但**不再证明当前生产产品需要第二生成后端**。P2 生产立项和 dispatcher
接入在此归档；只有最新共享语义下自然出现新的、可复现的 unmet-demand Tri 产品，且 P2
在不破坏模型边界/模拟门的前提下解决它，才允许重新评审。

以下 2026-07-27 至早期 2026-07-28 的 7-bin、phase 枚举与 P2 A/B 内容保留为历史调查证据，
不再代表当前生产状态。

#### [历史状态，已被 2026-07-28 共享修复取代] Case 9 曾剩余 7 个离散 hard bin 未覆盖

此前 §5.0.1 记录的 `transition-patch` / 重复 U-edge / 7-edge repair 失败属于历史诊断。
共享 Method-C 选择器在**中间 generation** 扩展本地 canonical phase support 后，Case 9
已经可以完成三层 materialize；该修复不按 `mode_grid`、FVCOM、landcover 类别或某个
parent id 分支，因此仍是共享 Method-C 修复，不是三角形特例。

当前最小复验使用 `niter_refine=1`，产物位于
`/tmp/earthmesh-case9-final-no-face-center-1785171397`；最终
`quality_summary.json` sha256 为
`41d06d5657e58a574c8dac0462a3406baead990e55888489e79412fb76926142`。结果如下：

| 项 | 结果 |
|---|---:|
| `sjx_points` / `lbx_points` | `197154 / 104736` |
| 最终 tri cell / vertex | `197152 / 104736` |
| 自交 / 非法 polygon / orphan / non-manifold fan | `0 / 0 / 0 / 0` |
| edge-CV max / P95 / P99 | `0.2743 / 0.1622 / 0.2103` |
| aspect max / P95 / P99 | `2.0313 / 1.4720 / 1.6843` |
| `hfield_target_above_actual_count` | `22`（Fail） |
| `hfield_uncovered_hard_support_bin_count` | `7`（Fail） |

因此不能再把 Case 9 描述为“Method-C 无法用于 Tri”或“仍卡在 64 轮 repair”。
Method-C 已生成拓扑与几何合法的 tri/dual 网格；**当前唯一硬失败是源需求覆盖契约**。
这 7 个未覆盖 bin 是稀疏、彼此分离的 max-level landcover 阈值岛：

| bin | lon | lat |
|---:|---:|---:|
| `124760` | `-79.75` | `-3.25` |
| `138445` | `-77.25` | `6.25` |
| `172683` | `121.75` | `29.75` |
| `172984` | `-87.75` | `30.25` |
| `180927` | `-76.25` | `35.75` |
| `204127` | `3.75` | `51.75` |
| `226468` | `14.25` | `67.25` |

已完成并撤回的反证实验：

1. `niter_refine=0` 与 `1` 得到同一组 7 个缺失 bin，排除逐 pass spring 为根因；
2. 增加 face-center 采样没有消除这 7 个 bin，并使三个既有两层 HField fixture 在 pass 2
   跨父边界；该改动及其单点测试已撤回，不能作为通用修复；
3. immediate M ring、aligned seed ring 和放宽 parent-boundary demand traversal 均未改变结果；
4. 直接扩张 hard raster，以及仅在中间 pass 扩张 hard parent support，均把缺失从
   `7` 恶化到 `53`，已全部撤回；
5. 最终局部几何仍是约 `42–50 km` 的 level-1 尺度，故不是单纯 lineage/报告口径错误。

上述结果把根因收窄为：离散 0.5° hard island 在约 100 km 父层采样上可能没有独立
canonical seed；将它强行加入当前单一 stride-3 phase/component 选择又会改变全局闭包，
造成更多 hard bin 丢失。**现有单 phase 选择尚未保证任意碎片化 raster bin 的逐 bin 覆盖。**

下一步只允许一个有界实验，不再继续扩大支持半径：

1. 把 7 个 exact bin center 只作为验收 witness，不膨胀选择输入；
2. 在冻结 parent 上，对每个不连通 hard-demand component 枚举有限 canonical phase；
3. 稳定排序：exact hard-bin 覆盖优先，其次 materialize/topology 合法，最后 cell count；
4. 先在 M0 24-run 成功矩阵验证零回归，再以 Case 9 验证是否从 `7` 降到 `0`；
5. 若不存在全覆盖且可 materialize 的 phase 组合，诚实报告该离散需求对共享 Method-C
   不可实现，再重新评估隔离的 triangular-primal 后端；不得用 dilation、阈值放宽或
   case-specific patch 隐藏失败。

triangular-primal P0/P1 研究代码保留为隔离后备证据，但**不再是当前第一优先级**。

#### [执行更新 2026-07-28] phase 路线已关闭；剩余缺口是面积需求与点采样的表达失配

按上一节的唯一有界实验，曾在 M0 诊断态临时枚举最终 pass 的 canonical phase；实验后
诊断入口已删除，未进入生产选择器。正式配置使用 `global_niter=5000`、
`niter_refine=1`，基线仍为 `7` 个 uncovered hard bin / `22` 个
`target_above_actual` 单元。清理后的最终复验位于
`/tmp/earthmesh-case9-final-clean-1785176717`，输出 gridfile sha256 为
`b66f010074bbc7a1fcb6ca5c301f4e166ade063ff2d93f34c724c73ab43750a4`，与相位实验前
基线逐字节一致。

最终 pass 共有 `35` 个 hard-demand component，每个 component 恰有 `9` 个 canonical
phase class。7 个 witness 只映射到 component `19 / 21 / 28`。对这三个 component 分别
运行全部 `8` 个非基线 phase，共 `24` 次正式 A/B：

- `24/24` 均成功 materialize，未出现拓扑或几何错误；
- `24/24` 均保持 `uncovered=7`、`target_above_actual=22`；
- 单元数在 `196764–197289` 之间变化，说明 phase 确实改变了选择，但没有改变这 7 个
  hard-bin 覆盖结论；
- 产物目录：
  `/tmp/earthmesh-case9-phase-feasibility-production-1785175033`。

因此可以关闭“换 component phase 即可补齐 hard bin”的解释。更直接的诊断是：7 个
hard bin 的持久化需求层级均为 `3`，但在最终父层中，各 witness 最近的 M 点以及该 M 点
所有相邻 U-edge midpoint 的最大拓扑采样层级均为 `2`。也就是说这些是**有正面积的
0.5° raster demand**，但当前 HField→Method-C 选择接口只通过 M 点与边中点把连续面积
需求离散成 anchor；这些碎片没有进入 level-3 demand anchor 集合，后续 phase、seed 与
repair 自然无从恢复。

又做了一次不进入生产的反证：在 HField 三层完成后，将 7 个 bin 的精确 0.5° bbox 作为
level-3 几何区域再跑一次共享 Method-C。该运行仍然拓扑/几何合法，并只增加 `44` 个
tri cell，但覆盖由 `7/22` 恶化为 `8/25`；产物位于
`/tmp/earthmesh-case9-hard-bin-region-repair-1785176081`。因此“生成后补 7 个小 bbox”
也不是通用闭合：rad3 原子重构会改变邻域单元覆盖，并可能暴露新的 hard bin。

当前结论必须分两层表述：

1. **Method-C 拓扑生成器可用。** Case 9 已完成三层共享 Method-C，最终 tri/dual
   拓扑、几何与海陆 mask 均合法；
2. **当前 HField 选择契约不完备。** 它不能保证任意碎片化面积 raster 的逐-bin hard
   coverage。相位枚举、raster dilation、face-center、局部 seed ring、生成后 bbox 修补
   均已被数据否定，不能继续以 repair 类或参数补丁堆叠。

所以下一阶段不再修改 Method-C 主线，而重启已经隔离的 triangular-primal P2 实验：

1. 复用 `rust/earthmesh_mesh/tests/primal_refine_research.rs` 已通过的 P0/P1 共形细分与
   dynamic-valence 拓扑门；
2. 冻结 Case 9 当前合法 Method-C tri primal 作为 A 臂，只在 research test/binary 中
   对 `target_above_actual` 三角形做确定性的 shared-edge conforming closure，形成 B 臂；
3. B 臂必须同时通过 `coverage=100%`、Euler/边互反/无 hanging edge、两次哈希一致，
   并在一个全球平滑 tri 与一个区域海岸 tri 负对照上不退化；
4. 在上述三案例通过前，不增加 backend 抽象、不接 dispatcher、不改变 hex/dual 主线；
   若 P2 仍不能闭合，则记录为离散 hard-demand 与当前生成能力不相容并停止，不再扩大
   Method-C repair。
共享 Method-C 已越过原拓扑故障，先完成上述 phase/component 可行性判定，改动更小，
也更符合“原 Method-C 能用则不大改”的约束。

---

### M0 冻结时采用的测量定义（已完成）

以下内容保留为 M0 的验收契约，不再是待实施的代码改动。

> 「行为不变」而非「不改代码」：新增指标字段与计数上报确实要动代码。
> 判据是**生成结果逐位不变**——同输入下 `nCells`、选择掩码、拓扑表、
> 全部既有质量字段与改动前完全一致；改动只增加新的输出。

具体定义：选定 **恰好 3 个**已存在的真实引擎配置（全球六边形 + 圆形细化；全球六边形 + 碎片化阈值；
区域六边形 + bbox），把 NXP 提到质量指标有统计意义的规模（建议 NXP=81，与 `cases/` 里的实际产出量级一致），
对每次运行输出一条机器可比较的记录，内容为：

- `nCells`、`actual_max_level`、`transition_faces`、wall-time、峰值内存；
- 每个 pass 的：可动 M 点数 / 总 M 点数、可动边中 `mrow_multiplier != 1.0` 的比例；
- edge-CV 与 level-normalized area-CV，**分别在「可动集邻接单元」与「其余单元」上统计**；
- 固定 `niter ∈ {0, 50, 500, 5000}`；不得用 50→500 的某个指标漂移替代 5000 的实际测量；
  只有结果逐位相同的真实 plateau 才可在未来版本考虑跳过；
- 同输入重复 2 次的关键数组一致性哈希。

#### 为什么它的信息增益最高

一次测量同时判定 M1、M2、M3 三个阶段是否值得做：

| 观测结果 | 结论 |
|---|---|
| 可动集占比很小（预期 < 10%）且 edge-CV 随 niter 早早平台 | **M1 收益上限极低** → 跳过 M1，直接 M2；文档的顺序应当调换 |
| edge-CV 随 niter 下降但平台，且坏单元集中在可动集内 | M1 值得做，**且必须保留 mrow 乘子** |
| 坏单元根本不在可动集内 | 弹簧与本问题无关 → 全部预算给 M2；M3 的启动条件此时才第一次有证据 |
| 三个 case 的 `actual_above_target / nCells` 差异很大 | 只能说明需要 M2a 归因；在原因位出现前，不能判定差异来自闭包、需求几何还是消费路径 |

#### 为什么它比继续优化算法更优先

因为**目前没有任何人能说出「再迭代一次算法之后指标变好了还是变坏了」**。
`Stat5` 无分位数、真实引擎断言是布尔式的、工作树已有 78 个文件 `+11165/−1474` 的混合改动。
在这个状态下任何算法改动都无法归因。这个行动**不改网格算法语义**，但会触及报告结构、序列化和上报接口，
因此属于低风险而非零风险；必须用网格数组与既有报告字段逐位一致证明行为不变。

#### 需要修改或观察哪些文件

**观察（不改）：**

- `rust/earthmesh_mesh/src/method_c_nest_spring/mod.rs:303-331`（可动集定义）
- `rust/earthmesh_mesh/src/method_c_nest_spring_iteration/mod.rs:88-99,174`（两套目标构造）
- `rust/earthmesh_mesh/src/method_c_spawn_hfield/mod.rs:1046-1058`（生产弹簧调用点）
- `rust/earthmesh_cli/src/refine_pipeline/global_source.rs:602-612`（球面驱动闭包）

**最小改动（仅测量）：**

- `rust/earthmesh_quality/src/lib.rs`：给 `Stat5` 增加 `p95/p99`，或增加 `count_above(threshold)`；
  给 `GeometryMetrics` 增加超阈值计数。**只加字段，不改任何既有算法。**
- `rust/earthmesh_quality/src/io.rs`：序列化新增字段。
- `rust/earthmesh_mesh/src/method_c_nest_spring/mod.rs`：让 `spring_nest_with_radius_projection`
  返回（或经 `earthmesh_core::progress` 上报）可动点数与 shaped 边比例。**不改迭代数学。**
- 新增一个 `#[ignore]` 的真实引擎测试文件 + 一个 `scripts/` 下的运行脚本
  （对齐现有 `scripts/run_slow_fixture_e2e.sh` 的形制），产出 JSON 记录到固定目录。

**范围硬边界：** 不动 `earthmesh_hfield`、不动 `method_c_spawn_*`、不动选择/闭包/修复逻辑、
不新增细化用例特例。

#### 可执行验收标准

1. 一条命令可重复运行 3 个 case，产出 3 份 JSON；
2. 同输入两次运行，JSON 中除耗时/内存外全部字段逐字节相同；
3. 记录中能直接读出：每 pass 可动点占比、shaped 边占比、可动/非可动分组的 edge-CV 与归一化 area-CV、
   niter 四点扫描曲线；
4. 3 个 case 的硬需求覆盖率 = 100%（`MethodCHfieldDemandCoverage::validate` 未报错即为通过）、
   拓扑错误计数 = 0；同时记录 `quality_verdict`、`self_intersection_count`、
   `invalid_polygon_count`、`aspect_ratio_max`，不得把「生成成功」误写成「网格可用」；
5. 冻结点被记录：commit SHA + `run_manifest.json`
   （`earthmesh_core::run_manifest`，`rust/earthmesh_cli/src/cli_runtime/mod.rs:17-46` 已有实现）。

#### 失败后如何解释（而不是继续盲调）

| 失败形态 | 唯一允许的解释路径 |
|---|---|
| 某 case 跑不起来 / 报覆盖错误 | 这就是 M0 的第一个产出：一个**可稳定复现的失败**。记录它，不修它，进入 M2 归因 |
| 三个 case 指标彼此矛盾 | 先定位配置、需求几何与消费路径差异；R4 配对实验已证明量化语义不同，但不得据此单因归因。**不去调 g/niter** |
| 指标全部「看起来还行」但与直觉不符 | 说明阈值未校准（R5）。这时才引入外部参考网格，且只做这一件事 |
| niter 扫描完全平坦 | **不允许**解释为「再多迭代就好了」。平坦只否定继续增加迭代；进入 §5.1 的 A/B/D 对照后再决定是否关闭 M1 |

**明确禁止的后续动作：** 因为某个 case 的数字不好看就调 `hfield_g`、`niter_refine`、`halo`、
`max_transition_row` 或质量阈值。

---

### 5.1 HField 弹簧边界

#### 能解决的问题

- **层内连续尺寸**：compat 目标只有 `2^-k` 台阶，HField 能表达台阶之间的尺寸。这是唯一真正新增的能力。
- **过渡带内的尺寸单调性**：梯度限制场天然给出单调的目标，可以消除「过渡行尺寸非单调」这一类局部缺陷。
- **多判据合成的一致性**：过渡目标与选择所用的是同一个场，消除「选择用 A、平滑用 B」的不一致。

#### 不能解决的问题

- **不能改变任何邻接关系**（`spring_nest_with_edge_targets` 逐字段克隆
  `u_edges/w_faces/m_neighbors/...`，`method_c_nest_spring/mod.rs:241-259`）。
- **不能移动父层节点、nest 内部节点、往期 pass 的节点**（可动集定义，`:310-328`）。
- **不能减少闭包引入的单元**。
- **不能修复非流形、孤立单元、极区空洞**——这些都在拓扑层。

#### 能否降低 edge-CV / area-CV

- **edge-CV**：只在过渡行邻接单元上可能改善，且**上限受 `angle_ratio` 的 `clamp(0.15, 1.2)` 约束**
  （`method_c_nest_spring_iteration/mod.rs:286`）——目标本身被角度项重整过，纯尺寸目标的影响被压缩。
- **归一化 area-CV**：改善空间更小。面积主要由 1→4 细分的层级结构决定，节点微调只影响过渡行面积。
- **实测结论**：现有弹簧能显著消除退化和自相交，但不是单调、统一的收敛器。
  G-CIRCLE 从 500 到 5000 次仍改善（edge-CV max `0.4213 → 0.3682`）但保持 warn；
  G-FRAGMENT 反而退化（`0.4866 → 0.5129`）；R-BBOX 继续改善
  （`0.3420 → 0.3234`）并保持 pass。因而它是必要条件而非充分条件，且固定迭代次数
  不是通解。所有被报告的 worst cells 都在可动邻域内，**M1 的验收仍必须用分组统计**，
  否则无法区分目标函数上限、过迭代与拓扑平台。
- **G-FRAGMENT 是 M1 的首个受控载体**：四个 niter 点的 `cell_count=78212`、
  `refined_cells=14383`、`transition_faces=16842` 均不变；两个 pass 的
  `selected_seed_hash` 分别恒为 `5df090bfa17280a3` /
  `93eaaa5928a07470`，`final_selected_faces` 恒为 `3384/5193`。
  因此这里不是「仅凭 nCells 猜测选择不变」，而是选择掩码、level 与拓扑规模均有记录证明不变。
  M1 的可证伪靶子是消除或显著减弱 `500 → 5000` 的 edge-CV 反转，同时
  G-CIRCLE/R-BBOX 不得退化。

#### 是否可能损害父层边界、极点、接缝、Method-C 不变量

- **父层边界 / 极点 / 接缝：不会直接损害**——这些节点不在可动集内，且
  `spring_nest_with_edge_targets` 结尾强制 `adjusted.validate_topology()?`（`:260`）。
- **但有两处真实风险**：
  1. **mrow 整形丢失**（`method_c_nest_spring_iteration/mod.rs:174` 置 1.0）。
     这是最大的风险。可动集全部取自过渡围裙，因此 shaped 边占比结构性地高于全网格
     （全网格绝大多数边两侧 `mrow` 均为 0），但**不是 100%，实际占比是 §5 必须测出的量**。
     风险的成立不依赖占比大小：该子集在现有 parity 测试中被显式排除（`:106-116`），
     即「非零 + 无测试覆盖」。
  2. **反退化面积下限口径改变**：compat 用 `dist00/2^(max_mrlu-1)`，HField 用「可动边最小目标」
     （`:150-165`）。若 HField 在局部给出比 `dist00/2^(l-1)` 更小的目标，`min_area_squared` 会变小，
     反退化保护变弱。这在碎片化阈值场下值得警惕。
- **确定性**：已有位级确定性测试（`src/tests/method_c_hfield_spring.rs:177-180`），可继承。

#### 应移动哪些节点、保护哪些节点

- **移动**：维持现有可动集不变（`ngr == 当前 grid_number` 且邻接 `mrow != 0`）。
  **不要在 M1 里扩大可动集**——扩大可动集是一个独立的、风险完全不同的改动，必须单独立项和验证。
- **保护**：父层节点、`impent` 十二个五边形点、周期缝的 `m_prognostic != im` 副本点、
  `boundary_rows` 涉及的过渡行骨架、区域网格的域边界节点。
  当前这些**已经由可动集定义自动保护**，无需新增机制。

#### 怎样证明质量平台来自拓扑而不是错误的弹簧目标

一个决定性的对照实验，四个分支跑在**同一个冻结的拓扑**上：

| 分支 | 目标 | 期望 |
|---|---|---|
| A | compat（level + mrow + angle） | 基线 |
| B | HField 采样 × **保留** mrow 乘子 | 若 ≈ A，说明层内连续尺寸无用 |
| C | HField 采样 × 丢弃 mrow 乘子（即当前 `with_edge_target_lengths`） | 若显著劣于 A/B，**直接证明 C2 的判断** |
| D | compat（level + mrow + angle）不变，仅把可动集放开到**当前 generation 的全部 M 点**（`move_interior=true`；一次性诊断，不进生产） | 只隔离「既有可动集是否限制质量」这个变量 |

四臂必须从同一个 G-FRAGMENT `niter=500` 冻结网格起步。先看 A 在追加迭代后能否复现
或接近 `500 → 5000` 的反转；若不能，生产反转来自逐-pass 几何反馈，而不是末端后处理目标，
当前 M1 harness 只能给出边界证据，不能宣称修复根因。B 的成功也不能只看 edge-CV max：
P95/P99、area-CV、aspect、自相交/无效多边形、拓扑、覆盖与确定性必须同时不退化。

判据：若 `edge-CV(A) ≈ edge-CV(B) ≈ edge-CV(D)`，则平台**确实来自拓扑**，M3 的启动条件成立。
若 `edge-CV(D) ≪ edge-CV(A)`，则平台来自**可动集限制**，正确的下一步是重新设计可动集，
而**不是**去做翻边。

原「放开可动集 + 同时去掉目标、只保留反退化下限」会一次改变两个变量，不能把结果归因于
可动集，因此不并入 M1 harness。若 A/B/D 全部触底后仍需估计固定连接关系下的乐观质量界，
再把无目标约束实验作为独立诊断；M1 当前不为它增加代码。

> 这条判据是本评审建议的「M3 启动前必须有的证据」，文档中缺失。

> **[M1 拓扑冻结结果 2026-07-26]** 正式产物：
> `target/mesh-refinement-m1-formal-1785058181/m1_topology_frozen.json`；独立重复：
> `target/mesh-refinement-m1-repeat-1785058627/m1_topology_frozen.json`。
> 两次运行的 base/A/D gridfile SHA-256 与全部质量数值逐位一致，B/C 也以相同错误终止。
> G-FRAGMENT `niter=500` 的冻结基线精确复现 M0：
> `nCells=78212`、edge-CV max/p95/p99=`0.486637/0.225076/0.314055`、
> normalized area-CV=`0.158960`、aspect max=`3.768734`，且自相交、无效多边形、
> 拓扑问题和未覆盖硬需求均为 0。
>
> - A（compat，追加 4500 次/每 generation）保持相同拓扑、覆盖与单元数，
>   edge-CV max 回升到 `0.511703`，接近逐-pass 生产曲线的 `0.5129`；
>   p95=`0.231451`、area-CV=`0.162850`、aspect=`3.836358` 也同步退化。
>   因而反转可在固定拓扑末端复现，根因不需要选择反馈才能出现。
> - B（连续 HField + 保留 mrow）与 C（连续 HField + 去 mrow）在 4500 次运行中均于
>   spring iteration 失败；50 次冒烟的末端转换还分别出现非局部球面外心。
>   这两条当前原语都未通过几何稳定性门，**不得接入生产**；B/C 无法用于证明目标平台。
>   后续精确诊断
>   `target/mesh-refinement-m1-failure-diagnostic-1785060417/m1_topology_frozen.json`
>   定位到：B 在 `ngr=3` 第 `3189/4500` 次、C 在第 `461/4500` 次失败，
>   触发判据均为相邻三角形 Heron 面积平方变为 0。两者的 scratch 均已成功建立，
>   且前一 generation 已完成 4500 次，因此这是迭代中的几何坍缩，不是目标数组或
>   scratch 初始化失败。
>   该次 HField `dmin=26904.83 m`、`min_area_squared=9.824768e16`；
>   同一 NXP=81、generation-3 compat 口径为
>   `dmin=24710.87 m`、`min_area_squared=6.991228e16`。HField 下限反而高
>   `40.5%`，所以「HField 最小目标使面积保护变弱」不是这个 fixture 的根因，
>   也不应再做会降低保护的 compat-floor 替换。
>   定点重放产物
>   `target/mesh-refinement-m1-target-trace-1785061975/m1_topology_frozen.json`
>   进一步记录了失败三角形进入最后 32 次 Jacobi 迭代前的几何。B 的失败三角形三条
>   raw HField target 为同层 nominal 的 `1.47–1.50×`，C 为 `1.35–1.38×`；
>   因而「失败边被 0.5× 的过小 raw target 持续压缩」被该 fixture 否定。B 的失败
>   三角形 mrow 为 `(-5,-4)/(-4,-4)`，实际乘子也均为 `1.0`；B 比 C 晚失败说明
>   保留 mrow 改变了上游 generation 几何演化，但不能归因成失败三角形上的直接乘子保护。
>   最后一轮中，B 的一条边由 `25.38 km` 降到 `0.298 km`，C 的一条边由
>   `10.69 km` 降到 `0.140 km`，Heron 面积平方同时归零；此前 32 点还显示明显振荡，
>   不是向 raw target 的平滑单调收敛。当前最小结论是：一次 Jacobi 的合成节点位移
>   不保证下一状态保持正面积。raw target 仍会被 `angle_ratio` 与面积项重整，因此
>   这份轨迹尚不能证明整个目标场全局可行或不可行；但已经排除了对失败边做 target clamp
>   作为首修。
>
>   扩展求解器轨迹
>   `target/mesh-refinement-m1-solver-trace-1785064266/m1_topology_frozen.json`
>   进一步排除了「raw HField target 本身过小」：B 的失败边 HField base target
>   约 `30.35 km`，同层 compat base target 约 `20.59 km`，compat 要求的收缩反而更强。
>   真正进入求解器的 target 会被 `angle_ratio=0.15` 压到约 `4.6 km`；
>   崩溃前 current/solver-target 已达约 `38–47×`，单个 M 点一次 Jacobi 位移最高
>   `126.5 km`。面积项介入很晚，且在面积趋近 0 时可放大到极端值。轨迹呈振荡而非
>   单调压缩，因此根因是当前显式 Jacobi 力模型缺少保持几何合法性的步接受条件，
>   不是单个 HField bin 的 target clamp 问题。
>
>   同一诊断新增了 Delaunay 侧口径，避免只看 Voronoi/hex 报告：
>   冻结 base 的三角形 `shape_quality_min=0.794708`、`max_edge_ratio=1.860272`、
>   非正面积数为 0；A 分别为 `0.783913/1.898533/0`，D 为
>   `0.794867/1.859877/0`。这证明 A 的退化在 Delaunay 侧同样可测，且 D 没有形成
>   对所有指标的一致改善。
>
>   退化步护栏已按相同规则实际测试，而不是继续停留在建议：
>
>   1. 要求每步不低于当前/面积 floor 的严格单调护栏会在 A/B/C 起始即拒绝，说明
>      「面积单调不降」与现有 compat 力模型不兼容；
>   2. 仅检查输出 W-face 正面积的护栏未覆盖求解器实际使用的
>      `edge_neighbor_u` 三角形，属于错误口径，结果不作为算法证据；
>   3. 修正为与求解器完全一致的三角形模板后，正式产物
>      `target/mesh-refinement-m1-exact-guard-1785066237/m1_topology_frozen.json`
>      显示：A 无需任何回退且逐位复现原结果；C 的失败由 `ngr=3` 第 `461` 次推迟到
>      第 `1820` 次，但仍出现零面积；B 完成弹簧后仍在 Voronoi 转换阶段报
>      `M point 13496 has a non-local spherical circumcenter`。因此最小正面积 backtracking
>      只能延迟或转移失败，不能使 B/C 成为可用网格，也没有产生可评价的 HField 质量结果。
> - D（compat，仅扩大到当前 generation 全部 M 点）完成且保持所有硬约束，
>   edge-CV max=`0.499264`，比 A 好但仍比冻结基线差；p95=`0.239570`
>   反而是四臂中最差，area-CV=`0.157385` 略好。扩大可动集能移动极值，但不能形成
>   全指标通解，也不支持进入生产。
>
> 因此当前 M1 结论是：停止把固定迭代数、未经稳定性保护的直接 HField 目标替换或
> 单纯扩大可动集当作修复。现有逐-pass compat 弹簧仍保留。**生产接入关闭，最小步长
> 回退路线也在当前 fixture 上关闭**：它已被实测为不足，而不是尚未测试。M1 若重启，
> 前置条件应是提出能同时约束 Delaunay 正面积、局部球面外心合法性和全局质量目标的
> 新诊断方案，并继续用同一 A/B/C/D、Delaunay+Voronoi 全指标验收；不得再以调整
> `niter`、target clamp 或回退次数重开路线。

#### [状态更新 2026-07-26] `niter_refine` 的停止语义

现有数据只证明**有限迭代退化**，尚未证明 5000 次已经到达数学不动点；也不能写成
「任何目标、任何可动集都退化」：A/D 只覆盖 compat 目标下的原可动集与当前 generation
扩大可动集，B/C 没有产出可用 HField 网格。准确结论是：**扩大可动集不能消除
G-FRAGMENT 的末端退化，当前 HField 目标与最小正面积回退也不能提供替代解。**

因此 `niter_refine` 在当前力模型中更接近**迭代正则化/停止参数**，而不是「越大越接近正确解」
的收敛参数。正式冻结矩阵的三个案例已经否定统一固定迭代数：

| case | 观测 |
|---|---|
| G-CIRCLE | 5000 次的 edge-CV max 继续改善，但 p95 与 area-CV 持续变差，仍为 warn |
| G-FRAGMENT | 500→5000 的 edge-CV max/p95/p99、area-CV、aspect 全部退化 |
| R-BBOX | 500 次已 pass；5000 次多数指标继续改善，但 area-CV 变差 |

为避免把主观权重或 case 魔数写进算法，已对冻结的 24 条记录做**无权重顺序支配分析**：
`target/mesh-refinement-stopping-analysis-1785067230/analysis.json`
（输入 `measurements.json` sha256
`f113623c70f3cb01295a4a601186f63d17d540f4bfdf25ff859577dc176e4081`）。
规则只做一件事：候选首先必须通过 quality fail、几何、拓扑和覆盖硬门；若当前已接受网格
在 edge-CV max/p95/p99、normalized area-CV、aspect 五项上全部不差，且至少一项严格更好，
则拒绝新候选并回滚；否则保留新候选，不人为裁决指标间的 trade-off。

| case | 0 | 50 | 500 | 5000 | 顺序规则最终保留 |
|---|---|---|---|---|---|
| G-CIRCLE | 不可行 | 接受 | 接受（非支配） | 接受（非支配） | 5000 |
| G-FRAGMENT | 不可行 | 接受 | 接受（非支配） | **被 500 完全支配，拒绝** | 500 |
| R-BBOX | 不可行 | 接受 | 接受（非支配） | 接受（非支配） | 5000 |

这个结果证明「best-so-far + 完全支配回滚」能无权重地拦住已知 G-FRAGMENT 反转，同时不误删
另外两个案例的继续改善；但它只是**最低限度的回归护栏**，不是唯一多目标最优解，也不是
在线收敛证明。冻结点只有 `{0,50,500,5000}`，不能据此拍一个 checkpoint 间隔；生产逐-pass
弹簧还会反馈到下一 pass 的选择和单元数，在线实现必须能原子保存/恢复整张当前 pass 网格，
并证明关闭该功能时输出逐位不变。

为避免在线每个 checkpoint 都重建 Voronoi 对偶，又从冻结 gridfile 离线重建了 Delaunay
代理统计：
`target/mesh-refinement-delaunay-proxy-1785070379/delaunay_proxy_reconstruction.json`
及
`target/mesh-refinement-delaunay-proxy-1785070379/delaunay_proxy_dominance_analysis.json`。
代理只使用最小三角形面积（越大越好）、最大三角形长宽比和三角形 edge-CV max（越小越好）；
结果如下：

| case | Delaunay 代理最终保留 | 五项正式指标最终保留 | 覆盖状态 |
|---|---:|---:|---|
| G-CIRCLE | 5000 | 5000 | 完整 |
| G-FRAGMENT | 500 | 500 | 完整 |
| R-BBOX | 5000 | 5000 | 内部 Delaunay 完整 |

新增的 hex→Delaunay 诊断读取器没有放宽严格三角形产品读取器，也没有改生产 hex 读取器。
它使用权威 W 环的反向引用次数显式分类 M 行：3 个不同 W 且反向度数为 3 才是内部三角形；
1–2 个不同 W 且反向度数一致的是区域边界 dual 顶点；其他组合直接报错。R-BBOX@500 的实测
分类为 2 个占位行、35044 个内部三角形、381 个边界 dual 行、0 个非法行。24/24 冻结记录均
可重建、重复结果一致，三个 case 的顺序支配选择都与五项正式指标一致。

这仍不是生产回滚的充分证据：现有 gridfile 不持久化 Method-C 可动边 mask，所以代理统计的是
全部内部 Delaunay 三角形，不是精确可动边集合；381 个区域边界 dual 顶点也尚无经过验证的便宜
质量代理。更重要的是，三角形视图不能复现 hex 可行性门：G-CIRCLE/G-FRAGMENT@0 的 hex
视图分别有 28/52 个自相交单元，而内部三角形视图均为 0；G-CIRCLE@50 和 R-BBOX@0 的
Delaunay 最大角仍为 91.57°/97.27°，hex 自相交却已经为 0。因此「存在钝角」不能替代
hex 自相交判据，最大角/钝角计数只能作为诊断量，不能作为生产硬门。

因此当时决定**不实现末代 checkpoint/rollback，也不改生产弹簧**，先用外部 MPAS 参考网格
校准现有 warn 阈值。下方状态更新记录了这项校准的结果：现有 absolute-max 门不具备普适性，
回滚的产品价值反而下降，故精确可动边、区域边界代理与在线 checkpoint 实验继续不启动。
不得直接把离线规则接入生产，也不得把它包装成新的“自动最优 niter”。

#### [状态更新 2026-07-26] 外部 MPAS 质量门校准

已从 [MPAS-Atmosphere 官方网格目录](https://mpas-dev.github.io/atmosphere/atmosphere_meshes.html)
下载并以同一 `--mesh-quality --kind hex` 路径测量六张网格。完整来源、sha256、转换后 gridfile
和质量 JSON 见
`target/mpas-reference-calibration-2026-07-26/calibration_summary.json`。

| 网格 | 单元数 | edge-CV max / p95 / p99 / >0.35 | aspect max / p95 / p99 / >4 / >10 | 当前 verdict |
|---|---:|---|---|---|
| 官方 x1.40962，120 km 准均匀 | 40962 | 0.194 / 0.151 / 0.160 / 0 | 1.656 / 1.465 / 1.507 / 0 / 0 | pass |
| 官方 x1.163842，60 km 准均匀 | 163842 | 0.194 / 0.151 / 0.160 / 0 | 1.656 / 1.455 / 1.489 / 0 / 0 | pass |
| 官方 x4.163842，92–25 km 变分辨率 | 163842 | 0.552 / 0.249 / 0.316 / 636 (0.388%) | 20.761 / 1.898 / 2.327 / 41 (0.025%) / 6 (0.0037%) | **fail** |
| 官方 x4.655362，46–12 km 变分辨率 | 655362 | 0.423 / 0.231 / 0.301 / 572 (0.087%) | 4.999 / 1.784 / 2.200 / 40 (0.0061%) / 0 | warn |
| 官方 x4.535554，60–15 km 变分辨率 | 535554 | 0.465 / 0.195 / 0.253 / 206 (0.038%) | 5.651 / 1.637 / 1.941 / 27 (0.0050%) / 0 | warn |
| 官方 x6.999426，60–10 km 变分辨率 | 999426 | 0.420 / 0.202 / 0.239 / 91 (0.0091%) | 5.350 / 1.651 / 1.830 / 29 (0.0029%) / 0 | warn |
| G-CIRCLE@5000 | 115982 | 0.368 / 0.189 / 0.273 / 14 (0.012%) | 3.345 / 1.655 / 2.250 / 0 / 0 | warn |
| G-FRAGMENT@500 | 78212 | 0.487 / 0.225 / 0.314 / 261 (0.334%) | 3.769 / 1.900 / 2.516 / 0 / 0 | warn |
| G-FRAGMENT@5000 | 78212 | 0.513 / 0.231 / 0.315 / 197 (0.252%) | 3.849 / 1.934 / 2.502 / 0 / 0 | warn |
| R-BBOX@5000 | 17563 | 0.323 / 0.221 / 0.266 / 0 | 3.026 / 1.923 / 2.307 / 0 / 0 | pass |

x4.163842 的 fail **只来自**默认 `aspect_ratio_fail=10`；另外三张变分辨率网格为 warn。
四张变分辨率网格的 edge-CV p99 均低于 0.35，aspect p99 均低于 2.33，且超阈值单元占比
始终很小。这不是放宽阈值的充分理由，却是对“当前单点最大值门具有普适物理含义”的重复反例。
两张准均匀 x1 跨 120/60 km 的结果几乎重合并全部通过，说明读入、对偶环和质量计算链路
没有随分辨率系统性劣化。

六张网格把结论从单个反例推进为跨分辨率与 4×/6× 细化倍率的稳定观察，但仍不能直接形成
`hex_variable_resolution_v1` 正式门：当前矩阵全部是官方可接受的全球正样本，没有区域产品、
明确不可接受的负样本或模拟稳定性/误差标签。现阶段只冻结
`candidate_distribution_envelope.status=diagnostic_only`，不从样本最大值反推生产阈值。

分布口径同时修正了“EarthMesh 在全部指标上优于官方 x4”的过强结论：G-FRAGMENT 的
aspect p99（2.516/2.502）高于 x4（2.327），只是极值显著更低；它与 x4 应解读为处于相近
质量包络，而非证明数值模拟质量全面更好。新增字段还使 G-FRAGMENT@500→5000 从原五指标下的
“严格退化”变成 trade-off：5000 的 max/p95 较差，但 edge-CV 超阈值数量由 261 降到 197，
aspect p99 由 2.516 略降到 2.502。因此原离线完全支配回滚不再具有稳定的指标集合依据，
checkpoint/rollback 继续关闭。

校准过程中还修复了一个独立的共享导入缺陷：MPAS/FVCOM 转换器插入紧凑 sentinel 行后，
仍把源文件第一个实体映射到 canonical id 1，与 sentinel 冲突。现在 canonical id 1 只保留
给 sentinel，实体从 2 开始；标准一基与历史零基输入复用同一转换边界。该修复不改变生产
Method-C 网格算法。

因此 checkpoint/rollback 的产品化判定仍为**关闭**，且阻塞理由从“尚未校准”升级为
“现有可行性门已被官方变分辨率网格证伪”。aspect 与 edge-CV 的分位数/超阈值计数现已补齐，
但六个全球正样本仍不足以修改 verdict；不得直接把 `aspect_ratio_fail` 调到 21。下一步只补
可接受的区域 hex 产品、明确拒绝或模拟不稳定的对照，并继续寻找 tri 产品参考集，再按
`cell_view` 与产品契约校准分布门。

#### [状态更新 2026-07-29] NOAA NGOFS2/FVCOM tri 正样本

已用 NOAA/NCEP NOMADS 发布的 NGOFS2 运行产品
`ngofs2.t03z.20260727.2ds.f000.nc` 作为首个外部 tri/FVCOM 正样本，并以同一
`--mesh-quality --kind tri` 路径测量。来源、源文件/拓扑子集/转换后 gridfile sha256
及摘要见 `target/fvcom-reference-calibration-2026-07-29/calibration-summary.json`。

| 单元数 | edge-CV max / p95 / p99 / >0.35 | aspect max / p95 / p99 / >4 / >10 | min angle | eta / NSR min | 拓扑与有效性 | 当前 verdict |
|---:|---|---|---:|---|---|---|
| 569405 tri | 0.253 / 0.137 / 0.173 / 0 | 1.878 / 1.383 / 1.515 / 0 / 0 | 31.619° | 0.710 / 0.613 | χ=-187 与 189 个边界环一致；1 component；负面积、自相交、非法多边形均为 0 | warn |

初次导入时全部三角形被报为负面积；根因是 FVCOM 文件使用顺时针连接，而 EarthMesh
质量/输出契约使用逆时针。修复位于共享 FVCOM 转换边界
`rust/earthmesh_cli/src/mode_file_io/fvcom.rs`：写入 EarthMesh gridfile 前按球面有向面积
规范化为逆时针；没有放宽质量门，也没有修改网格生成算法。修复后除有向面积符号外，
edge-CV、aspect、角度和拓扑统计保持不变。

因此 `gate_calibration` 现在对 tri 标为 `provisional`，参考集为
`noaa_ngofs2_fvcom_tri_2026-07-29_v1`，不再错误声明没有外部参考；但一个运行产品正样本
仍不足以形成 triangular-primal 通用门。缺口仍是明确的负样本、跨日期/版本稳定性和
FVCOM 之外的三角形产品覆盖，现有阈值保持不变。

流程教训：外部参考校准应与 M0 并行，并在任何“为了过 WARN 而改算法”的工作之前完成。
本次 M1 诊断仍证明了 HField 直接替换不安全，但若先完成校准，追逐 0.35 极值门的优化优先级
会在进入 B/C/D 实验前就被正确下调。

---

## 6. 分阶段验收门

### 6.1 顺序调整建议

**原顺序：** M0 → M1(弹簧) → M2(归因+最小化) → M3(翻边) → M4(JIGSAW)
**建议顺序：** M0 → **M2a(归因，行为不变)** → M1(弹簧，修正定义) → **M2b(最小化，可选)** → M3 → M4(归档)

> **[状态更新 2026-07-26]** M0 已冻结；正式矩阵的 48 条 pass 记录全部
> `unexplained_selected_faces=0` 且 `seed_reconstruction_matches=true`，当前三来源 M2a
> 归因闭合。production-cap 对照中 connectivity-bridge-only 低于 3%，不支持启动
> connectivity-bridge 最小化型 M2b。M1 末端拓扑冻结 A/B/C/D 已完成：A 复现反转，
> B/C 未通过几何稳定性门，最小正面积 backtracking 只能延迟/转移失败，D 不能全指标改善。
> 离线顺序支配规则能拒绝 G-FRAGMENT@5000 而保留另外两个案例@5000；Delaunay 代理在
> 三个 case 上均复现该选择，R-BBOX 的内部三角形与边界 dual 行也已显式分类；但 gridfile
> 不含精确可动边 mask，区域边界 dual 也没有质量代理。因此在线 checkpoint/rollback 的
> 跨 case 前置证据仍不完整。外部校准又证明官方 x4 变分辨率网格会被当前
> `aspect_ratio_max` 单点极值门误判为 fail，因此**先修质量门契约，再讨论回滚实现**；
> 暂不投入精确可动边和区域边界代理。
> 不改生产弹簧，也不把固定 5000 次写成生产默认值；当前在 M1 门停止，而不是继续堆叠启发式。

三点调整理由：

1. **M1 与 M2 对调**：M2a 的归因是行为不变的观测改动、回归风险较低；M1 当前定义存在未验证风险（C2），
   且其验收条件「总单元数与接入前一致」需要先有 M2 的归因才能确认「一致」不是两类误差抵消。
2. **M2 拆成 M2a（归因）与 M2b（最小化）**：文档把「打原因位」和「删单元」绑在一个阶段。
   前者零风险、信息增益高；后者要动闭包逻辑、风险高。绑在一起会让 M2 无法独立验收。
3. **不应实施的阶段：M4（JIGSAW）** 在可预见范围内不应实施。文档自己已列出足够的反对理由
   （非 OSI 许可、确定性未经实证、cell-id 稳定性），但仍把它留在路线图里会持续消耗决策注意力。
   建议把 M4 从阶段路线图降级为**已归档的备选记录**，只在 M0–M3 全部完成且明确失败后重新开启。

### 6.2 各阶段最小充分验收标准

| 阶段 | 最小充分验收 | 停止/回退条件 |
|---|---|---|
| **M0** 冻结 + 度量探针 | **已通过**：见 §5；24/24 成功、24/24 重复确定、3/3 诊断 parity、拓扑与覆盖失败均为 0。外部 MPAS 参考网格校准已作为独立任务完成，不改写 M0 冻结基线 | 任一 case 无法确定性复现 → 停在 M0，先修复不确定性来源 |
| **M2a** 归因（行为不变） | **当前三来源已通过**：48/48 pass 的未解释残差为 0，选择掩码可由 seed 足迹重建；报告已有独占计数与两两重叠矩阵。归因前后 nCells、选择掩码与拓扑逐位不变 | 存在无法定位来源的残差 → 说明还有未识别的扩张路径，**不进入 M2b，回到代码走查** |
| **M1** HField 弹簧 | **生产接入未通过，最小护栏路线已测完**：冻结 topology/nCells/level/覆盖不变；A 已复现反转，D 未形成全指标改善；B/C 已定位为 HField Jacobi 的零面积/非局部球面外心失效。精确三角形正面积 backtracking 仅把 C 的失败从 461 推迟到 1820 次，并把 B 的失败移到 Voronoi 转换，未产出可用 HField 网格。离线完全支配回滚能识别已知 G-FRAGMENT@5000 反转；内部 Delaunay 代理在三个 case 上均复现选择，但精确可动边和区域边界质量口径尚未覆盖。外部校准进一步证明现有 absolute-max 可行性门不适合直接控制变分辨率 MPAS 回滚。现有逐-pass compat 弹簧保持不变 | **停止生产接入与继续调参**：不再调整 niter、target clamp、回退次数，也暂不实现 checkpoint。先建立分布感知、分视图/产品的质量门契约；只有新门经外部与内部案例共同校准后仍显示回滚有产品价值，才补精确可动边、区域边界代理和诊断态 checkpoint |
| **M2b** 拓扑最小化（可选） | 仅当 M2a 显示某一类**可安全删除**原因占 `actual_above_target` 的显著比例才启动；删除后硬需求覆盖 100%、拓扑合法、质量不退化；每次删除可回滚 | 为减单元破坏任一硬约束 → 回退；改善幅度低于 M2a 基线分布确定的门槛 → 不值得，关闭 |
| **M3** 受限翻边 | **从当前路线图归档关闭**：现有有效网格处于官方 x4 的质量包络，不再为追逐未校准极值门证明固定拓扑下限 | 只有模拟稳定性/误差或重新校准后的产品硬门给出当前网格不可接受的证据，才重新执行 §6.5 三项技术启动检查 |
| **M4** 外部后端 | **不作为阶段实施** | — |

> **关于数值门槛：** 本表中一切百分比门槛（解释率、改善幅度）在 M0/M2a 产出基线分布之前
> **均为建议值，不是正式验收条件**。首轮外部校准已经否定 absolute-max 门的普适性，但尚未
> 形成分视图/产品的正式分布门；不得把 x4 的单个最大值反过来当作新阈值。正确顺序仍是：
> 内部基线分布 + 外部参考分布共同定门，再写入契约。

### 6.3 应该暂停而不是继续增加复杂度的阶段

**M2a 完成后必须暂停并重新决策。**

理由：M2a 会第一次回答「额外单元到底从哪来」。三种可能结局：

- 若归因显示 `actual_above_target` 主要由**全部被选 seed 的原子足迹** + `parent_closure` 构成
  → 这是**共形离散闭包的合法代价**，正确动作是**把它写进文档与验收契约，停止优化**，
  把预算转向阈值校准与案例矩阵扩充。
- 若主要由 `boundary_backtrack`（repair 的 fill/grow 路径）构成
  → 问题在修复策略的扩张偏置，M2b 值得做，M1/M3 都不相关。
- 若主要由 `phase_halo`（6 环扩张，`method_c_spawn_hfield/mod.rs:465-485`）构成
  → 先验证六环是结构需要还是保守余量；只有缩减后覆盖、相位和拓扑仍全部合法，
  才能把它列为局部可优化项，不能仅凭占比直接调小。

三种结局中有两种意味着**不应继续往下走**。这就是暂停点的价值。

---

### 6.4 `actual_above_target` 原因归因模型

#### 通用归因模型

原因位是**可组合的位集**（一个单元允许携带多个原因），在**选择/闭包阶段**记录，落在具体代码位置：

| 原因位 | 语义 | 记录位置（现有代码） | 性质 |
|---|---|---|---|
| `hard_demand` | 该单元本身被硬需求直接要求 | `method_c_spawn_hfield/mod.rs:407-459`（`demand_at_m` / `point_demand_at_m` / `anchors`） | **合法**，非「额外」 |
| `demand_tail` | anchor 兜底追加的最近 owner 足迹 | `method_c_spawn_hfield/mod.rs:822-868` | **合法需求表达**（顶点采样漏掉边细尾），不是删除候选；高占比提示采样/量化需要分析 |
| `initial_seed_footprint` | 属于初始需求驱动 seed 的 rad3 足迹 | `method_c_spawn_hfield/mod.rs:994-1006`（初始 seed 原因位） | **来源标签**，不是「全部被选 seed 原子足迹」的同义词；可与 tail/bridge 重叠 |
| `parent_closure` | 因父层 `mrlw == component_mrl` 约束或 pass 逐级下降而被抬高 | `method_c_spawn_hfield/mod.rs:557-560,690-698` + `method_c_spawn_pass/mod.rs:98`（`ensure_..._share_parent_mrlw`） | **合法**（2:1 嵌套的必然代价） |
| `phase_halo` | 由 6 环 stride-3 相位支撑扩张带入 | `method_c_spawn_hfield/mod.rs:465-485`（`phase_support` 6 轮扩张） | **待判定**：可能是结构需要，也可能包含保守余量；必须通过覆盖/相位/拓扑对照验证 |
| `connectivity_bridge` | 由 bridges 循环补入，用于连通两个已选 seed | `method_c_spawn_hfield/mod.rs:869-909` | **候选算法信号**：占比高提示需求碎片化、量化或 halo 可能有问题，但不能单凭计数归因 |
| `boundary_backtrack` | 由凹角闭合或周界修复的 fill/grow 追加 | `method_c_spawn_pass/mod.rs:94-97`（`close_method_c_concavities`）+ `:145-186`（fill-specific-M / fill-boundary / grow） | **算法信号**：占比高 = 选择器产出的掩码周界质量差，应回头修选择器而不是修修复器 |

#### 一个单元是否允许多个原因

**必须允许，且必须记录重叠矩阵。** 当前实现对
`initial_seed_footprint` / `demand_tail` / `connectivity_bridge` 输出 8 种 mask 组合、
各原因独占计数和两两重叠计数；
若强制单一原因（「先命中者赢」），归因结果会完全取决于代码执行顺序而失去意义。
报告应同时输出：每个原因位的**总计数**、每个原因位的**独占计数**（该单元唯一原因）、两两重叠矩阵。
**独占计数才是可优化空间的上界。**

`final_selected_faces` 与 `seed_reconstruction_matches=true` 已证明最终选择等于**全部被选 seed**
rad3 足迹的并集，因此不再增加一个数值恒等的「全部足迹」字段。

#### 怎样在硬需求覆盖、拓扑合法、质量不退化条件下最小化

优化目标（沿用文档表述，但补上归因约束）：

> 在硬需求覆盖 = 100%、拓扑合法、相邻层级跳变合法、质量不退化的前提下，
> 最小化 **`boundary_backtrack` 独占计数**，其次最小化 `phase_halo` 与 `connectivity_bridge` 独占计数。
> **不以 `nCells` 或 `actual_above_target_count` 本身为优化目标。**

操作顺序：

1. 删除候选**只从 `boundary_backtrack` / `phase_halo` / `connectivity_bridge` 独占的最外层单元**开始；
2. 每次删除后重算完整闭包与 `validate_topology()` + `MethodCHfieldDemandCoverage::validate`；
3. 任何覆盖丢失、非法周界、断连、质量退化 → 回滚该次删除；
4. `initial_seed_footprint` / `parent_closure` / `hard_demand` / `demand_tail`
   独占的单元**永不作为直接删除候选**；tail 只能通过改进采样或量化来减少。

#### 为什么 `actual_above_target = 0` 不能作为普适目标

三条独立理由，每条都足够：

1. **rad3 足迹是原子的**：一个 seed 的足迹要么整体细化要么整体不细化
   （`method_c_spawn_hfield/mod.rs:690-704` 的 `footprint_is_legal` 是全或无判定）。
   足迹内必然包含无需求单元。
2. **父层闭包是 2:1 嵌套的定义**：level-2 区域必须坐落在 level-1 父层之上，
   父层的存在本身就是「实际 > 目标」。
3. **目标口径本身偏松**：`topology_level_at` 用 floor + max-over-stencil
   （`earthmesh_hfield/src/lib.rs:704,745-754`），已经把目标向上抬。
   要求「实际 = 这个已抬高的目标」在数学上不自洽。

合理的验收形式是**分原因的上界**，例如 `boundary_backtrack 独占 / nCells < X%`，而不是总量为 0。

#### 哪些原因表明算法有问题

| 原因 | 高占比时的含义 |
|---|---|
| `boundary_backtrack` 独占占比高 | **算法问题**：选择器产出的掩码周界不满足 Method-C 三元组约束，修复器在事后扩张补救。应修选择器 |
| `connectivity_bridge` 独占占比高 | **候选算法问题**：需求场碎片化后 seed 连通性差；需要进一步区分量化、需求几何和 halo |
| `phase_halo` 独占占比高 | **待验证信号**：需要通过缩减对照判断六环是结构需要还是保守余量 |
| 全部被选 seed 原子足迹 / `parent_closure` 占比高 | **合法离散闭包成本**，不是问题。此时应停止优化并更新验收契约 |
| `hard_demand` / `demand_tail` | 定义上不属于「额外」，应从 `actual_above_target` 分母之外单独列出 |

---

### 6.5 受限翻边

#### 判断：**从当前路线图归档关闭；不是已经证明拓扑质量下界，而是当前没有产品证据要求突破它。**

文档 M3 的启动条件「M1 后仍有稳定的平台案例」是必要但**不充分**的——
因为平台也可能来自可动集限制（见 §5.1 对照实验分支 D），而不是拓扑。

#### 需要哪些证据才进入该阶段（三条，缺一不可）

1. **可动集不是瓶颈**：§5.1 的分支 D 实验显示，即使放开当前 generation 的全部 M 点，
   edge-CV 仍停在同一平台（`edge-CV(D) ≈ edge-CV(A)`）。
2. **目标不是瓶颈**：分支 B（HField × 保留 mrow）与分支 A 的平台一致。
3. **坏单元的空间结构支持翻边**：M0 的 `worst_cells` + `refine_level` 分布显示坏单元
   **集中在同层内部 patch**，而非父边界、接缝、极点或 mrow 关键边上。
   若坏单元恰好都在不可翻边的位置，翻边在定义上无效。

三条都成立，M3 才有意义。正式 M0 目前给出的是：

- R-BBOX 在 500 次已 pass，5000 次仅继续小幅改善，没有需要翻边修复的失败；
- G-CIRCLE 到 5000 次仍在改善，尚未证明固定拓扑平台；
- G-FRAGMENT 的选择掩码与拓扑规模在四个 niter 点严格不变，但质量在 5000 次反转，
  且分支 A 已在末端冻结拓扑上复现反转；D 仅改善 max、同时恶化 p95，B/C 则未通过
  几何稳定性门，不能把当前结果解释为拓扑下限；
- M0 只记录 worst cells 是否处于可动邻域，尚未完成第 3 条要求的同层内部 patch 空间归属。

因此 M3 不再作为 M1 后的默认下一阶段。现有网格已经落在官方 x4 参考质量包络内，
而 B 无法稳定完成、坏单元空间归属也未满足技术启动条件。只有模拟稳定性/误差或重新校准后的
产品硬门证明当前网格不可接受时，才重新执行上述三条检查；单独出现 edge-CV/aspect 极值
WARN 不得重启 M3。

#### 若确需实施，边界如下

**启动条件：** 逐 case 判定，默认关闭；只在 M0 记录中被标记为「已证明触底」的 case 上启用。

**可翻边的边：** 同层（`u_edges[iu].mrlu` 两侧一致）、内部、非周期缝（`u_prognostic[iu] == iu`）、
非父边界、两侧面 `mrow == 0`、两端 M 点均不在 `impent`、两端 M 点 `ngr` 一致。

**禁止翻边的边：** 任何 `mrow != 0` 的过渡行边、跨 `mrlu` 边、周期缝/极区连接边、
`boundary_rows` 涉及的边、任一端为五边形点的边、
翻转后会破坏覆盖锚点（`MethodCHfieldDemandCoverage::anchors`）的边。

**必须原子更新的数据**（以 `method_c_nest_spring/mod.rs:117-135` 与 `:241-259` 中
弹簧一次调用就必须逐字段搬运的结构为清单）：
`u_edges`（`im`/`iw`/`mrlu`）、`w_faces`（`im`/`iu`/`iw[0..9]`/`mrlw`/`mrow`/`ngr`）、
`m_neighbors`（`iu`/`iw`/`npoly`）、`m_metadata`（`mrlm`/`mrlm_orig`/`ngr`）、
`m_lineage`/`w_lineage`/`next_m_lineage`/`next_w_lineage`、
`m_prognostic`/`u_prognostic`/`w_prognostic`、`boundary_rows`。

**回滚与确定性要求：** 每次候选操作前保存最小回滚集；操作后立即 `validate_topology()` +
`coverage.validate()`；任一失败完整回滚。候选排序必须是稳定全序（最坏质量优先，
同分按 Canonical id 升序），不得依赖 HashMap 迭代顺序或浮点比较的 NaN 行为。
最大工作量上限 + 无改善即退出。

**为什么 `refine_delaunay_lop_one_based` 不能直接进入生产路径**（三条，均可验证）：

1. **数据模型不同**：它操作的是 `cells_on_triangle: &mut [[usize;3]]` 与 `LonLatDegrees` 数组
   （`refine_lop/mod.rs:17-19`），与 `MethodCDelaunayMesh` 完全无关；上述十余个派生结构一个都不更新。
2. **更新方式不可用于生产**：旧面被置为占位 `[1,1,1]`、新面追加到 `num_mp[iter-1]` 之后
   （`refine_lop/mod.rs:75-80`）。这个「追加新面、清空旧面」模式会破坏 `w_faces` 的 Canonical
   索引连续性，而 Method-C 的所有互反表都按索引寻址。
3. **几何判定不是球面判定**：`checked_lop_edge_flip` 在 lonlat 平面上取质心，
   换日线靠「经度跨度 > 180」启发式（`refine_edge_flip/mod.rs:64-72`），
   且**根本没有 in-circle 判定**——它只做拓扑翻转与质心计算，Delaunay 判据在调用方。
   极区附近这套判定不可靠。

**结论：** 真正的工作量不在几何判据，而在写一个新的、小而完整的 Method-C 派生表原子更新器 + 回滚点。
这是一个独立的中等规模工程，**必须在有上述三条启动证据之后才立项**。

---

## 7. 通用案例矩阵

12 例，pairwise 覆盖（不做笛卡尔积）。每例标注必须命中的维度组合。

| # | 案例 | 覆盖维度 | 关键设置 |
|---|---|---|---|
| 1 | 全球大气六边形 + 中纬圆形细化 | 全球 / 六边形 / circle / level 2 | AtmosphereMpas，NXP≥81 |
| 2 | 全球陆面六边形 + 碎片化阈值场 | 全球 / 六边形 / 碎片化阈值 / level 1–2 | 复用 `rust/earthmesh_cli/tests/refine_pipeline.rs:124-140` 的棋盘式 LAI 构造，放大到真实分辨率 |
| 3 | 全球 + 跨日期变更线 bbox | 换日线 / bbox / 六边形 | **[2026-07-28 已复验]** NXP=81、`w=170/e=-170/s=-20/n=20`、两层、5000 次 spring：90107 cells，22447 个 level≥2 cells；`lon>170` / `lon<-170` 分别 10170 / 10289，`χ=2`、零边界/拓扑/几何错误、`target_above_actual=0` |
| 4 | 全球 + 北极圆形细化 | 北极 / circle / 六边形 | **[2026-07-28 已复验]** NXP=81、中心 `(30°,89°)`、半径 600 km、两层、5000 次 spring：70337 cells，3475 个 level≥2 cells，细化纬度 `82.212°–89.999997°`；南北极均存在、`χ=2`、零边界/拓扑/几何错误、`target_above_actual=0` |
| 5 | 全球 + 南极 polygon 细化 | 南极 / polygon / 六边形 | **[2026-07-28 审计结论：当前契约不支持]** Project 可通过 `specified_close` 文件进入 polygon 路径，但 `HRegion::Polygon` 明确保留平面 lon/lat ray casting，并在 `earthmesh_hfield/src/lib.rs:160-163` 写明“不得用于 polar caps”。因此此例不能作为应通过的生产回归；在引入球面 polygon contains 语义前，南极极冠应使用已通过的 great-circle `specified_circle`，不得用 polygon 假装覆盖 |
| 6 | 区域六边形 + bbox | 区域 / 六边形 / bbox / 域边界 | **[2026-07-29 已通过组合证据]** 区域 Atmosphere 的 bbox domain + specified refine 真实 Method-C 测试通过；独立 Land 的真实 15″ IGBP + 区域 Project 也通过质量、拓扑和 hard coverage 门。Project capability matrix 另锁定区域 bbox + bbox refine lowering；尚未把三项合并成单个可分发 NXP81 fixture，故不声明字节级耐久回归 |
| 7 | 区域六边形 + 闭合流域（含内部孔洞） | 区域 / 流域 / 内部孔洞 / close | **[2026-07-29 已通过]** `test/fixtures/watershed_with_hole.shp` 提供外壳 + 内孔，`basin_refinement.shp` 提供域内 specified-close；`scripts/run_basin_hole_regression.sh` 跑 NXP=81、两层、5000 次 global/逐 pass spring。结果 211 Hex cells、实际 max-level=2、24 个 specified hard bins、`target_above_actual=0`、`χ=0=expected`、单连通、2 个边界环、零负面积/自交/非法 polygon/错向共享边/orphan，质量 `pass`；`static-netcdf` 两次独立输出 hash 均为 `44c67c05c3463ab186ec665c9395d626954b956b157e83fa76b11bb467bbd807` |
| 8 | 区域海洋三角形 + CoastalOcean 精确海陆 mask + 小分量清理 | 区域 / 三角形 / mask / 孤立分量 | 触发 `remove_isolated_ocean_one_based`（`rust/earthmesh_cli/src/mask_postproc_ocean/renewal.rs:173`）与 `cleanup_masked_topology_one_based`（`:203`）。**⚠ 见下方 tri/hex 口径注** |
| 9 | 全球海洋三角形 + landcover mask | 全球 / 三角形 / mask | `MaskedOceanTri` 契约。`case9_projected_hfield_20260728` 已生成 `210048` 个三角形，`116/116` active hard bins 全覆盖、`target_above_actual=0`，拓扑和几何合法；P2 在同一网格上为 `passes=0`、`changed=false`。`case9_native_15arcsec_20260729` 禁止 coarse projection，当前仍在 pass 2 `TransitionPatch` 失败，不能声明原生路径已闭环。旧 `107/114 → 114/114`、`+641` cells 的 P2 A/B 仅保留为历史构造能力证据。**⚠ 见下方 tri/hex 口径注** |
| 10 | LandOceanCoupled 区域 | 耦合 / 双产品支撑掩码 | **[2026-07-28 已通过]** 真实区域 landcover 阈值 Project 触发 `coupled_hfield_product_support` 与 land/ocean/combined 三产品：143 Hex cells，80 land / 63 ocean / 33 mixed coastline，`χ=1`、单连通、零 orphan/自交/非法 polygon/错向共享边，hard coverage 与 target/actual 均闭合；coupling `pass`、unresolved fraction 与 mass residual 均为 0。combined hash `83896c29388a14956e1d19deb3541a03ff035269348bbd814ad176e9bbc58ae8` |
| 11 | MeritHydro corridor（窄走廊） | hydro / corridor / 走廊窄于栅格 | **[2026-07-29 适配器压力已通过，完整 Project 部分通过]** `corridor_hfield_uses_positive_swept_area_across_seams_and_poles` 用 20 km corridor 对 10° HField bin，`slender_polygon_crosses_only_the_bins_with_positive_area` 用 0.2° hydro polygon 对 10° bin，均证明无中心命中时仍按精确正面积激活；球面生产路径同时报告 `spacing/h > 1`，不再静默。真实 MERIT/CaMa + NXP42 Project 也已闭环，但尚未把 sub-bin 走廊与真实外部数据完整 Project 合并为同一条 E2E |
| 12 | Cartesian 区域保护案例 | Cartesian / 周期缝 / 保护回归 | **[2026-07-29 已通过生产测试与配对量化]** `native_cartesian_xy` 路径的 mDomain=5、显式 XY 米制 HField、地理 threshold + origin 三条 release 测试均通过。同一 circle 的配对单测证明硬需求中心同为 level 2，而 600 km 过渡点为球面 floor level 1、Cartesian ceil level 2；尚未运行两套完整 Project 的成对网格对比 |

案例 3、4 的耐久脚本同时锁定 release CLI、完整质量门、细化位置与 gridfile sha256；
最新清单位于
`target/refinement-boundary-regression-latest/boundary-regression.json`。
极区 hash 为
`9eb298241a6820f3941f5c143cb5f129cbc25aea6e27131afad793d78a27db8f`，
日期变更线 hash 为
`8d766758de6c953d96d0c32745a9a05620cdfc5bed0779be6dbdba0479f1b979`；
均在共享 Hex 输出定向接入后的两次独立 `static-netcdf` 运行中一致。旧 hash 只对应
接入前的任意环方向或不同 NetCDF 链接方式，不再作为当前输出契约。

### 每类必须检查的指标

| 指标类 | 具体项 | 判据 |
|---|---|---|
| **拓扑** | orphan、non-manifold vertex fan、邻接非互反、duplicate/dangling edge、misoriented shared edge、Euler χ（全球 = 2） | 全部 = 0；χ 与域类型匹配（`rust/earthmesh_quality/src/lib.rs:200-235` 已有全部字段） |
| **覆盖** | 硬需求覆盖率；`target_above_actual_count` | 覆盖 = 100%；`target_above_actual_count = 0`（当前已是 Fail 门，`rust/earthmesh_quality/src/lib.rs:1495-1504`） |
| **质量** | edge-CV（max / P95 / 超阈值计数，**分可动集内外**）；level-normalized area-CV；min_angle；`max_adjacent_resolution_ratio` | 与冻结基线比较，不退化；绝对值先只记录不设门（等外部校准） |
| **单元数** | `nCells`；`actual_above_target_count / nCells`；**M2a 之后：各原因位的独占计数** | 与基线比较；未解释残差可逐例定位（数值门槛待基线分布确定） |
| **性能** | wall-time、峰值内存、每 pass 耗时 | 首轮只记录分布；正式门槛在基线形成后确定 |
| **确定性** | 同输入重复 2 次的关键数组哈希；`run_manifest.json` | 逐字节一致 |
| **额外（按案例类型）** | 换日线例：跨 ±180° 单元的经度不出现 360° 跳变<br>极区例：极点唯一、无极区空洞<br>mask 例：清理后连通分量数与保留策略一致<br>hydro 例：走廊需求 100% 落在细化单元内<br>Cartesian 例：与球面例对同一 h 场的层级分布差异被显式记录 | — |

> **⚠ tri/hex 口径注（新增 2026-07-26，见 C10）：** 案例 8、9 是三角形视图，其余为六边形视图。
> 按当前实现，两者被同一套 `QualityThresholds` 衡量（`cell_view` 不参与判定），
> 而 `min_angle_warn_deg = 25.0` 对 hex 近乎死门、对 tri 有效，且 tri 单元数约为 hex 的 2 倍、
> 面积约 1/2。因此 **tri 案例的 `nCells`、`normalized_area_cv`、`actual_above_target/nCells`、
> `min_angle` 不可与 hex 案例并列解读**。M0 记录中每条 run 必须标注 `cell_view`，
> 跨视图指标标记为不可比。阈值本身的分视图校准应与 R5（外部参考网格校准）一并进行，
> **不要在此刻拍数字**——那正是 §8 第 4 条所反对的做法。

**规模要求：** 每个案例采用足以形成稳定质量统计的固定规模；球面 Method-C 基准建议 NXP=81，
其他后端按等效单元规模和可接受运行时间确定。NXP=6 的现有测试保留为快速冒烟，**不作为质量证据**。

---

## 8. 应立即停止的做法

1. **停止在这个工作树上继续叠加算法改动。** 已核实：85 条状态记录、78 个文件、`+11165/−1474`，
   混合了 HField、Method-C、质量、mask、hydro、Studio、GUI 和测试。
   在冻结基线之前的任何算法改动都无法归因。
2. **停止把 `spring_nest_with_edge_targets` 当成可直接接入的成品。**
   它会丢弃 mrow 整形，而受影响的 shaped 边子集非零且恰是现有 parity 测试未覆盖的部分。
3. **停止把 `method_c_hfield_spring` 的 3 项通过当作 M1 的正面证据。**
   那三项证明的是与 compat 的等价性和确定性，不是质量改善。
4. **停止用 `actual_above_target_count` 的绝对值判断算法好坏。**
   口径本身偏松（max-over-stencil），且其中包含不可消除的合法闭包成本。
5. **停止让球面与 Cartesian 在未声明语义契约的情况下各自量化。** 这里要停的是「语义未定义」，
   不是「实现不同」——两个后端可以、也应该保留各自的实现，但必须先写下并测试同一套
   覆盖与过渡语义。在此之前，两个后端的任何质量对比结论都不成立。
6. **停止用 NXP=6 的真实引擎测试作为质量证据。** 362 个单元上的 edge-CV 没有统计意义。
7. **停止把 JIGSAW 当作生产依赖或主线修复。** 许可、确定性、cell-id 稳定性三条反对理由
   仍然成立；只允许在 tri/hex 分流的隔离研究中作为用户本地安装的外部 oracle，
   不链接、不分发、不进入默认 dispatch。
8. **停止在没有归因的情况下调整过渡带宽度**（`phase_support` 的 6 环、`halo`、`max_transition_row`）。
   被审文档自己已警告这会造成跨父边界或需求丢失。

---

## 9. 优先级建议

### 9.1 [状态更新 2026-07-28] 当前优先级

| # | 当前动作 | 停止条件 |
|---|---|---|
| 1 | 保持共享 Method-C 作为 Tri/Hex base generator；冻结 projected Case 9 已验证的 phase-support 选择语义，不再扩 phase/repair/hard support | `case9_projected_hfield_20260728` 保持 `116/116`、`target_above_actual=0`、拓扑/几何合法；`case9_native_15arcsec_20260729` 另行按同层 canonical 合法化任务单推进，不得把 projected 成功外推为原生闭环 |
| 2 | triangular-primal P2 生产路线归档；保留 `#[cfg(test)]` research 代码、writer/reload/`.2dm` 证据，不接 dispatcher、不抽象 backend | `case9_projected_hfield_20260728` 是 `passes=0`、`changed=false` 负对照；原生 15″ 当前阻塞是 `TransitionPatch`，不是已证明的 unmet-demand，因此不以该失败直接重启 P2 |
| 3 | 全球 hex 正样本矩阵已有 2 张准均匀 + 4 张变分辨率；tri 已有 1 个 NOAA NGOFS2/FVCOM 区域运行产品正样本。下一步只补区域 hex、tri 负样本/稳定性和其他 triangular 产品，不改现有阈值 | 正负样本、区域产品与跨产品重复测量共同给出可重复的可用/不可用分界 |
| 4 | 两层质量契约：先按 topology family 定义 triangular-primal 与 polygonal-dual 基础门，再按 FVCOM/MPAS/CoLM 等 adapter 叠加格式、边界和模拟验收。现有绝对 max 门先只记录后校准 | 各拓扑族有足够的正负参考集，各 model adapter 有自己的模拟/格式验收；样本不足时显式标记 `provisional` 或 `missing_reference_set`，不得互借阈值 |
| 5 | 保持 M1 生产接入、checkpoint/rollback、M2b 与 M3 关闭 | 只有重新校准后的产品硬门或模拟证据证明当前网格不可接受才重启 |
| 6 | 案例矩阵只补真实缺口。**[2026-07-29]** 流域内孔 fixture、Hydro raster-spacing 以下走廊压力例、Cartesian 同一 demand 配对量化均已完成；Cartesian 尚缺两套完整 Project 的成对网格对比 | 不新增 polygon schema、不为矩阵造后端。南极极冠 polygon 在球面 contains 语义定义前明确标为 unsupported，生产使用已验证的 circle |

**明确不做：** 不把 `aspect_ratio_fail` 直接调到 21，不为单个 case 选择 niter 魔数，
不继续实现精确可动边/区域边界代理来服务已关闭的回滚机制。

#### [执行更新 2026-07-27] triangular-primal P0/P1 已完成

隔离测试现位于：
`rust/earthmesh_cli/src/p2_primal_refine_research.rs`，仅由 `#[cfg(test)]` 私有模块编译，
没有接入生产 Method-C、CLI、Project schema、writer 或 GUI。

原语审计结论：

- 现有 `refine_onedivide_two_one_based` 是 Canonical 过渡 triangle 修补，不是通用
  conforming bisection；
- `refine_onedivide_four_connection_one_based` 只传播 marker；
- 现有 LOP 依赖一基追加/占位协议，不能直接作为 dynamic primal materializer。

最小 research-only shared-edge refinement 已证明：

- 闭球细分前后 `χ=2`，无 boundary/hanging edge；
- midpoint 在共享边上唯一复用，结果确定；
- 两级细分保持 parent/level；
- 合法 degree-8 primal 顶点可通过，不受 dual `[usize;7]` 容量限制；
- 退化 triangle 被 typed test failure 拒绝。

测试证据：research 4/4、既有 subdivision/LOP 10/10、
`earthmesh_mesh --lib` 145 passed / 0 failed / 1 ignored，clippy 与
`cargo check -p earthmesh_cli` 通过。

P0/P1 只证明原语。随后 canonical phase/component 24-run 矩阵已经证明共享 Method-C
无法通过换 phase 补齐 Case 9 的 7 个 hard source bin，因此按门槛重启了 P2。

#### [历史执行记录 2026-07-28] triangular-primal P2 真实 A/B 已通过；当前生产立项已归档

P2 直接读取清理后的 Case 9 最终 masked Tri gridfile 与其 immutable
`source-demand.json`，没有重建或修改 HField。实现仍只存在于上述 `#[cfg(test)]` 文件：

1. 对 `target > actual` 的父三角形请求三条共享边；
2. 同一边只生成一个球面 midpoint，所有相邻三角形复用该点，形成 conforming red/green split；
3. hard target 不允许降低；先在父三角邻接图上向外抬高过渡目标，直到相邻目标层级差 `≤1`；
4. 子三角继承父目标，逐轮推进到目标层级；最后重新执行正式 positive-area hard-support coverage。

直接只追逐离散 hard target 的第一次实验虽然补齐了 7 个 bin，但产生了 level jump 2–3，
已明确拒绝。加入上述无权重、只抬高不降 hard demand 的 2:1 过渡闭包后，结果为：

| 指标 | A：Case 9 当前最终 Tri | B：research-only primal P2 |
|---|---:|---:|
| triangle cells | 197152 | 197793（+641，+0.325%） |
| active / adequately covered hard bins | 114 / 107 | 114 / 114 |
| `target_above_actual` cells | 22 | 0 |
| graded target above actual cells（执行前） | 43 | 0 |
| refinement passes | — | 3 |
| Euler χ | -68 | -68 |
| connected components / boundary loops | 与 A 相同 | 与 A 相同 |
| max primal vertex valence | — | 10 |
| hanging / non-manifold / misoriented / invalid index | 0 | 0 |
| self-intersection / invalid polygon | 0 / 0 | 0 / 0 |
| determinism | — | 同输入重复两次逐结构一致 |

两个真实负对照也通过：

- 全球平滑 circle Tri：102535 cells，3600/3600 hard bins 已覆盖，P2 `passes=0`、
  `changed=false`，max primal valence=7；
- 区域 CoastalOcean Tri + active circle HField：3963 cells，306/306 hard bins 已覆盖，
  P2 `passes=0`、`changed=false`，Euler χ=0，max primal valence=7。

第二个真实正例随后使用同一真实 landcover 阈值源、但独立的 150 km/NXP=54 全球
CoastalOcean Tri 基网格；未修改 P2 实现：

| 指标 | A：Method-C 最终 Tri | B：research-only primal P2 |
|---|---:|---:|
| triangle cells | 114951 | 116046（+1095，+0.953%） |
| active / adequately covered hard bins | 106 / 84 | 106 / 106 |
| `target_above_actual` cells | 51 | 0 |
| graded target above actual cells（执行前） | 100 | 0 |
| refinement passes | — | 2 |
| Euler χ | -46 | -46 |
| max primal vertex valence | — | 10 |
| hanging / non-manifold / misoriented / invalid index | 0 | 0 |
| self-intersection / invalid polygon | 0 / 0 | 0 / 0 |
| determinism | — | 同输入重复两次逐结构一致 |

同机制的区域 CoastalOcean Tri + real landcover threshold 对照也已全覆盖：
18077 cells、8/8 active hard bins、P2 `passes=0`、`changed=false`、Euler χ=-1。

**当时结论（已被本节开头的最新 Case 9 负对照取代）：**

- Case 9 剩余 7-bin 缺口不是 Tri 拓扑不可实现；triangular-primal 共形细分已给出一个
  小幅增量且满足正式覆盖/拓扑/确定性的构造性解；
- 这不等于可以替换或分叉整个 Method-C。当前最小可行方向是“共享 Method-C 生成合法
  base Tri/Hex；仅 Tri 产品在最终 masked primal 上按需执行独立共形补齐”；
- Hex/MPAS 主线、Method-C canonical 表、`dimc=7` dual 表、g/halo/repair/质量阈值均未修改；
- 两个不同基网格尺度的真实 unmet-demand Tri 正例均已通过；research-only
  writer + reload/格式验收结果见下一节，这仍不是生产接入证据。
- 在带真实区域边界/OBC 的 unmet-demand 正例和至少一个 Tri 模型读入/短模拟验收完成前，
  不增加 backend abstraction，不接 Project dispatcher。

#### [执行更新 2026-07-28] research-only writer / reload / FVCOM 格式门已通过

没有新增 writer 或格式。P2 只把 research primal 映射到现有
`UnstructuredMesh` 紧凑 sentinel 契约，并复用：

- `derive_iap_w_to_m_one_based` 生成动态 vertex-to-triangle 环；
- `write_unstructured_mesh_netcdf_with_refine_levels` 写 NetCDF；
- `read_gridfile_mesh_points` + `quality_input_from_gridfile` 重新读取正式 Tri 视图；
- `write_standard_fvcom_from_gridfile` 生成现有 FVCOM/SMS `.2dm`。

最小 octahedron 写回单测验证了物理 triangle/vertex 数、逐 triangle refine level 与
Euler χ=2。加入 OBC provenance 研究夹具后，research 模块当前结果为
`8 passed / 0 failed / 3 ignored`。

两个真实正例随后都从**写出的文件**重新执行 hard-support coverage、拓扑和几何检查：

| 正例 | 写回 cells | hard bins | `target_above_actual` | Euler χ | boundary edges | FVCOM `.2dm` |
|---|---:|---:|---:|---:|---:|---:|
| Case 9 / NXP=81 | 197793 | 114/114 | 0 | -68 | 12529 | 197793 triangles / 105093 nodes |
| 独立 150 km / NXP=54 | 116046 | 106/106 | 0 | -46 | 8348 | 116046 triangles / 62151 nodes |

两者重载后的 `self_intersection_count`、`invalid_polygon_count` 与 topology issues 均为 0，
verdict 均保持 `warn`。独立 150 km 正例的两次 NetCDF 写出 sha256 逐字节一致。
这关闭了“research 结果只能存在内存中、现有格式无法表达
primal valence 10”的风险：现有 writer 的动态 `dimc` 能表达它，Tri/FVCOM adapter
也能消费。

第三个真实正例使用东亚 `100–150°E / 5–55°N` 区域
`CoastalOcean + Tri + FVCOM`、真实 IGBP landcover threshold 12：

| 指标 | A：区域 Method-C 最终 Tri | B：research-only primal P2 / 写回重载 |
|---|---:|---:|
| triangle cells | 11290 | 11397（+107，+0.948%） |
| active / adequately covered hard bins | 13 / 11 | 13 / 13 |
| `target_above_actual` cells | 5 | 0 |
| refinement passes | — | 2 |
| Euler χ | -12 | -12 |
| connected components / boundary loops | 9 / 30 | 9 / 30 |
| boundary edges | 1322 | 1335 |
| max primal vertex valence | — | 9 |
| self-intersection / invalid polygon / topology issues | 0 / 0 / 0 | 0 / 0 / 0 |
| FVCOM `.2dm` | — | 11397 triangles / 6354 nodes |

这关闭了“区域 mask 一定会让 P2 失效”的疑问，但**没有关闭真实 OBC 端到端门**。现有
`write_standard_fvcom_from_gridfile` 按契约不写 open-boundary segment；现有
`clean ocean → obc.nc4 → carved FVCOM NS` 链要求输入未裁剪源网格。把已经区域化的
P2 网格再次送入该链会被严格边界检查拒绝：
`boundary vertex 2 has 1 connections, expected 0 or 2`。没有放宽该检查，也没有用
“把全部边界环都当 OBC”之类的猜测绕过。P2 在最终 masked primal 上执行，因此生产接入前
必须把原始 OBC/coast 边界类别作为稳定 provenance 传入，并在边界边被二分时确定性插入
midpoint；这是边界语义传播，不是第二套网格 writer。

该最小传播现已在 `p2_primal_refine_research.rs` 落地，仍只在 `#[cfg(test)]` 中：

- 从源 gridfile 的 Canonical OBC id 通过 `gridfile_w_row_layout` 映射回源 W 行，再映射到
  research primal vertex；不从几何边界环猜测 OBC；
- 每轮共享边细分保留唯一 `edge → midpoint` 记录；若 OBC 段上的边被分割，就按轮次递归
  插入 midpoint；
- 写出前逐段断言相邻 OBC 顶点仍是最终 primal boundary edge，再复用现有
  `write_fvcom_mesh_2dm` 生成 `NS` 记录；
- 同一组合单测调用正式 `classify_boundary_orders_one_based` 产生非空 OBC，给父三角形施加
  unmet target，经完整 research P2 target loop、source Canonical id 映射、共享边 midpoint
  插入、最终边界边校验后，由正式 FVCOM writer 写出 1 个 `NS` segment；仓库现有
  `clean_ocean_window` production fixture 另行验证文件态 `obc.nc4 → FVCOM NS` 链，
  两组 release 测试均通过；
- 东亚真实 landcover 正例重跑结果保持 `11290 → 11397` cells、`13/11 → 13/13`
  hard bins、Euler `-12`、`target_above_actual=0`，说明 provenance 代码未改变无 OBC
  输入的细化结果。

这仍是**组合证据，不是带真实 OBC 的 landcover unmet-demand 端到端正例**，但此前的
OBC/IBC 来源缺口已经用既有数据闭合，没有新增 sidecar 或几何猜测。Ocean contain 阶段的
`ContainMesh.ustr_id[][2]` 原本就记录每个非结构单元落在 `Close` 来源域内的像元总数；
生产 Ocean Tri runner 现在把它作为 source-domain mask：来源域外的相邻 inactive 单元
定义 OBC，来源域内因陆地/海岸 mask 变为 inactive 的相邻单元定义 IBC。兼容性清理后来
激活的产品单元与来源域取并集，避免把合法填补误报成来源域外。共享旧分类入口保持原行为，
Hex 和非 Ocean Tri 路径不受影响。

生产文件态复验结果：

- `0.25°` 数据仅是 **OBC 分类诊断用的合成常量全海洋栅格**，不是实际 landcover，也
  没有替换项目源文件。需要区分：以下历史运行仍把真实 landcover 判定聚合进固定 HField
  bins，因此不能再称为“原生 15″ 空间语义”。该夹具得到 209 个真实边界顶点：
  152 OBC + 57 IBC，分为 8 个 OBC segments；
- 中国海真实 `input/landtype_igbp_update.nc` 为 `86400×43200`、240 samples/degree，
  即 15 arc-second（约 500 m）IGBP。该生产路径得到 79 个真实边界顶点：
  28 OBC + 51 IBC，分为 4 个 OBC segments；
- 两者都没有把最大边界环伪装成 OBC，分类直接来自 source-domain provenance。

此前较小的真实 IGBP `Close`（`106–113°E / 8–16°N`）在 clean-ocean 阶段拒绝中心
`38537`，现已定位为 raster→cell hard-demand 投影的口径错误，而不是 source-demand
本身越界：一个有正面积 ocean hard bin 可以同时与海洋产品单元和混合海岸/陆地单元相交，
旧 Ocean 路径把所有相交单元都提前视为 immutable；同仓库 landtype writer 则已正确规定，
保守投影必须先与实际 simulation-ready 产品 support 求交。Ocean renewal 现采用同一契约，
只删除了这条错误的提前拒绝，没有放松对产品内 immutable demand 的保留/连通性检查。

原失败项目使用同一真实 500 m IGBP、同一 `Close` 和同一参数完整重跑成功：

- Method-C 三层生成通过，最终 `1107` Tri cells、`606` vertices，质量 `pass`；
- `obc.nc4` 有 56 个 OBC 顶点、3 个 OBC segments，以及 43 个 IBC 顶点；
- 正式 `.source-demand.json` 已生成并绑定最终 gridfile；
- 2 个 active hard bins 全部 adequately covered；
- research P2 负对照为 0 pass、网格无变化，说明该真实产品本身不需要 P2。

这些运行没有重写 15″ 源文件，也没有删除产品内 hard demand 或重绑 artifact；但其
landcover 决策场确实被聚合到固定 HField bins。以下质量与拓扑结果保留为旧投影口径的
历史证据，不再作为原生 15″ 细化正确性的证明。
随后对同一较小真实 IGBP `Close` 做了共享读取路径性能修复：区域 intended-output support
不再扫描全球 `86400×43200` 栅格；精确像元聚合复用已计算的 domain/HField-bin active
结果；只有完全位于目标 bin 内的 source pixel 才省去冗余 bin clip，跨 bin 边界的像元仍走
原精确三重裁剪。第一次优化把完整运行由约 `802 s` 降至 `67.34 s`；继续让 landtype
HField 读取复用同一预计算区域 domain mask 后降至 `8.45 s`（约 `95×`）。输出仍为
`1107` Tri cells、`606` vertices、三层细化、质量 `pass`、`2/2` hard bins 覆盖，最终
gridfile sha256 与优化前相同：
`744bd2f68af371712796b157b55ef70270f1459c1f864ff0a119f8e2961a98ac`。
因此该优化没有改变当时既有的 HField 聚合口径、阈值、mask、hard-demand 或网格输出；
它并未解决 15″ landcover 到固定 HField bins 的信息损失。

较大的中国海 `Close` 已使用同一真实 15 arc-second IGBP 和最终实现完成生产式复验：

- 完整生成与 Ocean Tri 后处理耗时 `483.56 s`（约 `8.06 min`）；
- 最终 `4311` 个物理 Tri cells、`2311` vertices，实际最大细化层级 `3`；
- topology 全部通过：单连通、Euler `0`、2 个边界环，orphan、non-manifold fan、
  boundary degree violation、邻接不互反、悬挂/重复边、自相交和 invalid polygon 均为 `0`；
- HField 门全部通过：`target_above_actual=0`、`missing_level=0`、
  `uncovered_hard_support_bin=0`、实际相邻层级跳变 `>1` 为 `0`；
- 几何为 `edge-CV max=0.2713`、`aspect max=2.017`、最小角 `29.29°`。总 verdict 是
  `WARN`，唯一原因是 `angle_deviation_deg_max=43.54°`；报告同时明确标记
  `tri_absolute_max_shape` 缺少外部参考集，因此这不是拓扑、覆盖或模拟可用性失败。

本次同时关闭了 Ocean Tri hard-demand 恢复策略的语义缺口。中心 `40831` 是精确海陆
裁剪后没有任何 edge-connected ocean neighbor 的孤立海洋需求；中心 `42209` 是 canonical
边界兼容清理必须删除的 boundary ear。实验性强制保留 `42209` 会产生一个有 3 条连接的
边界顶点，证明它不能作为 simulation-ready Tri 产品的一部分。最终通用规则因此是：

1. hard demand 先与初始 ocean product support 求交；来源产品之外的 demand 属于正常
   land/ocean 交集语义，不报告为拓扑排除；
2. 对初始产品内、但不能通过 Tri 产品兼容性或 edge-connected ocean 清理的 hard demand，
   明确排除并记录；不允许后续 exact-component 恢复把它重新加回；
3. 质量仍以原始 raster hard bin 的正面积覆盖为准，允许同一 bin 由其他合法海洋单元覆盖。

按最终报告口径，上述中国海输出需要记录的 simulation-ready product exclusions 是
`40831` 与 `42209` 两个中心；早期运行打印的 45 包含了 43 个本来就在初始 ocean product
support 之外的 demand，已在不改变网格输出的前提下修正为静默求交。没有放宽海陆阈值、
拓扑门、HField coverage 或质量门，也没有为某个中心写特例。

收尾回归已覆盖完整 `earthmesh_cli` release 包：`830 passed / 0 failed / 31 ignored`。
同时通过 `mesh_quality_views` 5/5、`mkgrd_gridinit` 6/6（1 ignored）、
`mkgrd_mask_restart` 4/4（19 ignored）和 `mkgrd_restart_area_judge` 25/25。
其中同步修正的均是旧测试夹具：阈值源必须保留以验证 source-demand 语义绑定；标准
MPAS/FVCOM 期望使用当前 canonical sentinel 编号；Ocean restart 夹具改为真实共享边的
三角网格。没有为通过测试放宽生产语义、拓扑门或 immutable-demand 校验。

模型侧能力也已本机核查：仅发现 `mpiexec`/`mpirun`，未发现 FVCOM、ADCIRC、SCHISM、
FESOM 或其他可消费该 Tri 产物的模型可执行文件；仓库已有的是格式 writer/reader 与
adapter 测试，不是数值模式。因此本轮只声称 `.2dm` 格式门通过，不虚报模型读入或短模拟。

#### [执行更新 2026-07-28] 真实产品矩阵：共享 phase-support 修复闭合全球碎片 landcover 缺口

在不改变生产阈值、不增加特例、但仍使用旧固定 HField landcover 投影的条件下，又补了
区域 Hex 与全球 Hex 的生产参数复验。当前三类代表性产品的结果如下；它们不能替代原生
15″ 逐单元判定复验：

| 产品 | 实际输入与规模 | 拓扑 / hard demand | 质量结论 |
|---|---|---|---|
| 区域 Ocean Tri | 中国海 `Close`，真实 `86400×43200`、15 arc-second IGBP，最大层级 3 | 4311 个物理 Tri cells；`target_above_actual=0`、`uncovered_hard_support_bin=0`；单连通、Euler 0、2 个边界环，全部拓扑错误为 0 | `WARN`；唯一原因是尚未完成外部 Tri 校准的 `angle_deviation_deg_max=43.54°` |
| 区域 Atmosphere Hex | `100–150°E / 5–55°N`，真实 IGBP threshold 12，150 km/NXP=54，5000 次 global spring、500 次逐层 spring，最大层级 2 | 5609 个物理 Hex cells；`target_above_actual=0`、`uncovered_hard_support_bin=0`、实际相邻层级跳变 `>1` 为 0；单连通、Euler 1，self-intersection / invalid / orphan 均为 0 | `WARN`；仅 `cell_edge_length_cv_max=0.4138` |
| 全球 Atmosphere Hex | 原 `earthmesh-project-run-41203-1784795655169875000-0` 同配置，真实 fragmented landcover threshold 12，100 km/NXP=81，最大层级 2 | 117404 个物理 Hex cells；`target_above_actual=0`、`uncovered_hard_support_bin=0`、实际相邻层级跳变 `>1` 为 0；单连通、Euler 2，self-intersection / invalid / orphan 均为 0 | `WARN`；仅未校准的绝对极值形状门：`edge-CV max=0.4719`、`aspect max=4.6817` |

修复前，全球 Hex 的唯一未覆盖 hard bin 是 720×360 immutable demand raster 中的
`(ilon=437,jlat=194)`，中心约为 `38.75°E / 7.25°N`，目标层级 2。它与四个最终 Hex
单元有正面积重叠，但四个单元实际层级均为 0。追踪共享选择路径后确认：

- 该点附近的 M-point / U-edge midpoint 已正确采到 level-1/2 demand；
- `phase_support` 也已正确生成，但 `preserve_all_demands=false` 的中间 pass 又把
  canonical stride-3 遍历限制到实际 demand 点；
- 碎片 hard fragment 内没有 canonical seed，而合法 owner 位于既有六跳
  `phase_support` 内、实际 demand 外，因此遍历到不了 owner；anchor 随后因没有完整
  aligned footprint 被丢弃，pass 1 没有细化，pass 2 也就失去 parent interior。

最小共享修复只删除了这条重复的 demand traversal gate：component / generation guard
仍把搜索限制在既有六跳 `phase_support` 内，最终 selected topology 仍由 demand、coverage
和 canonical rad3 footprint 决定。它不是扩大 HField、面膨胀或按经纬度补丁。新增回归
`deeper_point_demand_uses_phase_support_to_reach_a_canonical_seed` 已证明旧逻辑失败、新逻辑
通过。

为验证是否只是 Method-C 漏采样 W-face 中心，做过一个最小诊断分支：在既有 M-point 与
U-edge midpoint 采样之外加入 W-face center。结果单元数由 116753 增至 117230（+477），
同一 hard bin 和同四个欠覆盖 cells 仍存在，且
`edge-CV max 0.4719 → 0.4936`、`aspect max 4.682 → 5.076`。该实验已撤销；它证明问题
不是“再补一个采样位置”即可解决，继续扩大采样只会增加单元并恶化形状。

无临时追踪的 release 真实产品复验结果：

- 全球 Hex：`116753 → 117404` cells（+651，+0.558%）；hard coverage 两项归零，
  topology 全通过；`edge-CV max 0.471940 → 0.471917`、`aspect max
  4.682273 → 4.681667`，没有用形状退化换覆盖；
- 区域 Atmosphere Hex：仍为 5609 cells，最终 gridfile sha256 与修复前逐位一致；
- 区域真实 Ocean Tri 小域正例：仍为 1107 cells、606 vertices、三层细化、质量
  `PASS`、2/2 hard bins 覆盖；坐标、连通性、refine level 与质量报告不变。内部 lineage
  id 因候选 phase traversal 顺序变化而重编号，不改变产品几何或拓扑语义；
- 共享回归：`earthmesh_mesh` HField 19/19、Ocean renewal 3/3 通过。

严格 `AutoRefine` 的第一次修复后复验又暴露了一个独立策略缺口：基础两层已没有
hard-coverage failure，非通过 gate 仅为 `aspect_ratio_max` 与
`cell_edge_length_cv_max` 两个未校准绝对极值；实际选中的 repair batch 只有一个
`cell_edge_length_cv` cell。旧策略虽然知道 conforming HField 的 edge-CV 不应靠增加
层级修复，却因同时存在 aspect 全局 gate 而错误进入局部 repair，新增候选随后在 pass 2
报 transition-triple 跨父边界。

最初的抑制判据直接读取 `repair_cells`，但该字段是受
`repair_batch_limit`（默认 1）约束的执行计划，不是缺陷普查；因此“首个计划单元是
edge-CV”不能证明不存在被排名遮住的 aspect repair。`worst_cells` 也不是严格替代物：
它截断到 50，且每个单元只保留当前最严重的一类缺陷。

最终修复没有扩张报告 schema，而是复用同一质量评分生成一个**排除
`cell_edge_length_cv` 的备用有界 repair batch**。只有 coverage / topology 已闭合、
首选批次全为 edge-CV 时才计算该备用批次；若存在 aspect、最小角或其他局部形状主缺陷，
同时替换“是否修”的判据和实际 repair 计划，避免“判断看 aspect、执行却仍细化 edge-CV”
的分层错误。单元回归已覆盖同一单元中 edge-CV 排名遮住 aspect、默认 batch=1 的情形。

真实全球 landcover 网格由此正确找到了一个 `aspect_ratio` 备用计划并实际尝试，而不是
因首个 edge-CV 样本静默抑制。该候选在 Method-C pass 2 的 transition-triple 闭包失败；
AutoRefine 现在把候选失败视为可选修复失败，保留已经验证有效的基线，而不是让合法网格
退出 2。最终严格复验 `exit=0`，记录
`decision=kept / local quality repair candidate failed; kept the last valid mesh`；输出仍为
117404 cells、hard coverage 与 topology 全通过，gridfile sha256
`0bd9a816ed579fab35567893cbc607ed495c621d3ab472400c3dd95259b6668b`，与 `Warn`
路径逐位一致。

该性质已作为端到端契约锁定：对 coverage / topology 已闭合、且不存在可执行非 edge-CV
备用计划的 HField，`Warn` 与 `AutoRefine` 最终 gridfile 必须字节一致；若存在真实 aspect
计划则允许尝试，但失败候选不得替换基线。AutoRefine E2E 4/4 通过，其中同时覆盖
transition-only 的 Warn/AutoRefine 字节一致，以及真实 aspect repair 执行并被严格改善
判据接受。

当前可以冻结本矩阵中的区域 Ocean Tri、区域 Atmosphere Hex 与全球 Atmosphere Hex
landcover hard-demand delivery。不能外推为所有尚未运行的产品/边界组合均已验证；
本轮已同时关闭严格 AutoRefine 的错误额外 pass，不再修改已经闭合的共享选择语义。

**仍未跨过的生产门：**

- `.2dm` 成功只证明格式适配，不等于 FVCOM/其他 Tri 模型的数值模拟稳定；
- 区域 unmet-demand、拓扑边界和写回已通过，生产 Ocean Tri 的 domain-edge OBC/IBC
  provenance 也已由合成全海洋和真实 500 m IGBP 文件态正例验证；当前真实 OBC 产品的
  hard demand 已全部覆盖，所以只形成 0-pass 组合负对照。只有真实产品再次出现 unmet
  demand 时，才需要补“非空真实 OBC + P2 实际运行”的端到端正例；
- P2 仍为 `#[cfg(test)]` 私有模块，Project schema、CLI、GUI、默认 dispatcher 和共享
  Method-C 均未改变。

因此不再写第二套 writer、不抽象 backend，也不再额外降低当时的 landcover 投影口径或
制造人工缺口；该历史结论不覆盖后续确立的“15″ 不得进入更粗 HField”硬契约。
P2 作为 research-only 构造能力和 0-pass 回归归档；最新 Case 9 已证明当前生产基线不需要它。
只有产品矩阵在最新共享语义下自然产生 unmet case，且具备真实边界与模型读入/短模拟正例，
才重新评审是否恢复生产立项。

#### [执行更新 2026-07-29] 区域闭合流域内孔 Hex 已完成真实 Project 闭环

案例 7 已从“schema/lowering 有能力、缺真实 fixture”升级为完整生产回归。新增的最小 WGS84
fixture 为 `test/fixtures/watershed_with_hole.shp`（外壳 + 一个内部孔洞）和
`test/fixtures/basin_refinement.shp`（域内 specified-close 条带），耐久入口为
`scripts/run_basin_hole_regression.sh`。

第一次完整运行暴露了两个共享问题，均修在所有调用者经过的边界上：

1. Method-C 内部 Hex incidence ring 允许任意方向，但最终区域 Hex writer 直接保留该顺序，
   产生负面积和同向共享边。`orient_hex_cells` 现在在所有最终 Hex 输出前统一相邻边方向并将
   每个连通分量规范为球面 CCW；Tri 路径、选择和 HField 不变。
2. SHP hole 通过零宽双桥降为 legacy even/odd close ring。HField 的 source/domain/bin
   三重相交旧路径要求简单多边形，因而把合法桥接域算成零面积，specified hard layer 静默为空。
   现复用已有垂直条带 even/odd 面积算法，通过包含排除计算精确三重交集；没有改成 bin-center
   近似，内孔仍排除，凸多边形快路径不变。

最终 NXP=81、两层、5000 次 global/逐 pass spring 结果：

- 211 个 Hex cells，实际 max-level=2，24 个 specified hard bins，hard max-level=2；
- `target_above_actual=0`、`uncovered_hard_support_bin_count=0`；
- `χ=0=expected`、单连通、2 个 boundary loops；
- negative/self-intersection/invalid/misoriented/orphan 全为 0，质量 `pass`；
- `static-netcdf` 两次独立运行的 gridfile SHA256 都是
  `44c67c05c3463ab186ec665c9395d626954b956b157e83fa76b11bb467bbd807`。

同一 release 可执行文件下，全局北极 circle 与跨日期线 bbox 各重跑两次，cells、细化位置、
拓扑/覆盖指标及当前 hash 均稳定。验证总计为 `earthmesh_geometry` 15/15、
`earthmesh_cli --lib` 299/0/5；未为内孔案例新增产品特例、阈值或近似裁剪。

#### [执行更新 2026-07-29] NOAA NGOFS2 同区域 Tri 分辨率可行性基准

NOAA/NCEP NOMADS 的 `ngofs2.t03z.20260727.2ds.f000.nc` 不是全球网格，而是北墨西哥湾
区域 FVCOM 产品。实测物理范围约为 `97.862–85.733°W / 21.789–30.781°N`，包含
303714 个节点和 569405 个三角形。源文件 sha256 为
`537617337ab3e8949c5b3fd45bf22ce0bb4938ce8318da721cab471b8a2eb9c3`；可复现实测和
汇总保存在 `target/ngofs2-earthmesh-benchmark-2026-07-29/`。

NGOFS2 的实际三角边长分布为：

| 指标 | min | P01 | P50 | P95 | P99 | max |
|---|---:|---:|---:|---:|---:|---:|
| edge length (km) | 0.0406 | 0.0754 | 0.3116 | 1.6061 | 4.2516 | 13.8760 |

这组分布说明“接近 NGOFS2”不能只解释为局部最细边达到某个值；其多数单元本身就是亚公里
尺度。按当前 Method-C 最大层级 5，仅让 level-5 标称边长达到 NGOFS2 的 P50，就需要
`base≈9.97 km`、`NXP≈803`。当前生成器先构造全球父网格，因此在区域 mask 前需处理约
`20×803² = 12,896,180` 个父层三角形。该规模尚未运行；直接启动它不能作为负责任的第一步。

先做了同一区域、同 `Ocean + Tri + CoastalOcean + FVCOM`、同 landcover threshold 12 的
缩放试验：

| 试验 | base / 最大层级 | 标称最细 | 结果 |
|---|---|---:|---|
| scale4 | 40 km / 5 | 1.25 km | 第 4 pass 失败：内部 nested grid 在 `87.115°W / 30.831°N` 跨 parent boundary |
| scale4 + 2° 生成缓冲 | 40 km / 5 | 1.25 km | 同一位置、同一内部 parent-boundary 失败；排除外部区域 bbox 过近 |
| scale16 | 40 km / 3 | 5 km | release 8.59 s 成功，最终 4374 Tri cells / 2369 个物理节点 |

成功的三层结果覆盖原 NGOFS2 bbox，`target_above_actual=0`、
`uncovered_hard_support_bin_count=0`；self-intersection、invalid polygon、orphan 和
project-aware topology issue 均为 0。形状指标为 edge-CV max/P95/P99
`0.2623/0.1950/0.2415`、aspect max/P95/P99 `1.9599/1.6030/1.8397`、最小角
`30.22°`。项目口径 verdict 为 `WARN`，仅由 `angle_deviation_deg_max=41.58°` 触发。
独立 `--project-quality` 复验与生成报告一致；裸 `--mesh-quality` 不含 Ocean mask 的合法
多分量期望，不能替代 Project 质量口径。

但其实际边长为 min/P50/P95/max `3.827/17.416/39.998/47.512 km`，P50 是 NGOFS2 的
`55.9×`，总三角数只有 NGOFS2 的 `0.77%`。因此当前证据只能说明 Method-C 能在该区域生成
合法的三层 Tri 网格，**不能**声称已经达到 NGOFS2 的分辨率或节点密度。

这里还有一个独立的输入尺度限制：项目读取的 IGBP 原文件仍是 15 arc-second，但本次
landcover demand 被组合进 `720×360` 的全球 HField，因此决策空间已经发生降采样。运行时明确报告
`259200/259200` 个 HField bins 对请求的局部 `h` 欠分辨。高分辨率源文件并不自动等于
高分辨率 sizing field；当前 0.5° HField 也无法表达 NGOFS2 的海岸、航道和水深驱动的
亚公里结构。

**[契约更新 2026-07-29]** 15 arc-second landcover 不允许在任何路径中静默投影到更粗
的 HField 或阈值 mask。共享读取入口现比较源与目标维度；目标任一轴更粗即返回
`source downsampling is forbidden`。该保护同时覆盖 landcover hard thresholds、
intended-product support 和 continuous-threshold landtype mask。等分辨率读取仍通过
回归。

同日已接入最小原生路径，但旧固定 HField landcover 投影保持禁止：

- categorical landcover 三项判据（类别数、主类占比、海陆比例）直接枚举每个当前
  Delaunay 三角形内的全部原始源像元中心；原始类别不先聚合为 `720×360` 或其他固定
  HField；
- 每层得到的逐 W-face hard demand 进入共享 Method-C canonical seed/rad3 闭包，
  specified/hydro/其他连续 HField 来源仍通过原有 sizing field 合并；Tri 与 Hex 继续
  共用同一张 Delaunay/Voronoi 对偶网格，不新增产品生成器；
- Land/Ocean/LOC 的 product support 不再从粗 landcover HField 推导。生成阶段先使用
  全域候选 support，最终由原生 landtype mask 后的真实产品网格反向绑定 support，
  因此没有用粗分类替代 15″ 分类；
- 分辨率从 NetCDF 文件维度本身推导，要求为全局 `360×180` 的整数倍，不依赖可缺失的
  CLI source-grid 元数据。

最小回归已覆盖：原始 NetCDF 像元触发逐面 demand、Method-C materialize、Delaunay/Tri
拓扑校验和 Voronoi/Hex 对偶构造；共享 HField 模块的“更细源必须拒绝下采样”回归为
60/60 通过。

**尚未声明生产完成：** 真实 `86400×43200` 文件已完成一次未降采样的 release
复验，证明逐像元需求读取与选择阶段吞吐可运行；但该 Case 9 仍在第二层
`TransitionPatch` materialize 失败，所以不能把“15″ 输入已被原生消费”外推为完整网格
已经可用。Cartesian-XY 和以 landtype 作为 mask 的 continuous-threshold 仍保持明确
拒绝；原生逐 pass face-demand 尚未作为独立 ledger 持久化，现有 HField artifact
只记录其余 raster hard layers 与最终产品 support。

#### [执行更新 2026-07-29] 15″ Case 9 的 non-triplet 闭包已前移到选择阶段

共享 HField 选择现在只通过完整 canonical seed/rad3 footprint 增长掩码，并在
materialize 前按固定点顺序闭合凹角、vertex-only contact 与非三元组周界。实现复用了
既有 contact 检测、周界构造和 rad3 原语，没有增加产品分支，也没有修改最终 Tri/Hex
视图或 NetCDF 格式。

真实 `86400×43200` landcover、NXP=81、max-level=3 的 release 复验记录在
`target/case9-native-15arcsec-seed-core-final-1785301981/run.log`：

- 运行明确报告 `uses all 86400x43200 source pixels; coarse HField projection disabled`；
- pass 1 添加 31 个完整 seed，得到 7362 个选中面、53 条周界，长度范围 18–120；
- pass 2 添加 68 个完整 seed，得到 4962 个选中面、37 条周界，长度范围 18–99；
- 两个 pass 的全部周界长度余数均为 0；后续 legacy non-triplet repair 均在首轮立即
  返回，耗时约 `0.0008 s`，没有再进行 face-level 生长；
- 回归 `hfield_vertex_contact_closure_preserves_seed_atomicity` 证明闭合后掩码可由
  最终 seed 集的 rad3 footprint 精确重建。

因此本轮修复的结论是：原先“选择输出非三元组周界、只能由事后 face repair 补救”的
缺陷已闭合，且修复保持 seed 原子性。

完整 Case 9 仍未通过。第二层在同一选择结果上报告独立的
`TransitionPatch`（`Current nested grid 3 crosses the parent boundary ... W face
3726`），并有 transition self-loop 见证；这不是 non-triplet 闭包的残留。曾试验过
周界 phase 旋转和 witness-local seed 扩张，它们只能减少或移动见证，不能将其清零，
同时增加过度细化，因此已全部撤除，未进入生产代码。下一步若继续，应单独推导
canonical transition 的选择前合法性约束；不得重新扩大 64 轮事后 repair 或把上述
启发式恢复为产品逻辑。

#### [执行更新 2026-07-29] TransitionPatch 已精确定位到跨层平衡，不是 `perim_fill3` 移植特例

OLAM 6.4 的 canonical `spawn_nest.f90::perim_fill3` 与 Rust 在 `iu51` 端点改写上的
逻辑一致（[OLAM source releases](https://sourceforge.net/projects/olam-model/files/)）。
因此 `iu51 == iu45` 时产生自环不是一个可用单行 guard 修复的 Rust 翻译错误；跳过该
写入只会把失败改成 `iw9 transition patch has no solid split edge`，不会生成合法模板。
该实验已撤销。

本轮新增的只读谓词
`method_c_transition_parent_boundary_witnesses` 直接从父层 perimeter triple 枚举
canonical `perim_fill3` 会消费的 8 个 W-face 槽，并在 materialize 前检查：

```
parent face mrlw == current pass
且该 parent face 不会在本轮被完整细分
```

它在真实 15″ Case 9 上给出：

- pass 1：`0` 个父边界见证，与成功 emit 一致；
- pass 2：`60` 个父边界见证；首批见证的所有 support face 均为
  `mrlw=1, flag=0`，而 pass 2 需要 `mrlw=2`；
- 第一个见证就是真实 emit 报出的 parent U `4054` / W `3726` 失败。

这证明主要阻塞不是“本层多选了一个 face”，而是**上一层没有为下一层的 canonical
transition 模板铺出足够的同层父面**。现有同层 fill/shrink repair 无法把 `mrlw=1`
变成 `mrlw=2`，所以继续增加 64 轮预算没有意义。

三种固定缓冲实验均未闭合，且已撤销：

| 实验 | pass-1 selected faces | pass-2 父边界见证 | 结果 |
|---|---:|---:|---|
| 全选中 seed 加一圈 thirdm 邻居 | `19,503` | 未进入 pass 2 | pass 1 提前触发 Valence |
| 原始 hard face 加一圈 W 邻接 | `7,443` | `50` | 仍为 TransitionPatch |
| 按剩余层数加面邻接（`2/1` 圈） | `7,794` | `47` | 仍为 TransitionPatch |
| 按 3 条 transition rows 加面邻接（`6/3` 圈） | `11,268` | `48` | 仍为 TransitionPatch |

见证数对固定宽度不单调，说明不能继续用“再加几圈”代替闭包。

Octree / p4est 对这里的可借鉴项是**架构而非算法**：明确局部不变量、只增标记、
工作队列传播、到不动点后再 materialize。对应到当前顺序生成器，下一实现应是跨层
工作队列：

1. 选择 pass `L` 后先运行上述精确谓词；
2. 若模板依赖的 face 仍在 `L-1`，生成一个 `L-1 → L` 的支撑细化请求；
3. 先 materialize 该低层请求，再重新采样和选择 pass `L`；
4. 只允许层级增加，并用 stable lineage 判定是否取得进展；
5. 谓词清零后才执行 pass `L` 的一次 materialize。

这与 octree 的 balance propagation 同形，但还必须同时保留 Method-C 特有的
stride-3、non-triplet、vertex-contact、valence 和 hard-coverage 谓词；不能直接移植
p4est 的树算法。该跨层队列尚未进入生产，真实 15″ Case 9 仍应明确报告失败。

#### [M0 原型复验 2026-07-29] 跨层队列闭合父支撑，但 Case 9 仍有独立同层 TransitionPatch

默认关闭的 `EARTHMESH_M0_CROSS_LEVEL_SUPPORT=1` 原型已实现上述最小队列：
支撑请求使用稳定 W-face lineage，回滚到父 pass 前的 checkpoint，补父层后重新采样并
重选子 pass。公开 gridfile lineage 与内部 W-face 行的映射由单元测试锁定；测试曾发现并
修正一个 `+1/+2` 行偏移，因此更早的 `163+49` 试跑不作为证据。

修正后的真实 `86400×43200`、NXP=81、max-level=3、release 有界复验记录在
`target/case9-native-15arcsec-cross-level-corrected-1785311643/run.log`：

- pass 2 依次请求 `163 + 33 + 4 = 200` 个新的父层稳定 lineage；
- 每次请求都实际增加父层 demand；没有重复请求或无进展循环；
- 第三次父层重建后，精确 parent-boundary 谓词清零；
- 随后出现 `21` 个同层 transition self-loop 预测，并以
  `M point 38 revisits U edge 920 before returning to start U edge 782`
  的 `TransitionPatch` 退出（状态码 2）。

因此 p4est 式分层工作队列对**跨层父支撑子问题**有效，但不是完整 Case 9 修复。
它仍保持 M0-only、默认关闭；生产路径和 64 轮 repair 默认语义未切换。下一步不得再用
固定宽度 grow 或局部 witness 特例，应把同层 self-loop/alias 合法化建模为独立的有限
canonical 模板约束；只有证明该约束的判定与移动集后，才能讨论替换事后 repair。

本基准不触发生产代码修改。下一道架构门是：先提供区域优先的 triangular-primal
生成/裁剪路径，以及由海岸线、bathymetry 或其他海洋尺度源驱动的高分辨率区域 HField；
随后再按 NGOFS2 同域分布和模型短模拟验证。继续降低全球 base、提高 repair 预算或只看
局部最细边，都不能回答 NGOFS2 等价性问题。

### 9.2 原评审优先级（历史记录）

| # | 建议 | 依据 |
|---|---|---|
| 1 | **执行 §5 的唯一下一步**：冻结基线 + 3 个真实引擎案例的行为不变度量探针（含 `Stat5` 分位数扩展）。这是所有后续判断的前提 | R1；`Stat5` 缺分位数（`rust/earthmesh_quality/src/lib.rs:94-100`） |
| 2 | **修正被审文档中的 C1–C9**，特别是 C2（M1 不是 drop-in）和 C7（跨后端量化不一致）。文档目前会把实施者引向一个有回归风险的 M1 | `method_c_nest_spring_iteration/mod.rs:174`；`earthmesh_hfield/src/lib.rs:704` vs `hfield_refine/mod.rs:471` |
| 3 | **把 M1 与 M2 对调，并把 M2 拆成 M2a(归因，行为不变) / M2b(最小化，可选)**，在 M2a 后设置强制暂停点 | §6；M2a 回归风险较低，且两种结局意味着应停止 |
| 4 | **统一两个后端的量化语义契约，而不是统一实现**。Cartesian 走解析锥、没有栅格，本就无法共用 stencil；要统一的是「硬需求必须被覆盖」与「渐变裙带不得被物化为额外拓扑层级」这两条语义，并用一致性测试同时约束两个后端。这正是问题 H「共享契约、后端分别实现」的落地形式 | R4 |
| 5 | **给 `limit_gradient` 的栅格分辨率要求加运行期校验**。**[2026-07-29 已实施非阻断 warning]** 当前会报告欠分辨 bin 数与最大 `spacing/h`，且不改变输出；窄 corridor 压力例仍待运行 | `earthmesh_hfield/src/lib.rs:506-511`；案例 11 |
| 6 | **在 M2a 中同时记录第七类原因 `demand_tail`**（anchor 兜底追加，`method_c_spawn_hfield/mod.rs:822-868`），并把 `hard_demand`/`demand_tail` 从 `actual_above_target` 的口径中分离出来单列 | C6 |
| 7 | **把 M3 的启动条件从「M1 后仍有平台」改为 §6.5 的三条证据**，并把 §5.1 分支 D（放开可动集的一次性诊断）作为其中的关键实验 | 平台可能来自可动集限制而非拓扑 |
| 8 | **把 M4 从活跃路线图降级为归档记录**，把「外部参考网格阈值校准」从 M0 拆出为独立可延后任务（它是数据获取任务，不应阻塞 M0） | 建议 8 与停止事项 7；M0 范围控制 |

---

**一句话结论：** 被审文档的方向对、问题 B/C/G/H 站得住，但它把一个存在未验证风险、
收益上限受结构约束的步骤（M1）排在了最前面，并低估了自己已有的真实引擎覆盖、
遗漏了跨后端量化语义尚未形成统一契约这一机制性风险。正确的下一步不是接弹簧，
而是**先让「变好还是变坏」这个问题第一次可以被回答**。
