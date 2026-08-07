# EarthMesh v3 网格构建技术指南：计算细节

版本：2026-08-04（在 2026-07-11 球面几何/拓扑/质量契约版之上，补入海洋掩膜拓扑清理、tri 弹簧默认值、质量比较容差、grid_preprocess 迁出与 h 场栅格推导）
性质：实现级技术文档。所有公式、常量与索引约定均以当前 Rust 源码核验；除第 3 节外，模块名即 `rust/earthmesh_mesh/src/` 下的目录名（第 3 节对应 `rust/earthmesh_refine_redgreen/src/`）。

---

## 0. 管线总览

```
                    ┌────────────── 初始网格 ──────────────┐
 NXP 因子分解 → 二十面体菱形展开 → 全球弹簧松弛 → Voronoi 对偶 + PCVT
 (gridinit)     (icosahedron_*)    (icosahedron_     (voronoi_grid/
                                    spring_grid)      voronoi_pcvt)
                    └──────────────────┬────────────────────┘
                                       ↓
              ┌──── 细化需求：阈值判据 / 指定区域 / h 场 ────┐
              │  (area_judge*, refine_controls, earthmesh_hfield) │
              └──────────────┬───────────────┬───────────────┘
                             ↓               ↓
   ┌── grid_preprocess 三角细化管线 ──┐   ┌── Method-C 嵌套 ──┐
   │ 打标 → iterA..G 过渡判定 →       │   │ 选面(thirdm/rad3) →     │
   │ 1→4/1→2 细分 → LOP 翻边 →        │   │ 周界 mrow → 表重映射 →  │
   │ 弱凹清理 → 重编号/排序 →         │   │ emit → nest 弹簧        │
   │ MPAS 式弹簧平滑                  │   │ (method_c_*)                │
   └──────────────┬───────────────────┘   └───────────┬────────────┘
                  ↓                                   ↓
        掩膜后处理（海陆域、孤立单元、边界曲线、水道、重索引）
        (mask_postproc_*)
                  ↓
        输出：MPAS 六边形 C-grid / FVCOM 三角网格 / CoLM 耦合文件
        (earthmesh_cli writers)          质量报告 (earthmesh_quality)
```

两条细化路线共享初始网格与输出端，方法论差异见 `docs/mesh_refinement_method_research_2026-07-02.md`；h 场层（第 8 节）是二者共同的上游。

**集成状态：两条路线都是运行时路径，由 `NL%refine_backend` 选择。** 上图左支（red-green 三角细化，第 3 节）位于 **`rust/earthmesh_refine_redgreen`**（27 个模块 + 24 个测试文件）。2026-08-06 起它不再只是"验证资产"：`refine_pipeline/global_source.rs` 按 `config.refine_backend` 分支，`"red_green"` 走 red-green 一路到 gridfile，其余走 Method-C 的 `spawn_nest_*` 与 Voronoi/PCVT 尾段。两条路线的能力**不对等**，red-green 当前只读具名区域——边界见 11.6。

依赖方向仍是单向的：该 crate 依赖 `earthmesh_mesh`，而 `earthmesh_mesh` / `earthmesh_project` / EarthMesh Studio **均不依赖它**；只有 `earthmesh_cli` 依赖它（桥接层 `redgreen_bridge` 与管线分支）。

左支同时作为**逐位对拍的内核库**保留——离散整数拓扑才能对参考实现做表级精确比对（第 6 节验收层级的 compat 模式），这份验证能力是连续/构造式内核给不了的。

---

## 1. 数据模型与索引约定

### 1.1 EarthMesh canonical algorithm 保真索引

全库刻意保留 EarthMesh canonical algorithm 约定：数组长度 n+1，**槽位 0（部分表还有 1）为占位**，有效 id 从 2 起；id 以 `usize` 存储，0/1 或负值作哨兵。循环样式 `for i in 2..=n`。删除的三角形连通行写 `[1,1,1]` 占位。此约定使整数拓扑表可与 参考基线**逐位对拍**。

**内存约定与文件边界的起点不同，这一点以前没写清。** 内存里遍历从 2 起；写进 gridfile 的逐单元元数据却是 `(1..=n)`——`m_refine_level`、`m_ngr`、`earthmesh_m_lineage` 都包含占位行 1。所以**占位行 1 必须携带一个自洽的值**，不能当成"不存在"：发射器里 `iwnew[1] = 1`、`imnew[1] = 1`，即占位行映射到自身。

实现单元血缘时踩到过这个差异：按"有效 id 从 2 起"把 id<2 的血缘填 0，输出的第一行祖先就是 0——一个不存在的行，而下游解析血缘时无从判断那是占位还是数据损坏。回归测试（`tests/method_c_lineage.rs`）断言的正是"每一行的祖先都必须是一个存在过的行"，占位行指向自身满足它。新增任何逐单元的文件级元数据时，占位行同样要给出可解析的值。

### 1.2 Method-C Delaunay 三表（`icosahedron_types`）

| 实体 | 字段 | 含义 |
|---|---|---|
| M 点（顶点） | `m_points: CartesianPoint(x,y,z)` | 球面（或平面）坐标，f64 |
| | `m_neighbors: { npoly, iu[7], iw[7] }` | 顶点度 npoly ∈ {5,6,7}；环序邻接边/面 |
| | `m_metadata: { mrlm, mrlm_orig, ngr }` | 网格细化级、原始级、代数 |
| U 边 | `{ im[2], iw[..], iu[..], mrlu }` | 两端点、两侧面、邻边、边细化级 |
| W 面（三角形） | `{ im[3], iu[3], iw[9], npoly, mrlw, mrlw_orig, mrow, ngr }` | 顶点/边/邻面、面细化级、过渡行号(isize)、代数 |

不变量（`validate_topology` 强制）：`face.iu` 与 `edge.iw` 互逆互指；活跃三元组唯一；全球网格 Euler 示性数 χ=2。**12 个五边形顶点**（`impent[12]`）是二十面体拓扑的必然（Σ(6−npoly)=12），其五价拓扑不会消除；它们与其他活跃 M 点一样参与弹簧位移（Canonical Fortran 中冻结 `impent` 的语句已被注释）。

### 1.3 grid_preprocess 侧结构

三角网格用平行数组表达：`mp/wp`（三角形中心/多边形中心经纬度，`LonLatDegrees`）、`ngrmm[i][3]`（三角形三邻接，槽位语义 = `IsNgrmm` 的对顶点编码 1/2/3）、`ngrwm[cell][..]` + `n_ngrwm`（多边形的三角形环，CCW）、`mrl_new`（1=未细化，4=已 1→4 细化）、`ref_sjx/ref_lbx`（细化标记）。

`IsNgrmm(a,b)`：两三角形共享两个顶点时返回 a 中**不在共享边上的顶点槽位**（1/2/3），否则 None——它同时充当邻接判定与对顶定位，贯穿细分/翻边/排序。

### 1.4 Voronoi 对偶映射

角色互换、**索引恒等**（无偏移）：`nma=nwd, nwa=nmd, nua=nud`；三角形 id 直接成为对偶 M 点 id，Delaunay 顶点 id 直接成为六边形 W 胞 id（与 Method-C `hex_grid.f90:voronoi()` 相同约定）。

---

## 2. 初始网格构建

### 2.1 NXP 与菱形展开（`gridinit`, `icosahedron_initial/diamonds/grid`）

二十面体 10 个菱形（diamond），每个按 NXP×NXP 划分。`method_c_gridinit_factorization` 从 NXP 分解出基础尺寸与倍增次数（选"阈值之下最大"的候选）。菱形角点经纬由黄金分割三角学给出；内部点按 EarthMesh canonical algorithm `fill_diamond` 的权重混合公式插值，权重分母 `i+j−1` 与 `2·NXP+1−i−j` 恒 ≥1。南北极行有专门覆盖。填充后派生 M/U/W 邻接（两遍法避免别名，`derive_icosahedron_*_neighbors_canonical baseline`）。

计数关系（全球）：`nwd = 20·NXP²`，`nud = 30·NXP²`，`nmd = 10·NXP² + 2`。

### 2.2 全球弹簧松弛（`icosahedron_spring_grid`，Method-C `spring_dynamics_globe` 谱系）

目标：把菱形展开的非均匀三角边长驱向准均匀。**逐迭代计算**：

1. 名义边长 `dist00 = β · 2πR / (5·NXP)`，其中 β 由 namelist 控制（当前配置默认 1.2；canonical 对拍案例常显式使用 1.0），R 为网格半径；`disto12 = dist00 / 1.2`。平面模式（mdomain≥2）`dist00 = Δx·√(2/√3)`。
2. 每条边：`dx = f32(x₂−x₁)`（**刻意单精度截断**，模拟 EarthMesh canonical algorithm 无 kind 的 `real()`；下同），`dist = √(dx²+dy²+dz²)`。
3. 对边的四条邻边（`EdgesOnedge_tri`/`iuun` 给出 iu1..iu4，即共享该边的两三角形的另四条边），用余弦定理算两对顶角的 2cos：
   `twocosphi₃ = (d₁²+d₂²−d²)/(d₁d₂)`，`twocosphi₄ = (d₃²+d₄²−d²)/(d₃d₄)`。
4. `ratio = clamp(twocosphi₃+twocosphi₄, 0.15, 1.2)`；目标长 `distm = disto12 · ratio`（等边三角形时 2cos60°×2 = 2→clamp 到 1.2→distm=dist00，即上限恢复名义长；钝角/退化则收缩到最低 0.15/1.2·dist00≈0.125·dist00）。
5. `frac_change = (distm − dist)/dist`；位移分量 `dx ← dx·frac_change`。
6. 每个 M 点累加 `x += Σⱼ dirs(j)·dx(iuⱼ)`，`dirs = ±relax`（relax 由 namelist 控制，当前配置默认 0.04；canonical 对拍常用 0.035；符号取决于该点是边的 im[1] 还是 im[2]——两端点反向受力）。**Jacobi 结构**：位移全部来自迭代开始时的快照。
7. 球面模式逐点投影回半径：`expansion = R/‖x‖`。
8. 包括 12 个五边形点（impent）在内的所有活跃 M 点都更新；Canonical Fortran 的五边形冻结分支是注释代码。
9. 坐标以 f64 累加（r8 模拟），每迭代的边差分、距离、目标长和 delta 保留 Canonical default-`real` 的 f32 舍入；全部迭代结束后整体 `x = f64(f32(x))` 一次（对应 EarthMesh canonical algorithm `xem(:) = real(xem8(:))`）。

迭代次数 namelist 控制（典型数千次）。已知理论性质：该法仅一阶最大范数收敛（Peixoto & Barros 2013 证伪了原论文的二阶声明）；自然弹簧长超临界值无稳定平衡——本实现的 clamp 与固定 β 避开该区。

### 2.3 Voronoi 对偶与 PCVT（`voronoi_grid/voronoi_gridinit/voronoi_pcvt`）

对偶 M 点（六边形网格顶点）初值 = 所环绕三角形三顶点的**重心**（算术平均/3）；随后 `pcvt` 用**外心**替换：仅当该点的三个 `iw` 邻接全部有效（`≥2`）才替换，否则保留重心（边界/占位防护，与 Method-C `pcvt()` 的 skip 条件一致）。

外心求解在**极平面立体投影**（polar stereographic）切平面内进行：以三点重心方向为投影极点，正/反投影矩阵互为转置；平面内用垂直平分线联立（Cramer），并按 |dx12| 与 |dx13| 大小选择条件更好的方程回代 xc（数值稳健分支）。退化（共线/零轴）回退重心。

球面多边形面积/定向用 **l'Huilier** 公式：半周长 s，`tan²(E/4) = tan(s/2)tan((s−a)/2)tan((s−b)/2)tan((s−c)/2)`，`sqrt` 前 `max(0,·)` 防负；跨日界线先做 ±360 单步经度校正。CCW 判定失败即反转顶点序（`GetSortNew`、`orderVerticesOnCell` 谱系）。

---

## 3. grid_preprocess 三角细化管线

> **集成状态（2026-08-06 起改造中）：内核齐备、驱动循环与接线在建，代码位于 `rust/earthmesh_refine_redgreen`。** 本节内核（iterB/C/D/E/F/G 判定、1→2 过渡细分、LOP 翻边、弱凹清理、`ngr_renew`）已逐个移植并由 73 个测试驱动；`refine_loop` 驱动循环、`num_ref_cal`、`OnedivideFour_renew` 与 CLI/GUI 接线尚未完成，因此主程序目前仍只走第 4 节 Method-C。
>
> **为什么把它扶正**：判定链遇到不可行的标记集时**扩张**它，从不拒绝——本 crate 的全部错误分支都是输入校验（数组长度、索引越界、连通性不闭合），没有一条是"这个区域形状不对"。Method-C 会拒绝：种子点阵每次跨三格、周界必须是 3 的倍数、过渡补丁伸到掩膜外两层面。这就是"任意海岸带都能细化"与"只能细化 Method-C 造得出的块形"之间的全部差别。Method-C 换来的是顶点价数锁在 {5,6,7}，那是六边形对偶可用的前提；直接吃三角形的模型（如 FVCOM）不为它付账。
>
> 本节的模块名对应 `rust/earthmesh_refine_redgreen/src/` 下的目录（其余各节仍对应 `rust/earthmesh_mesh/src/`）。它对主程序的依赖面很浅，只用到 `earthmesh_mesh` 的 `LonLatDegrees`、`is_ngrmm`、`BoundaryConnection`、`boundary_closed_curves_one_based`、`push_boundary_neighbor`、`robust_spherical_area_unit`、`spherical_centroid_degrees` 与两个 `Refine*Segments` 类型。`MethodCRefinementRegion` 留在 `earthmesh_mesh`，因为它是 Method-C 生产路径的类型。

驱动循环按"轮"（iter/level）推进：打标 → 过渡判定 → 细分 → 翻边 → 清理 → 重编号 → （最终）弹簧。

### 3.1 打标（三种来源，产物统一为 `ref_sjx ∈ {0,1}`）

