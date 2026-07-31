# 网格生成与细化算法图谱：外部技术调研报告

日期：2026-07-29
目的：系统扫描外部已有算法，补齐 `docs/mesh_refinement_method_research_2026-07-02.md` 的检索盲区，
并给出 EarthMesh v3 在图谱中的准确位置与可借鉴项。
方法：主题检索六个方法族的一手来源与综述，逐条标注置信度与出处。

> **与 2026-07-02 研究文档的关系**：该文档的五路检索全部落在「连续密度场 + 连续内核」一支
> （JIGSAW / OceanMesh2D / SCVT / FESOM / Rust 生态）。本报告补上它未覆盖的三支：
> **层次共形细分**、**octree/forest AMR 的 2:1 平衡**、**metric-based 自适应重网格**。
> 这三支恰好是 EarthMesh v3 实际所属或最接近的技术路线。

---

## 1. 摘要

> **本报告的定位（2026-07-29 修订）**：这是**外部算法图谱**，不是 Method-C 的实施方案。
> 调研结论支持**继续改良现有 Method-C**，不支持推倒重来，也**没有**发现可直接替代或直接移植的算法。

- 网格细化算法可归为六族。EarthMesh v3 的 Method-C 属于 **D 族（层次共形细分）**，
  与 **E 族（octree AMR）**「离散层级 + 局部标记 + 传播闭包 + 模板过渡」**共享范式**，
  但**合法状态空间不同**（见 §4.2）。此前「独创耦合」的说法应撤回。
- **2:1 balance 是 Method-C 闭包的一个必要子约束，不是全部闭包。** E 族为该子约束提供了
  正式定义、成熟算法与已发表的复杂度/可扩展性分析；但 Method-C 的合法性还叠加了
  canonical seed 可达、rad3 足迹闭合、vertex-only contact 消解、周界三元组分解、
  transition patch 可 materialize、parent-M 环价数上限、parent-mrlw/父边界一致性、
  hard demand 全覆盖等约束——案例 9 的实证失败大多落在这些约束上，而非层级跳变。
- **D 族并非「没有闭包」，而是拥有有限、可证明终止、materialize 之前执行的共形闭包**
  （NVB 的 compatibility chain、最长边二分的 closure）。EarthMesh 的差别在于其闭包是
  **materialize 之后的启发式修复**，且无收敛证明。
- **A/B/C 三族全部使用改拓扑算子**（插点/塌边/翻边/光顺）。NASA `refine` 明确指出：
  翻边穿插在分裂与塌陷之间，用于**逃离限制分裂/塌陷能力的拓扑构型**。这说明
  **翻边作为合法化工具是成熟做法**，但**不等于 Method-C 现在必须加入翻边**——
  它是条件性备选，见 §6.6。
- **质量定理的分布比此前描述的复杂**：A 族有 Ruppert/Chew 的角度/渐变/规模界；
  D 族的 NVB 与最长边二分有**有限相似类**与最小角不退化结果。准确的说法是：
  **p4est 的 2:1 balance 本身不保证 EarthMesh 的 Voronoi/hex 几何质量，
  Method-C 当前也没有足以替代外部网格校准的质量定理。**
- EarthMesh 所做的取舍（放弃连续自由度、换取嵌套层次与整数可对拍性）**有明确同行**：
  amatos（大气海洋，球面三角形，二分）与 ICON（二十面体三角形 2:1 嵌套）在同一侧。
- **Case 9 的最新实证把最直接的借鉴收敛到了“跨层传播”**：pass 2 的 canonical
  transition 模板依赖一批仍停留在 level 1 的父面。固定宽度扩圈不能消除该缺口；
  下一层必须能把支撑细化请求送回上一层，上一层 materialize 后再重新选择下一层。

---

## 2. 方法族分类

### A 族：连续尺寸场 + 点插入型生成器

**机制**：给定尺寸场 h(x)（或边界几何），通过插入 Steiner 点并维持 Delaunay 性质逐步细化。

**质量保证（本族最强项）**：

