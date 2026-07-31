# Method-C Case 9 合法化任务单

日期：2026-07-29

范围：真实 `86400×43200`（15 arc-second）landcover、NXP=81、max-level=3 的全球 Ocean Tri Case 9

状态：**研究原型进行中；生产路径未切换；Case 9 原生 15″ 路径尚未完成**

## 1. 当前结论

1. **Method-C 仍是共享的 Tri/Hex 生成器。** Tri 与 Hex 是同一套 Delaunay/Voronoi
   拓扑的两个输出视图；当前问题不是“不支持 Tri”。
2. **必须区分两条 Case 9 路径：**
   - 2026-07-28 的共享 phase-support + Ocean 产品支撑路径已达到 `116/116` hard bins、
     `210048` triangles；
   - 2026-07-29 的原生 15″、禁止 coarse HField projection 路径仍在 pass 2
     `TransitionPatch` 失败。
3. **跨层父支撑子问题已有有效原型。** 默认关闭的
   `EARTHMESH_M0_CROSS_LEVEL_SUPPORT=1` 使用稳定 W-face lineage 回滚并重建父 pass；
   修正后的运行请求 `163 + 33 + 4 = 200` 个父层 lineage，最终把 parent-boundary
   谓词清零。
4. **当前独立阻塞是同层 canonical transition 合法性。** 同一运行随后出现 `21` 个
   self-loop 预测，并以
   `M point 38 revisits U edge 920 before returning to start U edge 782`
   的 `TransitionPatch` 退出。
5. **生产尚未修复。** 跨层队列和 exact candidate scan 都是 M0-only、默认关闭；
   现有生产 repair 语义没有被替换。

权威证据：

- `docs/mesh_refinement_review_2026-07-25.md:2436-2438`
- `docs/mesh_refinement_review_2026-07-25.md:2525-2544`
- `target/case9-native-15arcsec-cross-level-corrected-1785311643/run.log`

## 2. 已完成与已否证

### 2.1 已完成

- [x] 15″ 源数据按原始 `86400×43200` 像元读取；不允许降采样或 coarse projection。
- [x] non-triplet、凹角和 vertex-only contact 的选择前闭合已有共享实现。
- [x] 跨层父支撑请求使用稳定 lineage，并由单元测试锁定内部/公开行映射。
- [x] 修正旧 `+1/+2` lineage 偏移；旧 `163+49` 数据作废。
- [x] 24-face 观测依赖域内完成 469 次精确 materialization：
  - 单候选 `14`；
  - 两两组合 `91`；
  - 三组合 `364`；
  - 成功项均为 `0`。
- [x] exact emit 已证明不是主要性能瓶颈：65 次共 `6.045 s`；候选扫描才是主要成本。
- [x] 增加默认关闭的 `EARTHMESH_M0_EXACT_CANDIDATE_SCAN` 诊断入口；不改变候选选择。

### 2.2 已否证或明确不做

- [x] 不提高 64 轮 repair 上限。
- [x] 不通过调 `g`、halo、transition rows、质量阈值或 landcover 类别特例绕过失败。
- [x] 不把价数上限从 7 改成 12；此前“Valence”中包含重复 U-edge 的错误分类。
- [x] 不再使用固定宽度 grow、“再加几圈”或单见证局部 fill 作为解法。
- [x] 不把 `mark_fill_rad3 + concavity closure` 的 24-face 三组合失败外推成全局 UNSAT。
- [x] 不在掩码空间套用未经证明的 Knaster-Tarski 最小不动点结论。
- [x] 不使用 `[lo, hi]` 层级区间代替包含 canonical 相位的非凸离散状态。
- [x] 不把重复 U-edge 建成巨型显式 DFA；环长至多 7，优先使用局部两两不等约束。
- [x] 小模型成立前不引入 SAT/CP/ILP 新依赖。

## 3. 架构决策

### 3.1 传递约束，不固化临时掩码

必须持久化的是：

- hard-demand bin 必须被目标层级覆盖；
- 父层支撑和层级差约束；
- canonical transition 模板合法性；
- M/U/W 互惠、自环、重复 U-edge、价数、周界和 parent-mrlw 约束。

临时 seed/face 掩码属于可回溯的搜索状态，不是不可撤销的不变量。

### 3.2 两类推理共享一个求解状态

1. **跨层传播：** 复用已验证的 stable-lineage 工作队列，补足父支撑。
2. **同层合法化：** 在有限 transition band 内搜索 canonical template placement，
   由现有 emitter 作精确 oracle。

这不是两个互相修补的独立生成器；二者必须作用于同一组 hard obligations 和模板约束。

### 3.3 结果状态

原型只允许输出：

- `SAT`：找到 assignment，将其嵌回冻结的完整状态后，**整网** emitter 与全部硬门验证通过；
- `PATCH_UNSAT`：在明确、固定的 patch 和变量域内穷尽；
- `INCOMPLETE`：资源耗尽、patch 未扩完、变量域未证明完备或证书无法验证。

patch 内约束满足但尚未通过整网 emitter 的结果只能记作内部
`PATCH_SAT_CANDIDATE`；不得对外报告 `SAT`。

只有 patch 扩到声明的问题域、变量域完备且排除理由可独立重放时，才允许报告
“相对于 Method-C canonical 模板规格的 `UNSAT`”。不得把 emitter 的单次失败自动描述为
UNSAT 证明。

## 4. 执行任务

### P0：修正文档真相源

- [x] **T0.1 给 Case 9 两条路径分配稳定 ID。**
  - 成功路径：`case9_projected_hfield_20260728`；
  - 原生路径：`case9_native_15arcsec_20260729`。
  - 验收：仓库中不再出现未限定路径的“最新 Case 9 已闭合/已可用”。
- [x] **T0.2 修正状态摘要。**
  - 重点检查：
    `docs/mesh_innovation_briefing_2026-07-29.md:79-80`，
    `docs/mesh_refinement_review_2026-07-25.md:84-87,150,1909,1978`。
  - 保留 2026-07-28 成功数据，但明确它不是原生 15″ 完整生产闭环。
- [x] **T0.3 在仓库简报记录外部 PDF 的证据边界。**
  - 标明 PDF 基于修正前的简报生成；
  - Case 9 状态以本任务单和评审文档的带路径记录为准；
  - 不维护仓库外 PDF 的独立勘误版本。

### P1：冻结可重放的 Case 9 同层问题

- [x] **T1.1 保存跨层闭合后的确定性 checkpoint。**
  - 包含 selected mask、target levels、face demands、stable lineage、canonical tables；
  - 记录输入、可执行文件和 checkpoint SHA256；
  - 两次运行必须位级一致。
- [x] **T1.1b 统计 21 个 self-loop 见证的父空间依赖连通性。**
  - 以稳定父空间 M/U/W 标识和 canonical 依赖边建图；
  - 同时把每个见证映射到所在周界分量，记录分量长度、模 3 余数及可能的分裂/合并关系；
  - 报告一个连通簇、少数簇或分散见证，并保存各簇及相关周界分量的依赖闭包；
  - 只有依赖闭包互不相交时才允许按簇独立或并行求解；
  - 单簇 `PATCH_UNSAT` 不得外推为其他簇或整网不可满足。
- [x] **T1.2 提取 M38/U920 的有限依赖 patch。**
  - 从既有 24-face 域起步；
  - 使用稳定父空间 M/U/W 标识；
  - 根据 T1.1b 决定单 patch、多个独立 patch 或联合 patch；
  - patch 必须包含见证依赖闭包和受影响周界分量的完整约束接口；canonical 半径只作次级扩张维度；
  - 若候选会让周界分量分裂或合并，必须纳入完整受影响分量，或把结果降级为 `INCOMPLETE`；
  - 不预设 24 faces 或固定半径完备。
- [x] **T1.3 固定 patch 外边界契约。**
  - 明确哪些外部模板槽、父层支撑和 hard obligations 被固定；
  - 对被切开的周界保存外部固定长度的模 3 贡献和连通接口；
  - 如果某个失败可能依赖 patch 外状态，结果只能是 `INCOMPLETE`，不能报 UNSAT。

### P2：最小确定性拓扑求解原型

- [x] **T2.1 定义最小变量。**
  - 主变量优先为 canonical template placement / local seed choice；
  - 必须存在 assignment 到当前 Method-C emitter 输入的精确映射；
  - 若映射不存在，停止，不另造第二套网格语义。
- [x] **T2.2 编码硬约束。**
  - hard coverage；
  - parent support / 层级差；
  - non-triplet perimeter；
  - vertex-only contact；
  - TransitionPatch canonical table；
  - U-edge 游走槽位两两不等（最多 21 对）；
  - valence、M/U/W 互惠和 parent-mrlw。
- [x] **T2.3 使用标准库实现确定性有限枚举/回溯。**
  - 固定变量顺序和值顺序；
  - 初版只禁止当前完整 assignment；
  - 每个候选调用现有 exact emitter；
  - 不加入 SAT/CP/ILP 依赖。
- [x] **T2.3b 在采信任何 `PATCH_UNSAT` 前通过已知 SAT 正对照。**
  - 从冻结 24 矩阵或 2026-07-28 成功路径提取同尺寸 patch；
  - 已知合法 assignment 必须能无损映射并由 emitter 重放成功；
  - 枚举器在不注入答案的情况下必须找到至少一个合法 assignment；
  - 任一条件失败均判为枚举器或变量域缺陷，所有 `PATCH_UNSAT` 结果作废。
- [x] **T2.4 实现确定性 patch 扩张。**
  - 24-face → canonical 一圈 → 下一圈；
  - 每次扩张保留前次证据；
  - 达到资源上限返回 `INCOMPLETE`。
- [x] **T2.5 生成可重放结果。**
  - PATCH_SAT_CANDIDATE：局部 assignment、边界和局部检查摘要；
  - SAT：局部 assignment 嵌回完整状态后的整网 emitter 输出与全部硬门摘要；
  - PATCH_UNSAT：完整变量域、边界、候选数和每个候选的失败分类；
  - INCOMPLETE：停止原因和未搜索范围。

P2 验收：

- T2.3b 已知 SAT 正对照通过；并且
- 找到嵌回完整状态后可由整网 emitter 接受的 Case 9 assignment；或
- 在明确 patch 内确定性穷尽，重复运行结果和证据哈希一致；
- 不修改生产选择、repair、Tri/Hex 输出或 NetCDF 格式。

### P3：证明泛化 nogood 是否值得做

- [x] **T3.1 统计完整赋值 blocking 的候选数、时间和内存。**
- [x] **T3.2 只对已证明闭合的稳定依赖域泛化 nogood。**
  - 每个 generalized nogood 必须能独立重放；
  - 不能仅凭 fail-fast 的第一个见证排除更大候选集合。
- [x] **T3.3 对拍完整 blocking 与 generalized nogood。**
  - 小 patch 的 SAT/PATCH_UNSAT 结果必须完全一致；
  - 任一不一致立即关闭泛化路线。

准入门：只有 T3 明确证明收益后，才评估现有求解器依赖；否则保留简单枚举器。

### P4：影子运行与生产准入

- [ ] **T4.1 默认关闭的 shadow solver。**
  - 只记录建议 assignment 和证据；
  - 不影响生产 gridfile。
- [ ] **T4.2 成功矩阵生产回归。**
  - 既有 24 次成功 emit 不得出现假违规；
  - 全球 Hex、区域 Hex、区域 Ocean Tri、极区、换日线、流域内孔均保持原有结果。
- [ ] **T4.3 原生 15″ Case 9 正向门。**
  - 必须使用全部 `86400×43200` 源像元；
  - coarse projection 保持关闭；
  - hard coverage、parent-boundary、self-loop、TransitionPatch、valence、M/U/W 互惠全部为 0；
  - 两次 release 运行 gridfile 和证据哈希一致。
- [ ] **T4.4 性能门。**
  - 单独记录求解、emit、质量检查和 I/O；
  - 禁止拿 debug/release 混合数据下结论。
- [ ] **T4.5 生产切换。**
  - 先以新合法化器替代同层失败分支；
  - 保留旧 repair 作为一个版本周期的诊断兜底；
  - 新旧结果冲突时 fail closed，不静默选择任一结果。