- **阈值细化**（`area_judge*`, `getref*`）：对判据栅格（LAI/坡度/土壤/SST/SSH/EKE…）逐三角形取均值/标准差与阈值比较；数据经 2D/3D 归约（`getref_mean_std_*`）。
- **指定细化**：bbox（跨日界线感知）、circle（大圆距离）、corridor（沿折线扫出的管道，河流细化用这个）、closed curve（射线交点奇偶，含共享顶点退化的容差处理）、Lambert 投影域。circle 可以成链（`refinement.specified_circle` 写 YAML 列表），用来表达海岸线这类分布式需求；链的每个成员独立成圆，不串成 corridor。
- **h 场细化**（默认路径；earlier 硬掩膜仅专家模式启用）：`ref_sjx[i] = 1 ⟺ mrl_new[i]==1 且 level_at(中心) ≥ 当前轮次`。梯度限制过的场保证逐轮标记集为嵌套收缩环。

### 3.2 过渡判定链 iterA..G（`refine_iter*`）

细分只允许 1→4；为避免非法拓扑，一串"judge"内核把初始标记扩张成**可行标记集**。关键计算（已与 EarthMesh-2.0.0 EarthMesh canonical algorithm 逐行对照）：

- **iterB**（`+=` 累加语义）：每个已细化三角形向三个未细化邻居注入 `mrl_in += 2`；随后 `set_dis` 轮传播：对 `transition_sum == 4` 且存在相邻的两个 `mrl_in==2` 邻居（HHH=[0,1,2,0,1] 环序判两连）者，`mrl_bk[自身] += 2, mrl_bk[对顶] += 2`（mrl_bk 从 mrl_in 克隆起步、逐轮回写）；最终 `mrl_in ≥ 4` 者标记。
- **iterC**（`=` 覆盖语义，**与 iterB 的不对称是原版设计**，EarthMesh canonical algorithm 注释明言"此处只有 0/2 两种取值"）：
  - 五边形胞：邻接三角形 `Σmrl_new > 10`（≥2 个已细化）→ 其余未细化邻居全部标记（五边形无弱凹容忍）。
  - 六边形胞 `Σ==12`（恰两个已细化）：若二者相对（槽位 j 与 j+3），把中间两个未细化者标记（对角细化 → 视作四连）。
  - 射线传播（每轮 `mrl_bk.fill(0)` 重置，与 iterB 不同）后构造 `ref_lbx_in[cell][槽位]`；**七边帽**：对不含细化三角形的 5/6 边形，相邻两条"射入"合并计 0.5+0.5，`Σ + num_edges > 7` 则把射入三角形标记（细化后边数不超 7 的约束）。
- **iterE**：`state_sum == num_edges + 6` 识别"恰两个已细化邻接"构型并回写 `lbx_refine`（写幂等，覆盖顺序无关）。
- **iterF/G**：保护单元（`impent` 及 `edge_counts < 5` 者）的标记回收，防止五边形顶点被过渡链波及。

### 3.3 细分几何

- **1→4**（`refine_onedivide_four*`）：三边中点为两端单位向量和归一化后的测地中点；严格反对径边没有唯一中点并返回错误。父三角形连通行清为 `[1,1,1]`，四子行填入（先 [1]/[2] 槽后补 [0] 槽的 EarthMesh canonical algorithm 次序）；`sjx_child` 记父→子。
- **1→2 过渡**（`refine_onedivide_two`）：被标记的过渡三角形找邻域中**唯一**满足态邻居（正向找 `mrl_new==4`，反向找 `==1`）——扫描用 `rfind`，等价 EarthMesh canonical algorithm 无 EXIT 的后写覆盖循环（多候选时取"最后命中"，正常构型唯一候选）；对顶点 w1 与公共边两端 w2/w3 确定：公共边中点 `tempc=(w2+w3)/2` 为新 W 点，两子三角形中心 = `(w1+tempc+w2)/3`、`(w1+tempc+w3)/3`。新点编号 `m₁,m₂ = num_mp[iter−1] + 2k+1, 2k+2`；`w₄ = num_wp[iter−1] + k+1`（EarthMesh canonical algorithm 先加后用的计数惯例）。
- 全程日界线规则：三点极差 >180° 触发 `CheckCrossing`（±360 单步），落点再校正回 [−180,180]。

### 3.4 LOP 翻边与弱凹（`refine_lop*`, `refine_isreverse_judge`, `refine_boundary*`）

Lawson 式对角交换：共享边三角形对 (a,b,c)/(a,b,d) → (c,d,a)/(c,d,b)，同时更新两三角形与四外邻居的 `ngrmm`（槽位相对、非几何绕向——`IsNgrmm` 编码保证一致性）。变体：`_sharp/_weak/_pair/_weak_pair` 分别处理锐角、弱凹、成对镜像折叠（`num_end−k` 折叠索引、`step_by(4)`，两端向中间收）。弱凹段构造含偶/奇配对 `k%2==0→k−1 else k+1`。方向判定 `isreverse` 用段压实游标（无匹配不推进，避免空洞）。

### 3.5 重编号与排序（`refine_renewal*`, `get_sort_new`）

`ngr_renew` 重建 `ngrwm/n_ngrwm`（`.skip(2)`——槽位 1 为 EarthMesh canonical algorithm 空行约定）；`GetSortNew` 对每个多边形的三角形环做邻接行走排序（起点取首个度 1 三角形，闭环取槽位 0；断链回退取第一个未用），`robust_spherical_area < 0` 则整环反转为 CCW。

### 3.6 MPAS 式六边形弹簧（`spring_dynamics/spring_edge_dynamics`）

结构同 2.2，但作用于六边形 C-grid：边邻居由 `EdgesOnedge_tri(4,·)` 给出，目标长来自 `distsOnEdge`（`target = distsOnEdge/1.2 · ratio`），方向符号 `CellsOnEdge(2,iu)==iw → +relax`。**精度口径**（对照 `MOD_grid_preprocess.F90:816-819` 修正后）：坐标差分量 f32 截断、r8 求模，自身边与邻边共用同一 `dist` 数组。全零距守卫返回 None（比 EarthMesh canonical algorithm 稳健）。

---

## 4. Method-C 嵌套细化

**出处**：Walko, R. L., & Avissar, R. (2011). *A direct method for constructing refined
regions in unstructured conforming triangular–hexagonal computational grids: Application
to OLAM.* Monthly Weather Review 139(12), 3923–3937. DOI `10.1175/MWR-D-11-00021.1`。

本节实现的就是该文的方法。论文的两步流程与代码逐条对应：① 闭区域内三角形连三边中点做
四分；② 外侧构造一到多行**过渡行**，以保持 conforming（不产生 hanging node）、控制分辨率
突变、并使三角形尽量接近等边——其手段之一正是把顶点价数限制在 {5,6,7}。论文自陈**主要
贡献即过渡行的构造方法**，这解释了为什么 4.2 节的周界规则如此严格，也解释了 §4.5 里半径
阶梯的行数只能实测、不能推导。

论文同时给出了指定细化区的便捷方式：**一串点的经纬度加一个影响半径**（radius of
influence）。这正是 §4.5 "点+半径"路径所用的机制——它不是本项目发明的表示法，是对该文
已有接口的复用。

### 4.1 选面（`method_c_selection*`, `method_c_spawn_hfield`）

选择的产物是 `selected: Vec<bool>`（W 面掩膜）。合法掩膜的三要素（缺一则周界行走器报 "exceeds 7-edge ring"）：

1. **thirdm 步进种子**：从起点沿"隔二取三"的格行走（stride-3 lattice）扩张 M 点种子集；每个弹出点检查其所有边 `mrlu == mrlo`（越代即报 "crosses the parent boundary"）；邻居入栈条件 `jdone 遍历数 < 2 且 被需求包含`。
2. **五边形格锚定**：若任一五边形被需求集包含，行走**必须**从该五边形起步（把 stride-3 子格钉在二十面体框架上；这是边界 3 对齐的来源）。区域路径还有"五边形仅邻近 → 从其行军至区域"的细化分支。
3. **rad3 足迹**：每个种子标记其半径 3 环内的 W 面，按种子的 `mrlm` 过滤 `mrlw == mrlo`（只选同代面）。掩膜 = 足迹并集——天然肥厚平滑。

h 场模式（M4）以 `level_at(质心经纬) ≥ pass` 替换几何包含，其余机制同源；逐 pass 1..max_level 推进，空选择即干净停止。

### 4.2 周界与过渡行（`method_c_perimeter*`, mrow）

细化边界强制为**3 的倍数条粗边的直线段**；跨原 2 粗行的空隙精确布 **3 条过渡行**（EarthMesh canonical algorithm `spawn_nest.f90` 注释原文语义）。`perim_mrow` 从边界行（mrlw 失配处）向两侧交替扩散行号：`mrow_temp2 = mrow ± jrow`，`jrow = mod(irow,2)`，循环 `2..=2·max_mrows`。顶点度全程限 {5,6,7}。掩膜不合法时 `method_c_mask_annealing` 单调侵蚀修复（上限 32 轮，全消或全保即停）。

### 4.3 表重映射与发射（`method_c_method_c_*`, `method_c_emit`）

每个被选面 1→4：分裂边（split-U）中点 = 端点加权平均，**先累加、最后统一投影半径**（`project_to_radius` 门控，与测试预言一致）；`perim_fill3` 处理过渡带的 iu 槽位改写，两种镜像模式——匹配槽 j 时写 (j−1) mod 3（"after"）或 (j+1) mod 3（"before"），与 Method-C `spawn_nest.f90:1443-1506` 的 if/elseif 链逐一对应。`emit_method_c_tables` 三条 id 分配环（iwnew/iunew/imnew 首见门控）重建全表，子面 `mrlw = mrlo+1`，出口强制 `validate_topology`。

### 4.4 nest 弹簧（`method_c_nest_spring*`）

在 2.2 公式上叠三项：

- **级缩放**：`target_base = (dist00/1.2)/2^(mrlu−1)`（每细化级目标减半）。
- **mrow 乘子**（过渡行几何渐变，即"级内伸缩"的原始雏形）：按边两侧面的 mrow 对查表
  `{(−2,−2):7/6, (−1,−2):8/6, (−1,−1):9/6, (1,−1):10/6, (1,1):11/12, 其余:1}`。
- **面积防退化**：`dmin = dist00/2^(max_mrlu−1)`，`minA² = 0.1875·dmin⁴`（0.1875=3/16，边长 a 等边三角形面积平方 = 3a⁴/16）；局部两三角形 Heron 面积平方 `s(s−d)(s−d₁)(s−d₂)` 取小者，`area_ratio = max(minA²/localA², 1)` 只放大不缩小目标长——防止过渡区三角形被压塌。

可动点 = 目标代（ngr）中邻接 `mrow ≠ 0` 面的 M 点（`move_interior` 可扩为全代）。仅对 `moveu/compu` 掩膜内的边计算（选择性 stencil）。h 场变体（M2）以 `h(边中点)/1.2` 直供 `target_base`、乘子恒 1、`dmin = min h`。

---

## 5. 掩膜后处理（`mask_postproc_*`）

- **域标记**：`IsInDmArea ∈ {0 占位, −1 陆, 1 海}`，landtype 栅格采样按 1024² 瓦片缓存或 ≤256MB 整读（逐点读法已废除——曾占 ocean 案例 30% 耗时）。
- **孤立海剥离**：顶点邻接计数新旧对照逐层内收（`num_add==0∨1` 停）。此为 `mask_postproc_isolated`，仅在 `mask_restart` 路径（`--run-mask-restart-ocean`）生效。
- **最大连通水体保留**（`mask_postproc_components`，2026-08 新增，`--project` 路径的 landtype carve 使用）：按中心点采样的海陆切割会在窄海湾与河口留下与主体不共边的碎块，触发 `orphan_cell` / `disconnected_mesh` / `non_manifold_vertex_fan` 三类 fail，且细化只会让碎块更多（分辨率越细，被"看见"的小海湾越多），AutoRefine 也修不了——`is_refinement_repairable` 只认 `min_angle_deg`/`aspect_ratio`/`cell_edge_length_cv`/`angle_deviation_deg` 四项。清理分两步交替迭代至收敛：① 以**共边**（共享两顶点，与质量检查器同口径）求连通分量，只保留最大的；② 对捏合顶点（其入域单元分成多个不共边扇区，即水体在一点自接触）只保留最大扇区——单靠 ① 消不掉它，因为两个扇区常在别处重新连通。平局按最小 canonical id 决定，保证确定性。由 `NL%isolated_ocean` 控制，Project lowering 对 `oceanmesh` 默认开启，`expert.isolated_ocean` 可覆盖。**这是破坏性清理**：被判为非最大水体的单元直接移出域，日志逐次打印删除数、捏合来源数与分量数。
- **边界闭合曲线**：海陆界 vertex-vertex 双邻接链 `bdy_ngr[2]` 行走成环（`num_points<3` 报错）；`num_bdy_long = [最长长度+1, 次长+1, 最长曲线 id]`（两槽位最大值跟踪，2026-07 修正版）。
- **水道加宽 / 顶点仅触海填充**：模板见 `mask_postproc_waterway`（首边即断的 EarthMesh canonical algorithm 惯用扫描）。
- **重索引**：old→new 映射一次生成、施加到所有引用表，占位槽 0/1 保留。
- 产物流向 FVCOM/OBC/CoLM writers 与最终 gridfile。

---

## 6. 质量度量与验收（`earthmesh_quality`, `check_mpas_mesh_topology`）

几何：面积由球面三角剖分得到，边长使用 haversine 大圆距离；每个顶点把前后大圆方向投影到该点切平面，以 `atan2` 和球面 winding 得到含凹角的严格球面内角。规则 n 边形的理想内角包含球面超额 `((n−2)π+a)/n`。紧致度使用球面等周归一式 `Q_s=a(4π−a)/l²`（`a=A/R²` 为 steradian 面积，`l=P/R` 为弧度周长），有效范围 `[0,1]`。aspect 仍为最长/最短大圆边。NaN 顶点单独计数并整体隔离出统计。欧氏 `triangle_eta_local/triangle_nsr_local` 仅是局部兼容量；任一边超过 15° 的粗球面单元会跳过二者并增加 `local_shape_metric_excluded_cell_count`。若没有局部样本，这些字段以 N/A（JSON/CSV 为 `null`）输出，其门禁跳过。拓扑检查覆盖索引越界、非流形边（>2 面共享）、孤儿胞、邻接互惠、共享边连通分量和非流形 vertex fan。Euler 示性数 `V−E+F` 始终报告；仅在调用方显式提供 expectation 时门控。最终 mask 拓扑未知前，Project 只对未裁切的 global Earth/atmosphere 给 χ=2；land/ocean/coupled 和所有 regional 结果都可能因掩膜产生边界、孔洞或多分量，因此保持 infer-only。门控分级 Pass/Warn/Fail，阈值默认 min_angle 5/20°、aspect 4/10（严格比较口径统一）。