- **Ruppert 算法**：对角度、边长、三角形数量和渐变（grading）同时给出有保证的界；
  被称为「第一个在实践中真正令人满意的、有理论保证的网格算法」
  （[Shewchuk, Delaunay Refinement Mesh Generation](http://www.cs.cmu.edu/~quake-papers/delaunay-refinement.pdf)；
  [Ruppert's Delaunay Refinement Algorithm, CMU](https://www.cs.cmu.edu/~quake/tripaper/triangle3.html)）。
- **Chew 算法**：证明不产生小于 30° 的角（除输入小角外），但**不保证渐变与规模最优**；
  若把角度界放宽到 <26.5°，则可同时得到良好渐变与规模最优
  （[Delaunay refinement algorithms for triangular mesh generation](https://www.sciencedirect.com/science/article/pii/S0925772101000475)）。
- **前沿-Delaunay 混合（2021）**：插入「skinny」三角形的 off-center Steiner 点，
  按最短边优先以前沿方式推进，得到规模最优、最小角 <30° 的网格
  （[A 2D Advancing-Front Delaunay Mesh Refinement Algorithm, arXiv](https://arxiv.org/pdf/1808.01539)）。

**代表实现**：Triangle（Shewchuk）、JIGSAW / JIGSAW-GEO（restricted frontal-Delaunay + hill-climbing）、
DistMesh（力平衡 + 符号距离函数 + 周期性重新三角化）。

**与 EarthMesh 的关系**：JIGSAW 是 2026-07-02 文档的首选候选内核，M4 已归档（非 OSI 许可、
跨平台确定性未经实证、cell-id 稳定性冲突）。DistMesh 是 M1（HField 目标弹簧）的已发表原型——
但它**周期性重新三角化**，不冻结拓扑，这正是 M1 在固定拓扑下失败的对照。

---

### B 族：CVT / Lloyd 优化型

**机制**：迭代把生成点移到其 Voronoi 胞的质心，收敛到 centroidal Voronoi tessellation。
变分辨率通过密度函数 ρ 实现，理论关系 h ∝ ρ^(−1/4)（球面 d̃=2；原文标注为「猜想 + 数值验证」）。

**已知性质**：

- Lloyd 收敛率随生成点数按 **O(1/k²) 退化**（Du/Emelianenko/Ju, SINUM 2006）；
- **Lloyd 预条件 LBFGS**（Yang/Gunzburger/Ju）把 65 万点 SCVT 降到 128 核 22–32 分钟，
  基于重叠区域分解，显式在 Voronoi 胞上积分；
- 参考实现 MPI-SCVT（Jacobsen et al. 2013, GMD 6:1353）为个人仓库、无 release、无维护。

**代表实现**：MPI-SCVT、geogram（BSD-3，CVT/RVD/鲁棒谓词）。

**与 EarthMesh 的关系**：MPAS 生产网格的历史路线。EarthMesh 的 PCVT 调整属同族思想，
但仅作为后处理而非生成内核。

---

### C 族：Metric-based 自适应重网格

**机制**：输入不是标量尺寸场，而是**黎曼度量场 M(x)**——每一点同时规定目标单元的
**尺寸、形状和取向**（各向异性张量）。网格器用局部算子把现有网格改造成在该度量下「单位长」。

**核心算子**（各工具一致）：

- 点插入（细化大单元）
- 边塌陷（消除小边）
- 边/面翻转（拓扑操作，改善质量）
- 顶点光顺

**关键实现细节**：NASA `refine` 把**翻边穿插在分裂与塌陷之间**，明确目的是
「逃离限制分裂/塌陷能力的拓扑构型」。MMG3d 的优化算子为节点插入、节点删除、面翻转与节点移动。

**代表实现**：MMG / ParMmg（Mmg2d / Mmg3d / Mmgs）、Omega_h、NASA refine、Feflo.a、pragmatic、EPIC；
PETSc 已集成 ParMmg。

**过程性质**：本质是**迭代**过程——网格与解成对收敛，需多次求解。

**与 EarthMesh 的关系**：**需求表达力严格强于 HField**（张量 vs 标量）。对海岸线、河道走廊
这类细长各向异性特征，度量场是自然表达。但本族全部依赖改拓扑算子，与 EarthMesh 的整数嵌套不兼容。

---

### D 族：层次共形细分（EarthMesh 所属族）

**机制**：在已有网格上标记待细化单元，按固定模板细分，并用共形闭合消除悬挂节点。

**两条主要路线**：

**D1. 二分（bisection）**

- **最新顶点二分（newest vertex bisection）**：有 30 年文献积累
  （[30 Years of Newest Vertex Bisection, Mitchell/NIST](https://math.nist.gov/~WMitchell/papers/wfmICNAAM15full.pdf)）。
- **最长边二分（Rivara refinement）**：非结构三角网自适应细化的经典技术
  （[Longest-edge algorithms for size-optimal refinement](https://www.sciencedirect.com/science/article/abs/pii/S0010448513001802)；
  [Parallel Triangular Mesh Refinement by Longest Edge Bisection, SIAM SISC](https://epubs.siam.org/doi/abs/10.1137/140973840)）；
  3D 有 [8-tetrahedra longest-edge partition](https://www.researchgate.net/publication/221561756_Mesh_Refinement_Based_on_the_8-Tetrahedra_Longest-_Edge_Partition)。
- **决定性性质**：只二分一条「细化边」，产生两个更小单元，得到
  **局部细化的共形网格与嵌套有限元空间**。
  **注意**：这**不等于没有闭包**——任意标记直接二分仍会产生 hanging node，
  需要 **compatibility chain**（递归二分邻居的细化边）；最长边二分同样把这一步称为
  **closure**。其优势在于该闭包**有限、可证明终止、且在 materialize 之前完成**
  （[30 Years of NVB](https://math.nist.gov/~WMitchell/papers/wfmICNAAM15full.pdf)）。

**D2. Red-green 细分**

- **red**：1→4 细分，产生悬挂节点（不共形）；
- **green**：把有悬挂节点的邻居二分，消除悬挂节点，恢复共形
  （[Grid Refinement — Adaptive Meshes](https://www.iue.tuwien.ac.at/phd/cervenka/node14.html)）。

> 这正是 EarthMesh **v2 Fortran** 的 `OnedivideFour` + `OnedivideTwo` + `Delaunay_Lop`，
> 也是 D 族的教科书形态。

**地球科学中的代表实现**：

- **amatos**（Behrens 等，AWI/汉堡，[Ocean Modelling 2005](https://www.sciencedirect.com/science/article/abs/pii/S1463500304000599)；
  [AWI EPIC 条目](https://epic.awi.de/id/eprint/11684/)）：
  为大气与海洋环流自适应建模而生；平面/球面/体网格；三角形或四面体；**按二分细化**；
  「被标记细化的三角形在细分时保证边的共形性」；层次数据结构 + OpenMP + 空间填充曲线域分解。
  **这是与 EarthMesh 问题域逐项匹配的先例，早约 20 年。**
- **ICON**（Zängl 等）：二十面体-三角形网格上的 2:1 嵌套；文献确认可在其上高效实施 AMR
  （[Comparison of AMR techniques for NWP, arXiv 2024](https://arxiv.org/pdf/2404.16648)）。

**与 EarthMesh 的关系**：**同族**。差别不止粒度，还包括 canonical 相位、双对偶视图、
固定价数表示与父边界/覆盖约束——见 §4.2。

---

### E 族：Octree / forest AMR 与 2:1 平衡

**机制**：空间被递归对分为 octant（层级是 2 的幂），细化由指示器/尺寸函数驱动；
再通过 **2:1 平衡**保证相邻单元层级差 ≤1；最后用**模板**从平衡后的树中提取共形网格。

**2:1 平衡的定义**（与 EarthMesh 的层级跳变约束等价）：

> 任一 octant 的邻居只能是同尺寸、一半大或两倍大。该平衡有利于在树的层级之间保持平滑过渡。
> ——[Low-Cost Parallel Algorithms for 2:1 Octree Balance](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)

**关键工程事实**：

- 在 octree 的常用操作（细化、粗化、分区、节点枚举）中，**2:1 平衡历来是 CPU 时间与通信量
  最昂贵的一项**（[同上](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)）；
- **p4est**（Burstedde / Wilcox / Ghattas, SISC 2011）提出可扩展的 forest-of-octrees 算法
  （[SIAM SISC](https://dl.acm.org/doi/10.1137/100791634)；[论文 PDF](https://p4est.github.io/papers/BursteddeWilcoxGhattas11.pdf)）；
  后续 **Low-Cost Parallel Algorithms for 2:1 Octree Balance**（Isaac / Burstedde / Ghattas 2012）
  给出显著更快的平衡算法（[PDF](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)）；
- 实测规模（**2012 平衡论文**）：**5.13×10¹¹ 个 octant、112,128 CPU 核**，
  平衡总耗时约 8 秒量级，折合约 **1.7 s / 每核每百万 octant**。

> **数字勘误（2026-07-29）**：本报告初稿写「220,320 核、每进程每百万 octant <10 秒」，
> 系采信检索摘要而未回一手来源，把 **p4est 2011 SISC 论文**的 220,320 核规模与
> **2012 平衡论文**的实验混为一谈。对 2012 论文 PDF 的文本抽取核对：
> `112,128` 出现 7 次、`5.13` 与 `1.7 s` 均在、**`220,320` 不出现**。已按此更正。

**模板共形提取**：文献综述了从 octree 生成共形网格的技术，其中**基于模板**的方法在
「带 2:1 平衡约束的线性 octree」上实现；为过渡构型设计模板，从强平衡 octree 中提取网格
（[Performance Study of Parallel Octree-based Conforming Tetrahedral Mesh Generation](https://www.researchgate.net/publication/261133387_Performance_Study_of_Parallel_Octree-based_Conforming_Tetrahedral_Mesh_Generation)；
[HybridOctree_Hex](https://www.sciencedirect.com/science/article/pii/S1877750324000711)）。

**代表实现**：p4est、Dendro、SAMRAI / AMReX / Chombo（块结构）、CUBIT/Sculpt、HybridOctree_Hex。

**与 EarthMesh 的关系**：**共享范式，合法状态空间不同**。octree 有严格树结构、
固定子单元与唯一最小平衡树，Method-C 没有；且 2:1 平衡只覆盖 Method-C 闭包的一个子约束。
对应与差异见 §4.1 / §4.2。

---

### F 族：地球科学生产实践（非算法族，是应用现状）

- **E3SM RRM**：已发布 Regionally Refined Model 网格库；
  **NARRM 是第一个成功交付气候生产模拟（含 CMIP6 DECK 与历史试验）的全耦合 RRM**；
- **E3SM-Arctic**（JAMES 2025）：区域加密耦合模型；大气用立方球谱元核心 + 区域加密；
- **SCREAM 旧金山湾区 100 m**（GMD 19:795, 2026）：加州 800 m → 区域外 100 km，
  湾区二级加密至 100 m；
- **MPAS-Ocean**：多分辨率 SCVT；
- 网格生成工具链：JIGSAW-GEO → MPAS-Tools（`build_spherical_mesh` / jigsawpy /
  `jigsaw_to_netcdf` / `MpasMeshConverter.x` / `MpasCellCuller.x`）→ compass。

---

## 3. 横向对比

| | A 点插入 | B CVT/Lloyd | C metric 重网格 | D 层次共形细分 | E octree AMR |
|---|---|---|---|---|---|
| 需求表达 | 标量 h(x) | 密度 ρ(x) | **张量 M(x)** | 标记/指示器 | 指示器/尺寸函数 |
| 单元尺寸 | 连续 | 连续 | 连续 | **离散 2^-L** | **离散 2^-L** |
| 共形性 | 天然（无层次） | 天然 | 由算子维持 | **由构造/闭合保证** | **由 2:1 平衡 + 模板** |
| 改拓扑算子 | 插点 | 移动点 | 插/塌/翻/光顺 | 视实现（v2 用 LOP） | 不需要 |
| 父子可追溯 | 无 | 无 | 无 | **有** | **有** |
| 质量定理 | **有**（Ruppert/Chew） | 部分 | 无（经验） | 无（经验） | 无（经验） |
| 收敛证明 | 有 | 有（速率退化） | 迭代收敛 | 二分：构造性 | **2:1 平衡：有** |
| 代表 | JIGSAW / DistMesh | SCVT / geogram | MMG / Omega_h / refine | amatos / ICON / **EarthMesh** | p4est / Dendro |

---

## 4. EarthMesh v3 在图谱中的位置

### 4.1 共享范式（不是结构同构）

EarthMesh Method-C 与 E 族**共享同一范式**——离散层级 + 局部标记 + 传播闭包 + 模板过渡。
以下是范式层面的对应，**不构成同构**：合法状态空间的差异见 §4.2。

| octree AMR（E 族） | EarthMesh Method-C |
|---|---|
| octant 层级为 2 的幂 | `base / 2^L` |
| **2:1 balance**：邻居只能同尺寸/一半/两倍 | 相邻层级差 ≤ 1 |
| **templated transition**：过渡构型模板 | mrow 过渡行 / rad3 足迹 |
| 由 sizing function 驱动 | 由 HField 量化层级驱动 |
| 平衡是最昂贵操作 | 闭包是最昂贵操作（实测 fill-boundary 占边际成本 96.9%） |

同时在 D 族意义上是 red-green 的变体：rad3 足迹 ≈ red，mrow 过渡行 ≈ green。

### 4.2 与同族先例的差别：不止粒度

octree 具有**严格树结构、固定子单元、唯一最小平衡树**。Method-C 在共享范式之外，
另外受到以下约束，它们共同决定了一个**不同的合法状态空间**：

| 约束 | octree 是否有 |
|---|---|
| 球面二十面体 + 五边形奇点（12 个） | 无 |
| canonical stride-3 相位（全球唯一同余类） | 无 |
| Delaunay / Voronoi 双视图（tri 与 hex 同时成立） | 无 |
| rad3 足迹 / mrow 过渡行模板 | 有类似物（模板），但更简单 |
| 固定价数表示上限（`iu: [usize; 7]`） | 无（价数由树结构决定） |
| parent-mrlw / 父边界一致性 | 部分（树的父子关系天然一致） |
| hard demand 全覆盖作为硬门 | 无 |

**因此「唯一实质差别是粒度」的说法应撤回。** 粒度是其中影响最大的一项，但不是唯一一项：

| 实现 | 最小细化单元 | 可达状态集 |
|---|---|---|
| 二分（amatos/NVB/Rivara） | **1 个三角形** | 最大 |
| octree（p4est） | **1 个 octant** | 大 |
| v2 Fortran | **1 个三角形**（任意标记） | 大 |
| **EarthMesh v3 Method-C** | **stride-3 锚点上的 rad3 足迹（约 18 面）** | **小约一个量级** |

粒度粗带来的直接后果：

- **优点**：对齐由构造保证，消掉了 v2 里 `weak_concav_*` / `sharp_concav_*` /
  `ref_sjx_isreverse_judge` 等五个特判子程序；整数拓扑可位级对拍。
- **缺点**：闭包不再是「补一个三角形」而是「补一整簇面」；某些在细粒度下合法的配置
  在粗粒度下**不可达**。案例 9 的 7-bin 缺口即此类。

### 4.3 需求表达的位置

EarthMesh 的 HField 是**标量场 + Persson 梯度限制**，位于 A 族的输入侧标准做法：
逐点取 min 合成（OceanMesh2D Eq. 13）、`|∇h| ≤ g ⟹ 相邻尺寸比 ≤ 1+g`。
**表达力弱于 C 族的张量度量场**——后者可表达取向与各向异性。

---

## 5. 关键发现

### 5.1 2:1 balance 是 Method-C 闭包的**必要子约束**，不是全部闭包

E 族为「相邻层级差 ≤ 1」这一子约束提供了正式定义、可扩展并行算法
（[p4est](https://dl.acm.org/doi/10.1137/100791634)）和已发表的复杂度/通信量分析
（[Low-Cost Parallel Algorithms for 2:1 Octree Balance](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)）。

**但 Method-C 的合法性是多个约束的合取**：

```
层级 2:1
+ canonical stride-3 seed 可达
+ rad3 足迹闭合
+ vertex-only contact 消解
+ 周界长度可按三元组分解
+ transition patch 可 materialize
+ parent-M 环价数不超表示上限（7）
+ parent-mrlw / 父边界一致性
+ hard demand 全覆盖
```

真实 15″ Case 9 的实证失败**大多不在层级跳变上**：canonical seed 不可达、
vertex-only contact、non-triplet perimeter、transition patch 都曾单独导致失败，
而这些**都不是 2:1 balance 能解决的**。

因此本报告初稿「EarthMesh 的闭包 = 2:1 balance」的等式应撤回。

**仍然成立的部分**：EarthMesh 当前的闭包是**事后 64 轮上限的非单调贪心**
（shrink/fill-M/fill-boundary/grow），**没有收敛证明**，需要 `attempted_masks` 环检测——
这是非单调搜索才需要的东西。E 族真正值得借的**不是那个算法，而是那套架构**：
局部不变量明确 → 只增单调算子 → 工作队列传播 → 有限状态下终止 → materialize 前完成。
其中“唯一最小不动点”只对 octree 的 2:1 平衡子问题成立；Method-C 的组合约束尚未证明
合流性与唯一最小解。见 §6.1。

### 5.2 D 族有**有限、可证终止、materialize 前**的兼容闭包（不是「没有闭包」）

初稿写「二分自动产生共形网格，无需修复循环」，这是对文献表述的误读，应更正。

NVB 综述明确：**任意标记三角形直接二分会产生 hanging node**，需要 **compatibility chain**
或额外细分；最长边二分同样把这一步称为 **closure**
（[30 Years of Newest Vertex Bisection](https://math.nist.gov/~WMitchell/papers/wfmICNAAM15full.pdf)）。
red-green 的 green 步骤本身就是闭包
（[Grid Refinement — Adaptive Meshes](https://www.iue.tuwien.ac.at/phd/cervenka/node14.html)）。

**准确表述**：

> D 族不是没有闭包，而是拥有**有限、可证明终止、在 materialize 之前执行**的共形闭包；
> 不需要 Method-C 这种**事后启发式修复**。

差别不在「有无闭包」，而在**闭包的位置与性质**：

| | D 族（NVB / red-green） | EarthMesh v3 |
|---|---|---|
| 位置 | materialize **之前**（作用在标记上） | materialize **之后**（作用在已建表上） |
| 性质 | 递归 compatibility chain，可证终止 | 64 轮上限的非单调贪心，无收敛证明 |
| 粒度 | 1 个三角形 | rad3 足迹（约 18 面） |

EarthMesh v2 Fortran 的 `iterA/iterB/iterC/iterG` 不动点循环即 D 族形态——**同一仓库里已有先例**。

### 5.3 固定拓扑是 EarthMesh 独有的选择，且有反例证据

调研覆盖的**所有**生成器都有改拓扑能力：

| 实现 | 改拓扑手段 |
|---|---|
| JIGSAW | frontal-Delaunay，边生成边三角化 |
| DistMesh | 周期性重新三角化 |
| SCVT/Lloyd | 每次迭代重算 Voronoi |
| MMG / Omega_h / refine | 插入/塌陷/**翻转**/光顺 |
| v2 Fortran | `Delaunay_Lop` 在生产路径 |
| **EarthMesh v3** | **无**（LOP 为 test-only） |

NASA `refine` 的说明尤其切题：翻边穿插在分裂与塌陷之间，用于
**「逃离限制分裂/塌陷能力的拓扑构型」**
（[Verification of Unstructured Grid Adaptation Components, NASA NTRS](https://ntrs.nasa.gov/api/citations/20200002748/downloads/20200002748.pdf)；
MMG3d 的等价算子见 [ParMmg in PETSc, arXiv](https://arxiv.org/pdf/2201.02806)）。
EarthMesh M1 实验中观察到的
「局部护栏只推迟塌陷，挡住后转为非局部球面外心失效」，与该描述属同一现象。

**注意语义**：在 v2 与 C 族中，翻边是**合法化工具**；EarthMesh 的 M3 当初是按
**质量优化工具**评估后关闭的。这两者不是一回事。

**但这不等于 Method-C 现在必须加入翻边。** 成熟做法的存在只说明这条路可行，
不说明它是当前必要出路——翻边应作为**条件性备选**，启动条件见 §6.6。

### 5.4 质量定理的分布：不是「D/E 族没有」

初稿写「D/E 族没有质量定理」，过强，应更正。

- **A 族**：Ruppert 给出角度、边长、单元数、渐变的同时有保证界；Chew 给出角度下界。
- **D 族**：NVB 与最长边二分有**有限相似类**结果，因而**最小角不退化**（下界只依赖初始网格）；
  最长边二分另有**有限闭包**证明
  （[30 Years of Newest Vertex Bisection](https://math.nist.gov/~WMitchell/papers/wfmICNAAM15full.pdf)）。

**准确表述**：

> p4est 的 2:1 balance 本身**不保证** EarthMesh 的 Voronoi/hex 几何质量；
> Method-C 当前也**没有**足以替代外部网格校准的质量定理。

因此「质量门必须靠外部对标校准」的结论仍然成立，但理由要收窄：
不是「所属族没有定理」，而是**现有的族内定理（针对二分三角形）不覆盖 Method-C 的
rad3/mrow 构造与 Voronoi 对偶几何**。

### 5.5 EarthMesh 的取舍有明确同行

选择 D/E 族即选择：放弃连续自由度与质量定理，换取**嵌套层次、父子可追溯、整数可对拍、
分层时间步**。

- **ICON** 明确以时间步经济性为理由选择离散嵌套（连续局部加密网格受全域最小胞 CFL 限制）；
- **amatos** 在同一问题域（大气海洋、球面三角形）做了同样选择，并用二分保证共形。

EarthMesh 不是孤例，而是 D 族在**更严格格点约束**下的一个实例。

---

## 6. 可借鉴项

> **前提（2026-07-29 修订）**：本报告**没有**发现可直接替代或直接移植到 Method-C 的算法。
> 下列各项借的是**架构与范式**，不是可以照搬的实现。
> 建议方向是**继续改良现有 Method-C**，而非推倒重来。

### 6.0 目标形态：最小可行架构

不移植 p4est，也不重写 Method-C，只借它的最小架构：

1. **为 Method-C 枚举真正的局部失败谓词**（层级跳变、canonical seed 可达、vertex-only contact、
   周界三元组、transition patch、parent-M 价数、parent-mrlw……）；
2. **优先为跨层约束建立只增加 level 的闭包算子**，并用 stable lineage 表示请求；
3. **用按层级排序的工作队列传播**：较高层发现父面支撑不足时，先回到较低层补细化；
4. 每次较低层 materialize 后，**重新采样并重新选择较高层**，不能复用已失效的 mesh id；
5. **证明状态空间有限且每次请求严格提高至少一个 face level**；若 lineage 请求重复且无进展，
   明确报告当前算子集无法闭合；
6. 每个 pass 在 preflight 谓词清零后只 materialize 一次；
7. **现有事后 repair 暂时保留为安全兜底**，而不是主算法。

该终止论证目前只覆盖“层级只增、最大层有限”的跨层传播，不自动证明周界三元组、
canonical 相位、价数等同层约束也存在合法解，更不能据此声称得到唯一最小网格。

### 6.1 优先参考顺序

| 序 | 来源 | 借什么 | 为什么排这个位置 |
|---|---|---|---|
| 1 | **v2 `iterB/iterC/iterG`** | 同一网格家族的不动点闭包与价数预测判据 | 唯一在**同一格点家族**上验证过的实现，改造距离最短 |
| 2 | **p4est** | 单调传播 + 分层工作队列的**架构** | 架构成熟、有正确性论证；但只覆盖 2:1 子约束 |
| 3 | **NVB / red-green** | 有限 compatibility closure 的构造与终止性证明 | 提供「闭包为何必然终止」的证明范式 |
| 4 | **amatos** | 细化粒度下沉（二分）到共形闭合 | **仅在 rad3 粒度最终证明不可行时**才考虑 |
| 5 | **翻边** | 合法化工具 | **仅在证明合法标记在现有模板下不可达后**才重新评估 |

### 6.2 v2 `iterB/iterC/iterG`（最高优先）

**来源**：本仓库 `git show v2.0.0:src/MOD_refine.F90`
（`iterB_judge:623` / `iterC_judge:695` / `iterG_judge:826`，外层 `iterA` 不动点循环 `:212-280`）。

**为什么排第一**：这是**唯一在同一格点家族上验证过**的闭包实现。它已经具备目标形态的
全部要素——只增标记、三层判定各自迭代到不动点、materialize 之前完成。
`iterC_judge` 的价数判据（`多边形边数 + 射入射线数（相邻射线合并计 1）> 7`）
是**在标记空间预测细化后价数**的现成形式。

**限制**：v2 的标记粒度是任意单三角形，rad3 粒度下判据的系数与算子都需重新推导；
且 v2 的 `> 7.` 系数为「1→4 + 1→2 过渡三角形」导出，不能照搬。

### 6.3 p4est 的传播架构（借架构，不借算法）

**来源**：Isaac / Burstedde / Ghattas,
[*Low-Cost Parallel Algorithms for 2:1 Octree Balance* (2012)](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)；
p4est（[SIAM SISC](https://dl.acm.org/doi/10.1137/100791634)、
[论文 PDF](https://p4est.github.io/papers/BursteddeWilcoxGhattas11.pdf)）。

**借什么**：§6.0 的核心要素——明确局部不变量、只增单调算子、工作队列传播、
稳定父子标识、materialize 前完成。Case 9 当前最直接的对应是：
pass `L` 的 transition preflight 发现父面仍在 `L-1` 时，排入 `L-1 → L` 支撑请求，
先完成低层请求，再重算 pass `L`。

**不借什么**：算法本身。它只覆盖 2:1 子约束，且依赖 octree 的严格树结构与规则邻接，
在球面 stride-3 格点（含五边形奇点、canonical 相位）上不成立。

**EarthMesh 原型结果（2026-07-29）**：默认关闭的 stable-lineage 分层队列在真实
15″ Case 9 上依次补入 `163 + 33 + 4` 个父层支撑请求，并把 parent-boundary 谓词清零。
这验证了工作队列对跨层平衡子问题的适用性。随后同一候选仍出现 `21` 个 transition
self-loop 预测，并因重复 U-edge 的同层 `TransitionPatch` 退出；所以该结果不能外推为
组合闭包收敛，更不能据此把队列默认启用。详细证据见
`docs/mesh_refinement_review_2026-07-25.md` 的对应 M0 原型复验。

### 6.4 NVB / red-green 的有限兼容闭包

**来源**：D 族标准做法（[Grid Refinement — Adaptive Meshes](https://www.iue.tuwien.ac.at/phd/cervenka/node14.html)；
octree 侧的模板表见 [Performance Study of Parallel Octree-based Conforming Tetrahedral Mesh Generation](https://www.researchgate.net/publication/261133387_Performance_Study_of_Parallel_Octree-based_Conforming_Tetrahedral_Mesh_Generation)）；
EarthMesh v2 的 `OnedivideTwo` 即其实例。

**借什么**：**闭包必然终止的证明范式**——有限相似类 + compatibility chain 的递归结构。
Method-C 需要的正是「为什么单调传播一定停」的对应论证。mrow 过渡行在结构上就是 green 模板。

**限制**：NVB/red-green 的终止性建立在二分的相似类有限性上；rad3 足迹不是二分，
该证明不能直接搬，只能作为构造论证的范式。

### 6.5 amatos 的共形二分闭合（条件性）

**来源**：Behrens et al., [*amatos: Parallel adaptive mesh generator for atmospheric and
oceanic simulation*, Ocean Modelling (2005)](https://www.sciencedirect.com/science/article/abs/pii/S1463500304000599)
（[AWI EPIC 条目](https://epic.awi.de/id/eprint/11684/)）。

**为什么适用**：问题域完全一致（大气海洋、球面、三角形、层次、共形），
且明确处理了「保证边的共形性」。

**启动条件**：**仅在 rad3 粒度最终被证明不可行时**才考虑。粒度下沉会放弃
「对齐由构造保证」这一优点，并可能把 v2 那五个凹角特判子程序重新引入，
因此不是无代价的改良。

### 6.6 翻边作为合法化工具（条件性备选）

**来源**：NASA refine 的算子交错策略
（[NASA NTRS](https://ntrs.nasa.gov/api/citations/20200002748/downloads/20200002748.pdf)）；
MMG3d 的面翻转（[ParMmg in PETSc, arXiv](https://arxiv.org/pdf/2201.02806)）；
v2 的 `Delaunay_Lop`（本仓库 `git show v2.0.0:src/MOD_refine.F90`）。

**语义澄清**：在 v2 与 C 族中翻边是**合法化工具**，而 M3 当初是按**质量优化工具**
评估后关闭的——重估时应按前者立项。

**启动条件**：**仅在证明「合法标记在现有 Method-C 模板下不可达」之后**才重新评估。
成熟做法的存在只说明这条路可行，不构成现在就要加的理由。在 §8 开放问题 2
（rad3 粒度下是否存在合法上界）有答案之前，不启动。

---

## 7. 不建议借鉴的

- **C 族的张量度量场**：表达力更强，但要求改拓扑算子，与整数嵌套不兼容。
  除非同时放弃 D/E 族的全部资产，否则不成立。
- **A 族内核整体替换**（JIGSAW/DistMesh）：2026-07-02 文档已归档（许可、确定性、cell-id 稳定性），
  本报告未发现推翻该结论的新证据。
- **块结构 AMR**（SAMRAI/AMReX/Chombo）：面向笛卡尔块，与球面非结构网格不匹配。
- **直接照搬 v2 的 `> 7.` 系数**：为不同细化算子推导，形式可借、系数必须重定。

---

## 8. 待验证的开放问题

1. **2:1 平衡算法在 stride-3 二十面体格点上的适配是否保持单调与收敛**——需推导，
   octree 的规则性在此不成立。
2. **rad3 粒度下是否存在可构造的合法上界**——简单的“选中全部 canonical seed/rad3
   候选”已在真实 15″ Case 9 上产生 non-triplet perimeter，不能作为合法上界。
   “全部 parent face 细化”是否能由现有 Method-C 表达并合法 materialize 仍未证明；
   因此当前只能证明跨层 level 请求有限终止，不能证明整个组合闭包必然成功。
3. **EarthMesh 的 tri 产品相对 NGOFS2 的质量位置**——已有外部参照
   （edge-CV max 0.253、aspect max 1.878、569405 三角形、0 单元超门），
   但自身 tri 产品的同口径数字尚未并列。
4. **各向异性需求是否为真实产品需要**（海岸/河道走廊）——若是，C 族的度量场表达
   值得单独评估；若否，标量 h 足够。

---

## 9. 参考来源

**层次共形细分（D 族）**
- [30 Years of Newest Vertex Bisection (Mitchell, NIST)](https://math.nist.gov/~WMitchell/papers/wfmICNAAM15full.pdf)
- [Longest-edge algorithms for size-optimal refinement of triangulations](https://www.sciencedirect.com/science/article/abs/pii/S0010448513001802)
- [Parallel Triangular Mesh Refinement by Longest Edge Bisection (SIAM SISC)](https://epubs.siam.org/doi/abs/10.1137/140973840)
- [Mesh Refinement Based on the 8-Tetrahedra Longest-Edge Partition](https://www.researchgate.net/publication/221561756_Mesh_Refinement_Based_on_the_8-Tetrahedra_Longest-_Edge_Partition)
- [Grid Refinement — Adaptive Meshes (red-green 说明)](https://www.iue.tuwien.ac.at/phd/cervenka/node14.html)
- [amatos: Parallel adaptive mesh generator for atmospheric and oceanic simulation (Ocean Modelling)](https://www.sciencedirect.com/science/article/abs/pii/S1463500304000599)
- [amatos (AWI EPIC)](https://epic.awi.de/id/eprint/11684/)

**Octree / forest AMR（E 族）**
- [p4est: Scalable Algorithms for Parallel AMR on Forests of Octrees (SIAM SISC)](https://dl.acm.org/doi/10.1137/100791634)
- [p4est 论文 PDF](https://p4est.github.io/papers/BursteddeWilcoxGhattas11.pdf)
- [Low-Cost Parallel Algorithms for 2:1 Octree Balance](https://p4est.github.io/papers/IsaacBursteddeGhattas12.pdf)
- [Performance Study of Parallel Octree-based Conforming Tetrahedral Mesh Generation](https://www.researchgate.net/publication/261133387_Performance_Study_of_Parallel_Octree-based_Conforming_Tetrahedral_Mesh_Generation)
- [Scalable Octree-Based Mesh Generation For Finite Element Computations](https://www.academia.edu/73475789/Scalable_Octree_Based_Mesh_Generation_For_Finite_Element_Computations)
- [HybridOctree_Hex: 基于 octree 的自适应全六面体网格生成](https://www.sciencedirect.com/science/article/pii/S1877750324000711)

**点插入与质量保证（A 族）**
- [Delaunay Refinement Mesh Generation (Shewchuk 论文集)](http://www.cs.cmu.edu/~quake-papers/delaunay-refinement.pdf)
- [Ruppert's Delaunay Refinement Algorithm (CMU)](https://www.cs.cmu.edu/~quake/tripaper/triangle3.html)
- [Delaunay refinement algorithms for triangular mesh generation](https://www.sciencedirect.com/science/article/pii/S0925772101000475)
- [A 2D Advancing-Front Delaunay Mesh Refinement Algorithm (arXiv)](https://arxiv.org/pdf/1808.01539)
- [A Review on Delaunay Refinement Techniques](https://www.researchgate.net/publication/262172029_A_Review_on_Delaunay_Refinement_Techniques)
- [Size-optimal Steiner points for Delaunay-refinement on curved surfaces (arXiv)](https://arxiv.org/pdf/1501.04002)

**Metric-based 自适应（C 族）**
- [Parallel Metric-Based Mesh Adaptation in PETSc using ParMmg (arXiv)](https://arxiv.org/pdf/2201.02806)
- [Verification of Unstructured Grid Adaptation Components (NASA NTRS)](https://ntrs.nasa.gov/api/citations/20200002748/downloads/20200002748.pdf)
- [Parallel Metric-based Anisotropic Mesh Adaptation using Speculative Execution (arXiv)](https://arxiv.org/html/2404.18030v1)
- [Sequential Metric-based Adaptive Mesh Generation](https://www.researchgate.net/publication/334611811_Sequential_Metric-based_Adaptive_Mesh_Generation)
- [Anisotropic Hybrid Mesh Adaptation Using a Metric Field](https://www.researchgate.net/publication/268483593_Anisotropic_Hybrid_Mesh_Adaptation_Using_a_Metric_Field)

**球面 / CVT（B 族与地球科学）**
- [Multi-resolution unstructured grid-generation for geophysical applications on the sphere (arXiv, Engwirda)](https://arxiv.org/pdf/1512.00307)
- [Fast Spherical Centroidal Voronoi Mesh Generation: Lloyd-preconditioned LBFGS (arXiv)](https://arxiv.org/pdf/1709.06924)
- [A multiresolution method for climate system modeling: SCVT (Ocean Dynamics)](https://link.springer.com/article/10.1007/s10236-008-0157-2)
- [Delaunay mesh generation for an unstructured-grid ocean general circulation model](https://www.sciencedirect.com/science/article/abs/pii/S1463500300000056)
- [Robust and Efficient Delaunay Triangulations of Points on or Close to a Sphere](https://link.springer.com/chapter/10.1007/978-3-642-13193-6_39)

**地球科学 AMR 与生产实践（F 族）**
- [Comparison of adaptive mesh refinement techniques for numerical weather prediction (arXiv 2024)](https://arxiv.org/pdf/2404.16648)
- [Adaptive Grids for Weather and Climate Models (Jablonowski, ECMWF)](https://www.ecmwf.int/sites/default/files/elibrary/2004/10138-adaptive-grids-weather-and-climate-models.pdf)
- [SCREAM at 100 m using regional refinement over the San Francisco Bay Area (GMD 19:795, 2026)](https://gmd.copernicus.org/articles/19/795/2026/)
- [E3SM-Arctic: Regionally Refined Coupled Model (JAMES 2025)](https://agupubs.onlinelibrary.wiley.com/doi/10.1029/2024MS004726)
- [Library of Regionally Refined Model (RRM) Grids for E3SM](https://e3sm.org/library-of-regionally-refined-model-rrm-grids-for-the-e3sm-atmosphere-model/)
- [E3SM Variable Resolution Mesh Design](https://e3sm.org/variable-resolution-mesh-design/)
- [Extending legacy climate models by AMR for tracer transport (GMD 14:2289)](https://gmd.copernicus.org/articles/14/2289/2021/)