## 5. 并行但不阻塞合法化的工作

- [ ] 按 Tri/Hex、全球/区域和产品族继续校准质量分布。
- [ ] 拓扑、自交、hard coverage、M/U/W 互惠继续作为硬门。
- [ ] 未完成外部校准前，不修改经验质量阈值，不用质量指标指导拓扑搜索。
- [ ] 检索 mesh-generation infeasibility certificate / UNSAT proof 先行工作；在完成检索前，
  “hard-demand 可满足性证书”的创新性标记为 `unverified`。

## 6. 停止条件

出现任一情况时停止扩大实现：

1. template assignment 无法无歧义地映射到当前 emitter；
2. patch 依赖持续扩张至整个 transition band，局部化没有实际收益；
3. generalized nogood 无法提供独立、稳定、可重放的依赖证明；
4. 小模型与 emitter 在任一冻结成功案例上不一致；
5. 为通过单一 Case 9 需要新增产品、landcover 类别或具体 M/U/W ID 特例。

停止不等于隐藏失败：应输出 `INCOMPLETE`、保留证据，并重新评估 canonical 模板表达力、
合法化翻边或更细粒度的共形细分方案。

## 7. 当前下一步

严格顺序：

1. 不再增加 component-phase 启发式；其六个 coherent phase 与 phase-aware 父层支撑均已实测；
2. T3.2/T3.3 已证明 exact nogood 可安全压缩证据，但其证明仍需逐一检查被阻断
   assignment；因此不把它接入搜索器，也不引入 SAT/CP 依赖；
3. 冻结选择掩码下，十个见证周界分量的独立 triple-offset 域已完整穷尽且无 SAT；
   不再恢复“统一旋转周界”启发式；
4. 当前 canonical phase 的 `2470` 个 legal seed 全选会在 preflight 形成两条
   non-triplet 周界，不能作为合法上界，也不能用“继续加 seed”代替形态约束；
5. 对 hard face obligations 作统一 0–6 圈同层膨胀时，self-loop 先降后升且始终无
   SAT；不把 buffer 圈数做成配置或生产修复；
6. 原始 `112` 个 hard faces 形成 `90` 个同层连通分量，其中 `78` 个是单 face；
   逐分量单圈膨胀 `0/90` SAT，证明单个局部 buffer 仍不足；
7. 逐步重分组的 exact-witness 贪心路径可把 self-loop 从 `21` 降至 `4`，但停在
   单步局部极小；不把该评分启发式接入生产；
8. 对局部极小处 `72` 个等值动作的两两组合，exact 全流水线单 assignment 约 `7.35 s`；
   已检查的 `214/2556` 个组合均保持 `4` 个 self-loop，因该域过窄且完整运行成本过高而
   停止，状态明确为 `INCOMPLETE`，不继续三元组合或扩大暴力穷举；
9. 保留全部 hard demand 的同层图形态学闭运算已扫描半径 `0–6`，`0/7` SAT；最大仅把
   self-loop 从 `21` 降至 `17`，并转成 `Valence`，不保留实验代码、不做生产参数；
10. emitter 输入面已核对：当前实现没有独立的 template ID、split 方向或翻边变量；
    `nest_wd`、transition split 与 canonical 写表均由 selected mask 和有序周界确定。
    因此不得把“新增一个 template 变量”伪装成补齐现有搜索维度；
11. Case 9 仍为 `INCOMPLETE`，**T4 明确阻塞**。继续实现前必须先选择并写清一种新的
    规格级契约：要么是保持全部 15″ hard obligations 的形态感知聚类/placement 契约，
    要么是带成功正对照的新 canonical transition 模板。两者都属于新规格，不是继续
    扩大现有暴力搜索；出现整网 exact-emitter SAT 前不得进入 shadow/生产准入；
12. 原生 15″ hard obligation ↔ legal-seed 支撑图已完成秒级普查：全部 `112` 个 hard
    faces 均有支撑，但 `44` 个覆盖分量共涉及 `870` 个 seed，最大分量有 `65` 个变量，
    且实际 topology witness patch 仍达 `154` 个候选。覆盖图不能证明拓扑独立，故不实现
    “逐覆盖分量朴素穷举”；
13. 把 `44` 条 coverage hyperedges 与 `10` 条现有 topology-witness patch hyperedges
    合并后，冻结 fixed-phase transition-band 图仍有 `1062` 个相关 seeds；最大耦合分量
    为 `241` 个变量。没有 symbolic topology constraints 或经证明的 nogood 前，不实现
    全局/逐簇指数搜索；
14. preflight 现为每个 self-loop witness 和每条周界保存精确 candidate-seed 依赖。
    Case 9 单 witness 范围为 `5–45`，但见证周界范围为 `13–381`；因此下一实现片必须是
    周界约束的符号判定/传播，而不是对 witness 局部变量继续穷举。
15. 严格下降可把 self-loop 从 `21` 降至 `1`，局部二元跳转可降至 `0`；但 `39/39`
    zero-self-loop assignment 的整网 exact emitter 均转为 parent-boundary。不能把
    “同层谓词清零”当作整网合法化。
16. “同层严格下降 → 请求父支撑 → 重建”的临时交替原型在十次 zero-self-loop 后仍
    重复产生约 `12` 个新 self-loop，并新增 `129` 条父层支撑请求；该 support treadmill
    已否证，实验分支已删除。
17. 最小父支撑候选的联合域并非单一局部 patch：`34` 个最小候选共享同一组 `28`
    条父面 lineage，但它们分成 `6` 个不相邻分量，父面图直径为 `353` 条边。停止条件
    2 已触发；不实现单 patch 联合求解器。
18. support-free 严格下降、二元和三元跳转可把 self-loop 从 `21` 降到 `1`；最后
    `7` 个候选的 `127` 个非空子集已全部检查，仍无 support-free 改善。该局部域已穷尽，
    但不得外推为其他下降路径或 Case 9 UNSAT。
19. 全局需求聚类同样不能直接采用：在不删除任何 15″ hard obligation 的前提下，
    允许每次连接最多增加 `7` 个中间面，只能把 `90` 个 hard 分量合并到 `50` 个，
    且确定性森林已需 `123` 个额外面（hard/augmented efficiency 下界约 `47.7%`）。
    下一规格只能是按稳定支撑分量分解的局部 shape-aware placement，不是全局连成块、
    固定半径 closing 或 coarse projection。
20. 原生 15″ 的 `max_level=4` 已实跑；它在新增 pass 3/4 之前，于 pass 2 精确复现
    level 3 的 `163+33+4` 父支撑请求和同一组 non-triplet 周界长度。因此增加最大层级
    不能绕过当前 pass-2 阻塞；不再运行等价的 level 5 对照。
21. 原生 15″、`max_level=3` 的 `NXP=162` 对照已实跑。基础分辨率加倍后，pass-2
    父支撑请求从 `163+33+4=200` 降到 `38+13=51`，失败周界从 `35` 条、总长度
    `952` 降到 `10` 条、总长度 `241`，但仍以一条 non-triplet 周界
    （长度 `34`）和 parent-boundary 冲突退出。提高基础分辨率能缩小问题，却不能
    消除当前 canonical placement 缺口；不再继续用更大 NXP 暴力绕过。

这是一道架构门，不是性能门。已知的 emitter-mapped 便宜变量均已检查或达到明确成本门；
继续枚举等值动作、扩大 Hamming 半径、增加 buffer 圈数或恢复 64 轮 repair 都不能提供
与其成本相称的新结论。

## 8. T1.1 / T1.1b 执行结果（2026-07-29）

默认关闭的 `EARTHMESH_M0_LEGALIZATION_CHECKPOINT_PATH` 现可在跨层父支撑清零后、
同层 pass 2 materialize 前写出完整可重放状态。checkpoint 与运行 provenance 分离：
前者保存 mesh、target levels、face demands、selected mask、stable lineage 和 preflight；
后者单独记录 release 可执行文件、生成 namelist 与原生 landcover SHA256，避免运行目录进入
checkpoint 字节。

两次并行 release 重放均以同一个 `TransitionPatch` 退出（状态码 `2`），但 checkpoint
逐字节一致（当时 schema v1）：

- run A：
  `target/case9-legalization-boundary-a-1785319553/legalization-checkpoint.json`
- run B：
  `target/case9-legalization-boundary-b-1785319553/legalization-checkpoint.json`
- checkpoint SHA256：
  `e407b59d320dda0e9e7438b7e0be00ebd146a0fd828f738898551fe5718450c7`

T2.2 开始后，schema v2 又把选择阶段的精确 hard-demand anchors 纳入 checkpoint。
当前原生重放为：

- `target/case9-legalization-hard-obligations-1785323328/legalization-checkpoint.json`
- checkpoint SHA256：
  `bdb78be72cd9a00d99eb81d8f3cdff9ca73fe9d36bfea98df34e316a08dd7d8e`
- schema：`earthmesh-method-c-legalization-checkpoint-v2`
- demand anchors：`112`

其 witness、cluster 和 patch 计数与 v1 相同；写出前同样通过 10 个 patch 的基线 face /
周界边界门。

同层 preflight 的确定事实：

- selected faces：`4227`；
- 周界分量：`36`；
- 所有分量长度均可被 `3` 整除；
- self-loop 见证：`21`；
- 按同一周界分量或父 W-face lineage 依赖相交聚类后为 `10` 簇，大小：
  `3, 4, 2, 1, 4, 2, 1, 1, 2, 1`；
- 这 `10` 簇分别落在周界分量
  `0, 4, 6, 7, 9, 19, 22, 27, 28, 29`，对应长度
  `60, 63, 75, 18, 105, 42, 18, 18, 24, 18`；
- 各簇父 W-face lineage 依赖闭包大小依次为
  `70, 92, 48, 22, 96, 48, 22, 22, 48, 22`。

T1.2 已把每簇映射到当前 canonical seed/rad3 emitter 输入：

- 全局 legal seeds：`2470`，当前 selected seeds：`109`；
- 每簇 candidate seed 数：
  `60, 45, 17, 8, 154, 40, 14, 14, 10, 18`；
- 其中当前已选 candidate seed 数：
  `6, 7, 3, 1, 12, 6, 1, 1, 2, 1`；
- 每簇可变 rad3 footprint 的 W-face 并集：
  `487, 562, 244, 124, 1014, 480, 139, 141, 176, 171`；
- checkpoint 同时保存每个相关周界分量的完整有序接口；patch 外 face 状态由完整
  `prepared_selected_faces` 固定。
- 第 `1` 簇的可变 seed 足迹除见证所在的周界分量 `4`（长度 `63`）外，还会影响
  无 self-loop 见证的周界分量 `26`（长度 `18`）；checkpoint 已把两个完整接口一并纳入，
  证明 patch 边界不能只由见证分量定义。

简表与证据哈希：

- `target/case9-legalization-hard-obligations-1785323328/legalization-patch-inventory.json`
- SHA256：
  `d9df39b8f5714aea48781eab4efffa79b456963b6feddd6bf9b43f866e9fec19`

边界解释：

1. Case 9 **仍未证明有解或无解**；T1 只把问题冻结并确定了 patch 形状。
2. `perimeter_len=955` 是某个 repair 候选产生后所有周界分量长度的扁平总和，不是一条
   `955` 边周界。
3. 首个见证的 `dependency_faces=6` 和四个 changed candidates 只描述当前局部 repair
   算子；不得外推为 template-placement 的完整变量域。
4. 21 个见证不是单一局部簇；T1.2 必须以这 10 个分量耦合簇为起点。只有证明簇间依赖
   闭合后，才允许独立求解。
5. 历史 `24-face` 域小于首个三见证簇的 `70-face` 依赖闭包；其 469 次失败不能作为
   该簇的 `PATCH_UNSAT`。
6. 当前 patch 已固定“现有 assignment 下”的完整周界接口；后续候选必须通过下节的
   exact boundary check。让分量跨 patch 合并/分裂的候选降级为 `INCOMPLETE`，不得用于
   `PATCH_UNSAT`。