球面多边形面积还提供结构化 API：同时返回 signed-minor、minor、major-complement、边界左侧 oriented 面积和 winding；major 只是补集候选，不会被自动解释为业务 interior。少于三个点、非有限/越界坐标、连续重复点、反对径边、自相交、零面积退化或三角剖分歧义均返回结构化错误，避免为共大圆退化 ring 伪造 `4π` 补集。旧的 `f64` 面积函数保留为明确的 minor-area 兼容包装，并把退化面积映射回历史 `0`。

质量报告记录 `cell_view`（`tri` 或 `hex`）以区分三角主单元视角和六边形/C-grid 视角；hex 视角优先消费 gridfile 的 authoritative `itab_w%im/n_ngrwm`，并在 W 点局地球面切平面按方位角排序，跨日界线的有效胞不会再被原始经度极差过滤。边数分布（三/四/五/六/七/其他边胞）作为报告观测项输出，是否合格由门禁和拓扑问题表判定。任何触发 edge-CV Warn 的单元也会写入 `worst_cells.geojson`，而不是只在汇总门中出现。

EasyMesh 给 EarthMesh 的实际借鉴限定在质量链路：三角主单元、拓扑诊断和质量门禁分开表达；EarthMesh 因此按 `tri-strict`/`hex-cgrid` 选择验收视角，而不是把边数分布混作失败条件。

验收层级：**compat 模式** = 整数拓扑逐位对拍（对 参考基线）；**fast/h 场模式** = validate_topology + 质量报告 + 行为断言（本仓库 M1–M4 测试即范本）。

---

## 7. 数值约定（全库统一）

| 约定 | 细节 |
|---|---|
| 地球半径 | `earthmesh_core::EARTH_RADIUS_METERS = 6_371_229.0`（EarthMesh canonical algorithm `erad`），geometry 由其换算 km，hfield 直接复用/导出该常量 |
| 混合精度 | 公共坐标接口存储 f64；与 EarthMesh canonical default-real 直接对拍的 source-grid、global spring 和 nest spring 路径，在坐标差、距离、目标长、位移累加和最终坐标写回处以 `as f32 as f64` 复刻单精度语义。`_f32` 投影变体同样保持该契约；h-field nest 复用同一兼容 kernel，输入边长先量化到 f32。精度截断是对拍的一部分，不能静默替换为全 f64 |
| 日界线 | 统一 ±360 单步校正（`CheckCrossing`/`unwrap_lon_around`/锚点展开）；跨界判据 = 极差 >180° |
| 极点/反对径 | 所有 `acos/asin` 与 haversine 中间量显式 clamp 到合法区间；测地距优先 atan2 形式；反对径质心/中点因不唯一而返回 None/Err |
| 定向 | CCW 基准 = `cross(v_i−c, v_{i+1}−c)·ĉ > 0`（球外视角），负则反转；退化面积 `max(0,·)` |
| 确定性 | 拓扑与集合构造使用固定遍历序、BTreeSet/稳定序打破平手；global/nest spring 采用 Jacobi 双缓冲，并仅把互不重叠的 edge/point 输出槽交给 Rayon 并行，不做浮点并行归约。单槽内累加顺序固定，且有 1/2/4 线程逐位一致测试 |
| 除零守卫 | 距离/面积/模长为零一律显式返回 None/Err（比两份 参考基线都严格） |

---

## 8. h 场层（`earthmesh_hfield`，2026-07 新增）

统一"阈值细化 + 指定细化"的连续目标尺寸场：

