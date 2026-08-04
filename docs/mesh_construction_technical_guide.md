# EarthMesh v3 网格构建技术指南：计算细节

版本：2026-08-04（在 2026-07-11 球面几何/拓扑/质量契约版之上，补入海洋掩膜拓扑清理、tri 弹簧默认值、质量比较容差、grid_preprocess 迁出与 h 场栅格推导）
性质：实现级技术文档。所有公式、常量与索引约定均以当前 Rust 源码核验；除第 3 节外，模块名即 `rust/earthmesh_mesh/src/` 下的目录名（第 3 节对应 `extends/earthmesh_grid_preprocess/src/`）。

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

**集成状态（重要）：生产路径只走 Method-C。** 上图左支（grid_preprocess 三角细化，第 3 节）已于 2026-08 迁出主程序，现位于独立 crate **`extends/earthmesh_grid_preprocess`**（27 个模块 + 22 个测试文件）。依赖方向是单向的：该 crate 依赖 `earthmesh_mesh`，而 `earthmesh_mesh` / `earthmesh_cli` / `earthmesh_project` / EarthMesh Studio **均不依赖它**——迁出后主程序零错误零警告编译通过，这是隔离性的编译期证据。`refine_pipeline` 的每个分支落点都是 `spawn_nest_*`。

左支作为**逐位对拍的内核库**保留——离散整数拓扑才能对参考实现做表级精确比对（第 6 节验收层级的 compat 模式），这份验证能力是连续/构造式内核给不了的。阅读第 3 节时请按"可测试的算法资产"而非"运行时可选路径"理解。

---

## 1. 数据模型与索引约定

### 1.1 EarthMesh canonical algorithm 保真索引

全库刻意保留 EarthMesh canonical algorithm 约定：数组长度 n+1，**槽位 0（部分表还有 1）为占位**，有效 id 从 2 起；id 以 `usize` 存储，0/1 或负值作哨兵。循环样式 `for i in 2..=n`。删除的三角形连通行写 `[1,1,1]` 占位。此约定使整数拓扑表可与 参考基线**逐位对拍**。

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

> **集成状态：未接入生产流水线，代码位于 `extends/earthmesh_grid_preprocess`。** 本节内核（iterB/C/D/E/F/G 判定、1→2 过渡细分、LOP 翻边、弱凹清理、`ngr_renew`）由测试驱动，主程序不调用其中任何一个。移植目的是保住对 `MOD_grid_preprocess.F90` 的逐位对拍能力（第 6 节 compat 模式）。生产细化见第 4 节 Method-C。
>
> 本节的模块名对应 `extends/earthmesh_grid_preprocess/src/` 下的目录（其余各节仍对应 `rust/earthmesh_mesh/src/`）。它对主程序的依赖面很浅，只用到 `earthmesh_mesh` 的 `LonLatDegrees`、`is_ngrmm`、`BoundaryConnection`、`boundary_closed_curves_one_based`、`push_boundary_neighbor`、`robust_spherical_area_unit`、`spherical_centroid_degrees` 与两个 `Refine*Segments` 类型。`MethodCRefinementRegion` 留在 `earthmesh_mesh`，因为它是 Method-C 生产路径的类型。

驱动循环按"轮"（iter/level）推进：打标 → 过渡判定 → 细分 → 翻边 → 清理 → 重编号 → （最终）弹簧。

### 3.1 打标（三种来源，产物统一为 `ref_sjx ∈ {0,1}`）

- **阈值细化**（`area_judge*`, `getref*`）：对判据栅格（LAI/坡度/土壤/SST/SSH/EKE…）逐三角形取均值/标准差与阈值比较；数据经 2D/3D 归约（`getref_mean_std_*`）。
- **指定细化**：bbox（跨日界线感知）、circle（大圆距离）、closed curve（射线交点奇偶，含共享顶点退化的容差处理）、Lambert 投影域。
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

   **公式的适用范围未标定，这是已知隐患**。上表的分界很陡（0.31% → >50%），但决定它的变量尚未找到——三个候选都被数据否掉：

   | 假设 | 反例 |
   |---|---|
   | 绝对间距 | 6.2 km 通过（NXP 81 两级），23.8 km 失败（NXP 21 两级） |
   | 间距 / `h_min` | NXP 21 两级的公式值 `nlat=840` 恰好落在失败区 |
   | 间距 / `h_base` | 比值 0.062 在 NXP 81 通过、在 NXP 21 失败 |

   即：`h_min/4` 在 NXP 81 上实测有效（四个算例），但对 NXP 21 两级会给出失败值。**低 NXP + 多级的组合可能被公式带进失败区**；兜底是未满足需求检查会显式报错并提示可调项，最坏情况是要求用户改配置，而非静默交付坏网格。

   正确的方向不是继续标定上界（那只是避开问题），而是在 level map 上按可物化尺度做形态学开运算：消除小于一个足迹的斑点，保留主体，边界几乎不动。这样栅格可以放心取细，碎屑由开运算处理。`HField` 目前只有 `sample`/`min_with_region`/`limit_gradient`，无形态学操作。

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