### T1.3 / T2.1 实现增量

`MethodCDelaunayMesh::selected_faces_from_method_c_seed_ids` 现复用生产
`method_c_rad3_faces_with_neighbors`，把有限 seed assignment 精确映射为现有 emitter
消费的 W-face mask；checkpoint 写出前会拒绝无法由 `selected_seed_ids` 重建的选择。

`MethodCDelaunayMesh::legalization_patch_boundary_check` 在替换 patch 内 seed
assignment 后运行同一套凹角与 non-triplet 准备，并同时检查 patch 外被改变的 face 和
受影响周界的固定外部点集。任一项改变时，该候选最多是 `INCOMPLETE`，不得用于
`PATCH_UNSAT`。已知合法选择的原样回放和人工制造的越界差异均由
`face_hard_demand_selection_checkpoint_is_deterministic` 锁定。

原生 15″ release 重放
`target/case9-legalization-hard-obligations-1785323328` 已对 `10` 个真实 patch 的当前
assignment 逐一执行该门；checkpoint 成功写出且 SHA256 仍为
`bdb78be72cd9a00d99eb81d8f3cdff9ca73fe9d36bfea98df34e316a08dd7d8e`，说明当前
assignment 的 exact 准备未改动任一 patch 外 face 或固定周界点集。运行随后仍以原有
`M point 38 revisits U edge 920` 退出，未虚报 Case 9 已解决。

这完成了 T2.1 的 local seed 变量域与 assignment→现有 emitter mask 的精确映射。
默认忽略的
`m0_legalization_checkpoint_first_toggle_boundary_probe` 又对每个真实 patch 的首个
deterministic seed toggle 执行该门，证据为：

- `target/case9-legalization-hard-obligations-1785323328/first-toggle-boundary-probe.json`
- SHA256：
  `bef7520bc65058176f28a495097397b094bb4330984a3ca6fd49f1bdf39d54d0`

结果为 `7/10 closed`、`2/10 INCOMPLETE`、`1/10 candidate_invalid`：簇 `6` 的候选
越界 `1` 个 face，簇 `9` 越界 `15` 个 face，
且两者的固定周界点集均改变，因此被明确降级为 `INCOMPLETE`。这证明 T1.3 的边界门
既接受原样回放，也会拒绝真实候选的跨界闭包；后续扩 patch 前不得把这两类候选计入
`PATCH_UNSAT`。簇 `7` 的首个 remove 会使 M point `4084` 的 hard-demand anchor
失去最后覆盖，因此由 v2 hard-obligation 门直接拒绝。

在其余边界闭合候选中，簇 `0/1/2/3/5/8` 的首个 add 把预测 self-loop 从 `21`
降到 `20`；簇 `4` 保持 `21`。所有可检查候选的 vertex-only contact 仍为 `0`，周界
分量仍为 `36`。这是 T2.2 的第一批候选级硬约束数据，不代表已找到 SAT assignment。

T2.2 采用混合式单一真相源：`legalization_patch_boundary_check` 直接检查 checkpoint
hard-demand anchors、parent-mrlw、non-triplet 周界、vertex-only contact、预测
TransitionPatch self-loop 和 patch 边界；其余 TransitionPatch 模板、U-edge 游走槽位、
valence、M/U/W 互惠与最终拓扑不另写第二套谓词，而是调用现有
`spawn_nest_pass_method_c_without_mask_repair` 作 exact oracle。

原生 checkpoint 的 first-toggle exact 复验：

- `target/case9-legalization-hard-obligations-1785323328/first-toggle-exact-oracle.json`
- SHA256：
  `3646133ff284ccbe6fc0432c821c8d3e5e3a3547c374e8856be491273dbc1ce9`

结果为 `0/10` exact materializable：`7` 个闭合候选仍被 exact emitter 分类为
`transition_patch`，`2` 个跨 patch 候选为 `INCOMPLETE` 且同样失败于
`transition_patch`，`1` 个 remove 因失去 M point `4084` 的 hard-demand anchor 被拒绝。
该结果只完成候选硬门与失败分类；没有枚举完整变量域，因此不构成
`PATCH_UNSAT` 或整网无解证据。

### T2.3 / T2.4 实现增量

测试态 `m0_enumerate_legalization_patch` 以 checkpoint 中排序后的 candidate seed
顺序作二进制有限枚举，每个 assignment 都经过 T2.2 的 boundary gate 和 exact
emitter。命中整网 emitter 只返回 `SAT`；资源上限、patch 越界或未分类错误均
fail-safe 返回 `INCOMPLETE`。已知可生成的小网格正对照可由枚举器在未预置 assignment
的情况下找到 `SAT`；这只验证枚举逻辑，尚不满足 T2.3b 所要求的 Case 9 同尺寸正对照。

Case 9 最小的第 `3` 簇（`8` 个变量）已在 release 下按“变量赋值直接映射到
emitter 输入”的口径穷尽 `256/256`。两个独立运行的 JSON 逐字节一致：

- `target/case9-legalization-hard-obligations-1785323328/cluster-3-enumeration-current-a.json`
- `target/case9-legalization-hard-obligations-1785323328/cluster-3-enumeration-current-b.json`
- SHA256：
  `8712cdadbdad14ab1d9b70cc6ec539b1340ae17664650cb71bf13da7a1e75859`
- 枚举器可执行文件 SHA256：
  `cc6067a8382bd5a3e29acfca2d7530debc3e11031bf2df48e0cb68562ccd31b5`
- wall time：约 `2 s`
- hard coverage：`62`
- non-triplet perimeter：`137`
- exact `transition_patch`：`54`
- patch boundary 越界：`3`
- 未分类错误：`0`
- SAT：`0`
- 结论：`INCOMPLETE`，不得写成 `PATCH_UNSAT`

早期文件
`target/case9-legalization-hard-obligations-1785323328/cluster-3-enumeration-release.json`
曾记录 `941.85 s`；该探针在每个 assignment 内又调用旧 non-triplet repair，既重复求解、
又不再是变量到 emitter 输入的直接映射，已被上述 direct-v3 结果取代，不得再用于当前
枚举成本估算或失败分类。

`expand_legalization_patch_one_ring` 复用 canonical third-M 邻接，把 patch 确定性扩张
一圈并重算 candidate seeds、mutable faces 和完整受影响周界接口。第 `3` 簇一圈后从
`8` 个变量增至 `20` 个变量，即 `1,048,576` 个完整赋值：

- `target/case9-legalization-hard-obligations-1785323328/cluster-3-ring-1-probe-v2.json`
- SHA256：
  `96b4855e56fb215187e31918324f72943152cf89afc3cab35e05f3fe21f1e4e9`

首个空 assignment 被结构化分类为 `hard_coverage`，无未分类错误。按 8-variable
direct 基线，旧的 `44.7` 天估算已经作废。枚举器现按“baseline assignment 优先，
再按 Hamming 距离、最后按 seed 顺序”作确定性遍历，以保持完整搜索空间同时先检查
最小编辑：

- Hamming 距离 `≤5`，累计 `21,700` 个 assignment：
  - 证据：
    `target/case9-legalization-hard-obligations-1785323328/cluster-3-ring-1-hamming-5.json`
  - SHA256：
    `7067014cedf9b1babda6d01497687043139bc50fc88dfc80a8d9aebf95c861f5`
  - `0` SAT、`0` 未分类错误；
- Hamming 距离 `≤6`，累计 `60,460` 个 assignment：
  - 证据：
    `target/case9-legalization-hard-obligations-1785323328/cluster-3-ring-1-hamming-6.json`
    与 `cluster-3-ring-1-hamming-6-repeat.json`；
  - SHA256：
    `77553f42bfa49f21a0e913d917eab0c3eda62918ade1c419acfbad1987fd889b`
    （两个独立运行逐字节一致）；
  - hard coverage `6,979`、non-triplet `35,754`、perimeter topology `44`、
    exact TransitionPatch `15,070`、patch boundary 越界 `2,613`；
  - `0` SAT、`0` 未分类错误，wall time `491.87 s`。

因此当前可重放结论仍为 `INCOMPLETE`：低 Hamming 距离已系统排除，但尚余
`988,116` 个赋值未搜索，且已搜索集合中仍有 patch boundary 越界候选。该证据不能
写成 `PATCH_UNSAT`，也不能写成 Case 9 无解。

枚举结果携带 checkpoint SHA、枚举器可执行文件 SHA、candidate seed 固定顺序、
搜索顺序、资源上限、已搜索/未搜索规模、硬门拒绝分类和 exact emitter 分类，满足
T2.5 对当前 `INCOMPLETE` 三态结果的重放要求。

T2.3b 已用冻结 M0 `G-CIRCLE` 成功网格完成同尺寸正对照。测试先用 exact emitter
独立确认一个 8-variable patch 的合法 assignment，再把枚举起点置空；枚举器未注入
答案，在第 `2` 个 assignment 找到 `[2]` 并由整网 emitter 接受：

- 冻结网格：
  `target/mesh-refinement-m0-formal-1785055022/G-CIRCLE-gon-n500-r2/m0_G-CIRCLE/result/gridfile_NXP0081_hex.nc4`
- 冻结网格 SHA256：
  `f98dc14d4285eaa7ba46924a254c1ae830e6de5806fd1c61eb80b3ef9d801f44`
- 证据：
  `target/case9-legalization-hard-obligations-1785323328/frozen-m0-sat-control.json`
  与 `frozen-m0-sat-control-repeat.json`
- 证据 SHA256：
  `5caa0d6e47948134da42da0e9dd9285cdb41ea4b20c881ff5e3b9412e1acaf0e`
  （两个独立运行逐字节一致）
- 枚举器可执行文件 SHA256：
  `8625a9238a27dc1ebe03f95b7266f26646bffa5b77d3b3bd33e4e456c0d785c0`

因此枚举器的 `SAT` 路径已有冻结成功矩阵正对照；当前 Case 9 结果继续保持
`INCOMPLETE`，原因是搜索和 patch 边界尚未穷尽，而不是正对照缺失。

### T3.1 朴素完整赋值 blocking 成本

release 枚举器用 `/usr/bin/time -l` 测得：

| patch | 已测赋值 | wall time | 最大 RSS | 证据 |
|---|---:|---:|---:|---|
| 原始第 3 簇，8 variables | `256/256` | `2.00 s` | `438,517,760 B` | `cluster-3-enumeration-timing.{json,time}` |
| canonical 一圈，20 variables | `4,096/1,048,576` | `33.70 s` | `589,742,080 B` | `cluster-3-ring-1-timing-4096.{json,time}` |

第二行约为 `8.23 ms/assignment`；若只按已测前缀线性外推，完整一圈约 `2.40 h`。
该数字只用于判断工程成本，不是复杂度证明。枚举器逐 assignment 流式检查，没有保存
候选全集，因此主要风险是 `2^20` 的时间而非 assignment 存储；RSS 增量来自扩张后的
mesh/checkpoint 与运行时工作缓冲。T3.2 若继续，只应泛化已证明闭合的稳定依赖域，
不得用 fail-fast 单见证换取不可靠剪枝。

### T3.2 / T3.3 exact nogood 与完整 blocking 对拍

测试态泛化器从一个 exact-classified 失败 assignment 出发，只在“所有自由变量补全均由
现有 hard gate 或 exact emitter 明确拒绝”时释放变量。`SAT`、patch boundary 越界或
未分类错误都会立即拒绝泛化；最终子立方体还会完整重放一次。冻结成功小网格的回归同时
锁定：已知 `SAT` assignment 不得产生 nogood。

Case 9 第 `3` 簇的 `8` 变量基线 assignment `[38675]` 得到一个仅固定
`seed 38436 = false` 的 nogood：

- 自由变量：`7`；
- 精确阻断：`128/256` 个 assignment；
- 阻断域内 hard coverage / non-triplet / exact TransitionPatch：
  `31 / 56 / 41`，合计恰为 `128`；
- 独立完整枚举与“剩余枚举 + nogood 证明”分类逐项一致：
  boundary incomplete `3`、hard coverage `62`、non-triplet `137`、
  exact TransitionPatch `54`；
