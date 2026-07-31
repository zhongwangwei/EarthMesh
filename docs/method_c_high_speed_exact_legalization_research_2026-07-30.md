# Method-C 原生 15″ hard demand：高速、精确、可验证合法化方案调研

日期：2026-07-30  
仓库基线：`ebddef6ab7afe2f96c70c1cb639791efb50dba50`（工作区含未提交研究改动）  
范围：原生 `86400×43200` landcover、禁止降采样/粗投影/删除 hard obligation；保留
Method-C 主生成器与 Tri/Hex 对偶输出；解决 Case 9 一类 `non-triplet perimeter`、
`TransitionPatch`、self-loop/Valence 和跨层父支撑耦合。

## 1. 结论先行

### 1.1 最可信的主线

不是继续增强 64 轮 repair，也不是把 p4est、Berger–Rigoutsos 或通用 SAT 求解器直接塞进
生产。最可信的方案是一个 **Method-C 专用的“编译模板 + 约束合法化 + exact 验证”层**：

1. **离线编译局部 canonical 合法模板。**
   复用现有 canonical split 表、`legalization_symbolic_check` 和 exact emitter，枚举并
   归一化有限局部构型，生成“输入边界签名 → 允许的 placement/输出签名”真值表；不手写
   第二套网格语义。
2. **运行时只求解 transition band。**
   hard obligations 固定为必须覆盖；变量改为 canonical placement/phase/support，而不是
   任意 face mask。局部约束用位集表传播；non-triplet 周界和跨层父支撑按完整约束分量建模，
   不能按几何半径截断。
3. **先传播、后小规模搜索。**
   先用表约束的 generalized arc consistency（GAC）删掉不可能值，再按约束图分量/分隔集做
   确定性搜索。只有实测剩余宽度仍过大时，才评估 CP-SAT/伪布尔求解器；初版不新增依赖。
4. **整网 exact emitter 是唯一 SAT 门。**
   求解器只产生候选 assignment；嵌回完整状态后，现有 emitter、hard coverage、M/U/W 互惠、
   拓扑与产品门全部通过才报告 `SAT`。

这不是凭空发明。它由四条成熟路线拼成：

- Pitzalis 等把**非局部 pairing**从贪心/树规则改写成 placement 变量上的二元 ILP，并明确
  禁止 under-refinement；
- Tong/Zhang 对 **3-refinement** 穷举局部状态、用模板替换实现 O(n) 快速路径；
- Compact-Table 用可逆稀疏位集高效维护表约束的 GAC；
- 有界树宽 CSP 的复杂度对变量数线性、对分隔宽度指数，说明应优化约束图宽度，而不是继续
  按 Hamming 半径枚举 `2^n`。

### 1.2 它为什么比现路线更可能“彻底”

当前失败不是一个评分错误，而是**多个硬约束同时成立**的问题：修 `TransitionPatch` 的动作
常把周界变成 non-triplet；同层动作又会触发父层支撑，重建后合法性前沿继续移动。顺序式
repair 只能“先修 A、再破 B”。新的合法化器在同一个模型里同时看见：

- 每个 15″ hard obligation 的覆盖；
- canonical placement 与 phase；
- 同层 rad3 footprint；
- 跨层 parent support；
- 完整周界分量的三元组约束；
- self-loop、重复 U-edge、Valence、M/U/W 互惠、parent-mrlw；
- 模板边界是否与固定区相容。

因此它解决的是根因——**缺少同时求解全部离散合法性约束的选择层**——而不是再加一种
repair。

### 1.3 诚实边界

该方案能彻底解决“当前 canonical 模板族内存在合法 assignment、但贪心找不到”的问题；
它不能保证当前模板族本身总有解。若完整、已验证的 transition-band 模型返回相对于当前
模板规格的 `UNSAT`，正确下一步是：

1. 离线扩充 canonical transition 模板；或
2. 在固定边界内启用更细粒度的共形细分/合法化翻边；或
3. 对相应拓扑产品切换到独立生成器。

不能删除、降采样或 coarse-project 15″ hard demand 来伪造成功。

### 1.4 方案决策矩阵

| 方案 | 能否保留全部 15″ hard demand | 能否处理 non-triplet/跨层耦合 | 运行时前景 | 结论 |
|---|---:|---:|---:|---|
| 继续 64 轮 repair / grow | 是 | 否，已有反例 | 差 | 关闭 |
| 弹簧/坐标优化 | 是 | 否，作用维度错误 | 无关 | 关闭 |
| 仅 p4est 式 2:1 balance | 是 | 只覆盖跨层子约束 | 好 | 作为工作队列参考 |
| 仅 AMR 聚类/buffer | 是（只增时） | 不保证模板合法 | 好 | 只作 warm start |
| 仅局部模板查表 | 是 | 会漏长周界和跨层接口 | 很好 | 快路径，不可单独交付 |
| 全局 ILP/CP-SAT | 是 | 可以 | 取决于模型宽度 | 备选重型求解层 |
| **编译模板 + GAC/分解 + exact emitter** | **是** | **可以显式覆盖** | **最好平衡** | **推荐主线** |
| JIGSAW/NVB 等独立生成器 | 是 | 换一套表达解决 | 成熟 | 当前模板族无解时 fallback |

合法化发生在 Tri/Hex 输出视图分离之前，因此主合法化器仍应共享；Tri 与 Hex 的质量指标、
阈值和后续优化继续分视图处理，不能反过来污染共享拓扑选择。

## 2. 当前仓库事实：为什么必须换搜索表示

权威细节见 `docs/method_c_case9_legalization_tasklist_2026-07-29.md`。与架构选择直接相关的
证据是：

| 事实 | 含义 |
|---|---|
| 原生 15″ Case 9 仍在 pass 2 失败 | 2026-07-28 的 116/116 成功不能替代原生路径 |
| 65 次 exact emit 共 6.045 s，仅占完整运行 2.78% | emitter 不是主瓶颈；不应优先重写 emitter |
| NXP=81 已知约束图最大分量 241 变量 | `2^n` 朴素枚举无工程意义 |
| NXP=162 把父支撑请求减 74.5%，但仍有 non-triplet | 提高基础分辨率只缩小问题，不是通解 |
| max_level 3→4 仍在 pass 2 同处失败 | 增加尚未生成的更深层不能反向修复当前层 |
| 934 个单 seed 中 45 个会合并/拆分周界分量 | 单 seed 的 mod-3 影响不可当作独立可加 delta |
| 96 个单步改善动作的 4560 个二元组合，SAT=0 | 局部下降邻域不足 |
| 原生跨层/同层交替 10 轮仍回到约 12 个 self-loop | 顺序式交替形成 support treadmill |
| 15″ hard faces 112 个均至少有一个 canonical seed 支撑 | 不是“单个需求不可放置”，而是组合冲突 |
| NXP=162 小簇前 8192/65536 assignment，SAT=0 | 枚举器可运行，但增长率不接受 |

结论：必须同时减少候选空间并增强约束表达；单纯让 exact emit 更快、继续 grow、继续加层或
扩大 Hamming 半径都不够。

## 3. 外部算法中真正可借鉴的部分

### 3.1 最接近：Generalized Adaptive Refinement（Pitzalis 等，2021）