1. **合成**：`h(x) = min_i h_i(x)`。判据栅格 → h 场（推荐 FESOM 线性式 `r = clip(s/s_t, 1, r_max)`, `h = h_base/r`）；区域（bbox/circle/polygon/corridor，全部日界线安全）→ 域内钉 `h_inside`（硬边界，交给限制器造坡）。
2. **梯度限制**：解 `|∇h| ≤ g` 的最大下界场 `h*(x) = min_y (h₀(y) + g·d(x,y))`（Persson 定理）。实现为球面 fast sweeping：4 序确定性扫描，双轴上风 eikonal 局部解（1D 候选 `a+g·Δx` 与二次联立 `((h−a)/Δx)² + ((h−b)/Δy)² = g²` 取小），经度周期、逐行 `cosφ` 度量。**推论**：邻胞尺寸比 ≤ 1+g；量化后每级环带宽 ≈ `h_level/g`（≈0.7/g 行），Method-C 套娃净距在 g ≤ 0.22 时构造性满足。栅格须解析局地 h（间距 ≤ h，理想 ≤ h/2）。
3. **栅格尺寸**（2026-08 起由 Project lowering 推导）：h 场在**三角形中心与边中点**处被采样，因此第 2 点的"间距 ≤ h"不是建议而是硬约束。栅格过粗时 `level_at` 采出的是混叠后的锯齿边界，Method-C 收到不合法掩膜，直到 `perim_fill3` 深处才报 `transition patch has no solid split edge`。

   引擎默认固定 720×360（经向间距约 55 km），**连单级 NXP 81 所需的 49 km 都不满足**，而 Project lowering 从不覆盖它——这是多级海洋细化在生产分辨率下跑不通的直接原因。

   `hfield_raster_size()` 按下式推导，显式 `nlon`/`nlat` 优先：

   ```text
   h_min = 2πR / (5 · NXP · 2^L)
   nlat  = ceil(πR / (h_min/4))     即  nlat = 10 · NXP · 2^L
   nlon  = 2 · nlat
   ```

   系数 1/4 是**实测值**，不是文档里那句"ideally half"：单级 NXP 81 在恰好 `h_min/2`（nlat 811）仍然失败，`h_min/4`（1620）才通过，`h_min/8` 无额外收益。两级之所以在自己的 `h_min/2` 通过，只因其 `h_min` 更小、绝对间距同样落在 ~12 km——**约束是绝对间距，不是比值**。下界 360（不使任何既有配置变粗），上界 8192。

   **代价为零**：同一算例 842×421 与 3240×1620 均耗时 42 s，梯度限制远不是瓶颈，网格生成才是。因此公式刻意取保守值，而不去找"恰好够用"的最小栅格。同理，按域裁剪栅格不值得做——数学上安全（`HField` 无 origin/extent、隐含全球；留 `(h_base−h_min)/g` 余量即与全球计算逐点相同，NXP 81 两级约 370 km），可省约 227 倍单元数，但省不出时间。

   **两端都会坏，需求满足度是判据**（2026-08 实测，NXP 21 两级）：

   | 栅格 | 请求面 | 可细化 | 最终单元 |
   |---|---|---|---|
   | 720×360（旧默认） | — | 全部 | 46516 |
   | 840×420（推导值） | — | 全部 | 46653 |
   | 3360×1680（`h_min/16`） | 5495 | **547** | **8160** |

   过粗 → 选面锯齿 → 显式报错；**过细 → 需求碎成小于一个 rad3 足迹的斑点 → 只有恰好装得下足迹的地方被细化，其余静默丢失**（曾 `exit 0` 交付比请求少 82% 的网格）。后者更危险，因此 `selected_faces_and_coverage_*` 现在直接度量**未满足需求**并在超过一半时报错：

   ```text
   demanded = ∪ 需求锚点的面
   unmet    = demanded 中未被 selected 覆盖者
   报错 ⟺ unmet > demanded / 2
   ```

   判据取"未满足面数"而非"被裁锚点数"是有原因的：裁掉父层围裙上的锚点是**合法设计**（它避免深层内部 pass 因边界行失败），且这类锚点所在区域仍会被同分量内别处的足迹细化。也不能取"存在无 alignable 面的连通分量"——海岸线是一整个连通分量，少数几个合法种子即可通过该检查，而其余部分照样丢失（实测该判据把上表第三行误判为健康）。

   **窗口以「每个基础单元跨多少栅格格」表达时归位**（2026-08 实测）：

   | `h_base/间距` | 结果 |
   |---|---|
   | 4 | ❌ 混叠，`perim_fill3` 报错 |
   | 6.9 – 12 | ✅ 两种分辨率均通过 |
   | 16 | ⚠️ NXP 81 通过、NXP 21 失败 |
   | 32 | ❌ 需求碎片化 |

   `nlat = 20·NXP` 恰为比值 8，落在窗口正中，且**与层数无关**。曾用过的 `h_min/4` 依赖层数，把低 NXP 多级推出窗口细端——NXP 21 两级推出 `nlat=840`（比值 16，失败），而引擎旧默认 360 反而能跑，那是一次真实的回归。窗口**宽度**仍与分辨率有关（16 在 NXP 81 通过、NXP 21 不通过），说明还有未找到的变量，因此取窗口中点而非任一边。

   与层数解耦还消掉了内存顾虑：NXP 21→420（2.7 MB）、81→1620（40 MB）、162→3240（160 MB），上界 8192 需 NXP 超过 410 才会触及。

   曾被否掉的三个候选变量，留作后来者的排除记录：

   | 假设 | 反例 |
   |---|---|
   | 绝对间距 | 6.2 km 通过（NXP 81 两级），23.8 km 失败（NXP 21 两级） |
   | 间距 / `h_min` | NXP 21 两级的公式值 `nlat=840` 恰好落在失败区 |
   | 间距 / `h_base` | 比值 0.062 在 NXP 81 通过、在 NXP 21 失败 |

   兜底仍在：未满足需求检查会显式报错并提示可调项，最坏情况是要求用户改配置，而非静默交付坏网格。

   **点+半径路径已验证可行**（2026-08，`earthmesh_mesh/tests/point_radius_coastal_demand.rs`）。用同一份海岸需求（按引擎口径 `landtype != 0` 从 landtype 提取的 1° 海岸块）构造圆链，直接调 `spawn_nest(&regions, ...)` 绕开 namelist 层：

   | 配置 | 结果 |
   |---|---|
   | 单级，19 圆 r=150 km | ✅ 通过 |
   | 两级，外 300 km + 内 150 km | ❌ `crosses the parent boundary` |
   | 两级，外 1200 km + 内 150 km | ✅ 通过 |

   约束是**层间距必须大于该层的父单元**，与 `perim_fill3` "子层不得贴近父层边界"的构造前提一致。父单元逐层减半，所以半径可以越往里收得越紧。**这个约束是几何的、可预先计算的**——与栅格那个至今找不到决定变量的经验窗口形成对照，这是"栅格是否必要"那条议题的实证支撑。

   **深度可达 Method-C 的 5 级上限**（同一测试文件）。NXP 21 下各层所需间距为 381 / 191 / 95 / 48 km，取半径 2000/1200/700/400/200 km：

   | 层数 | 面数 | 最深 mrlw | 有效分辨率 |
   |---|---|---|---|
   | 1 | 8821 → 9823 | 2 | 381 → 191 km |
   | 3 | → 12787 | 4 | → 48 km |
   | 5 | → 17341 | 6 | → 12 km |

   即从 381 km 基础网格细化到 12 km，总面数仅增加 96%，五层合计 1.1 s。对照之下 h 场路径连两级都依赖栅格恰好落在那个经验窗口内。

   **判据与归约已分层**（`earthmesh_cli/src/refinement_demand/`，2026-08）。细化不只有海岸：判据目录已有 14 项（陆 `lai`/`slope`/`dem`/`slope_max` + 4 个土壤导热参数，海 `sst`/`ssh`/`eke`/`sea_slope`，气 `typhoon`），外加分类判据 `landcover`，海床是下一个。这些判据问的是同一个问题——**这个源栅格格点需不需要更细的网格**——答案永远是一张 bool 栅格，而"bool 栅格 → 圆链"的归约与谁提出需求无关。所以两半拆开：

   | 层 | 内容 |
   |---|---|
   | 判据层 `refinement_demand::{landtype, threshold}` | 数据源 → `RefinementDemand`（bool 栅格）。新增判据只加生产者 |
   | 归约层 `reduce_demand_to_circles` | 任意 `RefinementDemand` → 圆链。半径下界、半个半径分块、重叠一半都在这里 |

   多个判据用 `RefinementDemand::union_with` 并成一份需求再归约一次，与 h 场取 `min h_i(x)` 对位——两条路线吃的是同一个输入，因此可以在同一判据上互相对照。归约层的测试全部用手工构造的需求，不碰任何数据源，这本身就是解耦的证明。

   半径下界 `materializable_radius_meters = 0.4 · base_cell`（实测：NXP 21 父单元 381 km 时 150 km 可细化）。归约保证**每个被标记的格点都落在某个圆内**：块是半个半径宽，格点到块心最远约 0.35 个半径，由构造保证。

   **海岸判据顺带修了一个漏洞。** 原先问的是"这个块里既有陆又有海"，海岸若恰好沿块边界走，陆块里没有海、海块里没有陆，两块都不算海岸，圆链直接漏掉这一段；块越小越容易踩到。改为逐格边界检测（与四邻中任一格类别不同即标记）后不存在这个盲区。南海实测（landtype 240/度，108–120°E / 18–26°N）:

   | 半径 | 旧规则 | 逐格边界 |
   |---|---|---|
   | 150 km | 54 圆 | 54 圆 |
   | 80 km | 112 圆 | **117 圆** |
   | 45 km | 218 圆 | **225 圆** |

   半径大时块大、踩中概率低，所以 150 km 两者一致；45 km 时旧规则漏掉 3%。合成回归测试（`tests/refinement_demand_landtype.rs`）取 1 格宽的块——单格必然只有一个类别，旧规则在那张栅格上永远不可能触发。

   另修一处继承来的边界缺陷：`floor + 1` 的经纬度→索引映射在**恰好 180°E / ±90°** 处会算出比栅格多一格的索引，而窗口读取器拒绝越界 bounds，于是贴着这些边缘的计算域会直接报错而不是返回海岸线。索引映射现在钳到源维度，判据读 halo 时也一并钳位（`halo_within_source`）。

   端到端测试（`tests/coast_refinement_regions.rs`）：真实 landtype → 圆链 → `spawn_nest` 细化成功，1.3 s；内陆 bbox 正确返回空。

   **Project schema 已支持圆链**（2026-08）。`refinement.specified_circle` 从单个对象放宽为 `SpecifiedCircleRefinements::{One, Many}`：既有工程写单个 mapping 照旧解析、lowering 输出逐字节不变（`inline:circle:...`），写成 YAML 列表则降为新语法 `inline:circles:lon=..,lat=..,radius_km=..;...`。至此从 Project/GUI 到内核的整条圆链路径打通，h 场不再是表达分布式需求的唯一途径。

   该枚举**手写 `Deserialize`（按 map/sequence 分派）而非用 `#[serde(untagged)]`**：untagged 在所有分支都失败时只报 "data did not match any variant"，把 `lonn:` 这种拼写错误的提示从"未知字段 `lonn`，可用 `lon`/`lat`/`radius_km`"降级成一句无信息的话。分派后内层错误原样保留，列表形式还会带上索引（`specified_circle[0]: unknown field \`lonn\``）。序列化仍走 untagged，输出形状不变。

   实现上有一处语义必须分开：`region_sources` 里多点共用一份半径的既有约定是 **Corridor**——沿折线扫出的管道，正是 v2 用来细化**河流**的形状（`examples/merit_hydro/gba/case.nml` 是可运行实例）。海岸圆链不是折线，它是栅格扫描出的一组互不相干的圆，串成 corridor 会在扫描换行处横跨地图连出假管道。所以 `inline:circles:` 逐个成圆下发（各自复用同一套父级 halo 推导），不走 corridor 分支；`merge_refine_regions_by_shape` 再把半径重叠刻意造成的重复圆折掉。域（domain）方向则明确拒绝圆链——域是单个区域，圆链描述的是细化需求。

   **尚未做的一环：判据不逐轮重算，分辨率相关的判据因此是错位的。** 现在三条路径都在细化开始前把需求算完：

   | 路径 | 判据在哪算 | 是否逐轮 |
   |---|---|---|
   | Method-C 直接路径 | `refine_pipeline/global_source.rs` 一次性组装 regions 再调 `spawn_nest` | 否 |
   | h 场 | `read_threshold_stats_on_hfield_masked` —— 在 **h 场栅格格点**上 | 否 |
   | area_judge/getcontain | 每轮读当前网格算 `IsInRfArea_sjx`，但只是对**固定的栅格掩膜**做包含判定 | 判据不重算 |

   `getref_mean_std_*`（逐三角形均值/标准差）**未移植到 Rust**；第 3.1 节那句描述的是参考 Fortran 算法。

   但"在网格单元上求判据"并非完全没有先例：水文交付路径已经这么做了。`hydro_delivery_intersections/writer.rs` 的 `write_earthmesh_intersection_geojson` 为**当前网格的每个单元**建一个 Lambert 等积平面，把单元与河道走廊都加密成大圆弧后投影求交，再按单元的球面面积归一，得到逐单元的 `river_fraction` / `coastal_fraction`；`hydro_delivery_refine_workflow` 随后按这些分数打分决定是否细化。所以缺的不是"逐单元评估"这个概念，而是**栅格源**的那一半——矢量源已有一套经纬缠绕无关、面积保守的实现可以照搬其结构。

   判据要分两类看：

   - **与分辨率无关**（`sst > 28°C`、`slope > 15°`、离海岸 < 50 km）:一次算清即可，逐轮重算得到同样答案。
   - **与分辨率有关**（单元内 landcover 异质度、子网格标准差、地形未解析方差）:必须逐轮重算，因为"还要不要再细"问的就是"这个单元里还剩多少没解析的变化"。

   **h 场里的三个"单元装了什么"判据已改为按层扫描**（2026-08）。`refine_num_landtypes`（单元内类别数）、`refine_area_mainland`（主导类别占比）、`th_sea_ratio`（海陆比）都属于第二类，而原先三个都在 h 场栅格格点上算：那个格点大小由基础分辨率推出（间距 = `h_base/8`），与被判定的单元无关，于是**每一层得到同一个答案，等于没在问这个问题**——参考 Fortran 是逐三角形问的。

   现在（`apply_cell_content_threshold`）改为一层问一次：第 L 层用一个"该层父单元大小"的 h 场格点块来问"这么大的单元会不会太杂"，命中就令 `h ≤ h_base/2^L`。粗层用宽块给粗 `h`、细层用窄块给细 `h`，`min` 累积成 Method-C 要的嵌套场；答案对块尺度单调，所以层自然嵌套。块是网格对齐的方块而非真实网格单元（h 场看不见网格单元），所以**尺度对了、位置仍是近似**——原先两者都不对。

   回归测试构造成每个 h 场格点恰好一个类别：逐格计数永远是 1，**旧实现对整张异质地图一格都不细化**；实测退回逐格实现后该测试失败。

   代价要说明：任何启用这三个判据的既有工程，输出网格会变——这正是第 8 节记的那条待决事项（"所有既有 h 场项目的输出网格会变"）的一个具体来源。这也是"栅格分辨率可用窗口找不到决定变量"的一个来源：分辨率相关的判据被绑在一个与网格无关的分辨率上，本来就没有正确答案可找。

   **逐层重算已实现**（`refinement_demand::nest::spawn_nest_adaptive`，2026-08）。`spawn_nest` 本身就是逐层的（`spawn_nest_internal` 里 `for pass in pass_levels`，每轮在上一轮的网格上细化），所以不必改内核：在 pipeline 层链式调用，每层调用前重新求一次判据即可。

   | 组件 | 作用 |
   |---|---|
   | `plan::plan_demand_at_scale` | 按给定单元尺度求出所有启用判据的需求并合并 |
   | `ladder::nested_circle_radii_meters` | 各层半径，按引擎自己的 halo 公式 |
   | `nest::spawn_nest_adaptive` | 逐层：重算 → 归约成圆 → `spawn_nest` 一层；需求为空即停 |

   分辨率相关的判据（landcover 异质度）在第 L 层用 `cell_meters = base/2^(L-1)` 作邻域，同一张栅格在不同层给出不同答案；分辨率无关的判据（`sst > 28`）各层答案相同，靠半径阶梯的 halo 满足嵌套。停机条件是"这一层没有判据要求细化"，报告里区分了"自然停机"与"撞到 5 层上限"。

   **实现中踩到的一个几何约束**：块划分随半径变化，于是各层圆心在层间偏移，深层的圆会落到浅层没细化的位置——实测在第 4 层报 `crosses the parent boundary`。修法是所有层用**最细一层的半径**做块划分，各层只改半径不改圆心，于是各层同心，退化成实测通过的单要素同心阶梯；块足够小，最细的圆也仍能覆盖自己的块。

   仍是近似的一点：判据是在**源栅格的等尺度邻域**上求的，不是在细化后网格的真实单元上求的（那需要未移植的 `getref_mean_std`）。所以**尺度对了、位置是网格对齐的方块而非真实单元**。

   **点+半径已是默认路线**（2026-08）。代码里这条路叫 **adaptive**（`&adaptive` 组、`AdaptiveRefinementRecipe`、`spawn_nest_adaptive`）——**"点+半径"与"adaptive"是同一条路的两个名字**：前者说的是形状（需求归约成圆），后者说的是行为（每层细化前重新求判据）。namelist 侧 `&adaptive`（`adaptive_on` / `adaptive_max_level` / `adaptive_base_m` / `adaptive_coastline`）与 `&hfield` 一样是 opt-in；Project 侧 `refinement.adaptive` 缺省即启用，而 `refinement.hfield` 改为**纯显式**——不再"缺省就发 h 场"。

   引擎里因此有三条细化路，`refine_pipeline/global_source.rs` 的分支链按此顺序选择：

   | 工程写法 | namelist | 引擎走哪条 | 需求来源 | 层级下发 | 逐轮重算 |
   |---|---|---|---|---|---|
   | 都不写 | 发 `&adaptive` | 点+半径 | 显式区域 + 判据 → 圆链 | 逐层 | **有** |
   | `hfield.enabled: true` | 发 `&hfield` | H 场 | 显式区域 + 判据 → 连续 h 场 | 量化后一次给 | 无 |
   | `adaptive.enabled: false` 且不写 hfield | 两组都不发 | **直接区域路径** | 只有显式区域，判据不参与 | 一次给全部层级 | 无 |

   第三条不是新东西，是这两条路出现之前 v3 本来就有的默认行为：match 末尾的 `mesh.spawn_nest(&regions, max_level)`（及 `as_atmosmesh` / `cartesian_xy` 变体），`regions` 就是从配置直接读出的 `specified_circle`/bbox/闭合曲线与 `refine_cal` 掩膜文件。

   关掉一个后端不会静默换上另一个——禁用就是禁用。GUI 的"细化方案"选择器相应改为三选一（点+半径 / H 场 / discrete），并新增 `set_adaptive_refinement` 命令；摘要里 `hfield_enabled` 现在只在工程真的要求 h 场时为真，否则面板会显示一组运行时并不生效的设置。

   实测（`examples/default/ocean_hex_global.nml` 改 bbox 105–125°E / 12–32°N，NXP 64，开 `&adaptive`，两层）：真实 landtype → 793 个圆 → 81920 → 87206 面，退出码 0。两层需求相同（51381 个源格点），因为海岸是分辨率无关判据，嵌套由半径阶梯保证。

   接线时踩到一个真 bug：自适应分支最初**只看判据，忽略了用户显式指定的区域**。`examples/projects/auto_refine.yaml` 恰好只有一个显式圆、没有任何判据，于是报"没有判据要求细化"，网格保持均匀而质量检查照样通过——静默少细化。显式区域是**指令**不是判据，必须照样下发；现在每层把该层的显式区域与判据导出的圆一起给 `spawn_nest`，并有回归测试锁住"只写圆、不开判据"这一档。

   **静默少细化的两道网已补上**（2026-08）。上面那个 bug 之所以能"报 pass 却没细化"，是因为这条路当时没有任何对账。现在：

   | 网 | 位置 | 触发条件 |
   |---|---|---|
   | 逐层物化检查 | `spawn_nest_adaptive` | 某层发了圆但网格没出现该层的面（圆对这一代太小，seed 不进去） |
   | 请求-结果对账 | pipeline 自适应分支 | 请求了细化（有显式区域或 `refine_spc`/`refine_cal`）却一层都没做 |

   两者都是**报错**而不是打提示——网格有效、质量检查照过、只是比请求的粗，这种失败不会有别处发现。实测把"忽略显式区域"这个 bug 撤回去，回归测试立刻给出 `deepest_level: 0`。

   **粗粒度的深度报告本来就覆盖了这条路**：`realized_max_level` 是在分支之后**从产出的网格量出来的**，所以自适应分支自动继承，`refine_realized_max_level=` 照样打印。深度不足对这条路而言要么已被上面两道网报错，要么是"需求耗尽"这一合法情形（`cli_mkgrd_output/print.rs` 那条 shortfall 警告由 h 场专属条件门控，对自适应不会误报）。

   **试过并放弃的一条：h 场的"硬需求未覆盖"门禁。** 参考实现里有个 Fail 级 gate `hfield_uncovered_hard_support_bin_count`——"有硬需求的 h 场格点最终没有任何单元以正面积覆盖它"。移植时接错了输入：那个 gate 要的是**梯度限制之前**的场（只有判据与显式区域直接钉住的格点），我喂的是梯度限制之后的完整场，于是每个 `h < base` 的格点都算硬需求，在一次全球极点算例上报出 279350 个"未覆盖"。

   正确接法需要在 h 场构建时捕获梯度限制前的中间态，而 `limit_gradient` 在 `hfield_refine` 里有 5 处调用、散布在各判据的应用中——这是改造 h 场构建，不是接一个门禁。**已撤回**（gate 函数与 2149 行覆盖计算一并删除，不留无调用方的代码）。

   不继续做的理由是优先级：h 场现在是**遗留路线**，为它新建诊断还要改造其构建流程，投入产出不合算。而默认路线的同类保护是**结构性**的——归约层保证"每个被需求的源格点必落在某个圆内"（块是半个半径宽，格点到块心最远约 0.35 个半径，有测试锁定），所以需求不会在"需求 → 圆"这一步丢失；"圆 → 网格"这一步由逐层物化检查兜底。


   ### 点+半径路径的文献位置（2026-08 查证）

   这条路由三段构成，其中**只有第三段没找到先例**。分清楚这一点，是为了在写作和引用时
   不把已有工作说成新的。

   | 层 | 内容 | 出处 |
   |---|---|---|
   | 机制 | conforming 四分 + 过渡行；细化区表示为**一串点加影响半径** | Walko & Avissar 2011, MWR 139(12) 3923–3937, DOI `10.1175/MWR-D-11-00021.1` |
   | 判据驱动 | 由 land type heterogeneity / topography / LAI / soil 等多目标判据自动决定各区分辨率 | Fan, Xu, Bai, Wei, Zhang, Lu 等 2024, GRL 51(6), DOI `10.1029/2023GL107059`（本项目自身的既有工作） |
   | 逐级复问 | 每细化一级后**按新的格子尺度重新求判据**，再决定下一级 | 结构化 AMR 的 regrid 循环（Berger & Oliger 1984, JCP 53(3) 484–512, DOI `10.1016/0021-9991(84)90073-1`）；未见用于全球测地网格 + 静态地表判据 |

   第三段与 AMR 的**动机不同**，值得在论述时说清：AMR 逐层重算是因为**解随时间演化**；这里
   逐层重算是因为**判据的答案本身依赖格子尺度**——"这个格子里混了几种地表类型"是一个关于
   格子的问题，格子不存在时答不了，格子变小后答案也随之改变。h 场把所有判据一次性压成一个
   场再量化，正是因此没有位置让 land-cover heterogeneity 说出"我刚做出来的这批格子仍然太杂"。

   所以准确的定位是：**把 Berger–Oliger 的递归重网格思想，接到 Walko–Avissar 的 conforming
   细化机制上，由静态地表判据驱动**。三者各自都不新，组合是新的。

   #### 逐个部件的归属（写作时按这张表主张，别按上面那张）

   上表分的是"血统"，粒度太粗，容易把既有代码说成贡献。按实现部件拆开是这样的——
   归属以 git 为准，不以印象为准：

   | 部件 | 位置 | 新不新 | 依据 |
   |---|---|---|---|
   | 点+半径**表示法** | `MethodCRefinementRegion::Circle` | 否 | Walko & Avissar 2011 即以"一串点加影响半径"指定细化区 |
   | halo 撑大公式 `rows × base/2^(t-1)` 逐级累加 | `region_sources::circle::push_..._with_parent_halos` | 否 | 引入于 `ebddef6`（2026-07-19），早于本轮工作；失败版有逐字相同的一份 |
   | 判据自动决定分辨率 | —— | 否 | Fan et al. 2024 |
   | **判据栅格 → 圆链的归约** | `reduce_demand_to_circles_on_blocks` | **是** | 本轮新建（`2c4a704`）。此前判据走 mask 文件（`read_method_c_calculated_refinement_regions`）或 h 场，没有"布尔栅格 → 分块 → 圆"这条路 |
   | 半半径分块 + 块心取圆心 | 同上 | **是** | 相邻圆重叠一半以保证沿曲线特征连续；针对 h 场需求碎片化的失败模式 |
   | 各级按**最细半径**统一分块 | 同上 | **是** | 保证各级同心；逐级各自分块会让深层圆落在父层未细化处，被引擎以"跨越父边界"拒绝 |
   | 半径阶梯自动生成 | `ladder::nested_circle_radii_meters` | 部分 | **公式是既有的**，新的只是反过来用：给定最内层半径与深度反推整条链 |
   | `MEASURED_PARENT_HALO_ROWS = 3.0` | `ladder.rs` | **是（实测结果）** | namelist 默认为 0，默认值下父子圆等大、链必失败。18 种配置扫出：2.0 挂 6 个，2.5/3.0 全过 |
   | 逐级复问判据 | `nest::spawn_nest_adaptive*` | **是** | 见上一节 |

   **写作建议**：不要主张"自动确定点+半径"——点+半径是 OLAM 的，容易被指出。能立住的
   是两点：① 判据产生的是逐格布尔场，而 OLAM 接口要的是点和半径，本工作给出这个**归约**
   （含保同心的分块策略）；② 因为判据的答案依赖格子尺度，把归约放进**逐级重复**的循环。
   halo 公式不要列为贡献。

   `rows = 3.0` 值得作为**实测结果**单独报告，因为它顺带暴露了一个反直觉现象：**可行集不是
   向上封闭的**——NXP 21、最内 200 km 时 1.5 行能嵌五级，2.0 行反而失败。"取大更安全"在这里
   不成立，这也是为什么 `tests/refinement_ladder_spawn.rs` 真的去调 `spawn_nest`，而不是拿
   规则自己验自己。

   **逐单元对账已接通**（2026-08）。细化步骤把它实际发出的圆写进 `<case>/result/adaptive_refinement.json`——与最终 gridfile、`namelist.save` 同目录，所以质量步骤从 namelist 路径就能找到；质量侧 `grid_quality_inputs/adaptive.rs` 读回后逐单元采样目标层级，交给 `earthmesh_quality::attach_adaptive_diagnostics`。读的是**运行时真正发出的圆**而不是重新规划，所以对不上必定是细化失败、不会是规划差异。

   两处与 h 场不同的地方，都是量出来的：

   **采样取单元中心，不取角点最大值。** h 场那套对角点取 max，适合平滑变化的场；圆有硬边界，角点在圆内而中心在圆外的单元会被系统性高估，而 **Method-C 选面本来就是按中心包含**。实测同一次运行：角点采样报 140/1643 少细化，中心采样报 97。

   **只对"差距超过一层"设 gate。** 圆的硬边界必然造成一批"中心刚进圆、但 Method-C 无法合法细化"的单元。实测那次运行 `max_target_actual_delta = 1`——每一处差距都恰好一层，正是边界效应的形态。`> 0 就告警`会在每次运行都触发，而每次都触发的 gate 没人会看；随手定个百分比阈值又是编造常数。差超过一层用硬边界解释不了，那才是 gate 的判据。计数（`target_above_actual_count` 等）照常写进报告，供跨运行比较和与 h 场对照。

   实测三个 gate 在真实两层海岸运行上全 `pass`；该次运行的 `verdict: fail` 来自既有的 `orphan_cell_count` 与 `aspect_ratio_max`，与这条路无关。

   路径关系已实测（`cases/<case>/` 下)：

   ```
   gridfile/gridfile_NXP0064_01_hex.nc4   ← gridinit 的输出，细化分支能看到的那个
   result/gridfile_NXP0064_hex.nc4        ← 最终 gridfile，质量步骤读的那个
   result/namelist.save                   ← 质量步骤读的 namelist
   ```

   两者**不同目录**，所以"在细化分支里按 gridinit 输出的同级目录写"这条捷径是错的。可行的路子是沿用 h 场诊断已有的做法：`hfield_diagnostics` 在 match 外声明、分支内赋值，随 `RefinePipelineRunReport` 传给知道最终输出路径的那一层，由它写进 `result/`；质量步骤再从 namelist 的同级目录读回。

   `AdaptiveNestReport::target_level_at` 已就位（读**运行时真正发出的圆**而不是重新规划，所以对不上必定是细化失败、不会是规划差异），是这条链在细化侧的那一半。上面两道网不依赖它。

   此前七轮从 project namelist 改造的尝试全部失败，**原因未查清**。已排除的假设：`hfield_on = .false.` 是有效的（`hfield_refine/mod.rs:192` 有 `if !enabled { return Ok(None) }`，实测保留段设 false 与整段删除同样返回 `None`），所以"段存在即进 h 场分支"的说法不成立。已知的干扰项是 `/tmp` 与 `'none'` 作为"未配置"哨兵在不同分支语义不一致（`has_configured_calculated_regions` 判 `!= "/tmp"`，而 `discover_mask_sources` 要求 fprefix 带父目录，`'none'` 两者都不满足）。这些是配置嫁接的障碍，与路径可行性无关——`examples/default/ocean_hex_global.nml`（circle）与 `examples/merit_hydro/gba/case.nml`（河流，用 `close`）都是可运行实例，而上面的最小测试直调 `spawn_nest` 已证明内核本身没有问题。

   **场级形态学不是出路（2026-08 实测两次均失败）**。在 level map 上按可物化尺度做形态学，两种算子都试过：

   | 算子 | nlat 420（原本健康） | nlat 1680（原本 90% 未满足） |
   |---|---|---|
   | 闭运算（先膨胀后腐蚀，半径 `3·base_m`） | **转为失败** | 转为通过，未满足 0.03% |
   | 膨胀（同半径） | 失败 | 失败（周界病态） |

   闭运算能救最病态的算例，证明**需求只是"太碎"，连起来就能物化**；但其腐蚀步会重塑原本合理的形状，把健康算例弄坏。纯膨胀则把需求糊成覆盖大片海域的巨块（改动 463 万栅格单元），周界随之病态。

   根因是**把每个需求栅格点都当成种子**：Method-C 本身是**稀疏**的「种子 + rad3 足迹」并集，而场级形态学等价于在每个栅格点放一个足迹。正确的做法是先把需求归约为稀疏代表点，再放半径——那就是几何区域路径（`MethodCRefinementRegion::Circle/Corridor`）已经在做的事。

   **更进一步：栅格本身可能是不必要的。** 梯度限制有闭式解——文档第 2 点的 `h*(x) = min_y (h₀(y) + g·d(x,y))`，对有限个几何源就是"点源锥 min"，可在任意查询点直接求值，无需网格。几何区域路径（`method_c_spawn/`）从不构造栅格，也从未出现本节这一整类问题；栅格只在需求本身来自判据栅格时才需要，而那也可以先归约成图元。若走这条路，本节的欠采样/过采样窗口、`nlon/nlat` 推导公式与未标定的上界将一并消失。代价是放弃 h 场"以 `h(x) = min h_i(x)` 统一阈值细化与指定细化"的架构收益（见 `mesh_refinement_method_research_2026-07-02.md`），且栅格→图元的归约必须保证覆盖不劣化，否则只是换一种形式丢需求。**这是待决的架构议题，不是已定方案。**

   上界 8192 对内存的影响同样**待定**：单场 `nlon·nlat·8` 字节，NXP 81/L=3 达 641 MB、封顶 1 GB，而多判据源建场取 min 时峰值是数倍。既有 h 场项目的输出会因栅格改变而改变——这是修复的固有代价（旧网格建立在欠采样场上），实测对旧默认下本就跑得通的算例影响在 ±0.3% 量级。