- 两次 release 重放 JSON 逐字节一致。

证据：

- `target/case9-legalization-hard-obligations-1785323328/cluster-3-exact-generalized-nogood-parity-v2.json`
- `target/case9-legalization-hard-obligations-1785323328/cluster-3-exact-generalized-nogood-parity-v2-repeat.json`
- SHA256：
  `ad7020171d73356bffea004a8586086fbf543790e3dfa5d7a8af89293b75cc59`
- checkpoint SHA256：
  `bdb78be72cd9a00d99eb81d8f3cdff9ca73fe9d36bfea98df34e316a08dd7d8e`

这完成 T3.2/T3.3 的正确性验证，但**没有证明搜索加速**：为得到并复核该 nogood，
泛化器执行了 `268 + 128` 次 exact assignment 检查，超过直接完整枚举的 `256` 次。
在没有独立、便宜的依赖定理前，exact nogood 只能压缩证据，不能减少 oracle 调用。
因此准入门结论为：保留简单枚举器，不接入 generalized blocking，不增加 SAT/CP 依赖。
Case 9 状态仍为 `INCOMPLETE`。

### 固定 phase 变量域审计

当前枚举域不是“全部 Method-C canonical placement”，而是 checkpoint 已选择的
**单一 canonical phase 内的 seed 子集**：

- `method_c_spawn_hfield/mod.rs` 在每个 pass/component 只确定一个 `start`；
- `legal_seed_ids` 由该 `start` 的 stride-3 遍历产生；
- T2 枚举器只切换这些 `legal_seed_ids`，没有 `start` 或 phase-class 变量。

因此冻结 M0 正对照证明的是“有解位于当前 seed 域时，枚举器能找到”，不证明当前
Case 9 变量域完备。2026-07-28 的 `24` 次 component-phase A/B 针对的是旧 projected
路径的 7-bin 覆盖缺口，不是当前原生 15″ pass-2 `TransitionPatch`，不能替代这次变量域
审计。

扩张规模也已实测而非按比例猜测：第 3 簇为 `8 → 20 → 26` variables，对应
`256 → 1,048,576 → 67,108,864` 个赋值。第二圈不是约 `50` variables，但按当前
`8.23 ms/assignment` 单线程外推仍约 `6.4` 天。故一圈全搜若无 SAT，只能形成
“固定 phase、canonical 一圈内无 SAT”的证据；若仍存在 patch boundary 越界，结果仍
必须标为 `INCOMPLETE`，不得直接外推为变量维度不全或模板族 UNSAT。

把所有与 canonical 一圈 mutable faces 相交的 rad3 placement 直接并入二进制变量域，
会把第 `3` 簇从 `20` 个变量扩成 `105` 个变量：

- 证据：
  `target/case9-legalization-hard-obligations-1785323328/cluster-3-local-phases-probe.json`
- SHA256：
  `13278c1aa8fd0bff143bac992f800869e1bf1f7d4294be460315e7ccc124974a`

这只是一个 exact-emitter 约束下的诊断超集，不等于生产中的 coherent component
phase；`2^105` 也不能作为可执行搜索空间。后续若固定 phase 一圈无 SAT，应把 phase
建成“组件级相位/placement 选择”，而不是把不同 phase 的 seed 当成彼此独立的 bit。

固定 phase 的 canonical 一圈随后已完整穷尽：

- 总赋值：`1,048,576/1,048,576`；
- SAT：`0`；
- hard coverage：`94,092`；
- non-triplet：`634,525`；
- perimeter topology：`56`；
- exact TransitionPatch：`290,281`；
- patch boundary 越界：`29,622`；
- 未分类：`0`。

聚合证据：

- `target/case9-legalization-hard-obligations-1785323328/cluster-3-ring-1-fixed-phase-aggregate.json`
- SHA256：
  `f37905c9d49aafe3164b09ad682e8da7262f23abaf6937c5803f8b522cf12216`

因此该域的准确结论是“固定 phase、canonical 一圈内 `0 SAT`”；由于仍有边界越界且
phase/template 域未完备，整体状态继续为 `INCOMPLETE`。

### emitter 变量边界审计

代码审计确认，当前 emitter 没有独立的“template ID”或 split-pattern 选择参数：

- `emit_method_c_tables` 的离散输入只有 selected mask 所导出的 `nest_wd`、有序周界和
  固定 Method-C 表；
- full subdivision 由 `nest_wd[iw].is_subdivided()` 唯一决定；
- `perim_fill3_method_c` 对每个有序三元组执行同一套固定 rewiring；
- `method_c_split_outer_edges` 只按现有连通性寻找唯一 solid edge，不是可回溯选择；
- 周界分量可改变的三个 triple 起点已经由独立 offset 全域实验穷尽。

因此“继续增加一个 canonical template-placement 变量”在现有 emitter 中没有精确映射。
当前可搜索自由度仍是：

1. 上游 component/phase 选择；
2. canonical seed 子集，即 selected mask；
3. 已穷尽的周界 triple 起点。

若要加入新的 transition 模板、翻边或非 rad3 placement，就属于扩展 Method-C 模板规格，
而不是补全遗漏变量。按照停止条件 1，在没有新模板规格与正对照前，不再为不存在的
template ID 编写求解器接口。

105-variable placement 诊断超集只跑了低 Hamming 前缀：

| 范围 | 已测赋值 | SAT | boundary 越界 | hard coverage | non-triplet | perimeter topology | TransitionPatch |
|---|---:|---:|---:|---:|---:|---:|---:|
| `≤1` | `106` | `0` | `0` | `1` | `36` | `3` | `66` |
| `≤2` | `5,566` | `0` | `7` | `99` | `3,281` | `265` | `1,914` |

证据分别为 `cluster-3-local-phases-hamming-1.json` 和
`cluster-3-local-phases-hamming-2.json`。这只排除了单个/两个 placement toggle，
不支持继续指数枚举。

### coherent component phase 扫描

原生 `86400×43200`、未降采样 Case 9 的失败簇 3 全部落在 pass 2 的同一个 demand
组件：

- baseline component index：`12`；
- M 点数：`214`；
- canonical phase 数：`6`；
- phase starts：`[5580, 38672, 38636, 38671, 38663, 38662]`。

第一轮 M0-only 诊断用 M lineage `67118` 定位该组件，并要求 phase 数仍为 `6` 后
才应用 ordinal。该轮有意让跨层支撑 preflight 保持 baseline phase，用来隔离“仅改变
同层 phase”的效果。六个 coherent phase 的结果为：

| ordinal | 结果 |
|---:|---|
| `0` | 原始 `M38/U920` TransitionPatch self-loop |
| `1` | parent-boundary，W face `90001` |
| `2` | non-triplet perimeter |
| `3` | non-triplet + parent-boundary |
| `4` | parent-boundary，W face `89748` |
| `5` | 原始 `M38/U920` TransitionPatch self-loop |

所有运行均为退出码 `2`，但 `1–4` 改变了失败类，证明 phase 是有效变量；没有 phase
单独在 baseline-closed parent support 上产生 SAT。聚合证据：

- `target/case9-legalization-phase-variants-final-1785342588/component-phase-sweep.json`
- SHA256：
  `12e77d533679274f2d8f7c4184be7d279748d6de1a217e0ae2b997e2d33fa656`
- executable SHA256：
  `eb7fcbdb5ffd89a13a87bec3fc4bc0ff6b633675296320f1d12e4c95e2e843d3`

该结果仍为 `INCOMPLETE`，不是模板族 UNSAT：phase `1/4` 已明确要求不同的父层支撑，
而本次扫描有意没有让诊断 phase 改写 baseline 跨层队列；多组件联合 phase 和更一般
template placement 也尚未搜索。

### phase-anchor 跨层支撑复验

M lineage 会在父层重新 materialize 后消失，component index 和 phase 数也会变化，
所以它们不能作为跨轮约束键。后续 M0-only 复验改用两组几何锚点：

- component anchor：原始 demand 组件中一个保留点的 Cartesian 坐标；
- phase anchor：所选 baseline phase 的 Cartesian 坐标。

每次父层重建后，只有包含 component anchor 的组件会被选中；该组件内与 phase anchor
最近的 M 点决定新的 phase class。这样传递的是几何 phase 约束，而不是临时 M ID 或
ordinal。该约束同时作用于跨层支撑 preflight、checkpoint 和 exact spawn。

非 baseline 的五个 phase 结果：

| baseline phase | 父层支撑请求轨迹 | 最终结果 |
|---:|---|---|
| `1` | `163 → 51 → 4` | `M38/U920` TransitionPatch self-loop |
| `2` | `163 → 43 → 4` | `M38/U920` TransitionPatch self-loop |
| `3` | `163` 后即失败 | non-triplet + parent-boundary |
| `4` | `163 → 41 → 4` | `M38/U920` TransitionPatch self-loop |
| `5` | `163 → 33 → 4` | `M38/U920` TransitionPatch self-loop |

组件在重建中实际经历了 `component 14/11/13/12` 和 `phase_count 2/5/6` 等变化，phase
anchor 仍能映射到相应 class；因此该复验不是固定 ordinal 重放。证据：

- `target/case9-legalization-phase-anchor-support-1785345442/phase-anchor-support-sweep.json`
- SHA256：
  `cd7a796be231f9308ec2a77c01f2e5211ed8b4ba7ddd37d68d0527aab98cf411`
- executable SHA256：
  `d2af192317f31be9a36a50502c3c5cbe0ad8c8decacf3218514bd74381dc5133`

结论收紧为：**组件级 coherent phase 选择不是 Case 9 的充分解法**。它能改变父层支撑
需求和失败类别，但在相应支撑闭合后仍没有整网 SAT。结果继续是 `INCOMPLETE`，因为
更一般的同层 template placement 尚未完成；exact generalized nogood 已由前文证明
不能减少 oracle 调用。不得把当前结果外推为 canonical 模板族 UNSAT。

### 多组件 phase 变量域审计

新 checkpoint 为每个 demand component 保存其 exact legal/selected seed 所属关系；
回归证明各 component seed 集无重叠，且并集分别等于 pass 级 legal/selected seed 集。
原生 `86400×43200` Case 9 重新生成 checkpoint 后仍保持：

- selected faces `4227`；
- 周界分量 `36`，全部余数为 `0`；
- self-loop 见证 `21`；
- patch `10`。

本次 checkpoint 与 inventory：

- `target/case9-phase-inventory-1785348325/legalization-checkpoint.json`
- checkpoint SHA256：
  `1c8182224690201495f9f8fd1eab4a969576b9d3e451bf8a1bf54b2d866486ba`
- `target/case9-phase-inventory-1785348325/phase-inventory.json`
  与 `phase-inventory-repeat.json`
- inventory SHA256：
  `ea0b7128d471255e29745193499fea2ed9f4b47efd8cc13c1f8351a4cf5ebbfd`
  （两次逐字节一致）。

结果不是“少数 patch 共用一个 phase 组件”：十个 patch 恰好映射到十个不同的 demand
component，且所有 candidate seed 均有唯一 component owner：

| patch | variables | component | phase classes |
|---:|---:|---:|---:|
| 0 | 60 | 0 | 2 |
| 1 | 45 | 7 | 2 |
| 2 | 17 | 10 | 2 |
| 3 | 8 | 12 | 6 |
| 4 | 154 | 13 | 2 |
| 5 | 40 | 22 | 2 |
| 6 | 14 | 1 | 2 |
| 7 | 14 | 8 | 2 |
| 8 | 10 | 11 | 2 |
| 9 | 18 | 5 | 2 |

按当前 checkpoint 的 component 划分，联合 coherent-phase 域为
`6 × 2^9 = 3072`。一次原生运行到 checkpoint 约 `255 s`；逐个全流水线枚举该域约需
`217 h`，且父层重新 materialize 后 component/phase 划分会变化，所以 `3072` 仍不是
稳定的静态 SAT 域。结合 component `12` 六个 phase 的既有负结果，本任务不再扩大
phase 启发式或启动全流水线笛卡尔积搜索；下一合法化方案必须直接处理同层 template
约束，而不是继续枚举 component phase。