论文：[Generalized Adaptive Refinement for Grid-based Hexahedral Meshing](https://pizza1994.github.io/pdf/gen_adapt_grid_hexmeshing.pdf)；
[公开实现](https://github.com/cg3hci/Gen-Adapt-Ref-for-Hexmeshing)。

它与 Method-C 的结构对应关系很强：

| 论文问题 | Method-C 问题 |
|---|---|
| balancing：相邻层级差 | parent support / 层级差 |
| pairing：等尺寸簇各边必须为偶数，非局部 | 每条周界长度必须为 3 的倍数，非局部 |
| under-refinement 禁止 | 15″ hard obligation 不得删除/降级 |
| cell 变量导致规则过强 | 任意 face toggle / 固定 grow 过强或盲目 |
| 改用 grid-vertex placement 变量 | 改用 canonical seed/phase/support placement |
| ILP 只增加必要细化 | 只增加必要支撑，最小化额外 faces |

论文明确指出 pairing 是非局部约束；它把变量从 cell 改为 grid vertex 后，用二元线性约束
扩大合法解空间，并在多层网格中按相邻层对构造局部二元子问题。其公开实现使用 Gurobi，
论文报告运行时间随输入单元数近似线性，median 约 710 input cells/s；但 pairing 仍占总时间
90% 以上。因此可借的是**变量选择、硬约束建模和分层结构**，不是照搬 Gurobi 或性能数字。

该论文也给出重要警告：局部子网格 formulation 会漏掉闭合环的全局排列，作者承认并未覆盖
全部合法解。这与 Case 9 的长周界、周界合并/拆分现象一致。因此 Method-C 不能只做局部
表匹配，必须保留完整周界分量或可靠分隔接口。

### 3.2 最匹配细化倍率：3-refinement 模板穷举（Tong/Zhang，2026）

论文：[Element-Saving Hexahedral 3-Refinement Templates](https://arxiv.org/abs/2512.14862)；
[公开实现](https://github.com/CMU-CBML/Element-Saving-Hexahedral-3-Refinement-Templates)。

该工作直接处理 3-refinement：

- 旧 3-refinement 条件会导致 10–100 倍过度细化；
- vertex-based 路线枚举 `2^8=256` 个局部顶点构型，归并为 10 个基本模板和 12 个组合模板；
- edge-based 路线覆盖 `2^12=4096` 个边构型，并提供通用模板兜底；
- vertex-based 模板替换只做 cell substitution，论文给出 O(n) 复杂度；
- edge-based 方法用更多模板和局部 greedy 换少量单元数，实测约慢 5 倍。

对 Method-C 最有价值的不是六面体模板本身，而是工程形态：

> 把昂贵的状态穷举和模板证明放到离线；运行时只做 canonical 状态编码、查表和受限传播。

Method-C 的 stride-3/rad3 与该论文的 3-refinement 不是同一模板，不能移植具体表；但可以用
同一方法从现有 emitter **生成自己的真值表**。这比手推第二套 `TransitionPatch` 语义更安全。

### 3.3 高速表传播：Compact-Table

论文：[Compact-Table: Efficiently Filtering Table Constraints with Reversible Sparse Bit-Sets](https://arxiv.org/abs/1604.06641)。

它把允许元组存成位集，变量域收缩时用字操作批量删除失效元组，并维护 residual support。
论文报告其在标准基准上优于多种既有 table propagator；其核心适合 Method-C，因为 canonical
局部模板天然是有限 extensional table。

初版无需引入完整 CP 框架：

- `Vec<u64>` 保存每个局部约束仍存活的模板元组；
- `(variable,value)` 预存支持位集；
- 工作队列只重算域发生变化的相邻约束；
- 回溯时复制小位集或记录撤销栈。

只有这个标准库版本在真实 transition band 上仍不够，才评估已安装/外部求解器。

### 3.4 为什么按约束图分解，而不是按地理半径

Freuder 的经典结果表明，k-tree 结构 CSP 可在变量数上线性、在宽度 k 上指数的时间求解：
[Complexity of K-Tree Structured Constraint Satisfaction Problems](https://cdn.aaai.org/AAAI/1990/AAAI90-001.pdf)。

这给出正确的性能指标：

- 不看全局 seed 总数；
- 不看见证到中心的半径；
- 看约束超图的分隔宽度/induced width。

Case 9 已证明 hard-coverage 分量并不是合法求解边界，完整周界和 topology-only seeds 会把它们
重新耦合。因此应以 coverage、周界接口、parent support、witness/template table 共同组成的
超图分解；几何 patch 仅作显示和缓存单位。

### 3.5 AMR 聚类：只借需求到 placement 的中间层，不当正确性证明

AMReX 的官方流程是 tag → Berger–Rigoutsos clustering → `blocking_factor` 量化 → grid；
`grid_efficiency` 低于要求时也不能破坏 `blocking_factor`：
[AMReX Grid Creation](https://amrex-codes.github.io/amrex/docs_html/GridCreation.html)。

这支持在 raw demand 与 Method-C placement 之间增加形态感知层，但本仓库的实测已经否定
“把 90 个 hard 分量强行连成少数大块”：代价会很大且仍不保证合法。故聚类在本方案中的
地位是：

- 提供初始 placement / warm start；
- 只增加支撑，不删除 hard obligation；
- `grid_efficiency` 只报告，不作为丢需求阈值；
- 最终仍由同一约束合法化器与 exact emitter 判定。

### 3.6 p4est、NVB、JIGSAW 的正确位置

- p4est 的 2:1 balance 是跨层支撑的成熟参考，但 Method-C 还有 mod-3 周界、canonical
  phase、价数和 M/U/W 互惠；2:1 不是完整解法。参考：
  [Low-Cost Parallel Algorithms for 2:1 Octree Balance](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)。
- Newest Vertex Bisection 用构造保证三角网共形，是当前模板族被证明无解后的细粒度 fallback，
  不是优先重写目标。参考：
  [30 Years of Newest Vertex Bisection](https://math.nist.gov/~WMitchell/papers/wfmICNAAM15full.pdf)。
- JIGSAW-GEO 用 Frontal-Delaunay 与优化生成球面 Delaunay/Voronoi 网格，是放弃 Method-C
  位级兼容时最成熟的独立后端候选。参考：
  [JIGSAW-GEO](https://arxiv.org/abs/1611.08996)。

## 4. 建议架构：Method-C Compiled Transition Legalizer

### 4.1 三层结构

```text
原生 15″ hard obligations
        │  （不可删除、不可粗投影）
        ▼
[层 A] placement / support 变量化
        │  hard coverage + parent support + phase
        ▼
[层 B] 编译模板表传播 + 周界/跨层联合求解
        │  GAC → 约束图分解 → 有界确定性搜索
        ▼
[层 C] 整网 exact emitter + hard gates
        ├─ SAT
        ├─ INCOMPLETE
        └─ 相对于当前模板规格的 UNSAT
```

### 4.2 变量：不要再以任意 face mask 为主变量

建议变量：

1. `placement(component, phase, seed/template)`：某个 canonical placement 是否采用；
2. `support(parent_lineage)`：该 placement 是否需要并选择父层支撑；
3. `face_selected(iw)`：由 placement 派生，不直接自由翻转；
4. `boundary_state(interface)`：局部模板在 transition-band 边界暴露的有限签名；
5. 可选 `perimeter_phase`：完整有向周界上的 `0/1/2` 状态，用于表达三元组步进。

hard demand 只产生约束，例如“每个 obligation 至少被一个达到目标层的 placement 覆盖”，
而不是先固定一张不可回溯掩码。

### 4.3 约束分成三类

#### A. 已经能精确表达的约束

- hard coverage；
- 单一 parent-mrlw；
- legal canonical seed / rad3 footprint；
- parent support / 层级差；
- 已知 phase 与 lineage 关系；
- patch 外固定面和固定接口。

这些已有 checkpoint、preflight 与 symbolic 接口，应直接复用。

#### B. 有限局部模板约束

- self-loop；
- M-ring 重复 U-edge（环长 ≤7，可用最多 21 个两两不等或直接进模板表）；
- Valence ≤7；
- TransitionPatch 的 canonical split 兼容；
- 局部 M/U/W 互惠；
- vertex-only contact。

离线编译时按旋转/镜像 canonical 化，输出允许元组、边界签名和精确重放摘要。

#### C. 非局部约束

- 周界分量形成、合并与拆分；
- 每个完整周界长度 `mod 3 = 0`；
- 多个局部 placement 共同触发的父支撑；
- 跨分量 parent-mrlw / phase 接口。

这类不能被“半径 1 patch”或独立 seed delta 近似。建议两条实现路径按难度递进：

1. **第一版：完整受影响周界分量作为一个 table/global constraint。**
   只用于 NXP=162 的两个小簇和已冻结正对照，先验证模型口径。
2. **高速版：边界接口动态规划。**
   对 transition-band 约束图做树分解；每个 bag 只传递边界连通分区、周界余数 `0/1/2`、
   parent support 和局部模板接口。复杂度取决于 separator width，而非全局 seed 数。

还可验证一个更紧凑的编码：selected/unselected 邻接诱导有向边界；边界顶点必须形成无 pinch
的简单环，并沿环传播 `0→1→2→0` 状态。闭环可赋值当且仅当长度能被 3 整除。该编码必须先
与 `method_c_perimeters_from_selected_faces` 在冻结矩阵逐项对拍，未证明前不能进入生产。

### 4.4 离线模板编译器必须以现有 emitter 为真相源

输入：

- canonical split 表版本哈希；
- 局部父层拓扑与 mrow/phase 类；
- 局部 placement/边界状态枚举。

对每个状态：

1. 用现有映射构造完整 assignment；
2. 运行 symbolic check；
3. 运行 exact materialization；
4. 保存成功模板、失败类别、边界签名、输出 M/U/W 摘要与哈希；
5. 对旋转/镜像等价状态只保留 canonical representative；
6. 为每个模板保存至少一个 SAT 正对照。

生成物可审阅、可重放、随 canonical 表哈希失效。这样“表”和 emitter 不会静默漂移，也
避免手写第二套几何/拓扑实现。

### 4.5 运行时算法

```text
1. 建立原生 hard obligation → legal placement 支撑关系
2. 提取完整 transition-band 约束超图
3. 固定 band 外状态；不固定 band 内临时 mask
4. 加入跨层 support 变量和完整周界接口
5. Compact-Table 风格 GAC 传播到不再删值
6. 域清空：报告相对于当前已编译规格的冲突，不改 hard demand
7. 分解独立分量；对低宽度分量做确定性 DFS/DP
8. 将候选嵌回整网，只调用一次或少数几次 exact emitter
9. emitter + 全部硬门通过才 SAT；资源耗尽则 INCOMPLETE
```

目标函数必须与可行性分离：

1. 先找到满足全部硬约束的 assignment；
2. 再最小化新增 faces / parent support；
3. `grid_efficiency`、质量分布只报告；质量门校准完成前不参与拓扑求解。

## 5. 求解器选择：先不加依赖

### 5.1 第一阶段：标准库位集 + 确定性 DFS

仓库当前没有 SAT/CP/ILP 依赖。最小实现可复用已有：

- `MethodCHfieldSelectionCheckpoint`；
- `MethodCHfieldLegalizationPreflight`；
- `legalization_symbolic_check`；
- `legalization_patch_boundary_check`；
- `legalization_exact_materialization_check`。

增加的核心只需：

- canonical template relation 的生成/加载；
- `Vec<u64>` 支持位集；
- 变量—约束邻接工作队列；
- 固定次序 DFS；
- 证据 JSON。

这一步先回答 GAC 能否把 NXP=162 的 16/35 变量域压到可搜索范围。若能，不需要通用求解器。

### 5.2 何时才引入 CP-SAT/伪布尔求解器

只有满足以下证据才评估：

- 局部模板表与 frozen SAT/失败矩阵完全一致；
- GAC 后仍有宽分隔集，确定性 DFS 超过明确资源预算；
- 模型能无歧义映射回现有 emitter；
- 外部求解器明显减少 exact oracle 调用或总 wall time。

若使用 Google OR-Tools CP-SAT，必须保留三态：`FEASIBLE/OPTIMAL`、`INFEASIBLE`、`UNKNOWN`；
官方文档明确 `UNKNOWN` 可能由时间/内存限制导致：
[CP-SAT Solver](https://developers.google.com/optimization/cp/cp_solver)。
其 assumptions 返回的是足以导致不可行的子集，不保证最小；真正 MUS 需另做优化，而且
assumptions 与并行求解不兼容：
[官方 troubleshooting](https://github.com/google/or-tools/blob/stable/ortools/sat/docs/troubleshooting.md#debugging-infeasible-models)。

因此：

- SAT 仍由 exact emitter 复核；
- solver `INFEASIBLE` 只能解释为相对于编码/模板表不可行；
- time limit 必须返回 `INCOMPLETE`；
- MUS/MCS 只诊断，绝不自动删除 hard obligation。

### 5.3 证明日志是后续能力，不是第一阶段门槛

[VeriPB](https://veripb.org/) 能检查 0–1 伪布尔推理的 SAT/UNSAT/最优性证书。它适合在模型
稳定后为“相对于 Method-C 模板规格的 UNSAT/OPT”提供独立 proof checker。但它不能自动
证明“编码等价于 emitter”；编码生成器与模板编译器仍需正反对照和 exact replay。

## 6. 性能预期与性能门

### 6.1 为什么会快

- 穷举移到离线，运行时查表；
- 位集传播一次处理 64 个模板元组；
- 只求解 transition band，不扫描所有边界 M 点；
- 约束图分解把指数项限制在 separator width；
- exact emitter 从“每候选一次”降到“每个最终候选一次或少数几次”；
- frozen checkpoint / template cache 用 canonical 表哈希失效，重复运行无需重建不变关系。

Tong/Zhang 的 vertex-template 路线证明“完整局部模板 + cell substitution”可以做到 O(n)；
Pitzalis 的结果说明非局部 pairing 可以用结构变量和二元优化求解，但其局部 formulation 仍
可能漏全局闭环。因此本方案把两者组合：局部快表覆盖常见状态，完整周界/分隔 DP 处理真正
非局部的少数情况。

### 6.2 不承诺虚假的常数时间

一般有限 CSP 仍可能指数；“彻底且高速”来自把指数项限定到很小的分隔面，而不是声称问题
变成多项式。生产性能门建议用相对口径：

- legalization 自身 ≤ 同配置原始生成 wall time 的 10%；
- exact whole-grid emit 次数为 O(候选解数)，正常快路径目标 ≤3；
- 不出现随 repair 轮数线性重复的全边界候选扫描；
- 时间/内存超限返回 `INCOMPLETE`，不降级 hard demand；
- debug/release、可执行哈希、模板表哈希全部记录。

10% 和 ≤3 是工程验收目标，不是文献保证；先在 NXP=162 与 G-CIRCLE 正对照实测再冻结。

## 7. 最小实施阶梯

### P0：离线真值表原型，不改生产

1. 只覆盖现有 frozen SAT 正对照、NXP=162 两个失败簇；
2. 从 exact emitter 生成局部 canonical 允许表；
3. 验证旋转/镜像归一化不会合并不等价状态；
4. 48/48 成功路径不得误报；所有已有失败 assignment 分类一致；
5. 记录表大小、生成时间和哈希。

停止条件：任一局部状态不能在不造第二套语义的前提下映射到 emitter。

### P1：位集 GAC，仍不搜索全局

1. 为 template placement、coverage、parent support 建表约束；
2. 完整受影响周界分量作为 global constraint；
3. 在正对照上必须保留已知解；
4. 测 NXP=162 两簇的域缩减率和约束图 induced width。

决策门：若 GAC 后最大 residual width 足够小，进入 P2；否则先改变量/接口，不增加 Hamming
深度。

### P2：确定性分量求解 + exact whole-grid gate

1. 先解 NXP=162 的 16 变量簇，再解 35 变量簇；
2. 同时包含 parent support 与同层 placement，不再交替贪心；
3. 候选必须嵌回完整 checkpoint；
4. exact emitter 与全部 hard gates 通过才 `SAT`；
5. 两次 release 证据逐字节一致。

### P3：回到 NXP=81 原生 Case 9

1. 建完整 transition-band 超图；
2. 分析 separator width，而非总变量数 241；
3. 必要时启用外部 CP/PB shadow prototype；
4. 不把 solver 超时写成 UNSAT；
5. 不用质量权重参与可行性。

### P4：生产准入

- shadow 默认关闭；
- 既有成功矩阵位级/契约不回归；
- 原生 `86400×43200` 读取，coarse projection 关闭；
- hard coverage、parent-boundary、self-loop、TransitionPatch、Valence、M/U/W 互惠全为 0；
- Tri/Hex 两视图分别通过其硬门；
- 两次 release gridfile 与证据哈希一致；
- 旧 64 轮 repair 先保留为诊断，不再是主合法化路径。

## 8. 若当前模板族确实无解

### 8.1 第一选择：离线扩模板，不改 hard demand

参考 3-refinement 论文的做法：

1. 从失败的边界签名出发；
2. 在固定外接口内枚举更细 canonical split；
3. exact emitter/拓扑检查验证；
4. 形成新的、通用的模板类和 SAT 正对照；
5. 运行时仍是查表，不为 Case 9 的具体 ID 加特例。

### 8.2 第二选择：局部细粒度合法化

若不存在固定 rad3 模板，可在 transition band 内评估：

- NVB/red-green 型单三角形共形闭合；
- 受限翻边作为合法化工具，而非质量优化；
- 固定外边界后局部重三角化。

这会改变 Method-C 对拍面，应作为独立能力版本，不可静默接入 compat 路径。

### 8.3 第三选择：按拓扑族切换生成器

- Tri/Delaunay 类：约束 Delaunay、NVB、MMG/JIGSAW 类生成器；
- Hex/Voronoi 类：JIGSAW-GEO/SCVT 类球面 Delaunay–Voronoi 生成器。

FVCOM/MPAS 只是下游格式或模型实例；生成器分流应按 Tri/Delaunay 与 Hex/Voronoi 拓扑族，
而不是写死产品名。

## 9. 明确排除

- 不降采样、平滑删除、coarse-project 或按 MCS 自动放弃任何 15″ hard obligation；
- 不提高 64 轮 repair 上限；
- 不再加固定 grow 圈数；
- 不用弹簧修组合拓扑；
- 不把 max_level=4/5 当作 pass-2 修复；
- 不把 NXP 提升当作通解；
- 不把 p4est 2:1 balance 等同于完整 Method-C 闭包；
- 不把 Berger–Rigoutsos 直接移植成球面“连成大块”；
- 不在小模型对拍前加入 SAT/CP/ILP 依赖；
- 不把局部 PATCH_UNSAT 或 solver UNKNOWN 写成 Case 9 UNSAT。

## 10. 最终建议

主线采用：

> **现有 Method-C emitter 驱动的离线 canonical 模板编译器 + transition-band 位集 GAC +
> 完整周界/跨层联合约束 + 约束图分解 + 少量 exact whole-grid 验证。**

它是当前约束下改动最小、证据最强、最可能兼顾“彻底、快速、保留 Method-C 熟悉主线”的
方案。近期不要先接 Gurobi/OR-Tools，也不要继续指数枚举；先做 P0/P1。若 P1 不能显著
压缩 NXP=162 的 16/35 变量簇，再以实测 residual width 决定是否引入 CP/PB。若完整模板
模型返回相对于当前规格的 UNSAT，则扩模板或启用独立拓扑后端，不牺牲 15″ hard demand。

## 11. 主要来源

1. Pitzalis et al. (2021), [Generalized Adaptive Refinement for Grid-based Hexahedral Meshing](https://pizza1994.github.io/pdf/gen_adapt_grid_hexmeshing.pdf)，[代码](https://github.com/cg3hci/Gen-Adapt-Ref-for-Hexmeshing)。
2. Tong & Zhang (2026), [Element-Saving Hexahedral 3-Refinement Templates](https://arxiv.org/abs/2512.14862)，[代码](https://github.com/CMU-CBML/Element-Saving-Hexahedral-3-Refinement-Templates)。
3. Demeulenaere et al. (2016), [Compact-Table](https://arxiv.org/abs/1604.06641)。
4. Freuder (1990), [Complexity of K-Tree Structured CSPs](https://cdn.aaai.org/AAAI/1990/AAAI90-001.pdf)。
5. Isaac, Burstedde & Ghattas (2012), [Low-Cost Parallel Algorithms for 2:1 Octree Balance](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)。
6. AMReX, [Grid Creation](https://amrex-codes.github.io/amrex/docs_html/GridCreation.html)。
7. Mitchell (2016), [30 Years of Newest Vertex Bisection](https://math.nist.gov/~WMitchell/papers/wfmICNAAM15full.pdf)。
8. Engwirda (2016), [JIGSAW-GEO](https://arxiv.org/abs/1611.08996)。
9. Google OR-Tools, [CP-SAT](https://developers.google.com/optimization/cp/cp_solver) 与 [infeasibility troubleshooting](https://github.com/google/or-tools/blob/stable/ortools/sat/docs/troubleshooting.md#debugging-infeasible-models)。
10. [VeriPB](https://veripb.org/)。

## 12. P0/P1 最小原型执行更新（2026-07-30）

已实现默认不接入生产路径的最小闭环：

1. `method_c_legalization_table`：标准库 `Vec<u64>` 支撑表和二值 GAC；
2. `compile_bounded_exact_legalization_patch_table_for_diagnostics`：复用现有 patch boundary check 与
   whole-grid exact materializer，穷举范围硬限制为最多 `20` 个变量；
3. 超过调用者界限或硬界限时立即返回 `INCOMPLETE`，不退回无界枚举；
4. 只有 exact emitter 接受的 assignment 才进入真值表；位集层不定义新的网格合法性。
5. patch 接口比较保留“分量归属 + 有序外部弧”；任何在接口证明之前返回的 hard/perimeter
   拒绝均使零-SAT 结果降级为 `INCOMPLETE`，不得据此形成 `PATCH_UNSAT`。

验证结果：

- 位集传播跨两个表约束传递固定值，domain wipeout 能稳定报告不一致；
- 既有 `face_hard_demand_selection_checkpoint_is_deterministic` 正对照同时通过
  checkpoint、preflight、symbolic、exact、真值表编译和重复运行一致性；
- NXP=81 Case 9 第 3 簇完整检查 `256/256`：`0 SAT`、`3` 个 patch-boundary
  incomplete、`137` 个 non-triplet、`62` 个 hard coverage、`54` 个 exact
  TransitionPatch，因此严格保持 `INCOMPLETE`，不能写成 `PATCH_UNSAT`；
- NXP=162 第 1 簇有 `16` 个变量，在原型安全界 `12` 下 `0` 次枚举并立即返回
  `INCOMPLETE`，证明新入口不会把生产路径重新拖回指数扫描。

证据：

- `target/case9-legalization-hard-obligations-1785323328/cluster-3-compiled-table-v0.json`
  SHA256 `791a1ba3904b4fb660d8f59b178271ee65f54d662b41aff09d7bef1a8ccaf84e`；
- `target/case9-native-nxp162-checkpoint-1785388485/cluster-1-compiled-table-v0.json`
  SHA256 `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

这一步验证了“现有语义编译为表 → 位集传播 → exact gate”的接口形状，但尚未解决
Case 9。下一步不是扩大穷举上限，而是把表的作用域从完整 patch 降到可复用的 canonical
局部模板，并以完整周界分量连接这些局部表；只有传播后的 residual width 实测足够小，才
允许进入确定性有限搜索。

## 13. 可复用关系与 residual width 执行更新（2026-07-30）

本轮没有把 fail-fast self-loop witness 直接固化成局部模板。复核现有数据流后确认：
`triple_index` 与 `component_triple_index` 来自完整有序周界；同一周界分量远处的增删也会改变
相位和三元组分组。因此，“某 parent U edge 在当前 witness 邻域内是否消失”不能脱离完整
周界接口成为静态终局约束。强行局部化会产生第二套、且可能漏解的网格语义。

已完成的最小安全基础：

1. `MethodCBinaryTableConstraint::canonical_relation`：把 exact emitter 编译出的允许关系与
   具体 mesh ID 解耦；
2. `rebind_variables`：同一允许关系可绑定到另一个等长、严格有序的变量作用域；
3. `analyze_method_c_binary_table_system`：GAC 后按共享 table scope 计算保守 residual
   connected-component width；该宽度是确定性搜索规模的上界，不冒充 treewidth；
4. 成功正对照中，完整 exact 表有 `4` 个 placement 变量，GAC 固定 `1` 个值，最大 residual
   component width 为 `3`；
5. NXP=81 与 NXP=162 两个 Case 9 冻结检查点三态结论均未改变，仍为 `INCOMPLETE`；
   NXP=162 仍在变量界限前 `0` 次枚举退出。

这一结果把下一接口进一步收紧：

> canonical 局部表只能描述不依赖远处周界重排的必要关系；non-triplet、self-loop 与
> TransitionPatch 必须由“完整有序周界分量 + 局部 canonical 关系”的联合约束连接。

在该完整周界状态尚未编码前，不把 witness-local 表用于排除 assignment，也不把 residual
width 解释为 Case 9 的可搜索宽度。

## 14. 完整当前周界作用域测量（2026-07-30）

已把 `perimeter_candidate_seed_ids` 接入 exact patch-table 编译器的三态安全门：

- checkpoint 缺少该普查时，作用域为未知；
- patch 未覆盖当前完整受影响周界的候选 seed 时，零 SAT 不得升级为 `PATCH_UNSAT`；
- 作用域数据只描述当前 ordered perimeter，仍不声称覆盖未来可能新建或合并的分量。

NXP=162 原生 Case 9 的两个失败簇实测为：

| 簇 | witness patch 变量 | 当前完整周界候选变量 | patch 是否覆盖完整作用域 | 枚举 |
|---|---:|---:|---|---:|
| 0 | 35 | 103 | 否 | 0 |
| 1 | 16 | 36 | 否 | 0 |

证据：

- `target/case9-native-nxp162-checkpoint-1785388485/cluster-0-compiled-table-v0.json`
  SHA256 `3ff780c7f0c45b5040529ebae7b0007ad2af8a811fa7c8a736f47e3855b83db5`；
- `target/case9-native-nxp162-checkpoint-1785388485/cluster-1-compiled-table-v0.json`
  SHA256 `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

因此“把完整周界分量直接编成一个真值表”被工程量否决：两张表分别需要最坏
`2^103` 与 `2^36` 个 assignment。下一步必须采用 §4.3 的边界接口动态规划/约束图分解，
不能提高 exact 枚举上限。该结果没有证伪 Compiled Transition Legalizer；它证伪的是
“单个完整周界 truth table”这一种实现。

## 15. ordered-perimeter 二进制 frontier 实测（2026-07-30）

在不定义新合法性语义的前提下，新增只读诊断：对当前完整周界的每个 candidate seed，
复用 canonical rad3 footprint，记录它触及的有序周界点；枚举所有环切点，把变量从首次到
末次触及位置保守保持为 live，报告最小线性 frontier。该值是原始二进制 ordered-interface
DP 的保守宽度，不是 treewidth，也不用于剪枝或 SAT 判定。

NXP=162 原生 Case 9：

| 簇 | 周界点 | 完整候选 | 点-变量 incidence | 单点最大候选 | 最佳切点 | 最小 frontier |
|---|---:|---:|---:|---:|---:|---:|
| 0 / component 7 | 33 | 103 | 601 | 25 | 7 | 31 |
| 1 / component 8 | 18 | 36 | 233 | 24 | 7 | 25 |

结论：有序周界把最坏状态从 `103/36` 个全局变量收窄到 `31/25` 个 live 变量，但原始
二进制 DP 仍需最坏 `2^31` / `2^25` 状态，不能直接作为高速方案。下一步若继续 Compiled
Transition Legalizer，必须先把 canonical transition 关系压成相位、模 3、局部模板类别等
小状态；不得把 31/25 个 seed bit 原样塞进 DP，也不得提高 exact 枚举上限。

旧 NXP=81 checkpoint 没有 perimeter candidate census，因此分析继续返回空并保持
`INCOMPLETE`，兼容路径没有猜测缺失数据。

证据：

- `target/case9-legalization-hard-obligations-1785323328/cluster-3-compiled-table-v0.json`
  SHA256 `791a1ba3904b4fb660d8f59b178271ee65f54d662b41aff09d7bef1a8ccaf84e`；
- `target/case9-native-nxp162-checkpoint-1785388485/cluster-0-compiled-table-v0.json`
  SHA256 `3ff780c7f0c45b5040529ebae7b0007ad2af8a811fa7c8a736f47e3855b83db5`；
- `target/case9-native-nxp162-checkpoint-1785388485/cluster-1-compiled-table-v0.json`
  SHA256 `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

## 16. 局部 M-ring 占用状态实测（2026-07-30）

原始 seed incidence 签名几乎不能压缩：

| 簇 | 完整候选 | 不同 incidence 签名 | 最大重复数 |
|---|---:|---:|---:|
| 0 / component 7 | 103 | 90 | 2 |
| 1 / component 8 | 36 | 36 | 1 |

因此不能把“触及同一批周界点的 seed”合并为变量。进一步把每个 candidate rad3 footprint
投影到当前周界 M 点周围的本地 W-face ring，并枚举这些局部 bit mask 的所有 OR 合成状态。
该计数包含空选择，且尚未与既有 baseline 选中面 OR，因此是每个周界点的保守局部上界：

| 簇 | 本地 ring 最大面数 | 单点不同 footprint mask | 单点最大 OR 状态数 |
|---|---:|---:|---:|
| 0 / component 7 | 6 | 13 | 29 |
| 1 / component 8 | 6 | 10 | 20 |

结论：

1. 原始 seed-bit frontier `31/25` 不适合直接做 DP；
2. 本地几何占用状态只有 `29/20`，可作为缓存子状态，但不能单独作为 canonical
   transition state；
3. 该数字不是完整周界状态数，也没有编码周界合并/分裂、三元组重排或
   `TransitionPatch` 全语义，不能用于剪枝、SAT 或 UNSAT；
4. 现有 canonical 回归提供了直接反例：保持同一 selected mask 与同一组 perimeter
   point 描述，仅将完整周界循环相位旋转一位，零偏移路径可 materialize，而旋转路径触发
   self-loop 并在 exact emit 失败。因此相位、完整有序周界、parent U identity 与
   `nest_wd` 抑制状态是必需信息；
5. 下一原型必须以完整 `MethodCHfieldPerimeterPointCheckpoint` 序列为状态主键，本地
   occupancy 只能作为派生缓存。不得实现单点/相邻点 mask DP。

该反例由
`method_c_perim_fill3_writes_canonical_weighted_transition_coordinates` 回归锁定；测试显式
验证旋转前后 perimeter point 多重集一致，只有循环顺序/三元组相位改变。

三份报告连续两次重放逐字节一致，SHA256 为：

- NXP=81 cluster 3：
  `791a1ba3904b4fb660d8f59b178271ee65f54d662b41aff09d7bef1a8ccaf84e`；
- NXP=162 cluster 0：
  `3ff780c7f0c45b5040529ebae7b0007ad2af8a811fa7c8a736f47e3855b83db5`；
- NXP=162 cluster 1：
  `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

## 18. hard coverage 支撑域测量（2026-07-30）

没有把 `demand anchor -> canonical seed` 直接写成求解约束。真实路径先合并 seed 的
rad3 footprint，再执行凹角闭包；闭包可能补入并非由任何单个 seed 直接覆盖的 face。
因此只看直接 footprint 的子句可能排除合法 assignment，不能作为安全剪枝。

bounded compiler 现在只读报告固定 patch 外 seed 与“全部 candidate 打开”两种状态下，
闭包前后的 anchor 覆盖数，以及固定部分尚未覆盖的 anchor 的直接 candidate 支撑域。
若变量数不超过 20，还报告强制采用直接 footprint 子句时剩余的 assignment 数；该数字
只估算理论剪枝收益，不参与 SAT、`PATCH_UNSAT` 或生产生成。

NXP=162 原生 15″ Case 9：

| 簇 | 变量 | anchor | 固定直接覆盖 | 固定未覆盖 | 直接支撑域 | 严格直接子句保留 |
|---|---:|---:|---:|---:|---|---:|
| 0 | 35 | 18 | 18 | 0 | 无待约束 anchor | 未枚举 |
| 1 | 16 | 18 | 15 | 3 | 3 个不同作用域；每个均有 10 个候选 | 65,408 / 65,536 |

簇 1 即使采用可能过强的直接 footprint 子句，也只排除
`128/65,536 = 0.195%`。簇 0 完全没有 patch-local coverage 约束。旧 NXP=81
第 3 簇作为对照：1 个固定未覆盖 anchor、2 个直接候选，严格子句保留 `192/256`；
这能解释旧 bounded 枚举中的部分 coverage 拒绝，但不能外推为 NXP=162 主线的杠杆。

决定：

1. 不实现 direct-footprint coverage 预剪枝；
2. 不为这点收益把 bounded compiler 改写为 DFS；
3. coverage 继续由现有凹角闭包后的 exact hard gate 判定；
4. 下一求解主力回到完整有序周界 `non-triplet` 与
   `TransitionPatch/self-loop` 联合约束。

证据：

- `target/case9-native-nxp162-checkpoint-1785388485/cluster-0-compiled-table-v1.json`
  SHA256 `2794303e9b09fa5ab357f3c44ad47a4d37634999a62ef9237a927850fc4639b7`；
- `target/case9-native-nxp162-checkpoint-1785388485/cluster-1-compiled-table-v1.json`
  SHA256 `9503db954e75ca80405f613281d3da9e3296f97f47b6e17b9c6e0952337f9020`；
- `target/case9-legalization-hard-obligations-1785323328/cluster-3-compiled-table-v1.json`
  SHA256 `fdf12a4ebf27932f7060861b1d11a0e299b2bded1fe80d351827a03ed37edfa4`。

## 19. selected-mask BDD 的先行否证（2026-07-30）

在实现 closure-normalized BDD 前，先计算一个更便宜的下界。对每个完整当前周界候选：

1. 将 canonical rad3 footprint 投影到该 ordered perimeter 各 M 点的本地 W-face 并集；
2. 计算所有候选 footprint 的不同 OR 状态；
3. 再在候选完整 rad3 footprint face 并集上重复；
4. 两者均设 `100,000` 状态硬上限，超过即停止并报告处理到第几个变量。

这只是 direct seed-union 状态，不含凹角闭包、周界重建或 exact emitter。完整 footprint
状态到周界投影状态存在自然投影，所以同一 direct OR 模型下完整状态数不会小于投影状态数。
但凹角闭包可能把多个 direct 状态归并为同一 closed mask；本实验不能单独否决
closure-normalized BDD。

NXP=162 原生 Case 9：

| 簇 | 候选 | 周界投影 face | 投影 OR 状态 | 完整 footprint face | 完整 OR 状态 |
|---|---:|---:|---:|---:|---:|
| 0 / component 7 | 103 | 132 | 第 25 个变量后超过 100,000 | 576 | 第 19 个变量后超过 100,000 |
| 1 / component 8 | 36 | 71 | 10,338 | 287 | 第 20 个变量后超过 100,000 |

簇 1 的 `10,338` 个投影状态看似可控，但加入不在局部周界环上的 footprint faces 后，同样
在只处理 20/36 个变量时突破上限。这说明投影压缩丢掉的带外 selected faces 正是状态爆炸
来源；它不能升级为安全主状态。簇 0 更早否决。

决定：

- 不实现未经闭包归一化的 direct-union selected-mask BDD；
- 不把 10,338 个投影状态误读成完整求解规模；
- 保留 full ordered perimeter、phase、parent U 与 `nest_wd` 作为终局语义；
- 下一决定性实验只测 bounded prefix 的 closure-normalized full mask 归并率；若仍接近
  1:1，再关闭 full-mask BDD 并转向约束级分解。

NXP=81 旧 checkpoint 不含完整 perimeter candidate census，继续返回空分析；没有根据缺失
数据猜测状态数。

证据：

- `target/case9-native-nxp162-checkpoint-1785388485/cluster-0-compiled-table-v3.json`
  SHA256 `e377b211142338b84c4c84b9f0a6e1fcf1b07e56fc5355632948d40a2b136dce`；
- `target/case9-native-nxp162-checkpoint-1785388485/cluster-1-compiled-table-v3.json`
  SHA256 `aad39471257cd896e4fddcb3e70c85015b678b8ea3a769ca08bc336652e21796`；
- `target/case9-legalization-hard-obligations-1785323328/cluster-3-compiled-table-v3.json`
  SHA256 `ebab4eb8a8e4031c6537378739a969717d08bff3237f9b92657db551a4f7b6b4`。

### bounded prefix 闭包对拍

随后对两个 NXP=162 簇的前 `12` 个有序候选完整枚举 `4096` 个 assignment。每个
assignment 都从 raw selected mask 重新运行现有
`close_method_c_concavities_for_level_with_neighbors`，同时与增量式
`cl(cl(A) ∪ B)` 状态推进对拍：

| 簇 | raw assignment | 不同 direct mask | 不同 closed mask | 最大 closed 重数 | 增量对拍 |
|---|---:|---:|---:|---:|---|
| 0 / component 7 | 4096 | 3464 | 3324 | 4 | **false** |
| 1 / component 8 | 4096 | 1840 | 1648 | 24 | **false** |

归并率本身有限，更决定性的是两簇的增量对拍均失败。当前例程在
`selected_count == npoly` 时跳过，在恰好 `npoly - 1` 时才扩张 rad3 footprint；
因此增加一个 incident face 可能关闭原本会触发的扩张。它虽名为“closure”，但不是集合论
上的单调闭包算子，不满足 `cl(cl(A) ∪ B) = cl(A ∪ B)`。

决定：

- 关闭以当前凹角例程为增量转移的 closure-normalized BDD；
- 不据此修改 production 凹角语义；这会是独立的行为变更，而不是诊断优化；
- 仍可从 raw assignment 每次重算闭包，但 direct 状态已爆炸且 exact emitter 不是瓶颈，
  因而不作为高速路线；
- 下一路线只能同时建模 concavity、non-triplet、coverage 与
  `TransitionPatch/self-loop` 约束，或另立项目重新定义并验证单调的凹角规则。

该非单调性质已由普通单元测试
`method_c_concavity_fill_is_not_monotone_over_selected_face_sets` 锁定。两份确定性报告：

- `cluster-0-compiled-table-v6.json` →
  `4bdcfa9655abcf61d2e04077ce1f4fd8312103952373e3e6371696bde958a2eb`；
- `cluster-1-compiled-table-v6.json` →
  `af33aa6ed71e538db49d96926a638070196384ff01af78f8efe086099dbc2aed`。

Case 9 仍为 `INCOMPLETE`；这些统计只读，生产路径未接入 legalizer。

## 17. 完整 exact 状态复用率实测（2026-07-30）

为避免再次用不充分的局部状态，bounded compiler 现在只对已经通过 triplet 前置门的
assignment 构造完整状态键：

1. 全部 selected parent W-face ID；
2. 每个周界分量的完整有序 `MethodCHfieldPerimeterPointCheckpoint` 序列；
3. exact 结果仍由现有 whole-grid emitter 给出。

若同一状态键出现不同 exact 结果，`mixed_exact_outcome_state_count` 会非零，表示状态键遗漏
语义。成功正对照与 Case 9 bounded 簇均为 `0`。

NXP=81 Case 9 cluster 3 的完整 `256/256` 枚举：

| 指标 | 数值 |
|---|---:|
| 全部 assignment | 256 |
| 通过 triplet 前置门、到达 exact 状态统计 | 57 |
| 不同完整 exact 状态 | 33 |
| 单状态最大 assignment 重数 | 4 |
| 同状态混合 exact 结果 | 0 |

完整状态缓存最多消除 `57-33=24` 次重复 exact 检查，即 exact-eligible 子集的 `42.1%`，
但只占全部枚举的 `9.4%`。此前分段计时已经证明 exact emit 只占 repair wall time 的
`2.78%`；因此新增状态缓存层不会改变主要成本，也不会解决 `103/36` 变量的 NXP=162
作用域。

决定：

- 不实现 production exact-state cache；
- 保留该统计作为完整状态键的正确性与复用率诊断；
- 下一性能/求解杠杆必须位于 exact 之前，优先处理主导拒绝项
  `non-triplet perimeter` 与 hard coverage；
- 不再用局部 occupancy、提高枚举上限或缓存 emitter 回避完整周界约束。

三份报告连续两次重放逐字节一致，SHA256：

- NXP=81 cluster 3：
  `791a1ba3904b4fb660d8f59b178271ee65f54d662b41aff09d7bef1a8ccaf84e`；
- NXP=162 cluster 0：
  `3ff780c7f0c45b5040529ebae7b0007ad2af8a811fa7c8a736f47e3855b83db5`；
- NXP=162 cluster 1：
  `40768924d75af6d5c76e571265c1c03939c40264038c8ebae5c025660e0f9331`。

## 20. symbolic 签名充分性探针（2026-07-31）

§14–§19 逐一否证了七种**提议的**状态编码，但都没有回答更前置的问题：**现有 symbolic
检查器记录的字段，本身是否构成 exact 结果的充分统计量？** 若连「用尽全部 symbolic
字段」都不充分，则继续设计第八种压缩编码没有意义。

本节只重读冻结的 single-toggle 证据，不运行 materializer、不改动生产代码、不引入新语义。
探针脚本与两份输出保存在同一证据目录。

### 20.1 方法

以 baseline 为起点的 `934` 个单 seed 翻转中，`96` 个通过 symbolic 门并进入整网 exact
emitter。对这 `96` 条记录，构造五个逐级包含的候选状态键（后者严格包含前者），
检验「签名相同 ⟹ exact 结果相同」是否成立。任一签名下出现两种 exact 结果，即证明该
签名不是充分统计量。

预测目标取两种粒度：**exact 见证身份**（`M<点>/U<边>` 或 `<失败类>:no-witness`）与
**仅失败类**（`transition_patch` / `valence`）。

### 20.2 结果

`96` 个 survivor 的 exact 结局只有 `6` 种：

| 结局 | 计数 |
|---|---:|
| `M38/U920` | 68 |
| `transition_patch:no-witness` | 23 |
| `valence:no-witness` | 2 |
| `M37/U911` / `M38/U929` / `M38/U956` | 各 1 |

充分性检验：

| 候选状态键 | 不同签名 | 压缩 | 见证身份 | 仅失败类 |
|---|---:|---:|---|---|
| A 仅 self-loop 数 | 4 | 24.00x | 不充分（3 组冲突） | 不充分（2） |
| B ＋周界分量数 | 8 | 12.00x | 不充分（3） | 不充分（2） |
| C ＋受影响周界 | 20 | 4.80x | **不充分（6）** | 不充分（2） |
| D ＋面增量＋vertex contact | 76 | 1.26x | 不充分（3） | 充分 |
| **E 全部 symbolic 字段** | **77** | **1.25x** | **不充分（3）** | 充分 |

全签名下仍冲突的三组（决定性证据）：

```
self_loop=20 components=36 affected=(0,) Δface=2  contact=0 non_triplet=0
    seed    40 → M38/U929
    seed 11253 → M38/U920

self_loop=20 components=36 affected=(9,) Δface=41 contact=0 non_triplet=0
    seed  5699 → M38/U920
    seed 39249 → transition_patch:no-witness

self_loop=20 components=36 affected=(9,) Δface=27 contact=0 non_triplet=0
    seed 39222 → M38/U920
    seed 39243 → transition_patch:no-witness
```

两次重放逐字节一致：

- `target/case9-phase-inventory-1785348325/symbolic-signature-sufficiency-{a,b}.json`
- 证据 SHA256 `0469aff69b599272726431620505995ed4c2e7da577dc8cf5ac521c506f162be`
- 探针脚本 `symbolic-signature-sufficiency.py`
  SHA256 `4d9f97d0860645d43d8c3dba563a92dfe124a0725121deade9f3cd4d0c60b738`

### 20.3 两点读数

**1. 充分与压缩之间没有中间地带。** 用尽全部 symbolic 字段、压缩率仅 `1.25x`，仍不足以
预测 exact 见证身份。这与 §16 的相位反例（同一 mask、同一 perimeter point 多重集，仅旋转
循环相位一位即改变 exact 结果）方向一致，并把它从单个反例推广为 Hamming-1 邻域上的
系统性结论。结合 §17（完整状态键充分但只压缩 `42.1%`），八次独立尝试给出同一模式。

**2. C 比 B 冲突更多（3 → 6），说明是特征选择错误而非精度不足。** 增加「受影响周界」
反而把同结局的分开、把不同结局的合并，即该字段与真实决定因素不对齐。继续在该方向上
细化字段不会收敛。

### 20.4 边界

- 这是 **Hamming-1 邻域上的充分性探针，不是 Myhill-Nerode 最小化**；它不能给出最小可区分
  状态数。真正的最小化需要逐 assignment 的前缀状态与结果记录，而现有证据 JSON 只保存
  聚合统计（4–8 KB），因此需要一次显式的 dump 改动才能进行。
- 「现有字段不充分」不等于「不存在紧凑充分统计量」，只说明**从 symbolic 检查器当前记录的
  字段中导不出**。
- 本证据集中没有 SAT，探针无法检验能区分 SAT 与失败的签名。
- 结论不改变 Case 9 的三态状态，仍为 `INCOMPLETE`；不构成 `PATCH_UNSAT` 或模板族 UNSAT。

### 20.5 决定

1. 不再设计第八种基于现有 symbolic 字段的状态压缩；
2. 若继续 Compiled Transition Legalizer 的 DP/BDD 分支，前置条件是**先做一次逐 assignment
   记录 dump 并完成真正的 Myhill-Nerode 最小化**，用实测最小状态数决定该分支的生死；
3. 在该测量完成前，§13 的 residual width 仍不得解释为 Case 9 的可搜索宽度；
4. 与之并行的另一杠杆是重新定义**单调**的凹角规则（见 `bounded prefix 闭包对拍`）：若
   `cl` 单调成立，状态空间的定义本身会改变，本节的否证结论不自动适用于新定义。

## 21. Myhill-Nerode 精确最小化（2026-07-31）

§20 证明现有 symbolic 字段不构成充分统计量，但没有回答 DP 分支的生死判据：
**最少需要多少个状态**。该数只能从完整枚举的行为等价性反推，而既有证据 JSON 只保存
聚合统计。本节记录为此做的一次最小代码改动与首个实测结果。

### 21.1 代码改动（默认关闭、向后兼容）

- `MethodCHfieldAssignmentOutcomeRecord { value_mask, outcome, exact_state_ordinal }`；
- `MethodCHfieldExactPatchTableCompilation` 新增 `assignment_outcome_records` 字段；
- 仅当 `EARTHMESH_M0_LEGALIZATION_ASSIGNMENT_DUMP` 置位时在枚举循环中记录，否则为空
  `Vec`，默认路径的分配与输出行为不变；
- 状态序号取自 `exact_states` 这一 `BTreeMap` 的迭代序，因此跨运行确定；
- 探针 JSON 只在记录非空时插入该键，**既有 compiled-table 证据哈希全部保持有效**。

向后兼容已实测：新旧报告的全部共有字段取值一致，仅
`compiler_executable_sha256` 与显式传入的 `max_variables` 不同。

### 21.2 方法

设候选 seed 有序，位 `i` 选中 `candidate_seed_ids[i]`。深度 `k` 的前缀固定低 `k` 位、
其余自由。两个前缀 Myhill-Nerode 等价当且仅当**在所有后缀下 exact 结果相同**。各深度的
等价类数即该变量序下 ordered-interface DP 的**精确最小宽度**——不是任何提议编码的上界。

### 21.3 结果：NXP=81 cluster 3（8 变量 / 256 赋值）

枚举分布与既有记录逐项吻合：`hard_coverage 62`、`non_triplet_perimeter 137`、
`exact:transition_patch 54`、`boundary_incomplete 3`；`0 SAT`；33 个完整状态键，
`mixed_exact_outcome_state_count = 0`。

| 深度 | 前缀数 | Myhill-Nerode 等价类 | 最大类 |
|---:|---:|---:|---:|
| 0 | 1 | 1 | 1 |
| 1 | 2 | 2 | 1 |
| 2 | 4 | 3 | 2 |
| 3 | 8 | 6 | 2 |
| 4 | 16 | 9 | 4 |
| 5 | 32 | **14** | 7 |
| 6 | 64 | 11 | 19 |
| 7 | 128 | 8 | 49 |
| 8 | 256 | 4 | 137 |

**最小 DP 宽度 = 14。**

两次 dump 与两次分析均逐字节一致：

- `target/case9-myhill-nerode-1785463953/cluster-3-dump-{a,b}.json`
  SHA256 `1a1b8711272e587f9d3e9fb7f36b4f85e0489ccf3c846e7451c73c5a44827e73`；
- `target/case9-myhill-nerode-1785463953/myhill-nerode-{a,b}.json`
  SHA256 `badcfd89abcf36318c74879ee939a08c7fb36244fb916b7614892acc13ae1cf9`；
- 分析脚本 `myhill-nerode.py`
  SHA256 `72937a2767f77cfe709db325823f122eacbb32d11c800e8799107afd92f7e797`。

### 21.4 三点读数

**1. 状态数不随变量数指数增长。** 8 变量 / 256 赋值下，任一深度最多只需 `14` 个状态；
宽度在深度 5 达峰后回落至 4，是良性 DP 剖面。这与 §14/§15 用**提议编码**得到的
`2^103` / `2^31` 不矛盾：那些是编码的上界，本节是行为等价性的下确界。

**2. 当前完整状态键有约 2.4 倍冗余。** 编译器记录 `33` 个状态键且确实充分
（`mixed = 0`），但真正需要区分的只有 `14` 个。

**3. 这修正了 §20 的一个推论，但不推翻 §20 本身。** §20 证明「结果不是任何现有 symbolic
字段的函数」，因而当时把「充分」与「可压缩」判为不可兼得。本节表明**紧凑充分统计量存在**，
只是它不是任何已记录字段的函数，必须从行为等价性反推。两节测的是不同问题。

### 21.5 边界

- 只测了一个簇，且是十个中最小的（其余 `10–154` 变量）。宽度 `14` **不得外推**到更大的簇。
- 只对该变量序成立；换序最小宽度会变，这同时意味着存在变量序优化空间。
- 本证据集中没有 SAT（四种结局全为失败），无法检验区分 SAT 与失败所需的状态。
- 最小性是相对于**预测已记录结果标签**而言；若 DP 还须产出见证，所需状态只多不少。
- Case 9 三态状态不变，仍为 `INCOMPLETE`；不构成 `PATCH_UNSAT` 或模板族 UNSAT。

### 21.6 决定

1. **DP/BDD 分支不再关闭**，但立项前必须在更大的簇上复测；
2. 下一测量为 NXP=162 cluster 1（`16` 变量 / `65,536` 赋值）；若其最小宽度仍为几十量级，
   §13 的 residual width 路线获得实测支撑，Compiled Transition Legalizer 的 DP 分支可正式立项；
3. 若最小宽度随变量数快速增长，则关闭该分支并转向约束级分解或改规格；
4. 在复测完成前，不据本节结果修改生产选择、repair 或任何输出。

## 22. 基础分辨率阶梯与变量序实测（2026-07-31）

### 22.1 NXP 阶梯：唯一单调的杠杆，趋势延续但未清零

同一份原生 `86400×43200` landcover、`max_level=3`、`EARTHMESH_M0_CROSS_LEVEL_SUPPORT=1`，
只改 `expert.nxp`：

| | NXP=81 | NXP=162 | NXP=243 |
|---|---:|---:|---:|
| 父支撑请求 | 200（163+33+4） | 51（38+13） | **44（39+5）** |
| 失败周界条数 | 35 | 10 | **5** |
| 失败周界总长 | 952 | 241 | **160** |
| 失败类 | TransitionPatch | non-triplet | non-triplet |
| wall time | — | 198 s | 229 s |

NXP=243 的失败周界为 `[48, 54, 18, 18, 22]`：**五条中四条已可被 3 整除**
（`48=3×16`、`54=3×18`、`18=3×6` ×2），只有 `22` 余 1。

证据：`target/case9-native-nxp243-1785466978/run.log`。

**读数**：这是全部实验里唯一让所有规模指标同向、大幅、持续下降的变量。但边际收益在
递减（周界 −71% → −50%），且当前阻塞已收敛为**单条周界的模 3 余数**——一个离散量，
不会随分辨率平滑趋零。因此**不应把继续提高 NXP 当作通解**；它的正确定位是
**把问题缩小到合法化器够得着的规模**。

### 22.2 变量序对最小 DP 宽度的影响：确认为边际

对 §21 的两个 dump 重排位序后重算 Myhill-Nerode 宽度：

| 簇 | 基线序 | 逆序 | 随机最小 | 随机中位 | 随机最大 | 最优/基线 |
|---|---:|---:|---:|---:|---:|---:|
| NXP=81 cluster 3（8 变量，200 次随机） | 14 | 13 | 11 | 15 | 19 | 0.79x |
| NXP=162 cluster 1（16 变量，40 次随机） | 185 | 163 | 164 | 201 | 255 | 0.88x |

三点：

1. **随机序中位反而劣于基线**（15 vs 14、201 vs 185），说明现有候选 seed 排序已优于随机；
2. 最优仅省 `12–21%`；把 `185` 压到几十需 4–5 倍改善，变量序给不了；
3. 改善幅度随规模下降（`0.79x → 0.88x`），该方向不随问题变大而变得更有用。

**结论：`185` 基本是该簇的固有宽度，不是排序不当所致。不投入启发式序优化。**
副产品：最差序 `255` 对最优 `163` 仅差 `1.55x`，真做 DP 时用现有序或逆序即可。

### 22.3 两条线在 NXP=243 汇合

- NXP=81：10 个耦合簇，最大 `154` 变量 —— DP 宽度外推约 `10^9`，**任何已知方法都够不着**；
- NXP=162：2 个簇，`35` 与 `16` 变量 —— 16 变量实测宽度 `185`，35 变量外推约 `9,000`，可行；
- NXP=243：5 条失败周界，其中 4 条已合法，剩 1 条余数为 1。

因此 NXP 提升与 Compiled Transition Legalizer 不是互斥的两条路：
**前者把问题缩到后者的能力范围内**。下一步在 NXP=243 上落合法化 checkpoint，
测量剩余簇的变量数与最小 DP 宽度；若其规模与 NXP=162 cluster 1 相当或更小，
则首次具备跑出整网 `SAT` 的现实条件。

Case 9 状态仍为 `INCOMPLETE`；本节不改动生产选择、repair 或任何输出格式。

## 23. 粒度下沉的接入点：表结构可行性探针（2026-07-31）

§8.2 曾把「在 transition band 内做 NVB/单三角形共形闭合」列为第二选择。本节用一次
只读的结构检查确定它的接入点，结论是**该形态在现有内部表下结构性阻塞**，而在
gridfile 之后接入则不阻塞。

### 23.1 两处价数约束不同

| 位置 | 约束 |
|---|---|
| 内部 Method-C 表 `IcosahedronMPointNeighbors` | `iu: [usize; 7]`、`iw: [usize; 7]`，**定长 7** |
| 弹簧/迭代路径 | 三处独立 `if npoly > 7`：`icosahedron_spring_grid/mod.rs:67`、`method_c_nest_spring_iteration/mod.rs:475`、`method_c_spring_iteration/mod.rs:123` |
| 输出 gridfile `UnstructuredMesh` | `w_to_m: Vec<Vec<i32>>`，**变长，无上限** |

NVB 二分不保证价数 ≤ 7：形状正则三角网的价数上界为 `2π/θ_min`，最小角 `30°` 即允许到
`12`，反复二分会使顶点度数增长。因此：

- **带内下沉（产物回喂 `emit_method_c_tables`）：阻塞。** 内部表放不下价数 `>7` 的局部
  拓扑；要做必须改 `[usize; 7]` 布局，波及全部消费者，已非「加在上面」而是改内核。
- **gridfile 之后下沉：不阻塞。** `w_to_m` 变长，天然支持任意价数。

### 23.2 修正后的接入架构

```text
二十面体基底 + Method-C 嵌套（levels 1..k）
        │  内部 M/U/W 表，价数 <= 7，现有语义与冻结矩阵全部不变
        v
    gridfile（w_to_m 变长）        <-- 分界面
        │
        v
NVB 二分细化（levels k+1..）
        v
    Tri 输出（FVCOM）
```

分界面在 **gridfile**，不在 emitter。三点后果：

1. Method-C 一行不改，血缘、对偶、24 组冻结回归原样；
2. NVB 作为独立后处理阶段，输入是合法共形三角网，无界面协商；
3. **hex/MPAS 产品只用到分界面之前那段，完全不受影响**——TRiSK 所需的
   Delaunay/Voronoi 对偶在分界面之前已经写出，不被二分破坏。

### 23.3 一处判断更正

此前认为「带内下沉最外科，且 patch 边界机制已可复用」。该判断错误：已建的
`legalization_patch_boundary_check` 等机制作用在**内部表**上，而带内二分根本进不了内部表。
真正的「加在上面」是 gridfile 之后接入；带内下沉看似外科，实际要动最深的结构。

### 23.4 下一个验证点

形态二的前提是 Case 9 在较浅 pass 能产出合法 gridfile 作为 NVB 输入。NXP=243 的失败发生
在 pass 2，pass 1 已走完，因此该前提可直接实测：以 `max_passes=1`、`max_level=1` 运行同一
配置，检查是否产出通过全部拓扑硬门的 gridfile。

本节为只读结构检查，未改动任何生产代码；Case 9 状态仍为 `INCOMPLETE`。

## 24. 形态二两端实测：分界面在 gridfile（2026-07-31）

§23 论证粒度下沉应接在 gridfile 之后而非 emitter 内。本节实测其两端。

### 24.1 输入端：Case 9 浅 pass 产出合法 gridfile

同一原生 `86400×43200` landcover、NXP=243、coarse projection 关闭，只把
`max_passes`/`max_level`/`max_iter_cal` 降为 `1`：

| | |
|---|---|
| wall time | `81 s` |
| 产出 | `gridfile_NXP0243_tri.nc4`（66 MB） |
| 单元 | `840,025` 个三角形 |
| `--project-quality` | **`pass`** |
| Euler | `-252`，与预期一致 |

gridfile SHA256 `0553082fa5802d93799a433036125a1810e46d155c6c278f6385695e94242464`。

> **[更正 2026-07-31]** 该 `pass` 的范围比初稿写的窄得多。把 `max_level` 由 `3` 降到 `1`
> 同时**削掉了需求本身**：流水线自身报告记录
> `hfield.max_level=1`、`target_level_0_count=840,025`，即**全部单元的目标层级都是 0**。
> 因此 `hfield_uncovered_hard_support_bin_count=0` 与 `hfield_target_above_actual_count=0`
> 是「无需求可覆盖」，不是「15″ 需求已满足」。
>
> 该结果证明的是**Method-C 能在 NXP=243 上产出合法基底**（840,025 单元、拓扑与几何硬门
> 全通过、`base_m=32,947.83 m` 与实测 level-0 中位边长 `32.692 km` 吻合），
> 这对形态二仍是必要前提，但不构成需求侧的任何结论。

### 24.2 输出端：`dimc` 动态，硬门接受价数 > 7

`unstructured_dimc`（`unstructured_mesh_support/topology.rs:369-377`）为
`max(实际最大价数, 7)`——**7 是下限而非上限**，与内部表的定长 `[usize; 7]` 不同。

只读探针对上述 gridfile 做 `600` 次共形边二分（围绕 `100` 个枢纽顶点，每个分其星形的
全部外边，使枢纽价数由 `6` 翻倍到 `12`）：

| | 原始 | 二分后 |
|---|---:|---:|
| 三角形 | 840,025 | 841,225 |
| **W 顶点最大价数** | 7 | **12** |
| **价数 > 7 的顶点** | 0 | **100** |
| 文件维度 `dimc` | 7 | **12（自动增长）** |
| 自交 / 非法多边形 | 0 / 0 | **0 / 0** |
| Euler 特征 / 不匹配 | −252 / 0 | **−252 / 0** |
| 重边 / 悬边 / 非流形 | 0 / 0 / 0 | **0 / 0 / 0** |
| 朝向错误共享边 | 0 | **0** |
| verdict | `pass` | **`warn`** |

**全部拓扑硬门 `pass`。** `warn` 来自几何质量项（`min_angle 24.66°` 略低于 `25°` 门、
`angle_deviation_deg_max 41.34°`），是探针粗暴二分所致——它随机选枢纽、无最长边规则、
无任何质量考量。真正的 NVB 有最小角不退化的已发表结果。

探针初版曾得 `fail`，`misoriented_shared_edge_count = 1998`：脚本未保留父三角形绕向。
按父三角形的循环序替换远端顶点后归零，该计数是探针缺陷而非格式或价数问题。

证据：

- 二分后 gridfile SHA256 `8f43a5e81cfb8cc67fd4d9ce940cc9f0217d3c0370571467b2f2c24f49c2c243`；
- 探针脚本 `bisect-probe.py` SHA256 `47a42336f7e3a12ed804a97c58af81c7dbb0afa115836671fff2371ab81f896a`；
- 两次质量报告见 `/Users/zhongwangwei/Desktop/Github/EarthMesh/target/case9-nxp243-pass1-1785468956/quality` 与 `/Users/zhongwangwei/Desktop/Github/EarthMesh/target/case9-nxp243-pass1-1785468956/bisected/quality2`。

### 24.3 结论：形态二两端均已验证

```text
Method-C（levels 1..k）--> gridfile        840,025 tri, pass
                              |
                              v   分界面：dimc 动态、价数无上限、拓扑硬门全通过
                        二分细化          841,225 tri, 价数 12, 拓扑全 pass
                              v
                          Tri 输出（FVCOM）
```

Method-C 内部表的定长 `[usize; 7]` **完全未被触碰**；hex/MPAS 所需的 Delaunay/Voronoi
对偶在分界面之前已写出，不受二分影响。

### 24.4 尚未验证

> **[更正 2026-07-31]** §24.2 对二分网格跑的 `--project-quality` **未评估 hfield 覆盖门**。
> 该 CLI 只接 project/gridfile/out_dir 三个参数，不传 namelist，而
> `project_quality.rs:282` 的 `if let Some(path) = target_namelist` 使整个 hfield 诊断
> 被跳过；两次报告中 `hfield` 指标均为空。因此 §24.2 的 `pass`/`warn` 只覆盖几何与拓扑，
> **完全没有触及 hard demand**。流水线自身运行时会传 namelist，其报告才含
> `hfield_*` 门。

1. **hard demand 覆盖校验未跑**——探针按几何选枢纽，不由需求驱动；且独立
   `--project-quality` 会跳过该门（见上）；
2. **质量门阈值对二分网格是否适用未知**——`min_angle 25°` 是为 Method-C 校准的，
   二分网格角度分布不同；这回到 R5 未完成的分族校准；
3. **真正的 NVB 未实现**——最长边/最新顶点选择、compatibility chain、血缘编码全部待做；
4. 二分后网格的 hex 视图未测，且按 §23 预期不应用于 TRiSK 产品。

Case 9 状态仍为 `INCOMPLETE`；本节未改动任何生产代码，探针为只读外部脚本。

## 25. 最终产物契约：哪些字段是规范的（2026-07-31）

形态二要求最终 gridfile 是一份**自足的扁平产物**——每个单元出现一次、独立可解释、
不依赖细化历史，因为该三角网不只服务 FVCOM。本节用实测确定现有 gridfile 距离该目标
还差什么，并据此钉死字段契约。

### 25.1 现状实测（NXP=243 单 pass，840,025 三角形）

| 检查 | 结果 |
|---|---|
| 三角形唯一性 | `840,025` 个，**唯一 `840,025`，重复 `0`** |
| lineage 唯一性 | `840,027` 唯一 / `840,027` 行 |
| 连接自洽 | 拓扑硬门全 `pass`，Euler `-252` 匹配 |
| `earthmesh_m_ngr` 分布 | `{1: 836,498, 2: 3,527}` |
| `earthmesh_m_refine_level` | `{0: 839,433, 1: 594}` |

前三项说明**几何与连接层面已经是扁平最终产物**。`ngr` 混有两代嵌套编号，是
Method-C 内部语义向产物的泄露；`refine_level` 则是通用的。

### 25.2 字段消费者审计

| 字段 | 唯一消费者 | 缺失行为 |
|---|---|---|
| `earthmesh_*_ngr` | 仅 `method_c_mesh_gridfile/mod.rs:149,209`（gridfile → `MethodCDelaunayMesh` 反序列化，即 restart/续算） | `unwrap_or(1)`，不报错 |
| `earthmesh_*_refine_level` | **`project_quality.rs:117-118`**：`missing_actual_refine_level_count`、`target_above_actual_count` | `optional_values_i32_exact`，可缺失 |
| `earthmesh_*_lineage` | 唯一标识与溯源 | 可缺失 |

`project_hydro.rs`、`project_quality.rs`、`fvcom_mesh_writer.rs`、`hydro_delivery_*`、
`colm_*` **均不读 `ngr`**（grep 全为空）。

### 25.3 契约

| 字段 | 地位 |
|---|---|
| `GLONM`/`GLATM`/`GLONW`/`GLATW`、`itab_m%iw`、`itab_w%im`、`n_ngrwm` | **规范**——几何与连接，自足 |
| `earthmesh_m_refine_level` / `earthmesh_w_refine_level` | **规范**——通用分辨率标记，任何细化方式都必须填 |
| `earthmesh_m_lineage` / `earthmesh_w_lineage` | **规范**——唯一标识与溯源 |
| `earthmesh_*_ngr` | **Method-C restart 专用**，下游不得依赖 |
| `earthmesh_*_refine_level_orig` | **诊断** |

三点后果：

1. gridfile 交给 FVCOM 之外的消费者时只需规范三组，`ngr` 可忽略；
2. 二分单元**不需要发明 `ngr` 语义**——填父值或留空，restart 路径 `unwrap_or(1)` 兜底；
3. 代价是二分后的网格不能再走 Method-C restart 续算。这是正确的：它已不是 Method-C 拓扑。

### 25.4 必须补的语义：二分单元的 `refine_level`

`refine_level` 是规范字段，且 **hard coverage 验收直接读它**。§24 的探针给新三角形填 `0`，
这是错的：二分单元实际更细却标为未细化，覆盖校验会把已达标区域判成未达标（反之亦然）。
§24 那次 `warn` 通过属于探针随机选中的枢纽恰好不在 hard bin 上，是运气而非正确性。

定义方式应为**由实际单元尺度反推等效层级**，与产生方式无关：Method-C 一级为边长
`x1/3`（rad3），二分一次为面积减半、等效边长 `x1/sqrt(2)`。该定义顺带修正一处既有缺陷——
当前 `refine_level` 语义绑定 Method-C 嵌套结构，本就不是通用分辨率度量。

在该定义落地前，形态二不得用于任何 hard-demand 验收；Case 9 状态仍为 `INCOMPLETE`。

## 26. 等效层级换算：由达到的尺度反推（2026-07-31）

§25 指出 `refine_level` 是规范字段且被 hard-coverage 验收直接读取，因此二分单元必须
填真实层级而非 `0`。本节实测确定换算底数并落地该定义。

### 26.1 换算底数由实测确定，不是按名称推断

量化器为 `level = log2(h_base / h)`（`earthmesh_hfield/src/lib.rs:640,704`），即
**一级 = 边长减半、面积 `/4`**。NXP=243 单 pass gridfile 的实测中位边长：

| refine_level | 中位边长 | 比值 |
|---:|---:|---:|
| 0 | `32.692 km` | — |
| 1 | `16.181 km` | `2.02` |

与 `log2` 约定一致。**「rad3」指 stride-3 种子格的足迹形状，不是边长比**；此前按
边长 `×1/3` 推算的换算是错的。

而一次二分是**面积减半**。因此换算是整数关系：

> **1 个 Method-C 级 = 2 次二分**，`L_equivalent = L_nesting + floor(bisections / 2)`

### 26.2 落地

`earthmesh_hfield` 新增两个纯函数（不依赖栅格，因此任何产生路径都能调用）：

- `refine_level_for_cell_size(h_base_m, h_m, max_level)`——由**达到的单元尺度**反推层级，
  沿用 `topology_level_at` 的 `floor(log2(...))` 约定；
- `refine_level_after_bisections(nesting_level, bisections, max_level)`——由嵌套层级与
  二分次数推等效层级。

**取 floor 是 hard-coverage 验收所需**：单次二分只把面积减半，单元落在两级之间，必须
报较粗的一级，而不能宣称尚未达到的目标。

四个测试锁定语义，其中一个以 §26.1 的实测中位边长作正对照，另一个交叉验证两个入口
不漂移（从 level 1 二分两次，必须与直接测量结果尺度得到同一层级）。
`earthmesh_hfield` 全部 `29` 个测试通过。

### 26.3 尚未接线

本节只提供换算，**未修改任何写出路径**。二分单元的 `refine_level` 仍需在实现二分时按
此定义填写；在此之前 §24 探针产出的网格不得用于 hard-demand 验收。
Case 9 状态仍为 `INCOMPLETE`。

## 27. 覆盖退让实测：硬需求不是阻塞点（2026-07-31）

### 27.1 被检验的假设

若 Method-C 的 repair 因「任何丢失 hard-demand anchor 的候选都被一票否决」而走投无路，
则允许它**记账式退让**——保留合法化自由度、记录让掉的 anchor、把残余交给更细粒度阶段——
就能既保住 Method-C 的变分辨率能力，又只在局部启用二分。这是「无法细化时才用 NVB」的
最小实现形式。

该假设可直接实测：`MethodCHfieldDemandCoverage::validate` 是全部覆盖约束的唯一汇合点。

### 27.2 改动（默认关闭）

- `validate` 在 `EARTHMESH_M0_COVERAGE_RELAXATION` 置位时返回 `Ok`，否则行为不变；
- 新增 `uncovered_anchors()`，**不受该开关影响**，因此松弛运行报告的退让集合是精确的；
- pass 成功返回时打印 `anchors=N conceded=M`。

`earthmesh_mesh` 全部 `159` 个测试在默认（关闭）路径下通过。

### 27.3 结果：

原生 `86400×43200`、NXP=243、`max_level=3`、`EARTHMESH_M0_CROSS_LEVEL_SUPPORT=1`
加上松弛开关，release 运行 `3 min 45 s`：

```
method_c coverage relaxation child_level=2 attempt=1 anchors=40 conceded=0 first=[]
method_c coverage relaxation child_level=2 attempt=1 anchors=79 conceded=0 first=[]
method_c coverage relaxation child_level=2 attempt=1 anchors=84 conceded=0 first=[]
```

随后仍以逐字相同的失败退出：

`perimeter lengths [48, 54, 18, 18, 22] cannot be grouped into transition triples
without crossing the parent boundary`

证据：`/Users/zhongwangwei/Desktop/Github/EarthMesh/target/case9-coverage-relaxation-1785473426/run.log`。

### 27.4 读数

**假设被否证，且否得干净。** 三次 pass-2 均成功返回且退让数为 `0`——覆盖否决**从未触发**。
repair 走投无路不是因为硬需求约束太紧：放松覆盖给了它更大的自由度，**但它没用上，因为
它缺的不是自由度，是可行解**。

真正的阻塞是周界的模 3 分解本身（`22 mod 3 = 1`，且不能在不越过 parent boundary 的
条件下分解），这是拓扑约束，与需求覆盖无关。

由此排除一整类解释：

| 假设 | 状态 |
|---|---|
| Method-C 撞墙源于硬需求约束过紧 | **实测否证**（`conceded = 0`） |
| 退让少量 anchor 即可通过 | 否证：不需要退让，退让也不通过 |
| MCS / 最小修正集是出路 | 否证：修正集为空 |

**因此「Method-C 尽力细化 + NVB 补缺」这一分工无效**：不存在「Method-C 差一点点」的
中间态，它卡在一个与需求无关的拓扑约束上。

### 27.5 对形态二入口的后果

「无法细化时才用 NVB」作为原则仍然成立，但**其判定不能由覆盖退让产生**。当前可确定：

- Method-C 在 NXP=243 上能产出**合法基底**（840,025 单元、全门 pass，见 §24.1 及其更正）；
- Method-C 在同一配置下做**需求驱动细化**必然撞 non-triplet；
- 二者之间没有中间态。

故形态二的入口只能是「Method-C 出基底 → 需求在 gridfile 层面重新求值 → 二分消化」。
放弃 Method-C 变分辨率是真实代价，但实测未发现保留它的技术路径——除非先解决
non-triplet 本身。

松弛开关保留为默认关闭的诊断；生产语义未变。Case 9 状态仍为 `INCOMPLETE`。


## 28. 合法化天花板与分级细化的价数（2026-07-31）

### 28.1 `max_level=2` 也失败：天花板在 1 和 2 之间

同一 NXP=243、原生 15″ 配置，只把 `hfield.max_level` 由 `3` 改为 `2`，release 运行
`3 min 43 s` 后以**逐字相同**的失败退出：

`perimeter lengths [48, 54, 18, 18, 22] cannot be grouped into transition triples
without crossing the parent boundary`

| `max_level` | 结果 |
|---:|---|
| 1 | 通过（840,025 单元，全门 `pass`，但目标层级全为 0，即无需求） |
| **2** | **失败，周界与 max_level=3 逐字相同** |
| 3 | 失败 |
| 4 | 结构性不可能（第 4 层在 pass 2 失败时尚不存在，见 §22） |

**因此不存在「保留一级变分辨率」的中间档。** 合法化天花板落在 `1` 与 `2` 之间，
而 `1` 恰好是「无需求」。

### 28.2 §23 的价数论证有误：分级细化最大价数为 6

§23 由「NVB 价数不受 `7` 约束」推出「带内下沉喂回内部表被结构性阻塞」。该论证有两处
错误：把价数当成**方法**的性质（它是**结果**的性质），并以理论最坏界 `2π/θ_min` 代替
实际值。

红细化（1→4）在区域内部**保持价数**：原顶点入射三角形数不变，新的边中点价数恰为 `6`；
代价只出现在过渡环的 green 闭合上，且增量有界。区域越大，边界占比越小。

只读探针在 NXP=243 单 pass 网格上做分级细化（核心红细化 + 邻接三角形按共享边数
1/2/3 路闭合），种子取三个角均为价数 6 的规则三角形：

| 半径 | 核心 | 生成 | 最大价数 | `>7` |
|---:|---:|---:|---:|---:|
| 0 | 1 | 10 | 5 | 0 |
| 1 | 4 | 28 | 6 | 0 |
| 2 | 10 | 57 | 6 | 0 |
| 3 | 17 | 84 | 6 | 0 |
| 4 | 24 | 115 | 6 | 0 |
| 6 | 40 | 179 | 6 | 0 |
| 8 | 58 | 260 | **6** | **0** |

**所有半径下最大价数为 6，没有任何顶点达到 7。** 基线网格本身为
`{6: 410,228, 7: 16}` 加约 19,000 个海岸边界上的低价数点。

§24.2 那次 `dimc` 增至 `12` 的探针**是人为最坏构造**——刻意围绕枢纽顶点连续二分以制造
高价数，不是分级细化的自然行为。

**结论更正：价数不构成带内下沉的阻塞。** §23.1 表格与 §23.2「带内下沉：阻塞」应按此
理解为：`[usize; 7]` 对**任意**二分是上限，但对**分级**细化不构成约束。

### 28.3 真正的阻塞点转移到 emitter 的输入面

带内下沉仍未打通，但原因换了：`emit_method_c_tables` 的离散输入只有「选中掩码 + 有序
周界 + 固定 Method-C 表」，**没有「这是算好的局部三角剖分，请收下」这个入口**——它自己
用 `perim_fill3_method_c` 构造过渡带。

这是**新增能力**，但不是改 `[usize; 7]` 布局，量级小得多。

### 28.4 一个尚未利用的结构事实

`method_c_perimeters_are_triplets` 是**全或无**判据：

```rust
perimeters.iter().all(|perimeter| perimeter.len() % 3 == 0)
```

一条不合格即整个 pass 失败。而实测失败为 `[48, 54, 18, 18, 22]`——**五条中四条合格**，
仅 `22`（余 1）不合格，占总周界长度 `160` 的 `13.75%`。

若判据改为**按分量**：合格分量照常 materialize，不合格分量退回不细化并交由更细粒度阶段
处理，则 Method-C 可保留其大部分变分辨率能力，而不是全盘失败。该改动尚未实现，
其收益取决于失败分量所覆盖的需求占比——这是可直接测量的。

Case 9 状态仍为 `INCOMPLETE`。