4. **量化**：`level = ceil(log₂(h_base/h))`（含 1e−9 防浮点毛刺），clamp 到 max_level。
5. **三个消费口**（前两个已接入生产，第三个仅测试覆盖）：
   - `level_at → spawn_nest_from_target_levels`（Method-C 选面，第 4.1 节）——**生产路径**，由 `refine_pipeline/global_source.rs` 的 h 场分支驱动。
   - `sample(边中点) → spring_nest_with_edge_targets`（4.4 节，级内伸缩）——**生产路径**。
   - `level_at → refine_marks_from_target_levels`（3.1 节打标）——调用点全在 `refine_hfield_marks` 的 `#[cfg(test)]` 块内，随第 3 节管线一同处于未接入状态。

   约束：h 场模式要求 `NXP % 3 == 0`（`global_source.rs:83`），因为量化层级要落在 Method-C 的 stride-3 子格上；Project lowering 会自动上调 NXP 并在日志中说明。

参数建议：海洋网格 g=0.12–0.2（涡旋对陡过渡敏感），大气 0.2–0.3；`h_base` = 未细化名义尺寸 `dist00`。

---

## 9. Project 水文后处理边界

当 Project 配置 `hydro_coast` 时，CLI 与 GUI 共用一个有界闭环：coarse gridfile 转成稳定 cell polygon；按真实 Project footprint 读取 MERIT-Hydro 窗口并生成 R2/R3/coast corridors；可选读取 CaMa `nextxy/uparea/rivwth/rivlen`，把下游连接转换成有限宽度测地 capsule（终端 reach 用圆帽），与 MERIT corridors 合并后共同参与交叠和细化，而不只是导出 sidecar；随后生成 target-level plan。bbox（含跨日界线）、circle、shapefile 和 close 均可路由，多部件保持分离；当前接口无法无损表达的 hole-bearing shapefile 会显式报错而不是填洞。circle 是小半球圆盘，半径不得超过四分之一地球周长。

生产 Project 路径固定使用 MERIT 原生 `stride=1`。五个二维变量通过 NetCDF bbox hyperslab 读取，不再整 tile 载入；查询包含域外一个原生 cell halo，并把所有 window/tile 放入统一坐标索引后判断 LAND/OCEAN 邻接，因此 footprint 边界和 tile seam 不漏 coast。稀疏 stride 会被拒绝，不能伪装成物理相邻。

交叠在每个 cell 的 Lambert azimuthal equal-area 平面完成：大圆边以 0.1° 上限加密，cell/corridor/domain 统一投影，同类交叠先做 union 再以 cell 面积归一，输出 `cell_area_m2`、`intersection_area_m2`、守恒 fraction 和生产 `colm_coupling.csv`。CaMa 的 `source/is_estuary/reach_id` 只在真实 clipped overlap 非零时进入输出，`estuary_fraction` 以河口几何自身 union 计算，不会与 MERIT 同类面积双计。跨日界线等价表示、80° 高纬、bbox 对蹖幽灵单元排除和重叠 corridor 守恒均有回归测试。

plan 通过 `hfield_target_cells_geojson + hfield_target_levels_json` 转成梯度受限 HField，并进入现有 Method-C `spawn_nest_from_target_levels_with_spring`；同一 canonical `cell_id` 的多 class 交叠先合并为一个细化需求，budget 不重复计数。Project 的请求层数会 clamp 到 Method-C 上限 5，正常首遍保留原 HField 的 `g/base/nlon/nlat/geographic_origin` 坐标契约。landtype 与 mean/std mask 以 longitude stripe 流式读取，且 source index 0 严格对应北侧 +90°。第二遍引擎必须以第一遍被测量的精确 production gridfile 作为 parent，不得按 NXP 重建名义粗网格；否则 cell identity 与远场几何会改变。第二遍引擎使用隔离输出目录，避免清理 coarse artifacts；完成后先计算 Project mesh quality。若目标层数 ≥3 且唯一的 edge-CV 过渡质量门仍告警，则不放宽阈值、也不盲目增加 spring 次数，而是以 `hfield_g=0.1` 扩宽渐变环带并只重建一次；随后对选定 final gridfile 重新计算 MERIT/CaMa、交叠、coupling 和 coupling-quality。`closed_loop_manifest.json` 记录 initial/final gridfile、adapter namelist、`quality_retry_applied` 和两种最终 verdict，最终质量报告附带实际 adapter HField 的 target-vs-actual diagnostics。为保持共形而产生的 `actual > target` 安全过细化只做诊断；只有 `target > actual`（需求未满足）、映射缺失或层级跳跃才提升 HField 质量门。GUI/CLI 的 Block policy 同时门禁 final mesh 与 coupling Fail。

真实外部数据验收由 `scripts/run_real_hydro_e2e.sh` 驱动：默认使用真实 MERIT/CaMa/landtype、生产 gridfile、原生 MERIT stride=1，并真实执行一层 Method-C。验收同时断言 bbox 无对蹖 ghost cell、真实 CaMa estuary 进入 CoLM CSV、唯一 cell budget，以及 final mesh/coupling 双 Pass。为控制测试时间把源网格初始 `niter=5000` 改为 0、把 Project hydro pass 限为 1；`EARTHMESH_REAL_KEEP_PRODUCTION_NITER=1` 恢复源 spring，`EARTHMESH_REAL_MAX_PASSES=3` 可扩大为多层参数运行。因此默认结果应称为“真实资产的有界闭环 E2E”，不能声称全部细化层数与弹簧收敛参数完全等同生产。

---

## 10. 计算参数速查