同一冻结父网格随后用 checkpoint 内的精确 M/U target levels 重放全部 `14` 个
非 baseline 单组件 phase（九个二相组件各一个，component `12` 的其余五个）。
target-level closure 只接受 checkpoint 中既有采样坐标，避免重新读取或投影 15″
landcover。每个 variant 均重建完整选择、preflight，并调用现有 exact emitter：

- `SAT`：`0/14`；
- exact failure：`12`；
- hard-rejected non-triplet：`1`；
- preflight non-triplet：`1`；
- self-loop 总数在可计数 variant 中为 `17–22`，baseline 为 `21`。

component `0/1/5/7/8/12` 的某些 phase 能把本组件 witness 降到 `0`，但分别留下其他
组件 self-loop，或引入 valence / parent-boundary；component `10/11/13` 的唯一替代
phase 未减少其本组件 witness，component `22` 仅从 `2` 降到 `1`。这说明 phase 是真实
自由度，但**单组件 coherent phase 不是闭合算子**，并且至少四个二相组件在冻结父网格
上不存在能单独清零本组件 witness 的 phase。

证据：

- `target/case9-phase-inventory-1785348325/single-component-phase-probe.json`
  与 `single-component-phase-probe-repeat.json`
- SHA256：
  `5dd3ec1845f330ea4e6a660b3615c7a8d9d356c4e767f72e38ea452dd22318a8`
  （两次逐字节一致）。

该实验仍不把联合 phase 域声明为 UNSAT：不同组件同时改相位可能改变全局 seed closure，
而父层重建还会改变 component 划分。但它已经否定“逐组件挑一个局部可行 phase，再做
笛卡尔积”的简单分解，因此不再为 `3072` 域增加多 override 生产代码。

### 周界分量独立 triple-offset 完整枚举

历史 `0/6` 周界方向/相位实验只让全部周界一起改变，不能排除不同闭合周界分量各自选择
三元组起点。当前原型因此只增加一个默认不执行的 exact 诊断入口：

- 每个闭合周界分量只允许 offset `0/1/2`；
- offset 只旋转该分量的有序周界，不改变 selected mask、周界方向、长度或 hard demand；
- 旋转后的分量仍直接送入现有 `perim_fill3_method_c` 与完整 emitter，没有第二套模板语义；
- offset 全为 `0` 时，输出与现有 canonical materialization 完全相同，由单元测试锁定。

在冻结 checkpoint `1c8182224690201495f9f8fd1eab4a969576b9d3e451bf8a1bf54b2d866486ba`
上，21 个 self-loop 见证落入 `10` 个周界分量：

`[0, 4, 6, 7, 9, 19, 22, 27, 28, 29]`。

保持其余 26 个无见证分量为 canonical offset，按 Hamming weight、分量索引和 offset
固定顺序穷尽 `3^10 = 59,049` 个 assignment。每个 assignment 都调用完整 emitter：

- `SAT`：`0`；
- `TransitionPatch`：`39,366`；
- `Valence`：`19,683`；
- 未分类错误：`0`。

两次独立 release 重放的证据 JSON 逐字节一致，SHA256 均为
`73139d40d4367c98f04c6a5b79b9e792dd5fa6e88312167ee0e52de3539560ef`：

- `target/case9-phase-inventory-1785348325/perimeter-component-offset-full-a.json`
- `target/case9-phase-inventory-1785348325/perimeter-component-offset-full-b.json`

并行重放时单次完整枚举约 `3291 s`；该时间只作 release 诊断成本记录，不写入哈希证据。

该结果的严格范围是：**冻结的 `prepared_selected_faces` 加十个见证周界的独立
triple-offset 域为 `PATCH_UNSAT`**。它不证明原生 Case 9、其他 selected mask、联合
seed/phase/template 域或 Method-C canonical 模板族整体 UNSAT，因此 Case 9 对外状态仍为
`INCOMPLETE`。生产选择、repair、Tri/Hex 输出和 NetCDF 格式均未改变。

### 当前 canonical phase 的全 legal-seed 上界实验

为了验证“只要继续增加当前 phase 的 seed，最终总能到达合法状态”，冻结 checkpoint
随后把当前 phase 的 `2470/2470` 个 `legal_seed_ids` 全部选中；baseline 仅选中 `109`
个。该操作保留全部 hard demand，并把选中面增至 `15,474`，但在 exact materialize 前
即失败：

- 状态：`preflight_failure`；
- 失败类：`non_triplet_perimeter`；
- 21 条周界中长度 `40` 与 `58` 的两个分量不能按 transition triple 分组，且继续填充
  会越过 parent boundary。

两次独立 release 重放逐字节一致：

- `target/case9-phase-inventory-1785348325/all-legal-seeds-a.json`
- `target/case9-phase-inventory-1785348325/all-legal-seeds-b.json`
- SHA256：
  `243ef21bc5e3bfaddc0ec9f02e61385ee72553b8aa062ec3c40e637d8f994d4d`

这不是 Case 9 UNSAT 证明，也不排除某个 legal-seed 子集可解；它证明的是更窄但关键的
事实：`legal_seed_ids` 只表达 phase/alignment 可放置性，并不构成对 Method-C 全部合法性
约束向上闭合的状态集。因此后续不能把“全选”当作合法上界，需求正则化若继续，必须是
形态感知的 canonical placement 选择并由 exact emitter 验证，不能退化为统一膨胀。

### hard face obligations 的统一膨胀反例

为区分“直接 grow selected mask”与“扩大约束后重新选择”，M0-only 探针从 checkpoint
的 `112` 个 hard face obligations 起步，每圈只沿同一 parent level 的 W-face 邻接增加
需求；每个 ring 都从冻结的 M/U target levels 重新运行现有 component/phase 选择、
non-triplet preflight 和 exact emitter。它不删除或投影任何 15″ hard demand。

结果：

| ring | hard faces | selected seeds | selected faces | self-loop | 结果 |
|---:|---:|---:|---:|---:|---|
| 0 | 112 | 109 | 4227 | 21 | TransitionPatch |
| 1 | 393 | 118 | 4392 | 16 | TransitionPatch |
| 2 | 885 | 169 | 5621 | 13 | TransitionPatch |
| 3 | 1536 | 167 | 5761 | 12 | TransitionPatch |
| 4 | 2326 | 254 | 7793 | 30 | TransitionPatch |
| 5 | 3228 | 310 | — | — | non-triplet preflight failure |
| 6 | 4229 | 396 | 10903 | 32 | TransitionPatch |

两次 release 重放逐字节一致：

- `target/case9-phase-inventory-1785348325/face-demand-dilation-a.json`
- `target/case9-phase-inventory-1785348325/face-demand-dilation-b.json`
- SHA256：
  `1ede2a13c5cf532cc58ae520d6306160f7c34ba6ed0c8f3395cdd8bb78a771e2`

uniform obligation buffer 确实能把见证从 `21` 降至 `12`，但第 4 圈后反转，第 5 圈还
切换成 non-triplet；`0/7` assignment 为 SAT。因此它只能作为需求形态敏感性的证据，
不能成为产品参数或闭包算法。若继续需求正则化，只允许在分量/形态级形成新的有限
placement 变量并由 exact emitter 验证；不得恢复固定圈数 grow。

### hard-demand 分量形态与单分量 buffer

同层 W-face 邻接普查显示，checkpoint 的 `112` 个 hard face obligations 被切成 `90`
个连通分量：

- 单 face：`78`；
- 两个 face：`8`；
- 三个及以上：`4`；
- 最大分量：`6` faces。

这给出了此前“15″ 需求高度碎片化”的直接离散证据。随后对 90 个分量逐一只增加该分量
的一圈同层 hard obligations，每个 variant 均从原始 target levels 重新选择并 exact
materialize：

- `SAT`：`0/90`；
- `74` 个 variant 的 self-loop 不变或增多；
- `16` 个 variant 把 self-loop 从 `21` 降至 `17–20`；
- 最优单分量为 component `13`：只增加 `3` 个 hard faces，self-loop 降至 `17`，
  但仍为 `TransitionPatch`；
- component `2` 降至 `18` 时转为 `Valence`，说明跨失败类干扰仍存在。

两次 release 重放逐字节一致：

- `target/case9-phase-inventory-1785348325/component-dilation-a.json`
- `target/case9-phase-inventory-1785348325/component-dilation-b.json`
- SHA256：
  `54a52b73c2c83ce801be9c2ba1b814788a3d34bb649720f9cd05fa8f0c82d9fa`

因此“按某个孤立分量加一圈”也不是解法。数据同时支持继续研究形态感知的**多分量聚合**
而不是统一 buffer：输入中 `78/90` 分量是单点，正是 AMR clustering 要处理的形状。
该结论仍只用于 M0 变量域设计；没有新增生产参数或选择分支。

### 多分量逐步聚合的下降路径与局部极小

为判断单分量结果能否组合，默认 ignored 的诊断器在每一步：

1. 按当前 hard-demand mask 重新计算同层连通分量；
2. 对每个分量分别增加一圈 hard obligations；
3. 完整重跑 selection、preflight 与 exact emitter；
4. 只接受仍为 `TransitionPatch` 且 self-loop 严格下降的候选；
5. 按 `(self-loop, added faces, stable first-face id)` 确定性选择。

该规则不是生产求解器，只用于寻找是否存在可观察的下降路径。冻结 Case 9 上得到：

| 累计步骤 | hard faces | self-loop |
|---:|---:|---:|
| 0 | 112 | 21 |
| 1 | 115 | 17 |
| 2 | 118 | 14 |
| 3 | 121 | 12 |
| 4 | 124 | 10 |
| 5 | 127 | 9 |
| 6 | 130 | 8 |
| 7 | 133 | 6 |
| 8 | 137 | 5 |
| 9 | 141 | 4 |

九次选择使用稳定 parent-face IDs
`[62012, 1869, 71484, 135541, 79215, 86568, 87861, 63691, 92772]`，
累计只增加 `29` 个 hard faces。随后对当前 `88` 个分量逐一再试一圈：

- `72` 个保持 self-loop `4`；
- 其余分别恶化到 `5–10`；
- `SAT`：`0`；
- 没有严格下降候选，状态为 `LOCAL_MINIMUM`。

最终段两次独立 release 重放逐字节一致：

- `target/case9-phase-inventory-1785348325/greedy-component-dilation-c.json`
- `target/case9-phase-inventory-1785348325/greedy-component-dilation-c-repeat.json`
- SHA256：
  `3f6fe9d31af125b104bf8145eced4e1f1a7b91ebbeecfae5a9cfd1c534c3bf39`

这证明多分量形态调整确实是有效杠杆，但也证明“最小 self-loop 贪心”不完备。不能通过
允许等值/恶化步、调 tie-break 或增加圈数把它包装成产品算法；下一步若继续，必须把
plateau 上的联合 placement 作为有限约束域搜索，或采用有明确 grid-efficiency/边界
契约的聚类器，并继续由整网 exact emitter 判定 `SAT`。

### plateau 二元组合的成本门

局部极小处有 `72` 个单步保持 `4` 个 self-loop 的等值动作，完整二元域为
`C(72,2)=2556`。诊断原型对每个组合重新运行现有 demand selection、preflight 与整网
exact emitter，不使用近似剪枝：

- 单组合冒烟：`7.35 s`，仍为 `TransitionPatch`，self-loop `4`；
- 12-way 确定性分片运行约 `84 min` 后，仅一个完整分片结束；
- 完整分片检查 `213` 个组合，全部仍为 `TransitionPatch`，self-loop `4`；
- 合计已检查 `214/2556`，`SAT=0`。

证据：

- `target/case9-phase-inventory-1785348325/plateau-pair-smoke.json`
  （SHA256 `7e2cd571b28a68ad379ee5b0c3b620830e720a6e259d4cba75564402e9daf1ce`）；
- `target/case9-phase-inventory-1785348325/plateau-pairs-12-shard-8.json`
  （SHA256 `8a47d142e9ac1739e31043f6dee9c6a21d95a3c4351a715c42452e29729e6783`）。