| 参数 | 值 | 出处 |
|---|---|---|
| 弹簧 relax | **0.04**（`EarthmeshConfig::default`）；canonical 对拍案例常用 0.035 | `earthmesh_core/src/mkgrd_config/mod.rs:60` |
| 弹簧 β（globe） | **1.2**（`EarthmeshConfig::default`）；canonical 对拍案例常用 1.0 | `earthmesh_core/src/mkgrd_config/mod.rs:59` |
| 嵌套弹簧迭代 | 海洋 2000 / 大气 5000（`niter_refine` 未显式指定时） | `earthmesh_cli/src/refine_runtime.rs:23` |
| 嵌套弹簧开关 | Project 按域自动派生：global→`SpringGlobal=1`，regional→`SpringRegional=1`（**tri 与 hex 同等对待**，2026-08 起） | `earthmesh_project/src/lowering/mod.rs` |
| 质量比较容差 | 精确值指标 1e-9；连续全域极值 1e-4（相对） | `earthmesh_quality/src/lib.rs` |
| twocosphi clamp | [0.15, 1.2] | 目标长比例窗 |
| 目标长除数 | 1.2 | `disto12 = dist00/1.2` |
| mrow 乘子 | 7/6, 8/6, 9/6, 10/6, 11/12 | 过渡行对 (−2,−2)…(1,1) |
| 面积底系数 | 0.1875 (=3/16) | 等边三角形 A² = 3a⁴/16 |
| 顶点度约束 | 5/6/7 | Method-C 拓扑规则 |
| 过渡行结构 | 3 行跨 2 粗行；边界段长 ≡ 0 (mod 3) | spawn_nest 注释语义 |
| 细化级上限 | 5 | `MethodCRefinementRegion` 校验 |
| 最小网格间距 | 0.001 m | `Method-C_METHOD_C_MIN_GRID_SPACING_METERS` |
| 退火轮上限 | 32 | mask annealing |
| h 场 g 推荐 | 0.15–0.3（海洋取低） | 第 8 节 |
| h 场栅格 | `nlat = ceil(πR/(h_min/4))`，下界 360、上界 8192 | `earthmesh_project` `hfield_raster_size()` |
| h 场栅格（引擎默认） | 720×360（直接跑 NML 且未给 `hfield_nlon/nlat` 时） | `hfield_refine/mod.rs:144` |
| 地球半径 | 6 371 229 m | 全库 |

---

## 11. 已知边界与提示

- 12 个五边形导致的 grid imprinting 是拓扑必然，任何优化只能缓解（Peixoto 系列）。
- 过渡行的 5/7 边胞是质量下限所在；h 场弹簧（级内伸缩）显著改善其形状但不消除其拓扑。
- `rfind`/`=` 覆盖等分支属于当前算法契约，已由拓扑与数值回归测试锁定。
- writers 的 NetCDF 变量布局本文未展开（属 `earthmesh_cli`，见各 `*_writer/*_io` 模块与对应测试）。
- h 场已接入 namelist/ProjectConfig、Cartesian-XY 和地理阈值数据路径，并由 `hfield_refine`/`refine_pipeline` 测试覆盖。
- **测试临时目录必须在进程内唯一**（2026-08 修）。`project_auto_refine_e2e` 原先用 `pid + SystemTime::now().as_nanos()` 命名临时根目录，而 macOS 上 `as_nanos()` 只有微秒粒度（实测遗留目录的 nonce 全部以 `000` 结尾），测试线程同微秒启动就会共用一个目录：一个测试写 `project.yaml` 时另一个读到半截内容（报 `unknown field 'ine'`），先跑完的 `remove_dir_all` 又会删掉另一个的产物（报 `Block quality report`）。实测复现率约 1/6（8 次 1 次、5 次 1 次），加进程内原子计数器后 10 次全绿。新增 e2e 测试沿用同一命名方式时须带唯一序号。

### 11.1 深度排查（2026-08）：五类"沉默失败"

一次系统性排查找到并修复的缺陷，都属同一族——**产物合法、质量检查通过、但不是被请求的那个网格**。列在这里是因为每一条都能再犯。

- **新建行的血缘留 0**（`method_c_emit`）。细分在边中点新建 M 点，而血缘只对*已有*行做了搬迁（`for im in 2..=self.nmd`），新中点保持 0——指向不存在的行。实测一次单级细化即产生 72 个这样的行。已有测试没抓到，是因为它只查 `gridfile_m_cell_lineages()`（读 W 面），没查 W 侧。**规律：新增任何逐单元的文件级元数据，占位行和新建行都要给出可解析的值；测试要覆盖 M、W 两侧。**
- **命名区域被按级过滤掉**（`refinement_demand::nest`）。逐级循环只取 `region.level() == level` 的区域，超出上限的命名圆圈无声消失。下游 `deepest_level == 0` 那道闸只在*什么都没细化*时才响，只要另有一个可达级别的区域细化成功就漏过去。**命名区域是指令不是判据，够不到就必须报错。**
- **保护标记按错误的单元种类建立**（`refine_pipeline::global_source`）。`hard_center_demand` 一律按 `w_points` 构建，但 `hex` 的单元中心是 W 点、`tri` 是 M 点。查表是带边界检查的，所以不会崩——只会保护无关单元、把该保的丢掉，正是这个数组存在的目的的反面。
- **保护只覆盖了一半流程**（`mask_postproc_components`）。`hard_demand` 传给了连通分量那一半，没传给紧接着的扇形剪枝；剪枝按"保留最大扇"删单元，会在同一轮循环里删掉前一半刚保住的单元。现在洞口保留优先看需求、再看大小。
- **两条细化路径消费的判据不一致**（`refinement_demand::plan`）。`refine_onelayer_*` / `refine_twolayer_*` 是 mean/std 成对开关（偶数槽比值、奇数槽比变化幅度），h 场两半都用，点+半径路径只读了 mean 半边。于是"在场量变化剧烈处细化"这类要求（陡坡、SST 锋面）在默认路径下静默失效。已补 `threshold_stddev_demand`（邻域总体标准差，与 h 场同义），并加了对着 spec 构造器本身断言的不变量测试，使新判据无法只接一条路径。

同批还修了两处非"沉默"问题：`adaptive_demand_bounds` 对 `Close`/`Any` 域回落到全球窗口（示例默认 `gridnum_perdegree = 120` 时是 43200×21600 ≈ 9.3 亿格，一个几度大的流域会先分配并扫完整个地球）；`scripts/run_basin_hole_regression.sh` 依赖仓库里并不存在的 shapefile，且引用了已撤销的 h 场 demand 产物变量——脚本改为自己生成 ESRI Polygon 域，现在能跑通，并真正验到 `boundary_loop_count == 2`。

一条排查方法上的教训：曾判定"`GridRegion` 没有挖洞表示，带内环的 shapefile 会被静默填平"，写完拦截才发现 `read_polygon_record` 早已调用 `assemble_polygon_rings` 做环桥接，洞是对的，而那道拦截反而会误伤"洞中岛"这个已支持的用例。**看代码推出的缺陷，必须先写出能失败的测试再动手改**——那个测试没红，就是诊断错了。

### 11.2 两个 Rust workspace(2026-08）

根 workspace 是九个 crate（`rust/*` 加 `rust/earthmesh_refine_redgreen`）；**`gui-tauri/src-tauri` 是独立 workspace，不在其中**。于是 `cargo test --workspace`、`cargo fmt --all`、`cargo clippy --workspace` 从仓库根跑，都覆盖不到 GUI —— 而且不会失败，只会为跑过的那部分报成功，读起来就是"全过了"。

v3.0.0-alpha3 的 CI 就栽在这里：本轮改了 `gui-tauri/src-tauri/src/*.rs`，根目录的 `cargo fmt --all` 碰不到它们，`fast` 和 `heavy` 两个 job 全绿、五个平台的 wheel 全部构建成功，只有 `gui` job 的 `make fmt-gui` 挂了。

动过 `gui-tauri/` 之后必须单独跑这四条（正是 CI `gui` job 的全部内容）：`make check-gui-js`、`make fmt-gui`、`make clippy-gui`、`make test-gui`。`make test-full` 会把引擎与 GUI 一起带上；根目录的 `cargo test` 不会。

Makefile 的 `fmt` / `clippy` 逐个列出 crate 而不是用 `--workspace`（因为 `earthmesh_cli` 需要 NetCDF，而 fast job 没有），所以**往 workspace 里加 crate 不会自动进入这些闸**，得同时改 Makefile 的列表。

### 11.3 自适应需求规划的代价（2026-08 实测与修复）

**症状**：一个真实工程（全球海洋、NXP 81、生产 IGBP 栅格、开 landcover 判据）跑了一小时
没有任何进展，也没有报错。

**定位方式**：`sample <pid>` 采样运行中的进程，而不是读代码猜。**全部采样帧落在同一处**：

```
plan_demand_at_scale (plan.rs:167)
  landcover_heterogeneity_demand  landtype.rs:112   1560
  landcover_heterogeneity_demand  landtype.rs:113   1025
  landcover_heterogeneity_demand  landtype.rs:116    356
```

即邻域嵌套循环与 `seen.contains()` 的线性查找。

**根因是复杂度，不是常数**。邻域半径取自**正在细化的那一代格子**，与源栅格无关：

| 量 | 值 |
|---|---|
| 源格边长（240 格/度） | 463 m |
| 半径 = 第 1 级格边长 / 2 = 49.4 km | **107 格** |
| 每个输出格的邻域 | (2·107+1)² = **46,225** |
| 全球输出格数 | 37.3 亿 |
| 单判据单遍 | **1.7×10¹⁴** 次读 |

单线程约需两天。**这不是"慢"，是跑不完**——而且它在小算例上完全正常，因为半径小。

**三处修复，全部精确而非近似**：

1. **逐类前缀和**（`refinement_demand::class_counts`）。窗口内某类的计数 =
   `S[b][r] − S[t][r] − S[b][l] + S[t][l]`，四次读。复杂度 O(n·r²) → O(n·C)，C 为类别数
   （IGBP < 20），**与半径无关**。三个邻域判据（heterogeneity / sea_ratio / dominant_class）
   共用这张表。
2. **需求栅格改位图**（`RefinementDemand` 内部 `Vec<u64>`）。`Vec<bool>` 每格一字节，区域算例
   看不出来，全球算例决定生死：3.48 GB/判据 → 445 MB。`demanded_count` 用 `count_ones`，
   并集是整字 `|=`。
3. **按纬度行并行**（`RefinementDemand::fill_par`）。rayon 本就在依赖树里，采样时其工作线程
   全部停在 `wait_until_cold`。

**为什么无损**：前 1、2 项全程整数——数的是同一批格子（裁剪与 `value_at_global` 逐条对齐，
窗口外的格子同样不计入 total），只有求和顺序变了；第 3 项每格的位只写一次、只依赖自己的
输入，划分方式不影响结果。

**std 判据只并行、不换算法**。前缀和要对 `f64` 累加，求和顺序改变会动到浮点末位——那是
"另一个答案"，不是"更快的答案"。它仍是 O(n·r²)，但只在 `refine_onelayer_*` /
`refine_twolayer_*` 开启时才走。要把它也降到 O(n)，必须先接受末位差异（影响只落在恰好卡在
阈值上的格子）。

**验证方式值得记下**：每一项都对拍它所替代的东西——前缀和对拍原嵌套循环（逐格、多个半径、
含各种越界裁剪）、位图对拍填充位不变量、并行对拍串行。**前缀和的对拍第一次是红的**
（154 vs 142）：我的 oracle 只在 `bounds` 内取样，而真实判据读的是**带 halo 的窗口**。是测试
写错了，不是实现错了。而既有的 10 个 landtype 测试全绿、没抓到这个差异——它们检验的是判据
的语义，不是"改写前后是否等价"。**改写既有算法时，既有测试通过不等于等价**。

实测（同一份数据，半径 10 / 30 / 60，邻域大小相差 33 倍）：用时 1.2 ms / 0.77 ms / 0.71 ms
——**不随半径增长**。

### 11.4 多重细化战役(2026-08-06):十三轮全球验证的机制地图

目标:让点+半径路径在全球尺度正确细化。十三轮真实工程验证(每轮 35–53 分钟),
从 +144 面推进到 +30,030 面、43 组中 29 组落地、全部拓扑门通过。每一层机制都
由一次真实失败暴露、修复并被守卫测试锁定:

| 层 | 缺陷(如何暴露) | 修复(提交) |
|---|---|---|
| 链条连通 | 11.4 万圆只长 144 面:圆够不着 297 km 外的下一颗种子 | 半径 2.5×base,三分辨率扫描定值(`1b3b430`) |
| 选面覆盖 | 单起点行走只到一个连通块,其余静默丢弃 | 逐组行走、并掩膜、单次发射(`8b2fd28`) |
| 同轮邻边 | 行走把邻组的细边当父界报错,大陆组含 98% 圆被整组拒 | mrlu 分粗细:粗=报错,细=跳过(`9715d4e`) |
| 凹陷填充 | rad3 填充不过滤代,把邻组细面混进单代掩膜 | 按掩膜代过滤(同上) |
| 占用地面 | 补丁伸出 2 环 + 共享顶点效应再多 1 环 | 3 环隔离带;裙边按 ngr 戳识别(mrlw 无法区分)(同上) |
| 周界修复 | 全局评分淹没坏块,预算 12 遍为单块设计 | 候选限定不合规周界,预算×块数(同上) |
| 执行顺序 | 小岛先行把大陆带的掩膜撕成 10 片碎片 | 大组先行;串行使碰撞只损失点名的一组(`9788c6e`) |
| 诊断基础设施 | 40 分钟/轮的全球验证无法迭代 | 全部组按细化顺序落盘 `refinement_groups.jsonl`(order/status/faces/reason/circles),秒级本地重放(同上) |

**第十轮补记(瓦片化)**:超过 500 圆的组按较宽轴中位数递归二分 —— 巨型带的
两种失败(起点相位决定的覆盖不可靠、自撞爆价数)都是**跨度**的属性而非需求的
属性,消除跨度比分别追两种失败更对。实测:落地率 14% → 72%,+65,328 面,拓扑
门全绿。剩余 25 个被拒瓦片的报错回到父网格坐标(`crosses the parent boundary
at M point 61085`)。下一步:把成功组也落盘,本地全序列重放即可免 50 分钟全球
验证。

**第十一轮:一次被证伪的假设**。当时判定 pass 1 下该错"只可能由 `mrlu < 1`
触发",据此推断裙边上存在 mrlu=0 的边,并加了对应分支。第十一轮结果与第十轮
**逐字相同**(25/59 拒绝、2,328 圆、196,548 面、同一个 M 61085)—— 该分支
从未在失败路径上执行,假设被证伪。教训:相同的输出不是"改动太小",而是改动
**没有运行**;先确认分支被走到,再讨论效果。

**第十二至十三轮:起点代次**。排除 mrlu=0 后,`crosses` 的条件
`edge_generation < mrlo`(edge ≥ 1)只剩一种解:**`mrlo ≥ 2`**。而第一级细化
跑在未细化球面上本该 `mrlo = 1`,那样这条错误根本不可能出现 —— 它出现了 9 次,
就是 9 次"行走起点落在已细化地面上"的证明。这是演绎,不是实测推断。

成因:`mrlo = m_metadata[start].mrlm`,而 `start` 由"离区域最近的 M 点"决定,
**完全不管它是否已被同轮先行瓦片细化**。一旦落在先行块上,周围所有正常的
未细化边(mrlu = 1)全被判为"比本代粗" → 假跨父界。

修法上走过两条弯路,都被既有测试挡下,值得记下:

1. **传 `pass` 当目标代** —— 被 `method_c_parent_mrl` 的断言语句直接否掉:
   "Canonical derives mrlo from the current starting M point, **not from the
   pass counter**"。在已细化网格上以 pass=1 细化嵌套区,本就该跑在代 2 上。
2. **过滤 `imcent`(几何锚点)** —— `imcent` 的代次是承重的:
   `method_c_refinement_start_point_for_regions_unadjusted` 用
   `m_metadata[pentagon_id].mrlm == m_metadata[imcent].mrlm` 挑选近旁五边形
   来触发行进。过滤它会拆掉这个机制,
   `method_c_near_pentagon_march_uses_marched_start_mrlm_for_parent_ownership`
   立刻失败。

最终落点:**不动查找过程,只校正结果**。起点照常由五边形包含/近旁五边形/行进
决定;仅当结果的 mrlm ≠ 区域自身所含的最粗代时,才改取该代中离锚点最近的点。
嵌套区整体位于父区内,所含点全带父代,最小值即父代 —— 规则依旧"从网格取
mrlo",与 canonical 契约一致。

**同期分开的三类拒绝**(此前混在一个计数里):

| 类别 | 数量 | 性质 |
|---|---|---|
| `crosses the parent boundary` 系列 | 16 | 起点代次误判,上述修复针对 |
| `iw6/iw9 transition patch` | 6 | 二阶邻面未被细分 |
| `selected no active W faces` | 3 | 圆内无可选面 |

`transition patch` 一类的旧消息是 "no solid split edge (0:[1,1], 0:[1,1],
0:[1,1])",读起来像表损坏;实际上 `nest_wd[iw].iu` **只在该面被细分时才填**
(`method_c_emit` 第 57–69 行),全零严格等价于"补丁够到了本轮不细分的面"。
`flag` 区分两种成因,且修法相反:`< 0` 是被另一个周界三元组抑制的面(掩膜在
此处缩颈),`0` 是掩膜压根没选中(掩膜在此处只有一层厚)。消息已改为直接说出
是哪一种。

### 11.5 `quality=fail` 的真实来源不是角度(2026-08-06)

全球验证一直报 `auto_refine quality=fail level=1`,此前被归因于 `min_angle_deg`
29° 达不到项目要求的 40°。读 `quality_summary.json` 的 `gates`/`topology_issues`
可知不是:

| 门 | 级别 | 值 |
|---|---|---|
| `min_angle_deg` | warn | 29.08 |
| `angle_deviation_deg_max` | warn | 42.04 |
| `isolated_refined_cell_count` | warn | 2 |
| **`disconnected_mesh`** | **fail** | **50 个边连通分量** |

角度全部只是 warn。判 fail 的是 `disconnected_mesh`。

**机制**(`mask_postproc_components::retain_largest_component_pass_one_based`):

```rust
if component_id == retained_component || (demanded && !alone) {
```

雕刻保留"最大分量" **加上**"任何含被需求格子且不止一格的分量"。实测:229 个
分量 → 保留 1 + 49 = 50。日志里"保留最大连通水体"这句话因此是误导的 —— 保留的
不止最大的那个。