该结果不是 `PATCH_UNSAT`：其余 `2342` 个组合未检查，且变量域只包含需求分量的一圈
膨胀。由于完整域只能证明一个很窄的启发式，而不能判定 Case 9 或 canonical 模板族，
运行已主动停止，小时级探针代码亦未保留。后续不得升级到三元组合；应直接研究映射到
现有 emitter 的同层 canonical template-placement 变量。

### 保留 hard demand 的图形态学闭运算

作为 AMR `tag → regularize → quantize` 架构的最小反证，M0-only 原型在同一 parent
level 的 W-face 邻接图上执行半径 `r` 的“先膨胀、后腐蚀”，最后与原始 hard mask
取并集，因此不会删除、降采样或 coarse-project 任一 15″ hard obligation。每个半径
都重新运行现有 selection、preflight 和整网 exact emitter。

| radius | regularized hard faces | components | selected seeds | selected faces | self-loop | result |
|---:|---:|---:|---:|---:|---:|---|
| 0 | 112 | 90 | 109 | 4227 | 21 | TransitionPatch |
| 1 | 114 | 88 | 109 | 4227 | 21 | TransitionPatch |
| 2 | 117 | 90 | 109 | 4227 | 21 | TransitionPatch |
| 3 | 132 | 89 | 107 | 4193 | 20 | TransitionPatch |
| 4 | 142 | 88 | 107 | 4193 | 20 | TransitionPatch |
| 5 | 151 | 88 | 97 | 3916 | 17 | Valence |
| 6 | 157 | 87 | 97 | 3916 | 17 | Valence |

两次 release 运行分别约 `38.21 s` 和 `36.40 s`，JSON 逐字节一致：

- `target/case9-phase-inventory-1785348325/face-demand-closing-a.json`
- `target/case9-phase-inventory-1785348325/face-demand-closing-b.json`
- SHA256：
  `cb70a0976e6aa05755bd421452bd816120f30848d4092cedf7aff37c62fbbf9e`

闭运算只合并了 `2–3` 个分量，未改变主要碎片结构；半径增大后又出现跨失败类干扰。
因此固定半径形态学闭运算不是 Case 9 解法，也不应成为 namelist 参数。实验代码已删除，
只保留证据。若继续需求正则化，必须使用具有显式 cluster/grid-efficiency 契约的
形态感知算法，并继续保持“只增加 hard obligations、exact emitter 作最终判定”。

### 原生 15″ hard obligation / canonical seed 支撑图

为判断“按 hard-demand 支撑分量求解”是否足够小，使用冻结原生 15″ checkpoint，对
`112` 个 hard faces 与当前 `2470` 个 legal canonical seeds 建立精确二部图。每个 seed
足迹直接复用 emitter 前的 same-level rad3 映射；没有重采样、删除或增加 hard demand，
也没有调用穷举器。

结果：

- unsupported hard faces：`0/112`；
- 与至少一个 hard face 相交的 legal seeds：`870/2470`；
- 支撑连通分量：`44`；
- `31/44` 个分量不超过 `20` 个 seed，`13/44` 个超过 `20`；
- 最大两个分量分别为 `65` 和 `50` 个 seed；
- 这些分量中当前 selected seeds 合计 `83`，而整张选择掩码有 `109` 个 seed；其余
  `26` 个不直接覆盖 hard face，却用于 phase/transition/connectivity 支撑。

两次 release 普查均在约 `0.14 s` 完成，JSON 逐字节一致：

- `target/case9-phase-inventory-1785348325/seed-support-graph-a.json`
- `target/case9-phase-inventory-1785348325/seed-support-graph-b.json`
- SHA256：
  `4c14cbe225670b325314d5877c9f81485aeebc5bb9da313852a64b22bbeb1c46`

这排除了“hard demand 没有 canonical placement”这一解释，但也否定了直接逐覆盖分量
穷举的工程可行性：单个 `65` 变量分量已有 `2^65` 个朴素 assignment，更重要的是
覆盖图看不见那 `26` 个 topology-only seeds，也不能表达周界合并、self-loop、Valence
和 TransitionPatch 耦合。已冻结的真实 topology witness patch 仍有最高 `154` 个候选，
所以覆盖分量不是合法的独立求解边界。

一次性普查探针及其批量 footprint 暴露接口均已删除，生产代码未改变。下一步若继续
求解器路线，必须以完整 transition-band topology constraint graph 分解，而不是以
hard-coverage 二部图分解；在有可靠分解前不启动指数搜索。

### 冻结 transition-band 变量—约束图

在上一节支撑图基础上，进一步把当前 checkpoint 已知的约束建成保守 hypergraph：

1. `44` 条 hard-coverage 支撑分量 hyperedges；
2. `10` 条 exact preflight topology-witness patch hyperedges；
3. 共享 seed 的 hyperedges 合并为同一耦合分量。

这一步只读取冻结 JSON，不运行 materializer、不修改选择，也不引入替代 phase 或新模板。
结果：

- coverage-relevant seeds：`870`；
- topology-patch candidate seeds：`380`；
- 两类约束并集：`1062`；
- coverage-only seeds：`682`；
- topology-only candidate seeds：`192`；
- 耦合分量：`38`；
- 最大五个分量变量数：`241, 105, 87, 57, 52`；
- 最大分量把 `5` 个 coverage constraints 与 topology patch `4` 合并；
- 仍有 `10` 个当前 selected seeds 不落在上述已知约束并集中，它们保持固定，不能据此
  宣称图已覆盖未来可能出现的失败依赖。

两份确定性证据逐字节一致：

- `target/case9-phase-inventory-1785348325/topology-constraint-graph-a.json`
- `target/case9-phase-inventory-1785348325/topology-constraint-graph-b.json`
- SHA256：
  `614e5bc64b1c4d7130061aef082fdc3b358b2fb390690a69496d87487345407a`

该图是当前 fixed-phase checkpoint 的保守约束闭包，不是完整模板规格：它不枚举
checkpoint 外的 phase classes，也不声称修复当前见证后不会产生新依赖。因此它能安全
否定“当前约束天然分成许多可穷举小簇”，但不能证明模板族 UNSAT。`2^241` 说明即使
按现有已知约束分解，朴素有限枚举仍无工程意义；下一步必须先获得可增量判定的 symbolic
topology constraints / 可验证 nogood，或改变需求/模板规格，不能直接开始求解器编码。

### self-loop / perimeter 的精确 seed 依赖

`legalization_preflight_from_selected_faces` 已经构造 legal seed 的 exact same-level rad3
足迹，只是此前只把依赖聚合到 patch。现在在不改变选择和 emitter 的前提下，preflight
额外保存：

- 每个 `MethodCTransitionSelfLoopCheckpointWitness` 的 `candidate_seed_ids`；
- 与 `perimeter_lengths` 同索引的 `perimeter_candidate_seed_ids`。

字段带 `serde(default)`，旧 checkpoint 仍可读取；新输出顺序由 stable seed ID 和周界
索引确定。成功网格回归同时锁定每条周界的候选列表严格递增。

原生 15″ Case 9 重算结果：

- `21` 个 self-loop witnesses，候选数范围 `5–45`；
- 排序后的 witness 规模：
  `5,5,8,8,8,10,11,13,14,14,16,16,16,18,20,22,31,35,41,43,45`；
- `36` 条周界的候选数范围 `13–381`；
- 当前十条 witness 周界的候选数：
  `151,87,111,26,381,63,24,42,13,36`；
- witness 候选并集仍为 `380` seeds，全部周界候选并集为 `2220` seeds；
- 把 coverage 与逐 witness hyperedges 合并后，最大分量仍有 `208` 个变量。

两份 release 证据逐字节一致：

- `target/case9-phase-inventory-1785348325/symbolic-dependencies-a.json`
- `target/case9-phase-inventory-1785348325/symbolic-dependencies-b.json`
- SHA256：
  `08c239641d9f1e5d22b09dd872e26d15dede0773f3d13229ad56ad9eaae6932d`

这说明 self-loop 见证本身已经局部化，但 non-triplet / 周界拓扑仍是长程约束；只按
witness 做回溯会再次把失败转移到周界约束。下一步只实现“给定 seed assignment，
增量或表级判定受影响周界是否合法”的 exact symbolic predicate；在该谓词与现有
preflight 对拍前，不实现传播器或全局搜索。

### 完整 seed assignment 的 symbolic / exact 有界验证

已复用生产路径中的 rad3 展开、凹角闭合、父层一致性、hard coverage、周界游走和
self-loop 普查，新增完整 canonical seed assignment 的只读 symbolic check；它不调用
emitter。另保留一个完整 assignment 的 exact materialization check，供局部搜索候选
嵌回整网后作最终判定。成功网格测试同时锁定 symbolic 结果与 preflight 一致，且 exact
materializer 接受原 assignment。

在冻结原生 15″ Case 9 上，对十条 witness 周界候选并集中的 `934` 个 seed 各做一次
相对 baseline 的单翻转：

- symbolic 合法：`894`；hard coverage 拒绝：`13`；perimeter topology 拒绝：`27`；
- `513/894` 个合法状态产生 non-triplet 周界；
- `45` 个状态发生周界分量合并/分裂，另有 `108` 个状态改变了声明目标周界之外的
  周界索引；
- `96` 个状态保持全部周界三元组且把预测 self-loop 从 `21` 降到 `17–20`；
- 这 `96` 个候选经整网 exact emitter 验证后 `SAT=0`：
  `94` 个 `TransitionPatch`，`2` 个 `Valence`。

两轮 symbolic 与 exact 结果分别逐字节一致：

- `single-toggle-symbolic-{a,b}.json`：
  SHA256 `ef35f173d03ae9192e46d25588dabe564fcc6fa2a357004622b200972bd24519`；
- `single-toggle-exact-survivors-{a,b}.json`：
  SHA256 `e14932c65f0476de4220c15b84ff7985af4a1664851534bc514eaae28e2a7fc2`。

因此不能把单 seed 对周界长度的影响建模成简单、独立、可相加的 mod-3 delta；周界拓扑
本身会重排。不过 symbolic check 可作为便宜且精确的前置 oracle，减少不必要的 emitter
调用。

最后仅对上述 `96` 个“单步有改善”动作做完备二元组合，共 `4560` 个 assignment：

- non-triplet：`130`；
- self-loop 未改善：`172`；
- 进入 exact emitter：`4258`；
- exact `TransitionPatch`：`4122`；`Valence`：`136`；`SAT=0`；
- 最佳三组把预测 self-loop 降到 `14`，仍全部为 `TransitionPatch`。

两轮结果逐字节一致：

- `improving-toggle-pairs-{a,b}.json`
- SHA256 `e7b89a67cc9396dbf408a0cd4c3b3d17f6f07486db0fa478795f3b63679ac4c6`

该二元域已穷尽，但只证明“当前 fixed-phase baseline 上，由单步 symbolic 改进动作构成的
一阶/二阶邻域无整网 SAT”。不得外推为 Case 9、legal-seed 域或 canonical 模板族
UNSAT。三元组合为 `C(96,3)=142,880`，而二元 exact 已耗时约 `128 s`；其证明范围仍然
过窄，故主动停止，不继续扩大 Hamming 阶数，也不引入 SAT/CP 依赖。

### 严格下降、局部二元跳转与跨层支撑耦合

在同一冻结 checkpoint 上，随后做了三个仅用于诊断的有界实验；相关临时探针和环境变量
分支均已删除，未接入生产路径。

第一步按 stable seed ID 决定平局，只接受能严格减少 self-loop 数的单 seed 动作。动态
重算候选后，预测 self-loop 从 `21` 依次降到：

`17 → 14 → 12 → 10 → 9 → 8 → 7 → 6 → 5 → 4 → 3 → 2 → 1`。

在 `1` 处仍有 `26` 个候选 seed，但没有任何单步严格改善，故这是当前动作集下的确定性
局部极小值，不是 SAT。两轮输出逐字节一致：

- `target/case9-phase-inventory-1785348325/strict-descent-{a,b}.json`
- SHA256 `cd5ba1770a06f931bcf794c2747d01a3443011ae782e93ec19877a56742314f2`