这是**设计取舍而非缺陷**:丢掉那 49 个分量就等于丢掉用户点名要细化的水体(内陆
湖、封闭海),而留下它们则违反 FVCOM 单一连通域的要求。质量门给的建议是"连通
它们,或作为独立网格导出"。需要产品决策,不应由实现单方面选边。注意 `min_angle
40°` 是项目设定值,引擎默认 25;Method-C 过渡行必然产生 5/7 价顶点,其三角形
达不到 40°,这一项即使细化完全正确也会保持 warn。

**尚未解决的最后一个缺陷**(精确诊断,未修):7,022 圆的巨型海岸带(全部圆的
86%)被拒。机制:带状掩膜两臂在一个顶点以两段弧接触(与雕刻的 pinch 同构),
两臂的过渡行各自加边,顶点价数破 7,发射后的邻居表重建拒绝全网格。失败点
M 132536 是**发射新造的点**,现有定点修复 `try_fill_method_c_specific_m_point`
因 `im <= nmd` 前置条件而跳过。两条候选设计:
1. 扩展 `MethodCRepairableKind::Valence` 载荷,在发射作用域携带 imnew/iwnew
   逆查,把后继网格坐标映射回父网格,让定点填充生效;
2. 发射前在父掩膜上精确预测"双弧且两侧都将细分"的顶点并预填 —— 注意:朴素的
   双弧触发器会打破 canonical 守卫(锯齿边角也是双弧),已试已撤。

**重放要点**:被拒组**不能单独重放** —— 它撞上的是前面各组已经放到网格上的
东西,所以 `refinement_groups.jsonl` 按 `order` 记录全部组而不只是被拒的,重放
必须按序回放到目标组为止。

**复现要点**:该失败只在**弹簧松弛后**的基础网格上出现 —— 未松弛网格上同一组
圆的行走早早卡住(+246 面),因为圆的包含关系随几何漂移。本地重放必须用
`from_icosahedron(nxp, niter>0, ...)` 或直接消费 `refused_groups.jsonl`。

**写作提醒**:细化面积 = 覆盖种子数 × 54 面,与圆半径无关(k 0.4→2.5 实测恒定);
过渡行净减面(-18/块)。"半径加大导致过度细化"是本轮曾犯的错误结论,勿再引用。

### 11.6 red-green 后端的运行时契约(2026-08-06 接线)

分支点在 `refine_pipeline/global_source.rs`,`match config.refine_backend.trim()`。
两个臂都产出同一个 `RefinedGrid`,差别在于**红绿臂填不了的字段留空,而不是编造**。

**它读什么。** 具名区域,以及点+半径判据规约出的圆——两者都经
`redgreen_marking_from_regions` 按三角形中心做包含测试,一层一次,
`refine_redgreen_level` 逐层推进。中心采样与海洋雕刻同规则,所以一个单元要么细化并
保留,要么两者都不。具名区域自带目标层级,所以每一层都按 `level() >= 本层` 取;判据
圆是**为本层**规划的,靠半径嵌套,直接加进来。

**`&adaptive` 不是 Method-C 专属的,它有两半。**

- **判据 → 圆**:在每层要产生的网格尺度上复问判据,把需求规约成圆。纯栅格计算,与
  后端无关,出来的就是一份普通区域列表。函数是
  `refinement_demand::nest::adaptive_demand_circles_for_level`,**两个后端走同一个**。
  层间嵌套由构造保证:每层都以最细半径 block,圆心重合,只有半径变。
- **圆 → 网格**:每个后端各做各的。**Method-C 专属的只有这一半,而这一半正是被
  suspend 的那半**(`METHOD_C_ADAPTIVE_SUSPENDED`):它的种子晶格三格一步、周长须为
  3 的倍数,数据形状的区域被拒而不是被近似。红绿把任何标记长到闭合为止,所以同一批
  需求它建得出来——**这就是这个后端存在的理由**。

所以 lowering 对两个后端都发 `&adaptive`,`global_source.rs` 也不再为红绿丢弃它。

**它拒什么(而不是悄悄少做)。** 下面每一项若被忽略,跑出来的网格都会有效、通过全部
质量检查、并且不是项目要的那张——而且它们都只在被明确写下时才出现,所以在分支
入口直接报错:

| 请求 | 为什么红绿臂服务不了 |
|---|---|
| `&hfield` | 目标层场只有 Method-C 读;lowering 只在显式要求时才发它 |
| Cartesian-XY | 红绿网格在经纬度上工作,x/y 米的点转经纬度是无意义的 |
| `NL%sfcgrid_res_factor` | 地表扩张是另一种操作,不是细化 |
| `refine_cal` 且 `&adaptive` 关、又没有 mask 文件 | 判据无人可读。mask **文件**照常服务(mask 文件就是换个名字的具名区域),只有"判据背后既没文件也没点+半径"才没地方去 |
| `RL%Istransition=.false.` | 过渡行**就是**红绿的收边步骤(red-green 的 green 那一半),不建它连单层都留悬挂节点——tri 模式下的大气示例实测 345 条开边。Method-C 在同样设置下照样闭合,所以这是红绿的能力边界而不是配置的问题 |

`RL%Istransition=.false.` 这条若不前置说明,会先以开边计数、到第 2 层再以
`ngrmm row N has invalid neighbor 0` 冒出来,读起来像网格缺陷而不像配置错误。注意引擎
**只在 `mode_grid='tri'` 下接受** `Istransition=.false.`(hex 会被自己的校验拒:"not
Istransition can only use in the tri"),所以这条拒绝实际只会碰到 tri 运行。
calculated 那条若不前置说明,会撞进区域读取器,以一句提 Method-C、还带着没人打过的
`/tmp` 路径的话失败。

**`tri` 模式已验**(2026-08-06):`atmosphere_hex_global.nml` 改 `mode_grid='tri'`,红绿与
Method-C 都 euler=2、单连通、无非流形、零边界环——**两者输出一致**。

**GUI 的 `algorithm` × `route` 现在确实是两个独立选择**,除了一种组合:红绿 + h 场
会被引擎拒。点+半径两个后端都服务。

**它报不了什么。** `state = None`(网格已在经纬度上,不经 Voronoi/PCVT)、
`method_c_metadata = None`(没有 mrlm/ngr/lineage)、`realized_max_level = 0`、
`transition_faces = 0`。判据跑起来时 `AdaptiveNestReport` **是**报的,和 Method-C 同一
个结构:海洋雕刻靠它保护判据要过的单元不被最大连通分量规则删掉,质量步骤靠写出的
`adaptive_refinement.json` 复问网格有没有达到圆要求的层级。**零在这里是"本网格上未测量",不是"测量结果为零"**——红绿
确实建了过渡带,只是不以 Method-C 的 `boundary_rows` 计数。请求的深度另有 `max_level`
承载,不要为了让日志好看而合成一个层数。运行记录的 `nma`/`nwa` 在无 state 时取自
gridfile 自身的行数。

**逐层链接的规则。** 上一层的标记要**在细化后的网格上重新问一次区域**,不能沿用上
一轮建好的数组,也**不能**过 `RedGreenOutcome::cell_renumbering`——那是逐单元的映射,
而标记是逐三角形的,两个数组连长度都不同。重新计算还偏安全一侧:判决链把上一层的
区域长到了请求之外,所以重算出的内部是两者中较小的那个,下一层只会被压得更靠里,
不会更靠外。

**接线时暴露的两个缺陷**(均已修,见 `8a32713`),都属于 11.1 的沉默失败族:

1. `refine_renewal_core` 把单元数组的 0 号槽留在 9999 哨兵上。0 号槽不是单元,但下游
   把它当单元读:gridfile 读者靠"第 0、1 行是否在原点"在紧凑布局(id = 行 + 1)与
   双占位布局(id = 行)之间二选一,**且逐数组各选各的**。于是单元数组读成紧凑、三角
   数组读成双占位,文件照常打开,每个连通性 id 错一行。
2. `refine_iter_c` 的 `ref_lbx_in` 固定 7 列,因为 7 是其度数规则分叉的边数上限。单元
   可以超过它(一层细化进上一层的过渡带就会产出),而"到达上限"不等于"允许越界索
   引一张按上限开的表"——那是进程中止,不是错误答案。

**已修:极点与日界线处网格不闭合(2026-08-06)。** 症状是红绿输出留下"没有对面三角形"
的边——覆盖极点的区域 NXP 21 起、跨日界线 NXP 33 起、岛周围的海岸判据环也会。

**根因是一段平面时代的脚手架。** 三个细分步骤(`refine_onedivide_four_renew`、
`refine_onedivide_two`、`refine_edge_flip`)都会在三角形经度跨度 > 180° 时把角点整体绕
极轴旋转 180°、在那个坐标系里算、再转回来——这是**平面经纬度求平均**才需要的。而 Rust
早已换成 `spherical_centroid_degrees`(单位向量求和归一化),**在 xyz 里日界线不存在**,
所以这个旋转是数学恒等变换,只剩下算术噪声。

噪声要命在于:**是否走这个分支是逐三角形判定的**。共享一条边的两个三角形,可能一个走、
一个不走,同一条边的中点就差一个 ULP(实测 2.842e-14 度)。而 `refine_ngr_renew_core`
**按 f64 精确相等合并新顶点**,于是不合并、共享边丢邻居、网格出洞。

极点是**误触发**:极区附近各单元经度呈扇形铺开,三角形经度跨度轻易超过 180°,离日界线
十万八千里也会命中。

删掉三处旋转即可。实测:NXP 21/33/45/63/81 × 北极/南极/日界线/赤道/中纬,两层,
**25/25 全部 open_edges=0 且拓扑一致**;`examples/default/atmosphere_hex_global.nml`
原样加 `NL%refine_backend='red_green'` 端到端通过。

护栏保留(`redgreen_open_edges`,每层数一遍开边):不是因为还预期有洞,而是因为它当初的
失败方式——**只有下一层会发现,而单层跑没有下一层**,会把带洞的 gridfile 写出去,而且
它打得开。

**已修:过渡行抬高的价数没有被翻边收回(2026-08-06)。** 症状是红绿的六边形对偶出现 **8
邻居**单元,撞上 `mask_postproc_neighbor_widths` 的 `hex => (7, 3)`——那个 7 是 **Method-C
的保证**(顶点价 ∈{5,6,7}),下游一切都按它开表。

机制:建过渡行的 1→2 分裂会给"被分开的那个角"加 1 价,而随后的 Lawson 翻边正是把它收
回来的一步。**翻边从来没跑过**——`refine_sharp_concav_lop_judge` 顶部的 `tran_degree`
读的是**它自己的输出计数**数组,而调用方每轮都新建全零的 `lop_counts`,于是 `tran_degree`
恒为 1、每个 segment 直接 `continue`,`flipped_triangle_count` 在所有运行里都是 0。
线索是同一个参数表里躺着一个带下划线、**完全没用**的 `_n_bdy_refine_segment`。

绑到 segment 的行数即可(`+1` 因为调用方吃掉本轮头部后已经递减过;吃空的 segment 以 0
到达,正好落进既有的 `== 1` 跳过分支)。实测 NXP 21/33/45/64/81 × 北极/南极/日界线/
赤道/中纬,两层:**50/50 全部 `maxdeg=7`、`over7=0`、`open=0`**,翻边数 72–1052。

顺带暴露并修掉一处:两个 segment 在交界处会看见同一对角,于是同一对被提两次,而翻边会
消耗它重建的两个三角形。`refine_delaunay_lop_one_based` 现在跳过已被消耗的对,与它本来
就跳过空槽位是同一个答案。

**三个 shipped 示例现在两个后端都通过**(`--max-tris 2000000`),拓扑一致:

| 示例 | red_green | method_c |
|---|---|---|
| land | euler=88 comp=109 | euler=89 comp=109 |
| atmosphere | euler=2 comp=1 | euler=2 comp=1 |
| ocean | euler=-86 comp=22 | euler=-87 comp=21 |

价数上限也进了护栏(`REDGREEN_MAX_CELL_DEGREE`):没有雕刻的运行(大气网格)否则会把读不了
的单元写出去而一声不吭。

**测试落点**:具名区域端到端在 `tests/refine_pipeline.rs` 的
`redgreen_backend_refines_a_named_circle_end_to_end`(NXP=21,两层,读回 gridfile 做
拓扑检查——只断言"单元变多"抓不到上面第 1 条);判据链在
`tests/redgreen_criteria_demand.rs`(合成海岸线走真实栅格计算——管线入口把
`NL%gridnum_perdegree` 锁死在 120/240,全球栅格约 1 GB,所以直接喂规划器);全流程判据
run 在 `tests/adaptive_criteria_only_run.rs`,**需挂载 `EARTHMESH_LANDTYPE`,否则跳过**;
链接规则与行布局在 `redgreen_bridge` 的单元测试;越界那条在 `refine_loop` 里。

### 11.7 HARP-DV 单次事务的代价（2026-08-07 实测）

局部算法很容易在门控上变成全局算法。HARP-DV 的 `propose_site` 每一步都是局部的
——定位、空腔、快照、插点——但它的**验收门控**最初调用了三个走遍全网格的函数，
于是「局部改动」按整个网格计费，整轮就是二次的。

在 NXP 24 / 48 / 96 / 192 的球面上（三角形数每级 ×4）实测单次提案耗时：

| 阶段 | 11520 | 46080 | 184320 | 737280 | 每 ×4 三角形的倍率 |
|---|---|---|---|---|---|
| 初版 | 61 µs | 183 µs | 733 µs | 2975 µs | ×3.0 ×4.0 ×4.1 → **线性** |
| 度数检查改用已知起点 | 95 | 189 | 530 | 1897 | ×2.0 ×2.8 ×3.6 |
| 闭合与拓扑校验改为局部 | 33 | 60 | 146 | 500 | ×1.8 ×2.4 ×3.4 |
| 球面守卫改用局部半径 | 17 | 33 | 87 | 275 | ×1.9 ×2.6 ×3.2 → **约 sqrt** |

三个全局项，都藏在看起来无害的调用里：

1. `vertex_degree(site)` 为找扇形起点而全表扫描。`sites_touching` 本来就随每个
   站点返回一个起点——用 `vertex_degree_from` 即可。每次提案调约 7 次。
2. `open_edge_count()` 与 `validate()` 遍历整个网格。局部改动只需检查被改动的
   三角形加外圈：其余部分改动前就是闭合且有效的，且没被碰过。新增
   `open_edges_in(region)` 与 `validate_region(region)`。
3. `insert_site` 的离球守卫调用 `sphere_radius()`，它对**每个**顶点求平均。改用
   落点所在三角形的角点半径——对松弛过的网格反而更准确，因为它的站点本就不在
   同一半径上。

剩下的 sqrt 项是无提示的定位游走，超出 sqrt 的那部分是缓存（737k 三角形的工作集
约 30MB）。`propose_site_near` 接受一个起点提示；按单元求值后提点的调用方总是
有的。

**教训与 11.3 同源，但入口不同**：那次是判据的 `radius_cells` 随分辨率增长，这次
是验收门控的检查范围随网格增长。两者在小 fixture 上都完全正常——本节四行数据里，
最小的那个网格从头到尾只差 3.6 倍。`transaction/tests.rs` 里
`a_proposal_touches_a_neighbourhood_whatever_the_mesh_size` 把这个性质钉成了不
依赖时钟的断言：一次提案触及的三角形数不随网格增长。

### 11.8 度数上界与尺度比上界互相冲突（2026-08-07 实测）

规格 §14 要求相邻单元有效尺度比 `q = max(s_i,s_j)/min(s_i,s_j) ≤ 1.75`。gridfile
另有一个硬上界：每单元 7 个邻居（`ItabW` 的行是 `[i32; 7]`，见 §11.7）。**单靠插点
守不住这两条。**

在 NXP 6 球面、目标为最粗单元 0.3 倍、半径 1200 km 的圆内细化，逐轮实测：

| 轮 | 需求 | 提交（其中 balance） | 未解决 | 最差比 | 超 1.75 的对数 |
|---|---|---|---|---|---|
| 0–3 | 3→10 | 2→9 (0) | 0–1 | 1.38→1.74 | 0 |
| 4 | 17 | 15 (0) | 2 | 2.099 | 52 |
| 5 | 33 | 31 (9) | 2 | 2.424 | 60 |
| 6 | 14 | 7 (5) | 7 | 2.213 | 40 |
| 7 | 9 | 5 (5) | 4 | 1.955 | 16 |
| 8+ | 4 | **0** | 4 | 1.955 | 16 |

第 8 轮起同样的 4 个需求每轮都被全数拒绝——它们的候选阶梯每一级都撞在度数门控上。
关掉 balance 做对照：最差 2.463、58 对超限。所以 balance 把大部分缺口收掉了
（58→16，2.46→1.96），但收不干净，而且**这不是调参能解决的**。

规格 §8.1 的阶梯本来就写着「尝试 r-adaptation → 仍不满足 → 尝试 h-adaptation」。
移点改变尺度而不增加度数，正是为这个冲突准备的一步。当前实现只有 h（插点），所以
残差写进报告的 `unbalanced_pairs_remaining` 而不是藏起来，运行以
`NoAcceptedTransactions` 结束而不是声称完成。

对照实验本身也钉成了测试（`without_balance_the_same_target_breaks_the_bound`）：
没有它，「balance 有效」那条测试可能只是因为选的目标恰好不产生陡梯度而通过，没人
会知道。

**顺带一条测量纪律**：第一次扫描时我同时改了目标倍率和圆半径，读出一个「要得更细反
而细化更少」的倒挂，差点当成缺陷去查。逐轮跟踪显示两个倍率行为完全相同——600 km 的
圈在 670 km 宽的单元上几乎不含单元中心。一次只动一个变量。

### 11.9 r-adaptation 没能收掉尺度比残差（2026-08-07 实测，含一次无效测量）

§11.8 记录的冲突——度数上界 7 与尺度比上界 1.75 单靠插点守不住——规格 §8.1 给的答案
是 r-adaptation：移点改尺度而不改任何人的度数。原语建好了（`flip_edge`、
`legalize_within`、`propose_move`，各自按定义验过），接进 balance 路径后实测：

| 配置 | 超 1.75 的对数 | 最差比 | 结束方式 |
|---|---|---|---|
| 仅插点 | 16 | 1.955 | 第 8 轮 `NoAcceptedTransactions` |
| + 移点，朝粗侧，无改善门控 | 32 | 1.971 | 跑满 40 轮 |
| + 移点，朝细侧，无改善门控 | 96 | 2.444 | 跑满 40 轮 |
| + 移点，朝细侧，逐站点改善门控 | 20 | 2.184 | 跑满 40 轮 |
| + 移点，朝细侧，邻域改善门控 | **12** | 2.054 | 跑满 40 轮 |

最后一行是第三次尝试：改善门控从「本站点的最差比」换成「扇形外扩一圈内所有相邻对的
违规平方和」。理由是移点会同时改变周围每一对的比值，只读一对的门控会接受那些「改善自
己、把违规推给隔壁」的移动。

**结论：三次都不行，而这个序列本身就是结论。** 每放宽一次目标，违规**数量**就降一点
（32 → 20 → 12），但没有一次收掉界，也没有一次收敛；而最差比反而从纯插点的 1.955 升
到 2.054。两个指标互相矛盾、代价是 5 倍轮次——证据不支持接线。

局部改善门控无论多宽都不够。目标必须是全局的，也就是 §13.3 的 Φ 本身；上面三个是已经
被排除掉的局部近似。函数留在代码里（标了 `#[allow(dead_code)]` 和原因），接线撤掉。 移动被硬门控接受但没让全局变好，而它一旦
提交，驱动就把「有东西被接受」读成进展，于是永远不触发 `NoAcceptedTransactions`。改善
门控（§13.3 的离散版）是逐站点局部的：局部最差比下降可以伴随全局恶化。原语保留——它们本身正确，`propose_move` 是公开且有测试的。

**这一节最该记的是一次无效测量。** 上表第四行第一次跑出来是 **0 对 / 1.750 /
`AllSatisfied`**，看起来完美收尾。那是在一个被破坏的网格上测的：`flip_edge` 的邻接修复
会重写两个三角形**以及跨它们边的所有三角形**，而 `propose_move` 的 patch 只盖了扇形加
一圈，所以环外那些重写回滚不了。回滚回去的网格既不是旧的也不是新的——`validate()` 报
四对邻接不对称，而每一条比值统计都照常算出了数。

把 patch 扩到两圈（翻转只在内圈进行）之后，同一配置是 20 对 / 2.184。

两个可复用的教训：

- **回滚必须盖住操作能改写的全部范围，不是它主要改动的范围。** 翻转改的是两个三角形，
  能改写的是它们加一整圈。差这一圈，回滚就静默失败。
- **在验证网格之前，任何测量都不算数。** `ratio_survey` 在损坏的网格上跑得很顺畅并给出
  了最好看的那组数字。现在那个测量脚本会先跑 `validate()`。

### 11.10 接通写出层，两条文档里没有的约束（2026-08-07）

把 HARP-DV 接到 gridfile 写出路径（`MeshState → TriangularMesh →
voronoi_grid_from_triangular_mesh`），十分钟内撞出两条设计文档没写、单元测试也测不到的
硬约束。两条都是「不接通就永远发现不了」的那种。

**一、`impent` 必须携带，不能从度数推导。** 欧拉公式在球面上给出
`#度5 − #度7 = 12`，所以任何有度数 7 站点的网格，度数 5 的站点都超过 12 个。实测：NXP 6
网格插一个点就变成 14 个。Method-C 的细化网格同理——这正是 `TriangularMesh` 把 `impent`
作为**字段携带**而不是计算的原因。站点 id 跨插点稳定（不重编号、不删除），所以初始二十
面体那 12 个 id 一直指着同样 12 个站点。

**二、那 12 个五边形必须保持度数 5。** 不只是「是那 12 个 id」，是度数不能变。
`method_c_mesh_from_triangle_seeds` 会直接拒绝：`protected pentagon 249 has degree 7,
expected 5`。而在五边形旁边插一个点就足以把它推到 7。

第二条是 HARP-DV 的**第三条硬门控**，和度数 ≤ 7、尺度比 ≤ 1.75 并列。它的代价是实的：
接上之后 §11.9 那个目标的 balance 残差从 16 对涨到 40 对，最差比 1.96 → 2.07。但这是
「能写出的网格」对「写不出的网格」的价格，没有选择余地。

**教训**：这两条都不是几何问题，是**输出格式的契约**。任何声称能产出可用网格的后端，
在接通写出层之前对自己的正确性都只有一半的把握。先接链路，再调参数——反过来会把时间
花在一个还不能落盘的网格上。

### 11.11 HARP-DV 网格的第一组质量数字（2026-08-07）

`NL%refine_backend='harp_dv'`，NXP 21，两层圆形目标，产出走 `earthmesh_quality::compute`：

```
单元数 5012   最小角 48.88°   最大角 160.06°   面积比 11.225   平均边长 195.5 km
拓扑：单连通、非流形顶点扇 0、非法顶点索引 0、Euler 失配 0
```

**可用。** 最小角 48.88° 是健康的（正六边形是 120°，退化的标志是趋近 0）；拓扑四项全清。

**两个值得盯的数**：最大角 160.06°，说明有被压扁的单元；面积比 11.2，是两层细化的梯度
（两层理论上约 16 倍，11.2 与之相符）。

这组数是 §13.3 那个连续目标函数 Φ 里 λ_q 项要标定的对象——在这条链路通之前，那三个权
重没有任何可测的锚点。现在有了：`harp_dv_output_passes_the_mesh_quality_gate` 每次运行
都会把它们打出来。

**注意这个测试断言什么、不断言什么。** 它只断言两件决定「网格能不能用」的事：最小角
大于零，以及拓扑四项。最大角和面积比是**打印**而不是断言的——把当前值钉成阈值，会让
下一个改进它的人先去改测试，而那个阈值本身没有依据。