第二步完整扫描该局部极小值的 `C(26,2)=325` 个二元动作：

- `192` 个产生 non-triplet；
- self-loop 直方图为 `0:39, 1:40, 2:16, 3:3`；
- 另有 hard coverage 拒绝 `23` 个、perimeter topology 拒绝 `12` 个；
- `39` 个 zero-self-loop assignment 全部进入整网 exact emitter，`SAT=0`；
- `39/39` 均失败为 parent-boundary，而不是同层 self-loop：
  `34` 个需要 `28` 条稳定父面 lineage，其余需要 `31–34` 条。

两轮结果逐字节一致：

- `target/case9-phase-inventory-1785348325/local-minimum-pairs-{a,b}.json`
- SHA256 `8f9ab6c5fa2961cd01b77e1b79724bd48d3ab7bc52c46e1b4006ba5a956aa056`

这证明局部 seed 动作能消除当前同层 self-loop，但会暴露跨层父支撑义务；因此
“同层合法化”和“父层支撑”不是可独立依次解决的两个问题。

第三步曾用临时、默认关闭的诊断分支验证最直接的交替算法：

1. 运行既有跨层父支撑闭包；
2. 对同层 self-loop 做严格下降，必要时做有界二元跳转；
3. zero-self-loop 后请求缺失父面 lineage，回滚重建；
4. 重采样并重复。

真实原生 `86400×43200` 15″ 输入的 release 运行记录：

- 初始跨层请求：`163, 33, 4, 37, 13, 8`，合计 `258`；
- legalization 额外父层请求：`28, 6, 5, 6, 3, 4, 3, 4, 6, 6`，合计 `71`；
- 启动同层求解后新增父层请求共 `129`；
- 连续 `10` 次把当前状态降到 zero-self-loop，重建后仍反复回到约 `12` 个 self-loop；
- 第十轮后人工停止，状态为 `INCOMPLETE_MANUALLY_STOPPED`，未得到整网 SAT。

证据：

- `target/case9-seed-legalization-exact-filter-1785382605/summary.json`
- SHA256 `2dd93907de71b72505a3eb8ae0279ac4f914c868207c1cb8558a271f51e9c3ac`
- `run.log` SHA256
  `49586320bd3960bf1281ace8bffb95e73222954424968cf5bc696d73713a3165`
- executable SHA256
  `9f0d3887b1ff47a36f3e9da19e8abe67fead0525ce1cbce6844a12fb135d8131`

结论不是 Case 9 UNSAT，而是拒绝这条交替贪心实现：父支撑请求会移动同层合法性前沿，
形成持续向外传播的 support treadmill。下一步不得继续增加 Hamming 阶数、支撑圈数或
repair 轮数；若继续求解器路线，最小变量域必须同时包含同层 canonical placement 与
父层 support choice，并先给 transition-band / 过度细化成本设明确上界。若该联合域在
每次重建后仍扩张，则停止内核搜索，转向显式、保留 hard coverage 的需求聚类契约。

### 联合域局部性判定

无需再次运行网格生成器，使用上述局部极小二元报告和冻结 checkpoint 对最低父支撑集合
做拓扑统计：

- `39` 个 zero-self-loop assignment 中，`34` 个达到最小父支撑数 `28`；
- 这 `34` 个 assignment 的父面 lineage 集合完全相同，故 `28` 条 lineage 是共同义务，
  不是 `28` 个可独立开关的局部变量；
- `28` 个父面在当前父面邻接图上分成 `6` 个诱导分量，大小为
  `8, 6, 5, 4, 4, 1`；
- 支撑面之间的最短路图直径为 `353` 条边；它不是围绕单见证的局部邻域；
- 全部支撑面均为 `mrlw=1, ngr=2`。

两次只读统计逐字节一致：

- `target/case9-phase-inventory-1785348325/joint-domain-locality-{a,b}.json`
- SHA256 `ccdcba0ca70efe49eb92693dbe0d82be11a1e81fcb28cc60a3253ec393d46b55`

这触发 §6 停止条件 2：当前 zero-self-loop 状态与父支撑的联合约束不是单一有限局部
patch。结合交替重建中持续移动的支撑前沿，继续实现 witness-local 联合求解器没有局部化
收益。Case 9 仍为 `INCOMPLETE`；下一步转入新规格轨道，定义只增加义务、绝不删除或
粗投影 15″ hard demand 的形态感知聚类/placement 契约，并仍以整网 exact emitter 作为
唯一 SAT 门。

### 父支撑前缀与 support-free 局部域

重放严格下降路径时逐步计算 assignment-aware 父支撑，得到：

- baseline：`0`；
- seed `31158` 后：`+8`；
- seed `37196` 后：再 `+9`；
- seed `34963` 后：再 `+5`；
- seed `56069` 后：再 `+6`；
- 其余九个单 seed 动作以及最终二元 zero-self-loop 动作均不增加父支撑；
- 整条路径父支撑只增不减，最终为 `28`。

两轮重放逐字节一致：

- `target/case9-phase-inventory-1785348325/support-prefix-{a,b}.json`
- SHA256 `149f42e54d0586fd0bc56fd882ffd72a84ebb9c4f8bd01af133d22bcaa3ed2dc`

这说明 `28` 条义务不是最后一个二元动作造成的，而是四个分散的早期同层选择逐步引入。
因此只在最后一个 self-loop 周围联合求解父支撑会切错问题边界。

随后增加一个更严格的诊断约束：任何中间 assignment 都必须保持父支撑集合为空。固定
tie-break 的单步下降先把 self-loop 从 `21` 降到 `3`；当前局部域内：

- 完整 `23` 候选二元域 `253` 项后，存在动作 `[5522,5523]`，降到 `2`；
- 新 `15` 候选域的二元 `105` 项无改善，完整三元 `455` 项中唯一改善动作
  `[34748,34970,69719]`，降到 `1`；
- 最后 `7` 个候选的全部 `127` 个非空子集均已检查，无 support-free 改善；
- 状态为 `SUPPORT_FREE_LOCAL_DOMAIN_EXHAUSTED`，不是 `UNSAT`。

两次 release 重放逐字节一致：

- `target/case9-phase-inventory-1785348325/support-free-descent-exhaustive-{a,b}.json`
- SHA256 `df66913262a409e11e3bc03ac910b3af44535244034d110bc60cbff0f09ad749`

一次性探针已删除。该结果只证明这条确定性下降路径的最终七变量域必须引入父支撑或离开
当前候选域；它不证明其他早期分支不存在 support-free SAT。

### 保留 15″ hard obligations 的聚类连接成本

使用冻结父面邻接图，只测量 `112` 个原生 hard faces 的连接成本；没有删除需求、没有
重采样或 coarse projection，也没有把连接结果交给生产选择器。原始 `90` 个分量中
`78` 个为单 face。按确定性最小生成森林统计：

| 每次连接最多新增面 | 剩余分量 | 新增面并集上界 | hard/augmented efficiency 下界 |
|---:|---:|---:|---:|
| 1 | 79 | 11 | 91.1% |
| 3 | 65 | 45 | 71.3% |
| 5 | 55 | 90 | 55.4% |
| 7 | 50 | 123 | 47.7% |

最近邻连接成本最大为 `93` 个面；若把全部分量接成一棵树，单条连接最多需要 `175`
个中间面，去重前连接面总数为 `1970`。两次统计逐字节一致：

- `target/case9-phase-inventory-1785348325/hard-demand-cluster-connectivity-{a,b}.json`
- SHA256 `ccabb1e07f69f8024ae2bd3ca6497dfbaa4a6ed042983b7640514474238ac4b4`

因此不能把 Berger–Rigoutsos 的“聚类”简化成把全球离散 hard 分量连成少数大块；那会
用大量过度细化换连通性。若继续该路线，只允许对稳定、相互独立的局部支撑分量分别生成
shape-aware placement，并把 grid efficiency 作为报告和准入量，不把某个经验阈值直接
写入生产。

### 增加最大细化层级的反例

使用相同的原生 `86400×43200` landcover、关闭 coarse HField projection、保持现有
Method-C 模板与 `EARTHMESH_M0_CROSS_LEVEL_SUPPORT=1`，只把：

- `max_passes: 3 → 4`；
- `hfield.max_level: 0 → 4`；
- `max_iter_cal: 3 → 4`。

release 运行在 pass 2 依次请求父支撑 `163, 33, 4`，随后给出与 level 3
`phase-inventory` 运行逐项相同的失败：

`perimeter lengths [63,18,18,18,69,36,73,24,18,21,105,18,18,30,18,18,18,18,18,18,42,18,27,18,18,18,18,18,18,18,18,18,18,30,18]`

无法在不越过 parent boundary 的条件下分成 transition triples。

证据：

- `target/case9-native-level4-1785387349/project.yaml`
  SHA256 `948123d59fbefb4c6d469474ac6aca779080af067087f7e4a763d0bcecf8a5d4`；
- `target/case9-native-level4-1785387349/run.log`
  SHA256 `d8a0130e37674b152e423932afd6cfaab2d7919e17fcba63218329c8cebe2cca`；
- executable SHA256
  `25ac6ba174f3e5a289d03e580f7bd51b07df13bb8d44afd236419ff3685b46ec`。

原因是原生 landcover 路径按当前 `pass` 在当前层面上重新读取全部源像元并生成
`face_demand`；新增最大层级只有在较低 pass 已成功 materialize 后才存在。当前失败发生
在 pass 2，所以 pass 4 尚未被创建，无法反向修复 pass 2。level 5 在现有顺序下同样先
经过这道 pass-2 门，故不再重复运行。

这也进一步区分了几何与拓扑：弹簧只移动 M 点坐标，不能改变重复 U-edge、non-triplet
周界或价数等组合关系；它不能作为该失败的修复。若未来允许局部可变 mrow，则属于新增
transition 模板规格，不是“多跑一层”或调整现有参数。

### 提高基础分辨率的对照

使用相同原生 `86400×43200` landcover、关闭 coarse HField projection、保持
`max_level=3` 与 `EARTHMESH_M0_CROSS_LEVEL_SUPPORT=1`，只把 `NXP=81` 提高到
`NXP=162`。release 运行仍在 pass 2 退出：

- 父支撑请求：`38+13=51`，相对 `NXP=81` 的 `200` 条减少 `74.5%`；
- 失败周界：`10` 条、总长度 `241`，相对 `35` 条、总长度 `952` 明显缩小；
- 仍存在长度 `34` 的 non-triplet 周界，无法在不越过 parent boundary 的条件下闭合；
- 退出码 `2`，wall time `198 s`。

证据：

- `target/case9-native-nxp162-1785388183/project.yaml`
  SHA256 `b1ce2b0f47c28e5896cc4aeee2700d07e206b9444aae38bfeaafeaae8c66cae2`；
- `target/case9-native-nxp162-1785388183/run.log`
  SHA256 `ddd952ebe614e98966f6e8defabd2b29a289be58745b08a9dfab353f356621cf`；
- executable SHA256
  `25ac6ba174f3e5a289d03e580f7bd51b07df13bb8d44afd236419ff3685b46ec`。

因此基础分辨率是问题规模的杠杆，不是完整修复。下一规格保持全部 15″ hard
obligations，只允许按经证明独立的局部支撑分量增加 shape-aware placement；grid
efficiency 只报告，不作为删除、合并或粗投影 hard demand 的阈值。最终 `SAT` 仍只由
整网 exact emitter 与全部 hard coverage 门共同确认。MUS/MCS 仅用于定位相对于当前
canonical 模板的冲突核心和非破坏性修复方向，不得据此删除 hard obligation。

同一配置又保存了 pass-2 合法化 checkpoint：

- checkpoint SHA256：
  `d2693b7d79acc4816a7bfa41db10f57b55f16cd725441bd7c5cf49997fea3198`；
- 需求锚点 `18`，selected seeds `19`，selected faces `816`；
- 所有 `10` 条选择前周界均可被 `3` 整除；
- 剩余 `2` 个 self-loop 分成两个独立见证簇；
- 两个 patch 分别有 `35` 和 `16` 个 candidate seeds。

对较小的 `16` 变量簇，按 baseline Hamming 顺序精确检查前 `8192/65536` 个 assignment，
覆盖全部 Hamming weight `0–5` 和部分 weight `6`，仍为 `0 SAT`：

| 分类 | 数量 |
|---|---:|
| non-triplet perimeter | 5387 |
| exact TransitionPatch | 2293 |
| Valence | 177 |
| perimeter topology | 157 |
| hard coverage | 94 |
| patch boundary incomplete | 84 |
| unclassified | 0 |

证据报告：

- `cluster-1-enumeration-1024.json`，SHA256
  `c2271262839794dd6555367c7a8e1749c6fbac279df99c3cc3bd8a43465eceb7`；
- `cluster-1-enumeration-skip1024-next7168.json`，SHA256
  `302b7d0a89fb8120e366107066b7418a548922fc215ff14d22b556485dd85723`。

两段耗时分别为 `32 s` 和 `219 s`。结果仍是 `INCOMPLETE`，不能外推为 patch 或
Case 9 `UNSAT`；但继续完整扫描该簇约需半小时，且不能解决另一个 `35` 变量簇，因此
不再用朴素枚举扩大证据。

## 执行更新：编译式表传播 P0/P1 原型（2026-07-30）

- [x] 新增标准库 `Vec<u64>` 二值 extensional table 与跨表 GAC；
- [x] 新增 bounded exact patch-table compiler，唯一真相源仍为现有
  `legalization_patch_boundary_check` 与 whole-grid exact emitter；
- [x] patch 边界签名保留分量与有序外部弧；尚未完成边界证明的 hard/perimeter 拒绝
  只能产生 `INCOMPLETE`，不能产生 `PATCH_UNSAT`；
- [x] 冻结成功路径正对照通过，并验证重复编译结果一致；
- [x] NXP=81 Case 9 第 3 簇完整 `256/256` 检查保持 `INCOMPLETE`：
  `0 SAT`、`3` boundary incomplete、`137` non-triplet、`62` hard coverage、
  `54` TransitionPatch；
- [x] NXP=162 第 1 簇在 `max_variables=12` 下对 `16` 变量快速返回
  `INCOMPLETE`，没有启动指数枚举；
- [ ] 将 exact 表从完整 patch 编译收窄为可复用的 canonical 局部模板表；
- [ ] 以完整周界分量和跨层父支撑连接局部表，测量 GAC 后 residual width；
- [ ] residual width 未达到小规模搜索门槛前，不接入生产、不增加 solver 依赖。

证据报告及 SHA256：

- `target/case9-legalization-hard-obligations-1785323328/cluster-3-compiled-table-v0.json`
  → `791a1ba3904b4fb660d8f59b178271ee65f54d662b41aff09d7bef1a8ccaf84e`；
- `target/case9-native-nxp162-checkpoint-1785388485/cluster-1-compiled-table-v0.json`
  → `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

当前结论仍是：接口原型成立，Case 9 未闭合。完整 patch 真值表只是正对照与边界测量工具，
不是生产算法；继续扩大变量上限会重回已否决的指数枚举路线。

### 后续更新：关系复用与 residual width（2026-07-30）

- [x] exact 允许关系可脱离 mesh ID canonical 化并重新绑定到另一等长变量作用域；
- [x] GAC 后输出保守 residual connected-component width；
- [x] 成功正对照从 `4` 个 placement 变量删去 `1` 个值，最大 residual width 为 `3`；
- [x] 两个 Case 9 冻结检查点重放后仍为 `INCOMPLETE`，未出现指数回退；
- [ ] 完整有序周界分量约束尚未编码；
- [ ] witness-local self-loop 表不得单独用作终局剪枝。

最后一项是安全限制而非缺失优化：self-loop witness 的 triple/phase 由完整 ordered perimeter
决定，远处周界修改也可能重排它。下一实现目标是完整周界状态与 canonical 局部关系的联合
约束，而不是继续缩小 witness 半径。

### 后续更新：完整当前周界作用域（2026-07-30）

- [x] exact table 编译器报告当前完整受影响周界的 candidate seed 并校验 patch 是否覆盖；
- [x] census 缺失、作用域未覆盖或边界未闭合时，零 SAT 只能是 `INCOMPLETE`；
- [x] NXP=162 簇 0：patch `35` 变量，完整当前周界 `103` 变量，`0` 次枚举；
- [x] NXP=162 簇 1：patch `16` 变量，完整当前周界 `36` 变量，`0` 次枚举；
- [ ] 不构造 `2^103` / `2^36` 的完整周界 truth table；
- [ ] 下一实现改为 ordered-perimeter 接口动态规划或约束图分解。

证据：

- `cluster-0-compiled-table-v0.json` →
  `3ff780c7f0c45b5040529ebae7b0007ad2af8a811fa7c8a736f47e3855b83db5`；
- `cluster-1-compiled-table-v0.json` →
  `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

这一结果只否决“单个完整周界 truth table”，没有否决 Compiled Transition Legalizer。

### 后续更新：ordered-perimeter frontier（2026-07-30）

- [x] 复用 canonical rad3 footprint，测量 candidate seed 对完整有序周界点的 incidence；
- [x] 所有环切点确定性扫描，输出保守 `min_linearized_frontier_width`；
- [x] NXP=162 簇 0：`103` 个完整候选压到 frontier `31`；
- [x] NXP=162 簇 1：`36` 个完整候选压到 frontier `25`；
- [x] 该指标明确标为二进制 DP 上界，不冒充 treewidth，不参与 SAT 判定；
- [ ] 不实现 `2^31` / `2^25` 原始 seed-bit DP；
- [ ] 下一步只评估 canonical phase / mod-3 / template-class 的小状态编码。

当前决定：Compiled Transition Legalizer 仍未被证伪，但“完整周界单表”和“原始二进制
frontier DP”两种实现都已被实测成本否决。Case 9 仍未解决，生产路径未接入 legalizer。

### 后续更新：局部 M-ring 状态压缩（2026-07-30）

- [x] seed incidence 签名去重收益很低：NXP=162 两簇分别为 `90/103`、`36/36`；
- [x] 将 candidate footprint 投影到周界 M 点的本地 W-face ring；
- [x] 单点所有候选 OR 后的状态上界分别只有 `29`、`20`；
- [x] 该指标只读，不进入 SAT/UNSAT、剪枝或生产生成路径；
- [ ] 以 exact emitter 对拍“相邻局部占用 + phase/mod-3”的 canonical 转移；
- [ ] 转移关系未证明前，不实现整圈 DP。

决定：继续 Compiled Transition Legalizer，但不能把状态变量简化成局部 occupancy。
`29/20` 只说明该数据适合作为派生缓存。已有 canonical 回归证明：selected mask 与 perimeter
point 集合不变、仅循环相位旋转一位，就会从可 materialize 变成 self-loop/exact failure。
因此主状态必须保留完整有序周界、分量/三元组相位、parent U identity 和 `nest_wd` 状态；
单点/相邻点 mask DP 路线关闭。Case 9 仍未闭合。

更新后的确定性证据：

- NXP=81 cluster 3：
  `791a1ba3904b4fb660d8f59b178271ee65f54d662b41aff09d7bef1a8ccaf84e`；
- NXP=162 cluster 0：
  `3ff780c7f0c45b5040529ebae7b0007ad2af8a811fa7c8a736f47e3855b83db5`；
- NXP=162 cluster 1：
  `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

### 后续更新：完整 exact 状态复用率（2026-07-30）

- [x] 状态键包含完整 selected W-face 集和全部有序 perimeter checkpoint；
- [x] 成功正对照与 Case 9 bounded 枚举中，同状态混合 exact 结果均为 `0`；
- [x] NXP=81 cluster 3：`256` assignments 中仅 `57` 到达 triplet exact 状态，
  折叠为 `33` 个完整状态，最大重数 `4`；
- [x] 缓存理论上只节省 `24/256 = 9.4%` 的总枚举检查；
- [ ] 不实现 production exact-state cache；exact emitter 已实测不是瓶颈；
- [ ] 下一步针对 exact 前的 `non-triplet` 与 hard coverage 搜索约束。

决定：完整状态键是正确的，但复用不足以成为 Case 9 的求解杠杆。Compiled Transition
Legalizer 下一阶段必须减少进入 exact 之前的候选空间，而不是缓存 exact 结果。

### 后续更新：hard coverage 支撑域（2026-07-30）

- [x] 区分直接 rad3 footprint 覆盖与凹角闭包后的最终覆盖；
- [x] NXP=162 簇 0 的 `18/18` anchors 已由固定 patch 外 seed 直接覆盖；
- [x] NXP=162 簇 1 只有 3 个 anchors 依赖 patch，形成 3 个不同的直接支撑域，每个
  支撑域含 10 个候选；
- [x] 更严格的直接 footprint 子句仍保留 `65,408/65,536` assignments，只排除
  `0.195%`；
- [x] 上述数据只读，不改变三态结果，两个 NXP=162 簇仍为 `INCOMPLETE`；
- [ ] 不实现 direct-footprint coverage 剪枝：凹角闭包可能合法地补齐直接 footprint
  未覆盖的 anchor；
- [ ] 下一主线回到完整 ordered perimeter 的 non-triplet 与 TransitionPatch 联合约束。

决定：hard coverage 是必须保留的 exact hard gate，但不是 NXP=162 Case 9 的搜索压缩
杠杆。为它改写枚举器或增加近似 clause 不值当，也可能漏解。

### 后续更新：selected-mask BDD 先行测量（2026-07-30）

- [x] 完整周界局部 face 投影 OR-state 计数设 `100,000` 硬上限；
- [x] NXP=162 簇 0：投影在第 25 个变量后超过上限；
- [x] NXP=162 簇 1：投影为 10,338 个状态；
- [x] 加回候选完整 rad3 footprint faces 后，簇 0/1 分别在第 19/20 个变量后超过上限；
- [x] 统计只读，不进入 SAT、`PATCH_UNSAT` 或生产路径；
- [ ] 不实现未经闭包归一化的 direct-union BDD；
- [ ] 下一步只测 bounded prefix 的 closure-normalized full-mask 归并率；闭包可能合并
  direct 状态，因此当前数据尚未否决这一种 BDD。

决定：局部投影的表面压缩不足以支撑 direct-union BDD；closure-normalized BDD 尚待一次
有界归并率测量。Case 9 仍为 `INCOMPLETE`，生产行为未改。

### 后续更新：bounded prefix 闭包对拍（2026-07-30）

- [x] 两个 NXP=162 簇各取前 `12` 个有序候选，完整枚举 `4096` 个 assignment；
- [x] 簇 0：`3464` 个 direct mask → `3324` 个 closed mask，最大重数 `4`；
- [x] 簇 1：`1840` 个 direct mask → `1648` 个 closed mask，最大重数 `24`；
- [x] 两簇的 `cl(cl(A) ∪ B)` 与从 raw assignment 重算结果均不一致；
- [x] 普通单元测试锁定现有 exactly-one-missing 规则的非单调性质；
- [ ] 不实现以现有凹角例程为增量转移的 closure-normalized BDD；
- [ ] 不在本任务中改 production 凹角语义。

原因已经定位：完整 M-ring 会跳过，而恰好缺一个 incident face 会扩张 rad3 footprint；
增加选择可能关闭一次扩张。因此当前例程不是单调闭包算子，不能安全提供增量 BDD 转移。
从 raw assignment 每次重算仍正确，但 direct 状态爆炸且 exact emitter 不是瓶颈，不是高速
解法。

确定性证据：

- NXP=162 cluster 0：
  `4bdcfa9655abcf61d2e04077ce1f4fd8312103952373e3e6371696bde958a2eb`；
- NXP=162 cluster 1：
  `af33aa6ed71e538db49d96926a638070196384ff01af78f8efe086099dbc2aed`。

决定：关闭当前 closure-normalized BDD 实现路线，转向约束级同时建模；Case 9 仍为
`INCOMPLETE`，生产行为未变。
