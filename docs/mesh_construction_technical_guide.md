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

> **集成状态（2026-08-06 接线完成，2026-08-19 复核）：可用的生产后端，代码位于 `rust/earthmesh_refine_redgreen`。** 本节内核（iterB/C/D/E/F/G 判定、1→2 过渡细分、LOP 翻边、弱凹清理、`ngr_renew`）已逐个移植；`refine_loop`（`refine_redgreen_round_one_based` / `RedGreenMesh` / `RedGreenSettings`）、`num_ref_cal`（`refine_num_ref_cal_one_based`）、`OnedivideFour_renew`（`refine_onedivide_four_renew_one_based`）都已导出，CLI 经 `refine_backend_name` 分派 `NL%refine_backend = 'red_green'`，GUI 的算法选项里也有它。运行时契约见 §11.6。
>
> **它不服务的那些会明确报错，不会静默退回 Method-C。** 它读具名区域和点+半径判据规约出的圆；`&hfield` 是 Method-C 的，native surface expansion（`NL%sfcgrid_res_factor`）也不服务，遇到就返回 `Unsupported` 并指名改用 `method_c`。这一点本身是修过的缺陷——旧分派对每个不认识的值都落到 Method-C，于是 `redgreen`、`harp-dv`、`method-c` 三种拼法各自静默产出 Method-C 网格（§11.37）。
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
40°` 是项目设定值；HARP-DV 事务角门默认关闭，质量报告仍在 25° 告警；Method-C 过渡行必然产生 5/7 价顶点,其三角形
达不到 40°,这一项即使细化完全正确也会保持 warn。

**尚未解决的最后一个缺陷**(精确诊断,未修):7,022 圆的巨型海岸带(全部圆的
86%)被拒。机制:带状掩膜两臂在一个顶点以两段弧接触(与雕刻的 pinch 同构),
两臂的过渡行各自加边,顶点价数破 7,发射后的邻居表重建拒绝全网格。失败点
M 132536 是**发射新造的点**,现有定点修复 `try_fill_method_c_specific_m_point`
因 `im <= nmd` 前置条件而跳过。

**2026-08-19 复查:这个前置条件比原诊断记的更糟——它是两个 id 空间的混用。**
价数错误在 `emit_method_c_tables` 里用 **`nmd0`**(子网格计数)调
`derive_icosahedron_m_neighbors_canonical_checked_with_prognostic` 时抛出,所以
`payload.m_point` 命名的是**发射后子网格**里的点;而 `try_fill_method_c_specific_m_point`
索引的是**父网格**。发射会重编号:`imnew` 每个父 M 点前进 1,每遇一条被细分的 U 边
再多前进 1,因此两个 id 空间**只在第一次细分之前重合**,而长在细分边上的子网格新点
根本没有对应的父 id。

于是 `if im <= self.nmd` 这道门做的是两件错事之一:**大于 `nmd` 时跳过了本该跑的
修复**(就是本节这个生产失败),**小于等于 `nmd` 时会去修一个毫不相干的父网格点**。

**这条分支是承重的,而且它承重的方式是打在错误的点上(2026-08-19 实测)。**

本 crate 的 lib 套件里,该分支被进入 **527 次**,其中 **379 次**返回了一个被阶梯采纳的
掩膜;每一次的 `im` 都远在 `nmd` 之内(136 对 513、189 对 711),所以门放行、填充照跑,
**填的是一个不是出问题的那个父点**。删掉它会让 Canonical 一致性测试
`method_c_rejects_reduced_canonical_nxp6_two_level_corridor_too_close_boundary` 失败,
套件从 29 秒涨到 213 秒。

也就是说:**当前与 Canonical 的一致,建立在一个打错位置的填充上**——它把掩膜扰动得
足够让阶梯收敛,而这和"修好了那个越界的点"是两回事。

**一条更正,连同它的成因。** 本节此前写的是"该分支触发 0 次,没有可观测行为依赖它"。
那是错的:那次测量用 `eprintln!` 而**没有加 `--nocapture`**,输出被测试框架捕获,于是
"没有输出"被读成了"没有触发"。加上 `--nocapture` 重测,数字是 527。**这是本次审查里
第二次同形的错误**——把"我没看到"当成"它没发生";§11.68 那条"超时的 job 对它之后的
事什么也没说"是同一句话的另一个说法。用 `eprintln!` 探针量 Rust 测试时,`--nocapture`
不是可选项。

#### 复现只要一个圆,不要 7,022 个(2026-08-19)

本节此前记的复现要点——巨型海岸带、必须在弹簧松弛后的基网格上——**过严了**。
实测这个配置就够:

```rust
MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25)
    .spawn_nest(&[RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 900_000.0,
        level: 1,
    }], 5)
```

**一个圆、NXP 6、`niter = 0`、1.58 秒**,稳定抛出价数错误。它报 `im = 521`,而父网格
`nmd = 363`——**id 落在子网格空间,这一点由此从代码推断变成实测**。

#### 候选设计一实现过并量过,卡在一个定位问题上(2026-08-19)

在 `emit_method_c_tables` 里用 `imnew` 与 `nest_ud` 建子→父逆映射(新造的边中点归给
该父 U 边的第一个端点),把价数错误的 id 翻译回父网格再抛。216 个配置的 sweep:

| | 修前 | 修后 |
|---|---|---|
| 成功 | 54 | **72** |
| 价数失败 | **18** | **0** |
| 其他失败 | 144 | 144 |

18 个价数失败全部转为成功,**全部通过 `validate_topology` 且父顶点价数全 ≤ 7**,其他失败
一个没动。翻译本身收敛:走廊案例上修 3 次、落在 3 个互不相同的父点,不打转。

**但它让一个 Canonical 一致性测试失败**——
`method_c_rejects_reduced_canonical_nxp6_two_level_corridor_too_close_boundary`。
注意该案例**仍然被拒**,"是否拒绝"这一点上与 Fortran 的一致性没破;变的是在哪里拒:

| | 消息 |
|---|---|
| 修前 | `Current nested grid crosses (or is too close to) the next coarser grid boundary` |
| 修后 | `perimeter lengths [44] cannot be grouped into transition triples without crossing the parent boundary` |

前者告诉用户该改什么,后者是修复自己制造出来的后果。该测试的第二条断言写的正是
"应该在三元组分组**之前**拒绝"。附带代价:`--lib` 从 28.6s 涨到 191s,因为阶梯越过
价数墙后要在三元组上再磨 45–53 轮。

**两条收手规则都被实测否掉了,不要再试(注意下列数字取自翻译已落地的树):**

1. **"同一个父点重复出现就收手"** —— 点不重复(实测 259/230/185),规则永不触发。
2. **"报第一个可修错误而不是最后一个"** —— pass 2 里三元组错误**本来就排在价数错误
   之前**,所以第一个仍然是它。

**剩下的不是编码问题。** 它在问:Rust 的 Method-C 该不该去救 Canonical 视为致命的掩膜?
自动周界修复本来就会为修价数而**扩张掩膜**,即细化用户没点名的单元;把这条路走通意味着
接受这个定位。这与本节前面那个 49 分量的取舍同类——**需要产品决策,不应由实现单方面
选边**。翻译本身(修正 id 空间混用)无论怎么定都是对的,但单独落地它就会改变接受行为,
所以两件事得一起定。

原记的两条候选设计:
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

### 11.12 三个后端同配置的质量对照（2026-08-07 实测）

NXP 21，115°E/25°N 处 2000 km 圆形区域，同一份 namelist 只换 `NL%refine_backend`：

| 后端 | 层数 | 单元数 | 最小角 | 最大角 | 面积比 | 边长 CV |
|---|---|---|---|---|---|---|
| harp_dv | 1 | 4446 | 56.39° | 159.86° | 2.73 | 0.185 |
| harp_dv | 2 | 5012 | 48.88° | 160.06° | 11.23 | 0.295 |
| harp_dv | 3 | 7240 | 43.72° | 162.10° | 65.34 | 0.584 |
| method_c | 1 | 4928 | **80.18°** | **149.60°** | 4.83 | 0.256 |
| method_c | 2 | 7064 | **75.56°** | **152.18°** | 19.56 | 0.522 |

**HARP-DV 的网格形状明显更差，差距不是噪声。** 最小角 Method-C 76–80° 对 HARP-DV
44–56°；最大角 152° 对 160°。这条对照还否掉了一个看着很像的推断：160° 不是基础二十面体
网格的性质（当初怀疑它与层数无关所以可能来自基础网格），Method-C 在同一份基础网格上是
150°。**一个在所有配置下都不变的数，仍然可能是被测对象的性质而不是背景。**

**HARP-DV 同时还在欠细化**：同样两层请求，5012 单元对 Method-C 的 7064，面积比 11.2 对
19.6。和它自己报的未解决单元数一致（两层 33 个）。

**一个之前没分离出来的事实**：一层时 HARP-DV 报 `AllSatisfied`、0 对超尺度界。**§14 那条
界在缓梯度下是守得住的**，垮的只是陡梯度（两层 8 对、三层 32 对）。§11.8/11.9 一直在最陡
的那个配置上测，所以看起来像是完全守不住。

这张表是 §13.3 里 λ_q 的标定对象，也给了它一个明确的目标：**把最小角从 49° 提到
Method-C 的 76° 附近**，而不是抽象地「提高质量」。

### 11.13 未解决单元的归因：96% 是度数（2026-08-07 实测）

§11.12 显示 HARP-DV 在欠细化（两层 5012 单元对 Method-C 的 7064，33 个单元报未解决）。
报告此前只给总数，而「33 个」不告诉任何人该改什么。按拒绝原因分类之后，NXP 21 两层：

```
degree 786   pentagon 30   not insertable 0   topology 0   no improvement 0   unmeasurable 0
```

**度数上界占 96%（786/816）。** 其余三项全是零：候选阶梯从来没有找不到合法点，拓扑从来
没有被破坏，也从来没有出现「合法但没变好」。

这一个数把剩下的工作方向定死了：

- **不是候选阶梯的问题**（`not_insertable = 0`，四级阶梯每次都有得试）；
- **不是事务门控的问题**（`topology = 0`，硬门控没误伤过）；
- **是度数墙**。而插点只会抬高度数，所以插点永远撞它。

这也回过头解释了 §11.9 三次改善门控为什么都失败：那三次都在调**尺度**这个杠杆——移动
按邻接比的改善与否接受或拒绝——而真正卡住的是**度数**。目标函数选错了变量，不管做得多
宽都不会有用。

下一次 r-adaptation 的尝试应该按度数来设计：把移动瞄准「降低邻域最大度数」，而不是
「降低邻域尺度比违规」。尺度比会作为结果跟着改善，因为让度数下来正是它守不住的原因。

`the_refusals_are_counted_by_kind` 把这个分布钉成了形状而不是数字（总数对得上、degree
是最大项、`not_insertable` 为零），因为把 786 写死会让下一个改进它的人先去改测试。

### 11.14 弹性调整解决尺度和五边形，解决不了度数（2026-08-07 实测）

§11.13 定位到度数墙占 96%，下一个自然的想法是弹性调整（spring）——它移动站点、不加
度数，看起来正好绕开。真实运行（NXP 21，`input/refine_spc_circle01/02.nml` 两层圆），
只改 `NL%niter`：

| `NL%niter` | 未平衡对数 | degree 拒绝 | pentagon 拒绝 | 单元数 | 实际层数 |
|---|---|---|---|---|---|
| 0 | 24 | 736 | 38 | 5010 | 1 |
| 200 | **8** | **990** | **0** | 5052 | 1 |

**帮上了，但不在度数上。** 尺度比残差 24→8；五边形拒绝 38→0（松弛后五边形彼此分得开，
候选不再落到它们旁边）。而 degree 拒绝反而从 736 涨到 990，请求的两层仍然只做到一层。

**原因**：度数是拓扑量，移动站点不改变它。弹性调整能把站点摆匀、把尺度梯度抹平、把五
边形挪开，但一个度数 7 的站点松弛之后还是度数 7。§11.13 说「下一次 r-adaptation 应该瞄
准降低邻域最大度数」——单靠移动做不到这件事。

**能改度数的只有边翻转。** Lawson 合法化在恢复 Delaunay 的同时重新分配度数，所以可行的
组合是「松弛 + 合法化」而不是「松弛」。两块原语都在：`mesh_spring`（通用）和
`MeshState::legalize_within`（§11.9 建的，当时是为 r-adaptation 的回滚安全）。

顺带一条：`NL%niter=200` 那一档把 pentagon 拒绝清成 0，这条本身就有实用价值——保护五
边形那条硬门控（§11.10）的代价可以靠松弛基础网格消掉，不需要改任何代码。

### 11.15 松弛 + 合法化也降不了度数（2026-08-07 实测，第四次否定）

§11.14 的结论是「能改度数的只有边翻转，所以可行的组合是松弛 + 合法化」。做了并测了：在
候选阶梯被度数拒绝的单元上，把站点移向邻居质心的一半，再 `legalize_within`，改善门控读
邻域最大度数。

**触发 0 次。** 两个 `NL%niter` 档位、真实运行，没有一次移动降低了邻域最大度数。

**为什么**：Lawson 合法化不是自由地重分配度数——它产生的是 **Delaunay 三角剖分，而那由
点的位置唯一决定**（一般位置下）。移动站点再合法化，得到的就是移动后点集的 Delaunay
剖分。如果那个剖分仍然含度数 7 的顶点，没有任何翻转序列能改变它。翻转只能恢复 Delaunay，
不能被引导去优化别的目标。

所以 §11.14 那句「只有翻转能改度数」字面上对、推论上错：翻转确实改度数，但**改成什么由
几何决定，不由调用者决定**。

至此对度数墙的四次尝试全部否定：

1. §11.9 移点，逐站点尺度改善门控 —— 更差
2. §11.9 移点，邻域尺度改善门控 —— 略好，不收敛
3. §11.14 基础网格松弛 —— 尺度和五边形改善，度数变差
4. 本节 松弛 + 合法化，度数改善门控 —— 触发 0 次

**共同点**：四次都在既有点集上做局部调整。度数分布是 Delaunay 剖分的性质，而 Delaunay
剖分由点集决定——所以真正的杠杆是**在哪里插点**，不是插完之后怎么调。

这把方向指向候选阶梯：现在四级阶梯（witness / 最远角点 / off-center / 最长边中点）都不
看度数。一个「优先选择使邻域度数最平均的位置」的候选规则，是第一个还没被否定的方向。
代码留着（`neighbourhood_max_degree`、`relaxation_destination`、
`degree_relieving_moves` 报告项），因为换候选规则时它们正是要复用的度量。

### 11.16 按度数预测排序候选：有效但依配置，且不改变交付（2026-08-07）

§11.15 把方向指向「在哪插点」而不是「插完怎么调」。Bowyer–Watson 的机制让这件事可算：
空腔环上每个站点恰好 +1 度,新站点度数等于环大小,**两者在写任何东西之前就知道**。
`MeshState::forecast_degrees` 做这个预测,候选阶梯据此稳定排序(超预算的往后排,预算内
的保持阶梯原序,所以 witness 在无差别时仍然领先)。

真实运行,NXP 21 两层圆:

| `NL%niter` | 未解决 | 未平衡对 | degree 拒绝 | 单元 | 实际层数 |
|---|---|---|---|---|---|
| 0，排序前 | 39 | 24 | 736 | 5010 | 1 |
| 0，排序后 | **51** | **12** | **1017** | 4974 | 1 |
| 200，排序前 | 35 | 8 | 990 | 5052 | 1 |
| 200，排序后 | **30** | 8 | **666** | 5052 | 1 |

**松弛过的基础网格上有效**：degree 拒绝降 33%，未解决降 14%，而且 §11.15 那个一直触发
0 次的度数缓解移动第一次动了(1 次)。**没松弛的基础网格上更糟**。

**但两档的实际层数都还是 1。** 请求两层、交付一层这件事没有改变——这是第五次尝试,也是
第五次没有改变交付结果。

保留它的理由是它在**该在意的那个配置**上有效(生产设置会松弛基础网格),而且预测本身是
精确的而非启发式的。但它不是突破,记在这里是为了下一个人不必重做这个实验就知道:**排序
比不排序好,但不足以翻过那堵墙。**

到此对度数墙的五次尝试:四次事后调整(11.9 两次、11.14、11.15)全部否定,一次插点前选择
(本节)部分有效。方向是对的,力度不够。

### 11.17 放宽度数上界不值得：HARP-DV 在 9 就饱和（2026-08-07 实测）

§11.16 之后剩下的唯一「能翻墙」的选项,是放宽 gridfile 的度数 7——它是 `ItabW` 从 Fortran
直译来的 `[i32; 7]`(`mesh_memory` 里 14 处),不是 CoLM/MPAS 格式的要求(MPAS 写出器的
`max_edges` 是参数)。动它意味着放弃或改造逐位对照 Fortran 的路径。

先量该放宽到多少。NXP 6 陡目标,只改 `HardGates::max_vertex_degree`:

| 预算 | 单元 | 提交 | 未解决 | 实际最大度数 | 超 1.75 |
|---|---|---|---|---|---|
| 7 | 448 | 86 | 6 | 7 | 24 |
| 8 | 453 | 91 | 2 | 8 | 12 |
| 9 | 464 | 102 | 3 | 9 | 12 |
| 10 / 12 / 16 | 464 | 102 | 3 | **9** | 12 |

**在 9 饱和。** 预算给到 16 与给 9 完全相同——所以要放宽的是 7→9,不是无上限。

**但收益太小,不值得这个风险**:把度数墙**完全拆掉**,单元只多 3.6%(448→464),未解决
6→3,尺度残差 24→12,而且**仍然以 `NoAcceptedTransactions` 结束**。它不解决「请求两层、
交付一层」——那是这个后端最显眼的短板,而度数不是它的全部原因。

所以那堵墙拆了也还有别的墙。§11.13 量到 degree 占拒绝次数的 96%,那是**拒绝次数**的分布;
这一节量的是**放开之后能多做多少**,两个是不同的问题,而后者才决定值不值得动格式。

`the_degree_budget_saturates` 把这个结论钉住:9 与 16 结果必须相同,且放开度数带来的单元
增长小于 10%。哪天不再成立,说明这一节的结论过期了,该重新考虑放宽 `ItabW`。

### 11.18 度数之后的第二堵墙是五边形，而它有零代码的解法（2026-08-07）

§11.17 发现拆掉度数墙之后运行仍以 `NoAcceptedTransactions` 结束,说明后面还有墙。把拒绝
分类在两个预算下各跑一次:

| 预算 | degree | pentagon | notins | topo | noimp | 未解决 |
|---|---|---|---|---|---|---|
| 7 | 49 | 35 | 0 | 0 | 0 | 6 |
| 16 | **0** | **33** | 0 | 0 | 0 | 3 |

**度数不设限时,剩下的拒绝 100% 是受保护五边形**(§11.10 那条:gridfile 重建要求那 12 个
站点保持度数 5,而在五边形旁插一点就把它推到 7)。

**这堵墙有零代码改动的解法**:§11.14 已经量到 `NL%niter=200` 把 pentagon 拒绝清成 0——
松弛过的基础网格里五边形彼此分得开,候选不再落到它们旁边。

所以两堵墙的账清楚了:

- **度数**:占拒绝次数 96%(§11.13),但拆掉只多 3.6% 单元(§11.17)——**挡得多,拆了不值**;
- **五边形**:度数不设限时占 100%,松弛基础网格即可消除——**便宜**。

而 §11.14 已经测过:松弛之后(pentagon=0)实际层数仍然是 1。所以**两堵墙都不是「请求两层
交付一层」的原因**,或者说不是全部原因。下一个该问的是:松弛的基础网格 + 度数预算 9,
两堵墙同时不在时,层数会不会到 2。那需要把度数预算做成可配置项——是本节之后第一件要做的
事,也是第一件需要新接口而非新测量的事。

### 11.19 「请求两层交付一层」是单位不匹配，不是细化失败（2026-08-07）

§11.13–11.18 花了五轮追两堵墙，起点是 CLI 报的 `refine_realized_max_level=1` 对
`refine_max_level=2`。**那个数是错的读法。**

按尺度直接量(NXP 6，目标 = 最粗单元的 1/4，即两次对折)：

| `niter` | 度数预算 | 站点 | 目标 | 最细单元 | **实际对折次数** |
|---|---|---|---|---|---|
| 0 | 7 | 362→481 | 169 km | 112 km | **2.60** |
| 0 | 9 | 362→498 | 169 km | 127 km | 2.41 |
| 200 | 7 | 362→489 | 171 km | 110 km | **2.63** |
| 200 | 9 | 362→488 | 171 km | 126 km | 2.44 |
| 200 | 16 | 362→488 | 171 km | 126 km | 2.44 |

**每一档的最细单元都比目标还细,实际对折 2.4–2.6 次而请求是 2 次。** 网格从来没有达不到
请求的深度。

**为什么报出来是 1**：`realized_max_level` 来自 gridfile 的 `w_refine_levels`，而
HARP-DV 填的是站点 depth。Method-C 的 level 是**嵌套趟数**——一整片区域被细化一轮算一
层；HARP-DV 是**逐单元连续插点**，一个需求的目标通常是原始粗单元(depth 0)，所以新站点
depth = 1，不管它把那块地方细化到多细。两个后端的「层」不是同一个量。

(顺带记一个中间错误：先把 depth 改成「邻域最深 + 1」,报出来 13——那是插点链的长度而不
是细化代数。链长和代数是两回事。)

**这一节推翻了 §11.13–11.18 的动机,但不推翻它们的内容。** 那五轮量出来的东西都成立:
degree 占拒绝 96%、放宽到 9 就饱和、第二堵墙是五边形、松弛能清掉它。只是它们回答的是
「什么在挡」,而不是「交付是否达标」——后者一直是达标的,是报告在说谎。

**教训**：跨后端比较一个指标之前，先确认两边算的是同一个量。`realized_max_level` 在
Method-C 下是趟数、在红绿下是 0(明说未测量)、在 HARP-DV 下是站点代数,三个后端三个含义,
而它们并排出现在同一个运行记录里。

### 11.20 给运行记录一个后端无关的分辨率量（2026-08-07）

§11.19 的教训是 `realized_max_level` 在三个后端下是三个量。补一个从**产出网格**量出来的:
单元宽度 `sqrt(A/π)` 的 P2 与 P98,以及 `log2(P98/P2)` 作为实际对折次数。同一份 namelist:

| 后端 | `realized_max_level` | 最细 P2 | 最粗 P98 | **实际对折** |
|---|---|---|---|---|
| method_c | 2 | 58.1 km | 791.8 km | 3.77 |
| red_green | 0 | 38.6 km | 631.9 km | 4.03 |
| harp_dv | 1 | 79.9 km | 1073.7 km | 3.75 |

左列三个含义,右三列可比。**HARP-DV 3.75 对 Method-C 3.77,差 0.5%**——而 `realized_max_level`
的「1 对 2」看起来像差一倍。

**用分位数而不是极值,这一步是被自己的第一版逼出来的。** min/max 版本给 method_c 报出
12.3 次对折、最细 2.4 km——NXP 21 标称单元约 300 km,2.4 km 只可能来自**陆地掩膜切割留下
的碎片单元**。极值统计在有碎片的网格上不是稳健量,而我正是在修一个会误导人的数的时候
造了另一个。P2/P98 把碎片排除在外。

### 11.21 给 HARP-DV 接 nest-spring：5000 次，质量变差（2026-08-07）

§11.12 的质量差距在角度上(HARP-DV 最小角 48.9° 对 Method-C 75.6°),而 nest-spring 正是
Method-C 用来抹平细化网格的手段,HARP-DV 从来没跑过。接上去试了 5000 次:

**第一次:一点变化都没有。** 逐位相同的质量数字,而耗时涨了 7 倍。原因是
`method_c_nest_movable_m_points` 里的 `if mesh.m_metadata[im].ngr != ngr { continue; }`
——只有 `ngr` 等于传入值的点才可动,而 HARP-DV 的网格里 ngr 全是 1,同时那个函数又拒绝
`ngr <= 1`。**一个标着 ngr=1 的网格是任何弹簧都不会碰的网格。**

**改成 ngr=2 之后弹簧真的动了,把网格弄坏了**:最小角 17.14° → **10.67°**,最大角偏差
69.60° → 73.87°。

**原因**:我给的目标边长是「最近面的当前尺度」,那等于告诉弹簧「保持现状」。Method-C 的
nest-spring 是配合 h-field 给出的**独立**目标长度用的——目标必须来自网格之外,否则弹簧
在一个自相矛盾的目标下把点挤到更差的位置。

接线撤掉了:一个默认会让质量变差的路径比没有这条路径糟。保留的是 `ngr` 那个修正
(`to_triangular_mesh_with_grid_number`),因为「ngr=1 的网格弹不动」这件事本身是对的,
下一次尝试还会需要它。

**下一次该怎么试**:目标边长要从判据来(判据本来就说了每个单元该多大——`TargetScale` 的
`target_scale_m`),不是从网格现状来。那才是 h-field 在 Method-C 里扮演的角色。

### 11.22 判据驱动的弹性平滑：有效，且 100 次就够（2026-08-07）

§11.21 的失败在于目标边长取自网格现状,等于让弹簧「保持现状」。改成取自**判据**——一个
要求 level L 的区域,就是要求基础单元对折 L 次,那是个弹簧可以拉过去的长度。单元宽度到
三角形边长的换算因子从这张网格上量,不是推导(两者差一个依赖对偶形状的系数,量比推短且
不容易错)。

`RL%niter_refine` 扫描,NXP 21 两层圆:

| 迭代 | 最小角 | 最大角偏差 |
|---|---|---|
| 0 | 17.14° | 69.60° |
| **100** | **23.09°** | **67.24°** |
| 250 | 22.68° | 68.18° |
| 500 | 22.16° | 69.45° |
| 750 | 22.23° | 69.62° |
| 1500 | 21.30° | 69.73° |
| 5000 | 19.05° | 74.56° |

**100 次两个指标同时最好**:最小角 17.14° → 23.09°(+35%),最大角偏差 69.60° → 67.24°。
之后单调退化,5000 次时最小角只剩 19.05°、偏差涨到 74.56°——**过松弛**。

这是这条线上第一个正结果,也是唯一一个。前面五次(§11.9 两次、11.14、11.15、11.21)全负,
区别只有一处:**目标来自判据而不是来自网格自己**。同一套弹簧、同一份网格、同样的迭代
次数,换个目标就从「弄坏」变成「改善 35%」。

默认仍是 0(关)。`RL%niter_refine` 是用户设的,而这张表说明了**它不是越大越好**——直觉
上「多迭代几次总没坏处」在这里是错的。

### 11.23 目标场改成梯度受限，最小角再涨 40%（2026-08-07）

§11.22 的目标场是圆内圆外的**阶跃**:圆内 `base/2^L`,圆外 `base`。那等于要求弹簧在一条
边上把边长砍掉四分之三,而它只能用一个薄片来回答。改成梯度受限——目标从圆边界往外按
每米 0.3 米的速率长回 base,取所有区域的最小值。这正是 h-field 相对于圆列表的全部优势。

同一份 namelist,NXP 21 两层圆:

| 迭代 | 目标场 | 最小角 | 最大角偏差 |
|---|---|---|---|
| 0 | — | 17.14° | 69.60° |
| 100 | 阶跃 | 23.09° | 67.24° |
| **100** | **梯度** | **32.33°** | **52.05°** |
| 250 | 梯度 | 30.81° | 50.53° |
| 500 | 梯度 | 29.61° | **49.66°** |

**最小角 17.14° → 32.33°,涨 89%**;最大角偏差 69.60° → 52.05°。相对阶跃目标(23.09°),
梯度限制又多贡献了 40%。

**跨过了一条实际的线**:质量门禁的最小角警告阈值是 25°(`DEFAULT_MIN_ANGLE_WARN_DEG`)。
不平滑时 17.14° **低于警告线**;梯度平滑后 32.33°,有 29% 余量。

迭代数上仍有权衡:最小角在 100 次最好,最大角偏差在 500 次最好(49.66°)且仍在下降。两者
反向,所以没有单一最优——100 是最小角优先的选择。

**三次目标场的演进**,同一套弹簧:

1. 网格现状 → 最小角 10.67°(比不平滑还差,§11.21)
2. 判据阶跃 → 23.09°(§11.22)
3. 判据 + 梯度限制 → **32.33°**

**弹簧从来不是问题,目标场才是。** 六次尝试里五次失败,失败的全是「拉向哪里」,没有一次
是「怎么拉」。

### 11.24 梯度常数不是杠杆，剩下的差距是结构性的（2026-08-07）

§11.23 的梯度限制常数 0.3 是拍的。扫了十倍范围(100 次迭代,其余不变):

| 梯度 | 最小角 | 最大角偏差 |
|---|---|---|
| 0.05 | 32.55° | 52.03° |
| 0.10 | 32.33° | 52.37° |
| 0.15 | 32.10° | 52.43° |
| 0.20 | 32.14° | 52.52° |
| 0.30 | 32.33° | 52.05° |
| 0.50 | 32.17° | 51.85° |

**幅度 1.4%,没有趋势。** 起作用的是「目标场连续」这件事本身,不是连续的斜率。这个杠杆
到此为止。

当时把**剩下的 32.3° 对 75.6° 判断成大概率结构性差距**。这个判断后来需要修正:
该实验只把梯度目标送入 Method-C 的 nest spring；旧 HARP 后处理仍是没有自然长度的
Laplacian 平均。因此它证明的是「零自然长度后处理在渐变网格上有上限」，不是「HARP 的
后处理路线无效」。后续 §11.57 把同一尺寸场接入 HARP 的事务化优化后，已经直接证伪了
“必须改插点方式才能继续改善”这个过强结论。

当时观察到的构造差异本身仍然成立:

- Method-C **细分三角形**——每个子单元是父三角形的规则四分,形状由构造保证;
- HARP-DV **插入 Delaunay 点**——对给定点集是最优的,但点集本身的分布没有任何保证。

弹簧能把点摆匀到什么程度是有上限的,因为它只能在既有连通性下移动。而 Method-C 的
{5,6,7} 度数约束和过渡行是**同时**约束连通性和位置的。

但不能由此推出「追上只能改插点方式」。能看见尺寸场、最差大角并在移动后恢复 Delaunay
合法性的后处理，当时还没有被实现和测试。

**已经拿到的**:32.3° 在质量门禁 25° 警告线之上有 29% 余量,而不平滑时 17.14° 在线下。
这是从「不合格」到「合格」,不是从「合格」到「更好」。

**交替插点与平滑也试了**(细化跑 4 轮,每轮之间弹一次,总迭代数不变):100 次时最小角
32.78°、偏差 51.11°——比单次平滑的 32.33°/52.05° 好 **1.4%**,而细化要跑四遍。400 次时
反而更差(28.91°)。**撤掉了**:1.4% 换四倍运行时间不划算,代码也复杂得多。

至此只说明旧的固定连通、零自然长度弹簧杠杆用尽。关于一般后处理的结论移至 §11.57。

### 11.25 把质量做成判据：机器都在，缺的是侵占规则（2026-08-07）

「HARP-DV 能不能也造出漂亮三角形」——从头看一遍,答案在规格里:§8.2 的四种停止语义我只
实现了 `TargetScale`(尺寸),**`MeshQuality` 从来没做**。而带角度保证的 Delaunay 细化是
已解决的问题:**Ruppert 在最差三角形的外心插点**,证明角度下界(≥20.7°,Chew 的变体 ≥30°)。

**机器本来就在**:候选阶梯第二级「最远单元角点」就是外心,第三级 off-centre 是 Üngör 对
同一想法的改进(更少的点达到同样的界)。缺的只是**提出要求的那个判据**——只跑
`TargetScale` 时,一个足够小但形状很差的单元永远不会成为需求,下游没有任何东西看它的形状。

加了 `MinAngle` 判据(witness 取最差三角形的外心)之后实测:

| 阈值 | 最小角 | 结果 |
|---|---|---|
| 关 | 32.33° | — |
| 15° | 32.33° | 无变化 |
| 20° | 32.35° | 几乎无变化 |
| 25° | — | **发散** |

25° 时:6649 次度数拒绝、跑满轮次、弹簧拒绝、最后输出崩在「退化外心」。**度数上界设成
7、9、12 都一样发散**,所以不是度数墙。

**是 Ruppert 的前提没满足**:它的终止性证明要求**先分裂被侵占的边界段**(segment
encroachment),否则在区域边界附近会无限细分。这条规则我没有实现。

**所以「HARP-DV 造漂亮三角形」这条路是通的,但需要的不是调参**:要实现 Ruppert 的侵占
规则(把细化区域的边界当作受保护的段,任何外心落进段的直径圆时先分裂段)。那是明确的、
有文献支撑的一件事,不是又一次启发式尝试——这和前面六次的性质不同。

判据类型保留在 crate 里(`MinAngle`),CLI 接线撤掉:一个会让运行崩掉的默认路径不该留着。

### 11.26 Ruppert 的侵占规则：质量细化从发散变成有保证（2026-08-07）

§11.25 定位到缺的是 segment encroachment。实现了:`MeshState::encroached_segment` 判断候选
点是否落在受保护段的直径圆内(等价于该点看这条段的张角是钝角),落在里面就改插段中点;
`AdaptiveMesh::protect_boundary_sites` 让调用方指定哪些站点在受保护边界上。

同一个 25° 角度目标,NXP 6:

| 角度目标 | 段保护 | 站点 | 最小三角形角 | 结束 |
|---|---|---|---|---|
| 20° | 无 | 484 | 17.59° | 收敛 |
| 25° | 无 | 11067 | **0.00°(退化)** | 跑满轮次 |
| **25°** | **有** | **489** | **21.09°** | **收敛** |
| 30° | 有 | 200000 | 0.00° | 预算耗尽 |

**站点数 11067 → 489(22 倍),而且正常终止;最小三角形角 0(退化)→ 21.09°。**

**21.09° 高于 Ruppert 的 20.7° 下界**——这是构造性保证,不是模板碰巧给的。代价是 5 个站点
(489 对完全不加质量判据的 484)。

30° 仍发散,也符合理论:Ruppert 的界约 20.7°,超过就没有终止性证明,那需要 Chew 的变体
(≥30°),它用不同的插点规则。

**这是整段里唯一一个「照着证明做就成」的结果。** 前面七次(§11.9 两次、11.14、11.15、
11.21、11.22 的阶跃版、11.24 的交替)都是启发式:改方向、改门控、改目标场、改迭代次数。
这一次实现的是一条有终止性证明的规则,一次就对。

**下一步是把它接到 CLI**:受保护段应该从细化区域的边界自动导出(圆的边界一圈站点),
而不是像测试里那样手工挑。那之后 `MinAngle` 就可以作为默认判据开启,HARP-DV 会带着
角度下界产出网格——那是 Method-C 没有的东西(它的 75.6° 是模板给的,没有下界证明)。

### 11.27 Ruppert 与弹性调整不相加，组合更差（2026-08-07 实测）

两者看起来正交——Ruppert 管**点插在哪**(细化中,改连通性),弹簧管**点坐在哪**(固定连通性
下移动)。接起来测,NXP 21 两层圆,六边形单元最小角:

| Ruppert | 弹簧迭代 | 最小角 |
|---|---|---|
| 关 | 0 | 17.14° |
| 关 | 100 | **32.33°** |
| 20° | 0 | 20.40° |
| 20° | 100 | 29.92° |

**弹簧单独最好,加 Ruppert 反而掉到 29.92°。**

推测的原因:Ruppert 的**受保护段不能移动**,而它额外插的点坐在外心和段中点上——那是终止性
证明要的位置,不是弹簧要的位置。两者对「点该在哪」的判断冲突,而弹簧被段锁住让不了步。

**但两者的价值不是同一种东西,这一点不该被这张表掩盖**:

- 弹簧给的是**更好的数**(32.33°),没有下界保证——换一份输入可能就不是这个数;
- Ruppert 给的是**构造性保证**(三角形角 ≥20.7°,§11.26 实测 21.09°),数字不如弹簧漂亮
  但它对**任何**输入都成立。

Method-C 的 75.6° 属于前一类——模板恰好给的,没有下界证明。所以「谁的最小角更大」和「谁
有保证」是两个问题,而生产上要哪个取决于是否有人逐份检查网格。

**没有试过的组合**:让受保护段也参与弹簧(把段上的点约束在段上滑动,而不是完全冻结)。
那是 r-adaptation 里 `SiteMobility::BoundaryCurve` 那个变体本来的用途——类型早就定义了,
从来没有实现。

### 11.28 两个 bug：弹簧的标定基准，和用站点集合冒充段列表（2026-08-07）

回头审这一段写得最快的代码,找到两个。

**一、弹簧的形状因子用全部边取中位数。** 注释写着「未细化网格的单元宽度」,但中位数是对
**包含细化短边**的全部边取的。细化区一大,中位数就塌,`shape_factor` 偏小,于是每个目标
都变小、弹簧会压缩整张网格。当前配置下修与不修差 0.08°(32.33 → 32.25),**因为细化边恰好
是少数——是运气不是正确性**。已按定义修:只用落在所有区域之外的边。

**二、用「两端点都是受保护站点」冒充段列表,这是不健全的。** Ruppert 的段是显式列表
(PSLG);「两端都在边界上」不是同一个谓词——两个恰好相邻但不在边界曲线上的站点会被当成
段,而每次分裂又增加这样的配对。

这一条是试图**修**它的时候暴露的:Ruppert 的归纳要求分裂产生的中点也是段的端点,补上这
条传播之后,最小三角形角从 **21.09° 掉到 12.29°**——伪段成倍增加,把候选全都转去分裂,
而分裂本身不改善角度。

**所以 §11.26 那个 21.09° 依赖于保护不传播,也就是依赖这个近似继续保持破碎。** 数字是真
的,机制不是 Ruppert。传播修复撤掉了(它让结果更差),但这一节留在这里,因为下一个人看到
21.09° 会以为 Ruppert 已经实现了。

**要真正实现,需要显式的段列表**:细化区域的边界离散成一串段,分裂时把一段换成两段。
`SphericalBoundaryModel`(`earthmesh_boundary`)已经有 `LoopType` 和 `BoundaryRole`,那是
放段列表的地方。

### 11.29 换成真正的段列表：Ruppert 在它的界内成立，界外发散（2026-08-07）

§11.28 指出用站点集合冒充段列表是不健全的。换成真列表:**跨越区域边界的边**(一端在内、
一端在外),分裂时一段换两段。

| 角度目标 | 段列表 | 站点 | 最小三角形角 | 结果 |
|---|---|---|---|---|
| 20° | 真列表 | ~490 | **>20.7°** | **收敛** |
| 20° | 无 | 484 | 17.59° | 收敛,但达不到界 |
| 25° | 真列表 | 4087 | — | **发散** |
| 25° | 站点集合(不健全) | 489 | 21.09° | "收敛" |

**两件事同时被纠正:**

一、**§11.26 那个 25° 收敛是假象**。不健全版把几乎所有候选都转去分裂伪段,**根本没在做
质量细化**——所以它"收敛"。换成稀疏的真段列表后,外心插点真的开始进行,然后发散。

二、**真实行为恰好是理论说的**:Ruppert 的证明保证到约 20.7°,20° 收敛且跨过界,25° 发散。
30° 要 Chew 的变体。之前以为"实现了 Ruppert 并在 25° 拿到 21.09°"是既超出理论又靠 bug
得到的,两头都不对。

**教训**:一个近似如果让结果**变好**,要先怀疑它是不是让算法根本没在跑。不健全版的
489 站点比健全版的 4087 少一个数量级——当时读作"高效",实际是"没做事"。

### 11.30 全链条审查：四个缺陷（2026-08-07）

按缺陷类别扫 HARP-DV 全链条,不是通读。四个,前三个已修。

**一、度数缓解路径吞掉 `propose_move` 的错误。** `if let Ok(Acceptance::Committed(_)) = ...`
把 `Err` 丢了,而那个 `Err` 只在**回滚失败**时产生——意味着网格已经不一致,而循环继续细化
它。改成 `?`。

**二、缓解移动算进 `accepted_this_cycle`,污染终止信号。** 缓解不服务任何需求,把它算作
「本轮有接受」会让 `NoAcceptedTransactions` 永不触发,一个只做缓解的运行会跑满轮次并报
错误的停止原因。实测只触发 0–1 次所以没咬到。改成不计入。

**三、站点表与三角剖分可能不同步,而失步是静默的。** `to_triangular_mesh` 用
`corner - MESH_STATE_FIRST_ID` 查站点表取 generation,查不到就 `unwrap_or(1)`——**读起来
正好像一张未细化的网格**。现在长度不等直接报错。

**四、`base_cell_m` 与 `TargetScale` 比较的量不是同一个,未修。** 公式
`2πR/(5·nxp)` 给的是名义单元**宽度**(NXP 21 时 381 km),而 `TargetScale` 拿它比
`sqrt(A/π)`(实测未细化 P98 约 1073 km)。**差近三倍**,所以 level-L 请求要的比 Method-C 的
level-L 细得多。

改成用网格中位 `sqrt(A/π)`(即被比较的那个量)之后**运行崩溃**:目标场整体位移,细化产出
的三角形被写出层以「非局部外心」拒绝。所以这个不一致在别处是承重的,撤回了。两半都记在
这里,下一次从测量而不是从公式开始。

**共同点**:一、三是**静默失败**(吞错误、静默默认值),二、四是**量与量的错配**。四个里有
三个和这一段前面记录的教训同形——§11.28 的形状因子、§11.19 的 `realized_max_level`,都是
「用推导代替测量」或「让错误安静地过去」。

### 11.31 单位错配的真相，和它为什么还没修（2026-08-07）

§11.30 的第四个缺陷查清了,而且我在那里的判断方向是反的。实测 NXP 21 基础网格:

| 量 | 值 |
|---|---|
| 公式 `2πR/(5·nxp)` | 381.3 km |
| 单元 `sqrt(A/π)` 中位 | **189.7 km** |
| 三角形边长中位 | **363.7 km** |

**公式给的是三角形边长(381≈364),不是单元尺度(190)。** 于是:

- `TargetScale` 拿 381/2^L 去比单元尺度 190,**level 1 的目标 190.6 对实际 189.7——几乎不
  要求任何细化**,level 2 才相当于一次对折。**这就是 §11.12 测到欠细化的原因**
  (5010 单元对 Method-C 的 7064)。
- 弹簧的 `shape_factor = 363.7/381.3 ≈ 0.95`,而弹簧要的正是边长——**它是碰巧对的**,
  也解释了 §11.30 把 `base_cell_m` 改成单元尺度会让弹簧目标减半、压缩网格。

**正确的修法是把两个用途分开各自量。做了,然后运行死在写出层**:
`M point 2600 has a non-local spherical circumcenter`。而新加的薄片门控(见下)报告
**0 次拒绝**——**出问题的三角形不在任何事务碰过的集合里**,所以它不是插点造的;弹簧守卫
也没触发,所以也不是平滑造的。这是写出层一侧的问题,本节没有答案。

**所以修正是量出来的、没有应用的。** 一条死在输出的链条比一条按文档记录的倍数欠细化的
链条更糟。`harp_base_lengths` 留在代码里并被调用(结果丢弃),下一次从它开始。

**顺带加的两道门控是真实改进,保留:**

- **薄片门控**:事务若会留下角度低于阈值、或**外心不局部**的三角形就拒绝。后者用的是写出
  层自己的谓词 `circumcenter_is_local_enough`(已从 `pub(crate)` 提升),不是近似——
  「大钝角三角形可以所有角都 >5° 而外心离得很远」正是代理指标会漏掉的。
- **弹簧守卫**:平滑后若最差角下降就丢弃平滑结果。优化让结果变坏时应该被丢掉,而不是
  传下去让写出层去发现。

两者在当前配置下都是 0 次触发——它们防的是修正单位之后暴露出来的那类失败。

### 11.32 修好尺度单位：细化深度到位，写出层退回机制（2026-08-07）

§11.31 量出 `2πR/(5·nxp)` 是三角形边长不是单元尺度,但没敢应用。这一节应用了,并解决了
它触发的写出层失败。

**两个用途分开各自量**:`TargetScale` 拿单元 `sqrt(A/π)` 中位(190 km),弹簧拿三角形边长
中位(364 km)。

**效果立竿见影**:同一份 namelist,单元数 4974 → **7723**,超过 Method-C 的 7023。
**之前测到的"欠细化"完全是这一个单位 bug**,不是后端能力不足。

**写出层失败的真因是弹簧。** 加了退回机制之后一次就看清楚了:平滑后的网格被
`pcvt_adjust_voronoi_grid_state` 以「非局部外心」拒绝,未平滑的可以写。所以现在的流程是
**试平滑 → 写不出就退回未平滑**,并在 stderr 说明。

弹簧为什么在正确需求下失败,这一节没有答案。排除了两项:迭代数(10 次也失败)和梯度常数
(0.05–0.50 全失败);`move_interior=false` 可写但什么都没动。

**同 namelist、均无平滑的真实对照**:

| 后端 | 单元 | 最小角 |
|---|---|---|
| method_c | 7023 | 27.40° |
| harp_dv | **7723** | 17.07° |

顺带纠正一个我一直在引用的错数:**Method-C 未平滑是 27.40°,不是 75.6°**——后者来自另一
个配置。真实差距是 17 对 27,不是 17 对 76。

**生产状态**:细化深度、可写出、诚实退回都到位;**最小角 17.07° 仍在质量门禁 25° 警告线
之下**,而 Method-C 在线上。这是剩下的唯一实质缺口。

**弹簧在正确需求下失败的原因仍未定位。** 已排除四项:迭代数(**1 次**和 100 次一样失败)、
梯度常数(0.05–0.50)、目标幅度(把每条边的目标夹到当前长度的 1.5 倍以内,同样失败)、
`move_interior=false`(可写但什么都不动)。单次 Jacobi 就产出不可写的网格,说明问题在弹簧
的位移本身而不是它被要求走多远。下一个该查的是 `spring_nest_with_edge_targets` 在
`mrow=0` 的网格上怎么算位移——Method-C 的网格每个面都带真实 mrow,HARP-DV 全填 0。

### 11.33 薄片门控就是质量杠杆：HARP-DV 两项都超过 Method-C（2026-08-07）

§11.32 之后缺口只剩最小角(17.07° 对警告线 25°)。弹簧在正确需求下无论怎么调都产出不可写的
网格,四项排除完仍未定位。**换用已经在手的杠杆:薄片门控。**

它原本只当作「防退化的地板」设在 5°。实测它**直接决定成品网格的最小角**:

| 门控 | 单元 | 最小角 |
|---|---|---|
| 5° | 7723 | 17.07° |
| 15° | 7723 | 17.07° |
| 22° | 7616 | 22.03° |
| 25° | 7419 | 25.01° |
| **28°** | **7371** | **28.12°** |

机制很直接:会造出薄三角形的插点被拒绝,候选阶梯就去试下一级,而后面几级(off-centre、
最长边中点)本来就更保守。**代价只有 4% 的单元。**

**同 namelist 的最终对照:**

| 后端 | 单元 | 最小角 |
|---|---|---|
| method_c | 7023 | 27.40° |
| **harp_dv** | **7371** | **28.12°** |

**HARP-DV 在两项上都超过 Method-C**:多 5% 的单元,且最小角更高、在质量门禁 25° 警告线之上
有 12% 余量。

默认取 28° 而不是门禁自己的 25°:门控是**接受下限**,成品会落在略高处,设在线上就没有余量
——实测 25° 给出 25.01°。

**这一节的方法教训**:卡在弹簧上做了四轮排除,而解法是一个**早就建好、当时只当作安全网**
的东西。我把它设成 5° 是因为把它想成「防止退化」,没想到它是「选择质量」——**同一段代码,
换一个用途的问法就换了一个量级的效果。**

### 11.34 把薄片门控推到拐点：30°，两项都赢（2026-08-07）

§11.33 定在 28°。往上推,找到质量与细化量的交换曲线:

| 门控 | 单元 | 最小角 |
|---|---|---|
| 5° | 7723 | 17.07° |
| 28° | 7371 | 28.12° |
| **30°** | **7132** | **30.00°** |
| 31° | 6162 | 31.01° |
| 32° | 4835 | 32.01° |
| 33° | 4425 | 33.07° |
| ≥36° | 4413 | 47.26° |

**≥36° 那几行不算数**:4413 就是 NXP 21 的基础网格,细化被全数拒绝,47.26° 是没细化的角度。

**30° 是拐点。** 31° 起单元数(6162)已低于 Method-C 的 7023——网格不再是被请求的那张网格。
30° 时 7132 单元 / 30.00°,对 Method-C 的 7023 / 27.40°,**两项都赢**,且在质量门禁 25° 警告
线上有 20% 余量。

它落在 Chew 算法的 30° 界上是巧合——这个数来自扫描,不是来自理论。

**曲线形状本身值得记**:5°→28° 之间单元数只掉 4.6% 而角度涨 65%,28°→32° 之间角度只涨 14%
而单元数掉 34%。**便宜的质量在低门控那一段,拐点之后是在拿分辨率换角度**——而分辨率正是
用户请求的东西。

### 11.35 「细化层数」现在的定义，以及一个没能量准的指标（2026-08-07）

**代码里的定义是明确的**:一个 level L 的区域,要求区域内单元的 `sqrt(A/π)` 达到
「**输入网格自身的中位单元尺度** ÷ 2^L」。基准从网格量出来,不是套 `2πR/(5·nxp)`——那正是
§11.32 修的 bug。

Method-C 的 level 是嵌套趟数:每趟三角形四分 → 面积 /4 → 宽度 /2,**同样是 L 次宽度对折**。
两者在定义上一致。

**但「实际交付了几层」目前没有可信的度量,而这一节是失败记录。**

运行记录里的 `refine_realized_halvings`(全球 P2/P98 之比)不是层数:它含二十面体自身的
不均匀和海岸线切割,两个后端在请求 2 层时都读出约 4 层。

试着补了一个「区域内中位 ÷ 区域外中位」的指标,读出 method_c 0.63、harp_dv 1.35——绝对值
可疑,请求 2 层不该只有 0.63。核对中位数发现:**区域外中位 429–460 km,而 NXP 21 基础网格
的中位单元是 190 km**,不可能;计数也只有约一半的单元(3649 对 7023),说明遍历
`w_to_m` 时跳过了大量单元。**没有发布**——一个自己核对不过的指标比没有指标糟。

**所以现在能诚实说的是**:请求的层数有明确定义、`realized_max_level` 三个后端三个含义
(§11.19)、全球对折数可比但不是层数、**区域层数还没有可信度量**。下一次从「为什么
`w_to_m` 的遍历只覆盖一半单元」开始。

**第二次尝试同样没成,把已知的记下来省得第三次重走**:遍历 `w_to_m` 时**不加任何过滤**能
覆盖全部单元(实测 `w_to_m=7132 w_points=7132 short=0 no_area=0`);一旦同时要求
`w_points.get(row)` 存在,覆盖率掉到约一半(7023 单元只统计到 3649)——而两个数组是等长的,
`get` 本不该失败。区域外中位仍读出 429–460 km,对基础网格已知的 190 km 对不上,两者很可能
是同一个原因。

**下一次不要从「怎么算中位数」开始,要从「`w_to_m[row]` 和 `w_points[row]` 是不是同一个
单元」开始**——这个假设我一直没验证,而它正是唯一新引入的条件。

**第三次尝试:找到了覆盖率的原因,面积仍然错。**

覆盖率是**面积的符号**:`robust_spherical_area_unit` 按契约返回**有符号**面积,而约一半的
单元绕向相反给出负值,被我的 `steradians <= 0.0` 跳过。取绝对值后覆盖率立刻满
(2894+4237=7131 对 7132)。

**但绝对值仍然错约 20 倍**:所有单元面积之和 255.7,而球面是 4π=12.57;中位单元读 450 km,
而 7132 个单元均分球面只能是约 151 km。函数契约没问题(单位球面有符号面积,仓库里只有测试
在用),**两个后端都错**——所以是我的用法。

最可能的原因:`w_to_m[row]` 的角点**不是按环序排列**,多边形自交,面积无意义。这与
§11.23 的教训同形——一个顺序错误的多边形仍然是闭合多边形,角点数和度数都对,只有面积
不对。

**下一次从「`w_to_m` 的角点顺序」开始验证**,而不是再改一遍中位数或面积公式。仓库里已有
`mesh_cell_vertex_ordering`,那大概就是该用的东西。

**第四次:排序和索引基都排除了,指标放弃。** 按方位角把角点排成环之后面积和从 255 变成
**304**(更差);把 `im` 当 0 基索引则变成 **1336**(远更差,所以一基是对的)。三个候选解释
——覆盖率、角点顺序、索引基——只有第一个(面积符号)是真的,而它修好之后面积仍差 24 倍。

**已排除**:面积函数契约(仓库里只有测试在用,契约写明是单位球面有符号面积)、覆盖率、
角点顺序、索引基、后端(两个都错)。

**没排除**:`w_to_m` 的角点是不是这个单元的角点。这是整件事唯一没验证过的假设,而前四次
都建立在它之上。验证它只需要一个断言:任取一个单元,它的每个角点到中心的距离应当都在
一个单元宽度的量级——如果读到的是别处的点,这条会立刻失败。

**四次尝试到此为止,指标不发布。** 生产判据不依赖它:细化深度由单元数和 `TargetScale` 的
定义保证,质量由最小角保证,两者都已验证。这只是一个还没做出来的观测量。

### 11.36 第五次成了：`w_to_m` 的行不是角点列表（2026-08-07）

前四次(§11.35)全建立在同一个没验证的假设上。读构造处一眼就看见:

```rust
n_w_to_m.push(count);                  // npoly，有效角点数
w_to_m.push(tabs.w[iw].im.to_vec());   // 完整 7 宽行，未截断
```

**`w_to_m[row]` 是完整的 `itab_w.im`(宽度 7),只有前 `n_w_to_m[row]` 个是这个单元的角点。**
其余是占位槽,而**占位 id 1 会解析成 `m_points[0]` —— 一个真实存在但位置无关的点**,把每个
多边形都撑大。这就是 24 倍。

按 `n_w_to_m` 截断之后,三重验证都过:

| 验证 | 结果 |
|---|---|
| 区域外中位 vs 独立测得的基础网格中位 | **189.45 km 对 189.7 km**(差 0.13%,不同代码路径) |
| 请求 2 层 → 实测对折数 | method_c **1.98**、harp_dv **2.10** |
| 覆盖率 | 7022 / 7023 |

**面积总和仍是 4π 的两倍,而这不影响指标**:少数跨极或跨日界线的多边形,
`robust_spherical_area_unit` 对它们返回补角面积,总和被污染而中位数不受影响。**用中位数
而不是总和或极值,恰好是对的选择**——§11.20 因为极值被切割碎片主导才改成分位数,这里同一个
理由再次成立。

**五次尝试的账**,四次失败各自排除了一个候选:

| 尝试 | 假设 | 结论 |
|---|---|---|
| 1 | 中位数算法 | 无关 |
| 2 | `w_points[row]` 与 `w_to_m[row]` 不同源 | 排除(同序构造) |
| 3 | 面积符号 | **真因之一,修好** |
| 4 | 角点顺序、索引基 | 均排除(改了更差) |
| 5 | **行未按 `n_w_to_m` 截断** | **真因,修好** |

**教训**:四次都在改「怎么算」,而错在「读到的是什么」。第五次先去读了数据是怎么构造的,
一眼就看见了。**在验证输入之前优化算法,是把力气花在正确性的下游。**

### 11.37 后端切换:分派会静默猜错,GUI 少一个后端(2026-08-07)

按「先查共有部分」的顺序查后端切换,三个缺陷,全部已修。

**一、分派用 `_ =>` 兜底到 Method-C,拼写错误静默换后端。** 实测:

| 写法 | 修复前 | 修复后 |
|---|---|---|
| `harpdv` | Method-C(7023 单元,静默) | 点名拒绝 |
| `harp-dv` | Method-C | 点名拒绝 |
| `redgreen` | Method-C | 点名拒绝 |
| `method-c` | Method-C | 点名拒绝 |
| `HARP_DV` | Method-C | **正确跑 HARP-DV** |

用户要一个后端、拿到另一个、没有任何提示——正是 §11.1 那类沉默失败。现在分派走一个具名
枚举,校验在 `EarthmeshConfig::validate_like_read_nl` 里,大小写不敏感。

**二、`mode_grid` 同样没有校验。** 十几处 `match` 都带 `_` 兜底读成 hex。一并加了校验;
Canonical 的未设置占位符 `/tmp` 显式放行(`tests/constants.rs` 钉着这个契约,而管线本来就把
它读作 hex)。**校验放在配置层而不是每处 match**——`mode_grid` 有十几处匹配,总会漏一个。

**三、GUI 根本没有 HARP-DV。** `RefinementBackend` 枚举只有两个变体,
`set_refinement_backend` 只认两个名字,前端下拉只有两项。**引擎发布了第三个后端,而 CLI 以上
没有任何一层能选它。** 补齐了 schema → lowering → Tauri 命令 → 前端选项整条链。

顺带修掉一处过时文案:前端把 Red-Green 标成「接线中,选中会报错说明」,而它早已端到端跑通。

**这三个是同一件事的三个面**:一个选项要能用,得在**每一层**都被认识——配置校验、分派、
schema、命令、界面。少任何一层,它要么不可达(GUI),要么被静默替换(分派)。

### 11.38 逐后端:两个组合会静默给出别的东西(2026-08-07)

**一、`&adaptive` 与 `&hfield` 同开,产出的网格比任何一个单开都少。** Method-C 的分支链先
取 adaptive,看起来只是「hfield 被跳过」;但配置 `&hfield` 同时会改变区域采集方式
(`use_hfield_regions`),于是两者交互出第三个结果。NXP 21 实测:

| 配置 | 单元 | 过渡行 |
|---|---|---|
| 只开 adaptive | 7023 | 3398 |
| 只开 hfield | **9510** | 4673 |
| 两个都开 | **4875** | 1914 |

代码里对这个组合没有语义,现在点名拒绝。

**二、HARP-DV 静默忽略它服务不了的选项,而红绿会拒绝。** 同一份 namelist 加上 `&hfield`:

| 后端 | 修复前 |
|---|---|
| red_green | 点名拒绝 ✓ |
| harp_dv | **照跑,产出 6450 单元,从未读那个场** ✗ |

补上了同一道守卫(h-field、笛卡尔 XY、native 表面扩张),三个后端现在对「服务不了的请求」
行为一致。

**已核查为正确的**:层数上限 1..=5 在三条路径上一致强制——直接路径拒绝,
`adaptive_max_level` 与 `hfield_max_level` 在解析处校验 `0..=5`,分支里的 `clamp(1,5)` 是够
不着的防御。

**共同形状,和 §11.37 一样**:一个选项被读进配置、却没有任何一层负责它,结果就是静默换成
别的东西。守卫要放在**分派处**,因为那是唯一知道「这个后端能做什么」的地方。

### 11.39 默认路线丢掉了弹簧,报告照写它没跑的迭代数(2026-08-08)

把三个后端 × 三条路线(点+半径 / H 场 / discrete)九个组合跑了一遍。除了 §11.38 已修的
两个 H 场组合,这个矩阵还翻出一个只在**默认路线**上出现的失效。

**症状**:同一份 namelist,只差一个 `&adaptive` 组:

| 路线 | `refine_spring_passes` | 实际移动的点 |
|---|---|---|
| 直接(无 `&adaptive`) | 2 | **5182 / 7023** |
| 点+半径(有 `&adaptive`) | 0 | **0 / 7023** |

两次都打印 `refine_spring_iterations=200`。

**根因**:`refine_with_method_c` 的 adaptive 分支硬写 `(refined, 0)`,循环里两处都调无弹簧
的 `spawn_nest` 重载。H 场路线走 `spawn_nest_from_target_levels_with_spring`,直接路线走
`spawn_nest_with_spring_and_max_mrows`——只有这一条漏了。而它是 GUI 的默认。引入它的那次提交
(4e93501)通篇没提弹簧,`nest.rs` 里也一处没有:是遗漏,不是决定。

**修法**:`spawn_nest_adaptive_with_named_regions` 收一个 `Option<AdaptiveNestSpring>`,循环
里用一个 `refine_once` 闭包统一决定是否加弹簧——两处调用点原本就是因为各写各的才一起漏的。报告
新增 `spring_passes`。

顺带修正:裸 `spawn_nest` 恒用 `MAX_MROWS_SURFACE`,atmosmesh 也一样;弹簧重载要求显式传宽度,
于是这条路线终于和其它路线一样按 `is_atmosmesh` 选 13。实测这个 fixture 下网格逐位相同,只有
报告的过渡面数从 3398 变 5807——约束在这个尺寸不生效,生产尺寸未验证。

**为什么测试没发现**:所有 adaptive 测试都断言单元数和层数,而弹簧**不改变任何一个**——它只移动点。
断言 `spring_passes > 0` 也不够,错数字和对数字一样好造。新回归测试比的是两次运行的坐标:配了弹簧,
点就得动。这和 §11.3 的教训是同一条——**留一个已知正确的实现当 oracle 逐点比对**;这里的 oracle 是
直接路线,修复后两条路线移动的点数完全一致(5182/7023)。

**GUI 侧**:「细化算法」和「细化方案」是两个互不知情的下拉,于是能选出 `harp_dv` + `H 场` 这种
运行时必然被拒的项目。现在 H 场选项在非 Method-C 下禁用并注明原因,切换算法时会把已选中的 H 场退回
——光禁用不够,浏览器会保留一个已选中的 disabled 选项。项目层 `validate()` 也补了同一条拒绝:项目
在运行之前很久就被编辑和保存,只在派发处拒绝意味着唯一的知情方式是启动一次运行然后看它失败。

**同一形状,再往前一个分支**:native `&ngrids`/`&nsfcgrids` 的 spawn 排在 Method-C 分支链的最
前面,且从不查看另外两条路线。NXP 6 实测:只给 `&nsfcgrids` 与 `&nsfcgrids` + `&adaptive` 产出
**逐位相同**的 435 单元网格,exit 0,日志里没有一行 adaptive 输出。已一并拒绝。

**这道守卫第一次写错了,值得单独记一笔。** 条件写成「配了 native 区域」就拒绝,64 个测试当场变红。
真相是这一对**并不总是**吞并:`refine_spc` 打开时 native spawn 会让位,H 场分支照跑——笛卡尔 XY
正是这样同时服务 `&ngrids` 和 h-field 的。守卫的条件必须和它所保护的那个分支**逐字相同**,而不是
「看起来该拒的样子」;差别不在措辞,在于分支链上游还有别的条件会改变谁先跑。这和 §11.1 的失败类
互为镜像:那边是该拒的没拒,这边是不该拒的拒了,而两者的成因是同一个——**没有把判断建立在实际
控制流上**。回归测试同时钉住两侧:该拒的拒,该跑的跑。

到这里,Method-C 分支链上每一对「没有任何一层负责组合」的选项都会点名拒绝:
native × adaptive、native × hfield、adaptive × hfield;跨后端的 h-field / 笛卡尔 XY /
native 表面扩张见 §11.38。

### 11.40 过渡宽度到底改不改网格,以及三个什么都不做的旋钮(2026-08-08)

**一、§11.39 里「只改报告数字」的说法是在没开弹簧的条件下测的,推广错了。**

直接对着两个宽度各跑一遍 adaptive 路线:

| 条件 | 面数 | 深度 | 过渡行 | 位置不同的点 |
|---|---|---|---|---|
| 无弹簧,NXP 21,单圆 | 相同 | 相同 | 744 vs 1640 | **0 / 4484** |
| 有弹簧,NXP 21,单圆 | 相同 | 相同 | 744 vs 1640 | **599 / 4484** |
| 有弹簧,NXP 30,五圆两层 | 相同 | 相同 | 663 vs 1379 | **1038 / 9236** |

机制清楚:过渡行决定弹簧能动哪些点。**不开弹簧时宽度是分类,开弹簧时宽度就是几何。**所以
§11.39 让 adaptive 路线按 `is_atmosmesh` 选 13,对 atmosmesh + 弹簧的运行是真的换了网格——那正
是它该做的(与其它每条 Method-C 路径一致),但先前的描述不完整。教训与 §11.3 同源:**一次测量
只回答它自己的条件下的问题**,「在这个 fixture 上逐位相同」不等于「这个量不影响网格」。

**二、三个 GUI 专家控件,任何后端都不读。**

`RL%set_dis_type`、`RL%num_rc`、`RL%vertex_pretect_layers` 会被解析、校验、lowering、写回
namelist、在 GUI 里作为可填控件出现——然后没有任何细化代码读它们。`RL%iterD` 同样(它没有 GUI
控件)。三个后端各测两个取值,网格**逐位相同**:

- `num_rc` / `set_dis_type` 只进 `SpringjustmentGridfileOptions`,而那个结构**只在测试里被构造过**;
  mkgrd 的细化路径给全局弹簧传的是 `niter/beta/relax`,没有这两个。
- `vertex_pretect_layers` 除了「`spring_global_type > 0` 时清零」这条校验之外,无人读取。

**这次特别记下测量条件**,因为第一遍测错了:头一轮两个弹簧都关着,于是「无变化」什么也证明不了
——和 §11.39 里 discrete 路线那个假象是同一个错误。重测时全局弹簧开到 `niter=60`、区域弹簧开到
`SpringRegional_type=1 / niter_refine=100`,并**先用 `niter=0 vs 60` 确认弹簧确实在跑**(网格
有变化),再判定这三个旋钮无效。

没有实现它们:从字段名反推 Canonical 的语义就是猜。改为在 GUI 里注明「本版不作用于网格,仅写入
namelist 以保持与 Canonical 一致」,并加了一条静态检查——将来谁实现了它,得先把这句话拿掉。

**三、HARP-DV 弹簧里的两处死代码和一条错注释。**

- `harp_base_lengths` 返回 `(单元尺度, 边长)`,而 `base_edge_m` 从未被使用(编译器一直在警告)。
- 梯度那里读了一个 `EM_G` 环境变量到一个没人用的局部变量,真正生效的是它下面的 `const GRADIENT`。
  也就是说,**一个读代码的人会去拧的旋钮,拧了没用**。
- 注释说 `shape_factor` 是「near one,是个 guard 不是换算」。实测 **1.91**(190 km 单元尺度 →
  363 km 中位边长),它就是换算。那句话描述的是除数为 `base_edge_m` 的旧版本——真按那样写,喂给
  边长弹簧的就是单元尺度,正是 §11.31 记的那个减半缺陷。**代码是对的,注释是错的**,而错注释比
  没注释更危险:它会把下一个人引向那个缺陷。

三处都清理后 HARP-DV 输出逐位不变。

**四、红绿:关掉弱凹消除必然产出写不出去的网格,而结构体默认值正是那个。**

`RL%weak_concav_eliminate = .FALSE.` 在 NXP 15、21、30 上**全部**失败:「produced a cell with 8
incident triangles」——超出 gridfile 的 `[i32; 7]` 关联行能装的 7 度。`.TRUE.` 三个规模全部成功。

失败是**响亮的**(拒绝而不是写出坏网格),所以不是 §11.1 那一类。但两处描述与之矛盾:

- `RedGreenSettings::default()` 就是 `eliminate_weak_concavity: false`,文档写「false 是默认,
  因为那是保住凹口形状的处理方式」——只讲了形状偏好,没讲它在 gridfile 路径上必然撞墙。
- GUI 的开关只说「未设置时用引擎默认」。

没有改这个结构体默认值:`RefineConfig::weak_concav_eliminate` 默认 `true`,namelist 路径每次都
覆盖它,所以生产路径本来就是对的;而不写 gridfile 的直接调用者可能真的要那个形状。改的是**两处
说明**,把实测代价写进去——一个默认值可以是「要求调用者明确知道自己在要什么」,但前提是文档说了
要什么。

**五、HARP-DV 的配置里也有一个:`patch_ring_depth`。**

有文档(「补丁围绕种子取几环邻居」)、有默认值 2、**没有校验**(同结构体其它每个字段都校验),
**没人读**。而 `HarpDvConfig` 自己的文档写着「Validated rather than trusted……每个字段都在这里
检查一次」,`deterministic` 那条更是明说「accepting the flag without honouring it would be a
promise nothing keeps」——这个字段正是那句话说的情形。

没有改成「校验它必须等于 2」,因为那个数本身就不成立:插入取的是空腔的一环,移动取的是那一环的
再一环,深度按操作而不同。**删掉**比记录一个半真的常量诚实。`HardGates` 与 `CandidatePolicy` 的
每个字段都查过,都有消费。

**这一节五条的共同点**:每一条都是「声明的意图」和「实测的行为」对不上,而不是崩溃。过渡宽度的
「只改报告」、三个旋钮的「可以设置」、`shape_factor` 的「near one」、弱凹消除的「false 是对的」
、`patch_ring_depth` 的「每个字段都会被校验」——五句话都写在代码或界面里,五句都被测量推翻。
**代码里的一句断言,和代码本身一样会过期**,而没有测试钉住的断言过期时不会有人知道。

### 11.41 复杂球面边界:模型在,没有后端用它(2026-08-08)

**`earthmesh_boundary` 就是那个子系统**,只是名字里没有 "BRep"——这个字串在全仓任何文件、任何
分支的历史里都没出现过。它的模块文档说的正是边界表示:边界作为**拓扑**而非点列,记下哪一侧是水、
哪个环是岛中之湖、哪一段可分割但不可跨越。

| | 状态 |
|---|---|
| 类型与不变量 | 已做:`BoundaryRole`/`LoopType`/`BoundaryVertex`/`BoundaryLoop`/`SphericalBoundaryModel`/`BoundaryError` + `validate()`/`topology_counts()`,214 行 + 146 行测试 |
| 依赖它的 crate | **零**(`grep earthmesh_boundary rust/*/Cargo.toml` 只匹配自己) |
| HARP-DV 是否用 | 否 |

crate 自己划了界:「类型与不变量在这里。适配策略——encroachment、段分割、滑动、窄特征策略——属于
做适配的那个后端。」**三个后端都没做那一半。** 按原分工看它是完成的;按「能处理复杂球面边界」看
没有。

**HARP-DV 现有的替代品窄得多**:`harp_region_boundary_segments` 把细化**圆**的边界离散成跨界的
网格边,作为 Ruppert 的保护段。它不知道海岸线、不知道湖、不知道不可跨越。

**而且这条路径默认是关的**,只在环境变量 `EARTHMESH_HARP_MIN_ANGLE` 设了值才建保护段、才加
`MinAngle` 判据。它不在任何文档、任何 GUI 里。NXP 21 实测:

| `EARTHMESH_HARP_MIN_ANGLE` | 单元 | 最小角 |
|---|---|---|
| 未设(默认) | 7132 | **30.004°** |
| 20 | 7007 | 30.002° |

**开了没有质量增益,还少 125 个单元。**定角度的是硬门禁 `min_triangle_angle_deg = 30`,不是这条
判据。§11.25 当年的结论(「安全但几乎什么都不做」)在保护段修好之后依然成立,所以那个结论不是
被不健全的谓词伪造出来的——这次是在正确实现上重测的。

保留了这个开关而不是删掉:删掉它,crate 里的 Ruppert 机制就从任何运行都够不着,那正是刚清理掉的
`patch_ring_depth` / `EM_G` 那一类。但它必须可发现,所以数字记在这里。

**另一处措辞被测量推翻**:`HardGates::require_closed_surface` 的文档说「区域网格把它设为 false,
改为按边界拓扑设门禁」。**没有那个替代门禁**——设成 false 就是跳过检查,什么都不放进去。没有任何
地方设过 false,所以今天没有运行受影响;但这句话会把下一个做区域边界的人引向一条不存在的路。已
改成说明真实行为,并指向 `earthmesh_boundary`——真要做区域边界,门禁要拿它当依据。

### 11.42 边界子系统按文档接起来,三个设计错误由测试抓出(2026-08-08)

按 `REFINE_CRATE_LAYOUT.md` 与 §11.28 的规定动工。两份文档给的约束和任务:

- **依赖方向 `mesh`/`boundary` → `refine` → 三个后端**。`boundary` 与 `mesh` **并列**。原打算让
  `earthmesh_mesh` 的闭合曲线走查器委托给 `earthmesh_boundary`,那会把并列改成串联,**没做**。
- **§11.28**:段列表该住在这个 crate 里。已做(见下)。

**新增三块,每一块的定型都是测试推翻了我先写下的假设:**

**一、`contains(lon, lat)`——球面上闭曲线两侧的绕数绝对值都是 2π。**
第一版用 `abs(turn) > pi`,于是地球另一面也被判成域内(跨日界线那个测试抓到)。球面上闭曲线**没有
天然的内部**:一侧 +2π,另一侧 −2π。改用**有符号**绕数,并定下约定——每个环逆时针(从球外看),
它围住的是自己左侧那一片;外环的左侧是域,洞的左侧是洞里的空。绕数在球面上求和,日界线与极点因此
不需要特例。

**二、`closed_rings(edges)`——无序边集装配出的环没有定向。**
同样的四条边换个顺序,走出的是相反方向(测试抓到)。而 `contains` 恰恰靠方向判边。所以这个函数
**不能**产出有定向的环:改为「从最小顶点出发、先走较小的邻居」,**可复现但明确不承诺含义**,需要
定向的调用方自己定。度数不变量在走之前就检查(每个边界顶点恰好两个邻居),违反时点名顶点——一个
继续走下去的走查器会返回一个环形的东西,而它不是边界。

**三、`orient_counter_clockwise`——按「哪个方向包住的边界顶点少」定向是没用的。**
环上的顶点两个方向都算在内。改用有符号球面面积,而 `robust_spherical_area_unit` 的符号约定与绕数
**相反**:实测逆时针经纬度方块返回 **−0.00487**,其逆序 +0.00487。按测量写,没按名字猜——猜反了
的话海岸线会「包住海洋」,而且是**一致地**反,所有包含判断同时反过来,反而最难看出来。

**`SegmentList`(§11.28 指派的那件)**:中立类型,`from_straddling_edges` 从「一端在内、一端在外」
构造,`split` 做 Ruppert 的归纳(一段换两段),`split` 对**不是段的边**返回 false——不健全那版
分不清这两种情况,于是每次插点都像在做边界工作,把自己的列表越滚越大(§11.28、§11.29)。HARP-DV
的构造已改为调用它,段列表开与关两条路径的输出**逐位不变**。

**`boundary_model_from_closed_curves`(CLI 侧)**:把 carve 的闭合曲线建成 `SphericalBoundaryModel`,
按嵌套判定 outer/hole,校验不变量,给出 `topology_counts` 那对数——文档说它是「细化必须保持的那对
数」,而三个后端都没调过。放在 carve 处而不是某个后端里:carve 在三个后端汇合之后,写在 Method-C
里就得为红绿再写一遍、为 HARP-DV 再写一遍,而**第三份就是它们开始互相矛盾的地方**。

已接进 `mask_postproc_domain/runners.rs` 的 carve 调用点。海洋 runner 的 fixture 上实测:
**1 个外边界、0 个洞**,模型校验通过。**报告而不强制**(§11.43 已改为强制,见那一节):这条路径上还没有「细化前」的那对数可比,
凭空定一个阈值就是编一个没人测过的数字。模型校验失败会大声说——校验不过意味着 carve 产出的曲线
根本不是边界,那么从它们取的每个计数都没有意义。

**hex 也接上了,但没有实测到。** tri 那条 renewal 会算边界连接是因为孤立海洋剔除需要它,hex 那条
直接返回 `boundary: None` 且完全不做 renewal。那是**两种 carve 各自需要什么**的差别,不是**各自有
什么**的差别——走一遍所需的四个数组两边都在。所以诊断改成自己算一份,不再只对一个 mode_grid 可用。

**hex 已实测**,不再只是「代码走得到」。走到那一步要先绕过三个各自具体的障碍,每一个都是运行自己
点名说出来的,值得记下来——它们是「拿 tri 的东西喂给 hex」会撞上的三堵墙:

| 障碍 | 运行怎么说的 | 原因 |
|---|---|---|
| contain 数组长度 | `IsInDmArea_ustr length 8 must cover ustr_points 14` | carve 走的无结构点,tri 下是 M 点、hex 下是 W 点,长度本就不同 |
| 网格形状 | `vertex 6 has more than 3 neighboring centers` | 原 fixture 是 tri 形状,M 点 6 落在四个 W 单元里;hex 要求每个角恰好三个 |
| 域太小 | `boundary closed curve has fewer than three points` | 四单元里只放两个进域,交界只有两点 |

最终的 hex fixture 是**四面体的对偶**——四个单元、四个角、每个角恰好三个单元,是满足那条规则的最小
排布;三个单元进域,边界绕着第四个走,恰好三个角。tri 与 hex 两条都报出 **1 个外边界、0 个洞**。

那对数也放进了 `MaskPostprocOceanDomainReport::boundary_topology`,不只打在 stderr 上:**断言不到的
诊断会悄悄失效**,而这一节里每一个被推翻的说法都是这么活下来的。

**当时未做的两条,§11.43 都已做掉**——留在这里是因为其中一条的理由是错的,值得对照着读:那对数
改成了强制(只强制单边界这一半);`AdaptiveMesh` 换成了 `SegmentList`,而「会牵动回滚路径」那句
**是没测就下的结论**,读代码即可推翻。

### 11.43 三件遗留,连同其中一句多虑的判断(2026-08-08)

**一、`AdaptiveMesh` 的段列表换成 `SegmentList`。** §11.42 里我写「换过去会牵动回滚路径」,
**那句是多虑**:读代码就能看到 `split_segment` 只在 `Acceptance::Committed` 之后调用,回滚永远
不需要撤销一次分裂。换完 54 项 HARP-DV 测试全过。判断该建立在控制流上,不是建立在「听起来危险」上。

**二、`EARTHMESH_HARP_MIN_ANGLE` 提升为 `RL%harp_min_angle_deg`。**

一个没有任何文档和界面提到的环境变量,意味着它守着的功能只能靠读源码才找得到。改成 namelist 字段,
并在解析处按 Ruppert 的界校验——**超过 20.7° 拒绝**,而不是让运行跑光预算才发现(§11.29 里 25° 就
是这样发散的)。NXP 21 实测:

| `RL%harp_min_angle_deg` | 退出 | 单元 |
|---|---|---|
| 0(默认) | 0 | 7132 |
| 20 | 0 | 7007 |
| 25 | **2,点名 20.7 的界** | — |
| 旧环境变量 = 20 | 0 | **7132(已失效)** |

字段的文档里写明了它买到什么:**什么都没买到**。20° 时 7007 单元 / 30.002° 最小角,关掉是 7132 /
30.004°——定角度的是硬门禁 `min_triangle_angle_deg`,不是这条判据。保留而不删除,是因为删掉之后
crate 里的 Ruppert 机制就从任何运行都够不着了。

**三、那对数从「只报告」改为强制,但只强制机制真正承诺的那一半。**

`renewal` 现在把**renewal 之前**的边界连接也带出来(两种 mode_grid 都算),于是可以跨 carve 比较。
强制两条:

- **模型必须校验通过**——孤儿 hole、hole 套 hole、捏合环意味着 carve 产出的根本不是边界,那么从它
  取的每个计数都没有意义。这是错误,不是提示。
- **那对数只许降不许升**。renewal 与孤立海洋剔除**只会移除**区域;carve 里没有任何东西会添一个岛
  或开一个湖。升了就是凭空造出了域特征。

**没有强制相等**:剔除孤立海就是 carve 的职责,要求前后不变会拒绝正确的运行。**一个单边界正是这个
机制真正承诺的东西,更紧的数字就是编一个没人测过的**——这正是上一节把它留成「只报告」的理由,而
真正缺的不是勇气,是想清楚该强制哪一半。

实测:tri 与 hex 三条路径都是「前 1 后 1 个外边界、0 个洞」。回归测试同时钉住两个方向——丢一个岛
是允许的,而反过来会被拦。

**验证缺口,以及为什么它值得单独补。** §11.42 的三条定向结论全都是作为**组合**验证的:建模型、问湖
在不在域内、答案对了。**两个方向相反的错误互相抵消也会通过这一关**——组合是对的,而单独用其中任一
半的下一个人会踩坑。

已分别对着同一个外部参照钉住,那个参照与本仓库的任何实现无关:三角形 A→B→C 从球外看逆时针,当且
仅当 `(A×B)·C > 0`。

| 事实 | 结果 |
|---|---|
| `contains` 的绕数符合右手定则 | 逆时针环含自身质心,反转则不含 ✓ |
| `robust_spherical_area_unit` 对逆时针环为**负** | −/+ 且等量 ✓ |

于是 `orient_counter_clockwise` 建立在两个各自成立的事实上,而不是它们的乘积上。

**这三条的分量,记在这里免得后来人误读**:它们不是躺在生产代码里活过多次提交的缺陷(那是 §11.38
到 §11.41 那些),而是**同一轮里写下、同一轮里被自己的测试拦下**的。流程正常工作,不是发现。其中
只有「球面闭曲线两侧绕数绝对值都是 2π」和「面积函数的符号约定与名字不符」两条有独立价值;
「无序边集不决定定向」按定义就是如此。

**还留着一个形状上的隐患**(§11.44 已根治):`contains` 需要定向 → `closed_rings` 给不出定向 → `orient_counter_clockwise`
用面积补上。中间那环交付不了第一环要的东西,靠第三个函数打补丁。只要有人绕过第三个函数直接把环塞
进模型,`contains` 就会静默给出反的答案——**一个靠调用方记得调某函数来维持的不变量**,正是本轮反复
在别处指出的形状。要根治得让类型造不出没有定向的 `BoundaryLoop`(构造时要一个已知内点或显式定向)。

### 11.44 把定向从「记得调某个函数」搬进类型(2026-08-08)

§11.43 末尾记了一个没根治的形状:`contains` 需要定向 → `closed_rings` 给不出定向 →
`orient_counter_clockwise` 用面积补上。**只要有人绕过第三个函数直接把环塞进模型,`contains` 就会
静默给出反的答案**,而且是一致地反——最难看出来的那种。

`BoundaryLoop::vertices` 改为私有,两个构造器代替它:

- `counter_clockwise(...)` —— 调用方**断言**这个顺序已经是从球外看逆时针的。类型检查不了这件事:
  球面闭曲线两侧都被它围住,环本身没有任何性质能说出「本来想要哪一侧」。
- `enclosing(..., model_vertices, interior)` —— 调用方给一个**已知在内部的点**,由构造器决定方向。
  这是 `closed_rings` 之后该接的那个:它给不出定向并且明说了。点落在环上、或环退化时返回 `None`
  ——两个方向都「含」环上的顶点,那里绕数无定义,硬给一个答案就是替调用方选边。

改完编译器立刻在 CLI 里点出**正好是我想防的那两处**构造。crate 内部的测试仍能编译(私有字段在 crate
内可见),保护针对的是外部消费者——而外部消费者正是唯一会绕过约定的那一类。

**这是本轮唯一一次把一条不变量从约定移进类型。**其余的守卫(§11.38 的组合拒绝、§11.39 的弹簧、
§11.43 的单边界)都仍然是「在正确的地方检查」,而不是「让错误状态构造不出来」。前者依赖有人记得检查,
后者不依赖——差别在这一节里是编译错误和静默反向答案的差别。

**更正 §11.44 的标题所说的。** 那次提交叫「让没有定向的环造不出来」,**说过头了**。`vertices` 是私有了,
但 `counter_clockwise(...)` 仍然收任意顺序并直接信它——外部调用方照样能造出定向错的环,只是必须先
说出「我在断言这件事」。准确的说法是:**从「静默默认」变成了「必须明写的选择」**。

而且断链当时还在原地:CLI 里 `closed_rings → orient_counter_clockwise(自由函数) → counter_clockwise(断言)`,
新加的那一环只是让断言显形。**`enclosing`(那个要一个已知内点的安全构造器)当时只有测试在用,生产
代码一处都没用**——我刚制造了本轮反复指出的同一形状的第四例:加了个能力,没人接上(前三例是
`earthmesh_boundary` 整个 crate 零依赖者、`patch_ring_depth`、`vertex_pretect_layers`)。

**现在接上了。** 洞的「内点」是湖心,carve 拿不到,所以 `enclosing` 不是 carve 要的那个;真正该加的是
**按面积定向的构造器** `BoundaryLoop::bounding_smaller_side`,把那个自由函数收进类型。CLI 改用它,
`orient_counter_clockwise` **删掉**——是删掉一个函数,不是再加一个。

面积在 `earthmesh_boundary` 里自己实现(它不依赖 `earthmesh_mesh`),用 Van Oosterom–Strackee 的立体
角。**它的符号与 `earthmesh_mesh::robust_spherical_area_unit` 相反**:同一个逆时针三角形,这里 +,
那里 −。两个同名形状、相反约定的函数,正是海岸线包住海洋的成因,所以两个都**各自**对着右手定则
`(A×B)·C > 0` 钉住,而不是互相对拍——互相对拍会让任一个的约定取决于另一个别动。

新增测试还钉住一条:`bounding_smaller_side` 与 `enclosing` 在两者都适用处**结果一致**。两个构造器
对同一个问题给不同答案,会比只有一个更糟——调用方按名字顺眼程度挑一个,就会得到不同的边界。

顺带删掉 `harp_worst_triangle_angle`:整轮构建都在报它 never used,指南也没记它为刻意保留。**一个每次
构建都报警告的死函数比死代码更坏**——它训练人忽略警告。

### 11.45 两份 boundary 代码不是遗漏,是两个概念(2026-08-08)

仓库里现在有三处走「边界」的代码,看着像重复:

| 位置 | 它找的是什么 | 用什么判定 |
|---|---|---|
| `earthmesh_refine_redgreen/refine_boundary*`(5 个模块) | 已细化 / 未细化的**分界** | `mrl_new == 4` 对 `mrl_new == 1` |
| `earthmesh_refine_method_c/method_c_perimeter*`(5 个模块) | 一个细分块的**周界** | `nest_wd[iw].is_subdivided()`,还带五边形邻近判断 |
| `earthmesh_boundary` | **地理域边界**:海岸线、盆地轮廓、湖 | 经纬度环 + 角色 + 洞的嵌套 |

前两个是**网格内部的、每一轮重算的**东西:这一遍标记到哪里为止。第三个是**输入数据、跨轮持有**的:
海在哪里。把它们合并会把两个不同的概念揉成一个类型,而那个类型对两边都不诚实。

**真正共享、也确实搬走了的只有一件**:把一堆边装配成有序闭环,以及「每个边界顶点恰好两个邻居否则
环不闭合」这条不变量。它现在是 `earthmesh_boundary::closed_rings`。剩下的「哪些边算边界」是后端自己
的事,`is_subdivided()` 与 `mrl_new == 4` 没有共同的抽象,硬造一个只会得到一个带两套分支的谓词。

依赖 `earthmesh_boundary` 的是 `earthmesh_cli` 与 `earthmesh_refine_harp_dv`。红绿与 Method-C **不
依赖它,这是判断而不是遗漏**——写在这里,免得下一个人看到「两个 crate 各有一份 boundary 代码」就去
合并。

**这个 crate 真正缺的不是合并,是生产者。** 模型目前只从 carve 已经算出的闭合曲线建;**从项目的
海岸线数据 → `SphericalBoundaryModel` 这条路不存在**。也就是说它现在只能*描述* carve 的结果,不能
把外部海岸线读进来*约束*细化——而后者才是「哪一段可分割但不可跨越」这套类型存在的理由。

**还有五种角色没有消费者**:`BoundaryRole` 六种里,生产代码只用 `HardDomain`;`is_impassable()` 与
`permits_edge_flip()` 在 crate 之外零调用。类型完整、没有任何后端按它们行事——和 §11.40 那三个旋钮
是同一形状,只是这次是我自己造的。要么接上,要么在类型上注明它们等着谁。

### 11.46 边界模型有生产者了,角色也有了第二个消费者(2026-08-08)

§11.45 记的最大缺口:模型只能从 carve 已走出的闭合曲线建,**只能描述结果,不能约束细化**。

**`boundary_model_from_regions`** 补上了这一半。运行自己的闭合曲线掩膜(`.nml` / `.nc4` →
`RefinementRegion::Polygon`)就是真实曲线的入口:一圈经纬度点加一个层级。嵌套与定向和 carve 那条
共用同一段逻辑(抽成 `nest_and_orient`),因为**答案的形状与曲线从哪来无关**——环套环就是洞,读出来
的环没有方向。只有角色不同,而两个调用方各自说明了为什么。

**角色是 `RefinementGuide`,不是 `HardDomain`,这个区别是承重的。**

细化掩膜说的是「在这里面细化」。没有任何东西禁止单元跨过它,落在它上面的边被翻掉也不损失什么——
这正是 `permits_edge_flip()` 对 guide 返回 true、对其它五种返回 false 的含义。标成 `HardDomain` 就是
声称「细化不得跨越它自己的细化区域」,那是假的,而且**迟早会有人按它行事**。

于是 §11.45 记的「五种角色零消费者」少了一种:`RefinementGuide` 现在由生产代码产出,
`is_impassable()` 与 `permits_edge_flip()` 第一次有了真实的输入。

**只有多边形产出环**。圆、盒、走廊的边界这个运行从未离散过,替它们凭空造一个环,就是往模型里放一条
背后没有数据支持的曲线,而之后从模型取的每个计数都会算上它。

**仍未做**:HARP-DV 现在仍**拒绝**非圆区域,所以这个模型还没有被任何后端读去服务多边形。生产者已经
在位,下一刀是让 HARP-DV 用 `contains` 服务多边形区域——那才会把「拒绝」变成「服务」。

### 11.47 HARP-DV 服务闭合曲线;角色与重载的逐个核对(2026-08-08)

**一、HARP-DV 从「拒绝非圆区域」变成「服务闭合曲线」。**

`TargetRegion::Polygon` 持一个 `SphericalBoundaryModel` 而不是点列——「这个单元在不在里面」正是模型
的主题,而岛中之湖要答「不在」,判据不该知道什么是湖。保护段的「内部」谓词走同一个模型,于是段列表
和产生它的那条需求**不可能对区域在哪产生分歧**。

NXP 21、一个 110–125°E × 15–30°N 的闭合方形掩膜、层级 2:

| 后端 | 结果 |
|---|---|
| harp_dv | **exit 0,4867 单元** |
| method_c | exit 2:`pass 2 polygon regions require explicit parent-level halo`(它自己的既有限制) |

细化确实落在曲线内:**8.3% 的单元落在占球面 0.35% 的区域里**,约 24 倍富集。

盒与走廊仍然拒绝,理由说清楚了:它们没有被离散过的边界,读不出目标尺度。**一个闭合曲线之所以能服务,
是因为 `SphericalBoundaryModel` 到位了**;这是 §11.46 那个生产者的第一个真实消费者。

每个掩膜各自建一个模型,不合成一个:合成之后「在里面」会对任一掩膜内的单元为真,那是另一个问题。

**二、四种角色为什么仍无消费者——三种没有数据源,第四种粒度对不上。**

| 角色 | 这条管线里有没有对应数据 |
|---|---|
| `MaterialInterface` | 无。carve 出来的海洋网格单元只存在于一侧,那是 `HardDomain` 而不是界面 |
| `EmbeddedFeature` | 无。MERIT 河道进来的是圆和走廊,不是必须出现在网格里的边 |
| `PeriodicSeam` | 无 |
| `OpenBoundary` | **有数据,但粒度不对** |

tri carve 的 `BoundaryOrders` 把边界顶点分成 `obc_order`(开)与 `ibc_order`(闭),那是**逐顶点**的;
而 `BoundaryRole` 挂在**整个环**上。一个环可能一半是海岸线、一半是开边界,按环给一个角色就是撒谎。
**要接上得让角色能落在段上而不是环上**,那是类型的改动,不是接线的改动。记在这里,免得下一个人以为
只差一根线。

**三、14 个 `spawn_nest` 重载逐个核对,一个零生产调用。**

`spawn_nest_cartesian_xy_with_spring_and_max_mrows`:生产 0、内部 0、只有 3 处测试。在用的是它的
deltax 版本,两者的差别是传给 `spawn_nest_internal` 的 `Some((nxp, niter, None))` 与
`Some((nxp, niter, Some(cartesian_dist00)))`——**没有 deltax 的那个用默认间距,而不是 Canonical
`spring_dynamics_nest` 的目标间距**。也就是说,伸手去用它的人会得到一个与任何生产运行都不同的弹簧。

没有删:它是有测试的公共 API,和 §11.44 删掉的那个每次构建都报警告的私有死函数不同。但它和
`spawn_nest_from_target_levels`、`spawn_nest_from_face_masks`(同样生产 0、只有测试)一样,属于
「测试证明它能跑,没有运行需要它」——**测试通过不等于有人在用**。

其余 11 个都有生产调用。

### 11.48 默认后端服务不了另外两个都能服务的形状(2026-08-08)

§11.47 撞见的:同一份闭合曲线掩膜,红绿 4761 单元、HARP-DV 4867 单元,**Method-C exit=2**——
`pass 2 polygon regions require explicit parent-level halo`。而 Method-C 是默认后端。

**根因不在守卫,在读取处。** 圆早就有
`push_method_c_circle_or_corridor_region_with_parent_halos`:`for parent_level in 1..level`,逐层
按 `halo` / `max_transition_row` 放大半径,生成父区域。Method-C 的嵌套要求二层区域坐在一层区域里,
否则它的周界没有过渡的余地——守卫说的正是这件事,而且说得准确。**闭合曲线的读取处压根没有这一步**,
只推一个 `mask.refine_degree` 层的多边形。所以拒绝是对的,缺的是圆有而多边形没有的那一半。

补上之后,同一份掩膜:

| 后端 | 修前 | 修后 | 曲线内单元占比(该区域占球面 0.35%) |
|---|---|---|---|
| method_c | **exit 2** | 4869 | 1.7%(4.9×) |
| red_green | 4761 | 6058 | 6.0%(17×) |
| harp_dv | 4867 | 5187 | 8.3%(24×) |

三个都在曲线内富集。Method-C 最低,因为父层 halo 把细化摊到更大范围——那是它的嵌套语义,不是缺陷。

**放大一个环和放大一个圆是同一个操作,也有同一个限制。** 圆加半径;环把每个顶点从质心向外推。
**凹环上这个偏移会自交**,跨度过大的环质心也不在里面——两种都返回 `None` 而不是emit,因为自交的父
区域会产生 Method-C 走不了的周界,而失败会落在离这里很远的地方。拿不到父区域时不 emit,下游那条
守卫仍会点名说缺什么。

`apply_parent_halos` 这个开关照圆的做法接上:H 场路线一次性定完所有层,要的是原样的曲线。

### 11.49 一个只在测它的那台机器上成立的断言(2026-08-09)

推 alpha3 之后 CI 红了,而本地全绿。挂的是
`earthmesh_refine_harp_dv::cycle::tests::the_degree_budget_saturates`:

```
assertion `left == right` failed
  left: 8      (Linux, CI)
 right: 9      (macOS, 本地)
```

**本地跑的和 CI 跑的是同一条命令**(`make test-fast`),同一个 crate 集合。差别只有平台。

**网格本身没问题**:预算 9 与预算 16 仍然产出**同一张网格**,饱和成立。变的是一个**离散**量——最大
顶点度数——而它是从**连续**几何读出来的。某个谓词上一个末位之差挪动了一次插入,顺带带走一个度数。

**两次修正,第一次也是错的。** 先改成 `worst < budget`(「界没有约束住它」),macOS 上立刻失败:那里
worst **正好等于** 9,而预算抬到 16 依然一样。**界被够到,不等于界是停住它的原因。**

**真正的性质就是那条相等**:两个预算、一张网格 —— 抬高预算什么都不买。它根本不需要任何度数。确切
数字属于指南里的一次测量,不属于代码里的一个常量:

> 从连续几何读出的常量,断言的是**测它的那台机器**。

`worst_seven == 7` 一并去掉——同一类风险,而且同样不承重。§11.17 记的「饱和在 9」现在明确为:
**macOS 9、Linux 8,结论不依赖于哪一个**。

**流程上的教训比这个断言重要。** 我一路只看本地门禁,把 CLAUDE.md 开篇那条「一条命令只覆盖它实际
跑到的那部分」当成了**两个 workspace** 的问题。它同样适用于**平台**:同一条命令,不同的机器,不同的
答案。alpha3 上最后一次绿的 CI 在 08-05,而我在 08-08 推了 137 个提交才发现。**推之前看一眼 CI 的
历史状态,比推之后看便宜得多。**

**更正 §11.49:那不是全部,而且我把病因说反了。**

断言修好之后 CI 还是红的,记为 `cancelled`。那不是取消,是**超时**:17:10:17 → 17:25:32,正好
`timeout-minutes: 15`。GitHub 把超时的 job 记成 cancelled,所以第一眼看不出来。

`fast` job 从 08-05 的 **~2 分钟**变成跑不完。逐 crate 计时:`earthmesh_refine_method_c` 一个就
超过 10 分钟;再逐测试:**`multilevel_failures_are_counted_by_the_gate_that_produced_them` 一个
就超过 4 分钟**,而它前面 46 个测试合计只要几秒。它在 `nxp ∈ {21,40}` × `levels ∈ {2,3,4}` 上跑
随机用例,NXP 40 四层在 debug 下极慢。

**这个文件是 `aa415d8`(把 Method-C 拆成独立 crate 那一刀)新增的,拆分前不存在。** 我当时给了两个
猜测——「本来就慢只是被暴露」或「拆分让它变慢」——**两个都不对**:是我在那一刀里新写了一个重测试,
放进了以「快」为名的门禁,同时把门禁的 crate 列表从 7 个加到 12 个。而 15 分钟的上限是按 7 个
crate、2 分钟的规模定的。**该先查文件来历,而不是先给两个猜测。**

改为 `#[ignore]`,并加进 `make test-slow`——仓库已有这个惯例(`icosahedron_init` 的 NXP64 校验、
CoLM 冒烟测试都在那里)。**只标 ignore 不接进 test-slow 就是「标了没人跑」**,正是本轮反复指出的形状。

| | 修前 | 修后 |
|---|---|---|
| `earthmesh_refine_method_c` | >10 分钟 | **40 秒** |
| `make test-fast` 整体 | 超时(>15 分钟) | **68 秒**,CI 上限 900 秒 |

**两个问题叠在一起,我修了一个就以为修完了。** 8 vs 9 那个断言在 1 分 06 秒就挂了,所以那一次没走
到超时——一个失败**遮住**了另一个,而它们的症状(CI 红)完全一样。

### 11.50 一个装作在检查的门禁(2026-08-09)

外部审查报告说 `make check-architecture` 失败,有三处 wildcard re-export。核实下来**比那更糟**:
本机没装 `rg`,于是

```
@if rg -n '...' rust --glob '*.rs'; then echo '...forbidden'; exit 1; fi
```

三条检查全部 `command not found`,`if` 取假分支,**target 退出 0 —— 报告成功**。它守着
`release-check`。**一个报告成功却什么都没检查的门禁,比没有门禁更坏**:它让人以为查过了。

这和 §11.38–11.41 是同一形状,只是这次长在门禁自己身上——而我这一整轮都在用门禁给自己背书。

改成 `grep -r --include`(POSIX,处处都有),并加 `--exclude-dir=target`:grep 不像 ripgrep 那样
读 `.gitignore`,不排除构建目录的话,六条来自 `target/` 的命中会把真正的三条淹掉。

**打开之后,先前被完全掩盖的违规**:

| 类别 | 数量 |
|---|---|
| wildcard public re-export | 3 |
| source-origin reference 命名 | 15 |

三处 wildcard 都换成具名导出。名字是**把导出收窄到编译不过为止**逐轮问出来的——第一轮只跑
`cargo build` 漏掉了测试用到的两个,`--all-targets` 才补齐。`earthmesh_refine::hfield` 那处更直接:
**没有任何调用方经由它引用**,一个 wildcard 转出整个 crate 的表面,却没有一个消费者。

15 处 reference 命名里,**只有一处是规则真正要禁的意思**(红绿 lib.rs 的「the Fortran reference」),
其余是英文里「参照物 / 基准」的普通用法,包括我这一轮自己写的两处。规则是个粗糙的正则,但把它们
改成 `baseline` / `yardstick` / `原版` 比给正则开洞便宜。

**这一条对本轮其余结论的意义**:我一路用「门禁全绿」给自己背书,而这道门禁从头到尾没在跑。
**「门禁通过」只在门禁真的执行了检查时才是证据。**

### 11.51 外部审查指出的两处,都是我漏的同类第二例(2026-08-09)

**一、全局 finest/coarsest 读整行 `w_to_m`,还丢掉一半单元。**

我修过**区域指标**上一模一样的缺陷:`w_to_m` 每行固定七槽,只有前 `n_w_to_m` 个是角点,其余是
placeholder id 1——它解析得到一个**真实但无关**的点,于是多边形由「一个单元 + 一个陌生人」构成。
**同一个文件里的另一处我没查。**

第二处还多一个毛病:`robust_spherical_area_unit` 返回**有符号**面积,而这里 `steradians <= 0.0`
直接 `continue`——约一半单元 winding 相反,于是**被整批丢弃**,报告的极值只是剩下那一半的极值。
区域指标那处的注释早就写明了这件事并用了 `abs()`。

**二、`maximum_patch_cells` 声明了、校验了、传给了谁都没有。**

`api.rs` 构造 `CycleLimits` 时不带它,事务处也从不检查补丁大小。这和我删掉的 `patch_ring_depth`
是同一类,但这个量定义明确(补丁快照的三角形数),所以**实现而不是删除**:进 `HardGates`(逐事务
门禁所在处),超限返回 `Rejection::PatchTooLarge`,计入 topology 类拒绝。

**我审这个结构体时漏了它,方法上的原因值得记**:我只统计了每个字段的**引用次数**,看到
`maximum_patch_cells` 有 3 次就放过——而那 3 次全是**校验**,没有一次是**消费**。
**「被引用」不等于「被使用」;要数的是消费点。**

**三、修这一条时我当场又犯了一次同样的错。** 在 `api.rs` 里写了

```rust
let mut gates = request.gates;
gates.max_patch_triangles = request.config.maximum_patch_cells;
let outcome = run_cycles(..., request.gates, ...);   // ← 传的还是原来那个
```

赋了值,传的却是旧的——**正是这条修复要治的那个病**。clippy 的 `assigned to, but never used` 抓到了。
写门禁的人自己也会犯门禁要防的错,这就是为什么门禁不能只靠自觉。

### 11.52 外部审查剩下的五条(2026-08-09)

**一、跨反子午线的窗口:一个报错、一个丢一半、一个只是过扫。**

需求窗口是**源索引上的单个矩形**,而 170°E 到 170°W 是被接缝分开的**两段**——没有任何一个
`minlon..maxlon` 能装下它。三处表现不同:

| 形状 | 修前 | 性质 |
|---|---|---|
| Bbox `west > east` | `source_bounds_for_bbox` 拒绝 `east <= west`,运行直接死 | **真错**(项目层明确允许这种窗口) |
| Circle 靠近接缝 | 盒子裁到 180,**另一侧从不扫描**,那边的需求无声消失 | **真错** |
| Close 跨接缝 | 普通 min/max,扫约 358° | **只是过扫,结果正确** |

修法:**跨接缝时退化为整条经度带**。这是超集,不丢东西;哪个源单元真的被需要,后面由
`GridRegion::contains` 逐格判定,而它本来就正确处理接缝。**过扫是安全的,欠扫不是**,而前两处
都在欠扫。Close 那处不动:把跨接缝的曲线和真正全球的曲线分开,要的是包含判定而不是外接范围。

**二、高纬 overlay 把球面压成经纬度平面。**

`complete-mask` 写出处用平面 shoelace 面积在**多个 surface class 之间**比大小选赢家。同一单元内
不同 class 的纬度分布不同,所以畸变**不会抵消**:偏极侧的 class 被高估。复现审查给的数值——单元
80°–89°N,掩膜覆盖其偏极那一半:

```
平面 fraction = 0.500
球面 fraction = 0.296     (sin 差之比)
高估          = 69%
```

两个 class 靠得近时,赢家会翻。改用 `LocalEqualArea`——**同一个 crate 里的 intersections 写出处
早就为同样的理由在用它**。测试同时钉住赤道附近两种读数一致,以免这变成一个到处乱抹的"修正"。

**三、HARP-DV 把三种结局合并成 `AllSatisfied`。**

demand 列表为空就报 `AllSatisfied`,而空可能意味着三件事:每个单元都够小了;还想细化的单元**都
触到了最小宽度**;剩下的需求**数据支撑不了**。只有第一件叫"满足"。

`StopReason::MinimumScaleReached` 早就定义了,**却没有任何赋值点**。现在 `evaluate` 回报一个
tally,三种结局分别对应 `AllSatisfied` / `MinimumScaleReached` / 新增的 `SourceResolutionReached`。

**触底是整单元跳过的**——连 MinAngle 这类质量判据也一起不问。这一点值得报出来而不是抹平。

**而原来有一个测试把错误语义钉死了**:`a_cell_at_the_minimum_width_stops_asking` 断言
`AllSatisfied`。**测试固定的是缺陷而不是行为**。改成断言 `MinimumScaleReached`,并补一个反面
测试:真正达标的网格仍报 `AllSatisfied`——区分三种结局,只在"满足"那种仍说满足时才有意义。

**四、CLI 只在 `unresolved_cells` 非空时才说话。**

于是 `BudgetReached`、`MaximumCyclesReached`、以及只有 `unbalanced_pairs_remaining` 的情形
**完全静默**——而静默读起来就是"这就是你要的网格"。改为按**停止原因**判断,并且只在详细那段不会
触发时打印:两行说同一件事,会训练人两行都不读。

**五、架构文档写着「Two refinement backends」,且把 Method-C 放在 `earthmesh_mesh` 里。**

两句都曾为真:HARP-DV 后来才有,Method-C 是我 `aa415d8` 搬走的。**标题里的数字是那种会在无人
改动周围句子的情况下悄悄过期的事实**,所以改的同时把它为什么过期也写进去了。

### 11.53 复查打回的四条,三条是我上一轮只做了一半(2026-08-09)

**一、`make clippy-full` 是红的,而我一直在读 `make clippy`。**

`clippy` 覆盖无 NetCDF 的那些 crate;`clippy-full` 再加上 `earthmesh_cli`。**这一轮的大半改动都在
`earthmesh_cli` 里**,而我全程报的是窄的那个。这是 CLAUDE.md 开篇那条警告的**第三种形态**:它是
关于两个 workspace 的,也是关于平台的(§11.49),现在还关于**两个名字只差一个词的 make target**。

**二、架构门禁仍分不清「没找到」和「没能查」。**

把 `rg` 换成 `grep` 只解决了当时那个具体原因(rg 没装),**结构原封未动**:
`if grep ...; then fail; fi` 里,工具缺失或出错都是非零,都走假分支,都读成"干净"。

grep 自己是分的:**0 找到、1 没找到、≥2 没能查**。现在显式读退出码,>1 一律判门禁失败,并先
`command -v grep` 确认工具在。两种情形都实测过:注入一个 wildcard 会失败,把 grep 替换成
`exit 2` 的桩也会失败——**不是又一次空转**。

**三、极区面积:`.abs()` 什么也没救到。**

`robust_spherical_area_unit` 对含极点的多边形返回**补面积**。实测三个顶点都在 89°N 的三角形:

```
返回   12.5654 sr   (≈ 4π)
真实    0.00096 sr  (极冠)
        差 13000 倍
```

反向只是变号,`abs()` 取的还是补面积。单元都远小于半球,所以**大于 2π 的那个一定是补**,取
`4π − a`。上一轮我加 `.abs()` 时只想着 winding,没想到还有补面积——而字段注释里早就写着这个风险。

**四、`maximum_patch_cells` 只挡了 insertion,没挡 move。**

`propose_move` 建完快照直接改。而 move 快照的**比 insertion 更大**(扇形的环的再一环,因为一次
翻转会重写整个环),所以漏掉的恰恰是更需要约束的那条。已补,并加测试。顺带把单位写进字段文档:
**名字叫 cells,比较的是 triangles**——名字是公共 API 不动,但两者不一致却不说,比哪一个都糟。

**这一节四条里有三条是"上一轮做了一半"**:窄 target、只换工具不换结构、只想到 winding 没想到补
面积、只改 insertion 没改 move。共同的形状是**修完第一个触发点就当作修完了**,而没有回头问"同一
个原因还有哪些出口"。

**五、停止原因的统计发生在判据之前,于是区域外的细单元也算「触底」。** §11.52 加的 tally 是在
读判据**之前**数的,所以任何小于最小宽度的单元都计入——**包括区域外、根本没被要求做任何事的单元**。
一个空区域判据、没有任何需求的运行,因此报 `MinimumScaleReached`。**这和 tally 本来要消灭的是同一
类错答案**:什么都没被要求的运行是满足,不是受阻。

改为先读判据再判触底:**想要工作却不能得到**才算触底,想要什么都没有的不算。代价是触底单元多走一遍
判据,买到的是「这个单元要不到」和「这个单元不想要」的区分。审查给的探针已固定为测试。

**六、HARP-DV 的结局只到 stderr,`adaptive_run: None`。**

`adaptive_run` 装不下它——那是 Method-C 的逐层圆记录,说不出周期数、拒绝数、停止原因。新增
`HarpDvRunRecord` 走完整条链路:`RefinedGrid` → 运行报告 → 打印。实测:

```
harp_dv_stop_reason=NoAcceptedTransactions
harp_dv_cycles=14
harp_dv_transactions_committed=2719
harp_dv_unresolved_cells=447
harp_dv_unbalanced_pairs=176
```

只对 HARP-DV 打印:另外两个后端没有周期、没有拒绝、没有停止原因,给它们打一行 `None` 是噪声,
而噪声会训练人跳过那一行——包括真正要紧的那次。

中途我又差点重演老毛病:字段加进了 `RefinedGrid` 并被解构,**却没有任何消费者**。`grep -c` 一查
就露了。**加了字段不等于接上了。**

### 11.54 HARP-DV 的 9521 个未满足：度数墙修复后，30° 才是错误默认（2026-08-11）

用户的生产项目（NXP 81、全球海洋、一级 land-cover 自适应）原来结束于：`9521`
个未满足单元、`2212` 个超邻接尺度对，`NoAcceptedTransactions`，度数拒绝 `153516`。
问题不只是报告：旧缓解代码移动的是“提出需求的单元”，而拒绝已经明确给出了真正会超过
gridfile 七邻接上限的顶点。现在按那个实际顶点做事务化移动并重新 Delaunay 合法化；尺度残差
使用受影响边上的局部增量目标，未变化的边不重算，因此局部比较等价于全局目标变化。五边形与
尺度墙也走同一套可回滚移动门禁。

保持旧的 30° 事务硬门实测后，未满足降到 `7470`、超尺度对降到 `680`，但拒绝第一大类变成
`sliver=295446`（超过 `degree=187841`），并跑满 20 轮、耗时约 19 分 35 秒。这个结果把第二个
根因定死：30° 是 §11.34 在 NXP 21 小网格上的经验拐点，不是当前单点 Delaunay 候选阶梯的
终止保证；项目真正的质量门是 25°，默认比交付要求更严格，正在用“拒绝细化”购买用户没有要求
的角度余量。

默认硬门改为与共享质量门一致的 25°。同一生产项目最终实测：

| 指标 | 原始 30° | 修复后 25° |
|---|---:|---:|
| 未满足单元 | 9521 | **1281** |
| 超邻接尺度对 | 2212 | **112** |
| 已提交插点 | 71687 | **82221** |
| r-adaptation 移动 | 9（旧缓解） | **11577** |
| 最小三角角 | 30.001° | **25.002°** |
| 结束 | NoAcceptedTransactions | **NoProductiveAdaptation（15 轮）** |

未满足减少 86.5%，尺度残差减少 94.9%，且最终角度仍过项目的 25° 门。剩余 `1281` 没有伪装
成成功：其中 `100` 个在最后一轮的每一级候选都只撞角度硬门，其余主要撞不可放宽的七邻接
gridfile 上限。代码不静默放宽这两个物理/格式约束，而是新增 `quality_constrained_count` 和
`NoProductiveAdaptation`；连续两轮 r-move 后物理需求数、平衡需求数都不下降就停止，不再把
“移动发生了”误报成“需求有进展”。

性能采样同时发现每轮 `evaluate` 已经构造了全网格 Voronoi 单元，`balance_demands` 又原样构造
一遍；一次 1 秒采样中主线程 459 个样本有 443 个落在第二次 `voronoi_cell`。现在 `evaluate`
把同轮尺度缓存直接交给 balance，删掉重复全扫描。生产全流程（含 2000 次 spring 尝试、海洋
carve 与写文件）为 **539.39 秒**；与修复中间态的约 19 分 35 秒相比缩短约 54%。每轮只打印一
条“插点 / r-move / 未满足 / 角度受限”进展，不再出现逐 raster band 的刷屏。

试过“度数移动后原地重试需求”：未满足只从 1281 变 1259，尺度残差却从 112 变 156，还增加
一条重复拒绝记账路径，已删除。这个 22 个单元的收益不值得更差的平衡和更复杂的状态语义。

### 11.55 HARP-DV 停滞恢复：扩大局部搜索，不放宽七邻接和 25°（2026-08-11）

§11.54 的 `1281` 不是不可满足性证明，只是四级候选阶梯加单点移动已经找不到下降方向。停滞期
现在额外尝试每个 Voronoi 角的两个 off-centre 位置和全部入射边中点；普通周期仍使用短阶梯，
所以额外成本只在整轮没有插点时发生。若单点移动无法降低阻塞顶点的度数，再把它与一个邻点
作为同一事务移动、统一 Delaunay 合法化、过硬门后一起提交；任一检查失败则两个坐标和整片拓扑
一起回滚。移动完成后同轮批量重试需求，而不是等下一轮或只在最后一轮补一次；退出前再统一
重读物理与尺度需求，最终计数不再引用移动前的旧列表。受保护边界仍只走 Ruppert 路径，不进入
这个启发式恢复分支。

同一 NXP 81 生产项目、硬门保持 `degree <= 7` 与 `angle >= 25°` 的实测：

| 指标 | §11.54 | 停滞恢复后 |
|---|---:|---:|
| 未满足单元 | 1281 | **100** |
| 物理 / 平衡需求 | 未区分 | **94 / 16（10 个重叠）** |
| 超邻接尺度对 | 112 | **80** |
| 活跃 HARP 单元 | 147833 | **149723** |
| 最小三角角 | 25.002° | **25.005°** |
| HARP 提交事务 | 82221 | **84111** |
| fallback 插点 / 联合移动 | 0 / 0 | **1311 / 122** |
| 结束 | NoProductiveAdaptation（15 轮） | **NoProductiveAdaptation（59 轮）** |

未满足相对上一版再降 92.2%，相对原始 `9521` 降 98.95%；代价是多 1890 个过渡单元，完整 HARP、
spring、carve 与写文件约 **1149.10 秒**。60 轮上限没有被用作特殊出口：第 59 轮由连续两轮
无物理/平衡需求下降的通用生产性判据停止。最终 100 个中没有“所有候选只撞 25°”的单元；94 个
仍是判据直接要求的物理细化，16 个是尺度平衡（10 个重叠）。运行记录分别输出这两个最终数量、
fallback 插点和联合移动，避免再用一个旧的总数猜原因。默认周期上限由 20 提到 60，但仍由
`NoProductiveAdaptation` 提前结束，不会靠空转把上限耗完。

### 11.56 HARP-DV 角度后处理的上限：最差三角形可修，但现有全局目标拒绝（2026-08-12）

生产项目（NXP 80、一级全球海洋细化、硬门 `degree <= 7`、`angle >= 25°`）的最差原始三角形
位于约 `179°E, 53°N`，三角形角为 **25.0565° / 25.0565° / 129.8875°**，三个顶点度数为
**7 / 7 / 4**。逐候选事务诊断给出：移动两个锐角顶点的 0.5 至 1/64 步候选全部撞 25° 薄片
硬门；移动度数 4 的钝角顶点 0.5 步则完整通过 Delaunay 合法化、度数、拓扑与 25° 硬门，局部
最小角升到 **29.8977°**，最坏 40–90° 偏差从 **39.8875°** 降到 **19.9199°**。

这排除了“局部几何无解”：解存在，现有 angle polish 没保留它，是因为 `AnglePolishScore` 把
物理需求、邻接尺度、饱和顶点、最小角、窗口外数量和平方惩罚做 Pareto 比较；一个显著修好最差
角但在任一轴上微幅退化的移动会变成不可比并被回滚。试过七条小修，均被真实或确定性网格否定：

| 尝试 | 完整耗时 | 最大角 | 结论 |
|---|---:|---:|---|
| 插点阶梯按预测角度重新排序 | 708.37 s | 129.8875° | 更改主细化轨迹，面积比恶化；撤销 |
| angle polish 改成最坏偏差优先 | 711.88 s | 129.8875° | 总惩罚略降、最大角不动；撤销 |
| 仅给 angle polish 增加 0.5 步候选 | 722.79 s | 129.8875° | 最大角和残留不动；撤销 |
| 三项硬约束与软角度目标分层比较 | 712.84 s | 129.8875° | 最大角不动，面积比恶化到 67.08；撤销 |
| 事务前后统一受影响站点评分集合 | 758.51 s | 129.8875° | 放行 6540 次移动，最大角和面积比仍不动；撤销 |
| 把最大角 >90° 直接作为 Delaunay 插点需求 | 小网格第 8 轮即停测 | — | 469→6900 cells 且未收敛；撤销 |
| 钝角顶点与对边端点联合移动 | 730.74 s | 129.8875° | 局部可修，但全局残留 606→618、尺度违例 120→128；整轮回退并撤销 |

七个实验的代码都已清理。保留的唯一加固是：angle polish 结束时除了不得增加待满足站点，还不得
增加全局超邻接尺度对；局部角度改善不能用新增过渡断层购买。

下一步不再调权重。最小可行的算法改动是**有界局部质量事务**：从最差三角形的可移动顶点开始，
在 3–4 环 patch 内联合搜索 vertex relocation 与已有 Delaunay legalization，以硬门（度数、25°、
闭合/拓扑、物理需求不增加、全局尺度违例不增加）过滤，再只按 `(worst angle deviation, total
angle penalty)` 词典序提交。单点无解时才升级到已有的 pair move / cavity 重三角化；仍无解再用
现有 off-centre/Steiner 插入。40–90° 是优化目标，不是当前理论可保证的硬区间；硬保证仍是
25°、七邻接、闭合与尺度连续性。

### 11.57 HARP-DV 尺寸场质量优化：自然长度、最差优先与局部梯度（2026-08-13）

§11.56 后按公开的球面 Delaunay 网格优化路线补齐了三个缺口，但没有引入 JIGSAW 代码、依赖或
新的后端：目标尺寸由 `CellCriterion::target_scale_m_at` 暴露，在 HARP 邻接图上以 `g=0.3`
延拓；自然边长由规则三角格的 `A_voronoi=sqrt(3)/2*l²` 换算；受影响 star 内的三角形按
area-length ratio `eta=4 sqrt(3) A / sum(l²)` 从差到好排序，并按 worst-first 词典序比较。
这与 JIGSAW-GEO 所述、Klingner--Shewchuk 采用的 quality-vector 语义一致：第一个不同的最差
项改善即可接受，并不要求排序后每一项都不退化。每个移动仍只走已有的
`propose_move_cached -> legalize_within -> hard gates -> rollback`，所以 25° 下限、七邻接、
闭合、物理需求和尺度连续性没有被软目标替换。

`check()` 本身仍没有最大角或 well-centered 硬门。`circumcenter_is_local_enough` 只验证同半球、
中心距离和等距残差，并不要求外心落在三角形内部。因此大角必须由后置质量目标显式看见，而不能
从既有硬门推断出来。

自然长度候选改善 eta 尾部，但 NXP41 一层圆代理只把 `eta_min` 从 **0.517303** 提到
**0.537507**，40°–90° 外的角从 **13540** 降到 **12703**。这确认 size field 管线工作，也说明
单一弹簧目标不足。随后对每个最差 star 用两条切向有限差分估计 eta 上升方向，仍通过同一事务
和 Delaunay 合法化提交。16 外轮的结果为:

| 指标 | 优化前 | 16 轮后 |
|---|---:|---:|
| `eta_min` | 0.517303 | **0.818619** |
| 40°–90° 外角数 | 13540 | **4207** |
| 最小角 | 25.679° | **33.902°** |
| 最大角 | 126.406° | **95.948°** |
| 物理 / 平衡残留 | 0 / 0 | **0 / 0** |
| HARP 质量优化耗时（release） | — | **103.7 s** |
| 全代理耗时（release） | — | **129.1 s** |

前两轮先用自然长度稳定尺寸场，后十四轮先尝试最差三角形的 eta 上升方向；同样的候选只调整
顺序，8 轮中间值就比每轮自然长度优先的 0.726458 / 9270 更好，16 轮继续降到上表结果。背景
尺度取自优化开始前输入网格的 cell-scale 中位数并冻结，避免用一个异常大胞控制全局，也避免
目标场追逐优化器自己的输出；整张空间目标场只构造一次，而不是每轮重建 Voronoi。生产
上限采用公开调度同阶的 16 轮，因为用户明确接受额外计算；16 不是理论常数，而是当前代理仍有
显著收益且成本约两分钟的有界停止点。

这次也明确了两种文件不能混做对照：`gridfile/..._01_hex.nc4` 是细化前中间网格，曾给出
`eta_min=0.944207`；真正的 HARP 输出是 `result/gridfile_..._hex.nc4`，其质量与内部 live-mesh
报告一致。上表优化前最大角一度误写成 `156.405°`，实际同一三角网的质量报告是
`126.405924886°`；前值是把相对正三角形理想角 `60°` 的最大偏差 `66.405344617°` 错加到了
`90°`。修正后满足 `max_angle <= 180° - 2*min_angle` 的平面局部必要关系。

公开证据支持「Delaunay 细化后还需要几何/拓扑 hill climbing」而不支持「只调最小角门即可」。
Engwirda 的 JIGSAW-GEO 球面多分辨率例子报告了优化后的 40°–80° 网格；VanderZee 等人的
well-centered 优化也以移动内部顶点改善最大角和最小角。EarthMesh 当前只实现其中最小且许可
安全的共同思想：尺寸感知移动、局部最差优先、Delaunay 恢复和事务回滚；没有复制受限仓库代码，
也没有加入 edge collapse、权重化 power diagram 或新依赖。

**当前保证与目标必须分开写**：硬保证仍是最小角 25°、degree <= 7、闭合和尺度连续；40°–90°
是显著改善但尚未达成的优化目标。16 轮代理还剩 4207 个窗口外角，说明下一阶段若要继续逼近公开
结果，需要在同一 HARP 算子族里加入拓扑调度（局部消除 degree <= 4 障碍、必要时现有
off-centre/Steiner 插点），而不是继续增加外轮或重新启用零自然长度 Laplacian。

### 11.58 HARP-DV 40°–80° 交付窗口：目标场诊断与顺序质量优化（2026-08-13）

40°–80° 是与停止原因正交的交付裁定：报告独立输出 `inside/above`、平方惩罚、不可测数和
`pass|fail|not_evaluated`；任何不可测三角形都不能得到 `pass`。冻结尺寸场反算出的 165978 个目标角
全部位于 **41.8078°–78.9947°**，所以 `g=0.3` 不是残留来源，实际几何/拓扑才是。

优化器复用 HARP-DV 已有的事务移动、Delaunay 合法化、25°/degree≤7/闭合硬门和精确回滚；自然长度
目标固定，不再运行与渐变网格错配的零自然长度 Laplacian。先做 16 轮 eta-first，再做 32 轮
40°–80° margin-first。每轮重算最差候选，1024 只是有日志的批大小；失败点完成一轮有界冷却后重新
进入排序。物理需求和邻接尺度超限数是不可交换的否决位；最坏尺度比例只在仍有超限边时生效。
另用相同 move/legalize 算子修复 degree<5 星形，没有 edge collapse、直接翻边或新依赖。

NXP41 冻结代理的保留结果（同一输入、完整运行）：

| 指标 | §11.57 基线 | 本轮 |
|---|---:|---:|
| 40°–80° 越界角 | 10508 | **2359** |
| 低于 40° / 高于 80° | 3208 / 7300 | **384 / 1975** |
| eta_min / eta_p1 | 0.818619 / 未记录 | **0.822416 / 0.894188** |
| degree<5 顶点 | 未记录 | **25** |
| 物理 / 平衡残留 | 0 / 0 | **0 / 0** |
| 完整代理耗时 | 129.1 s | **759.5 s** |

两类实验已撤销而没有留生产路径：强制全覆盖会恶化最差角；局部外心/质心插点即使改为逐 cavity
质量门控也提交 0 次。继续堆同类候选只增加复杂度。当前结果仍诚实输出 **fail**，尚未实现全角
40°–80°；剩余主要是 d5 的高角和 d7 的低角，需要后续局部拓扑重排，而不是再增加移动轮数。

### 11.59 HARP-DV leaf-site retirement：先证明价值，再开发删除事务（2026-08-14）

自适应点现在记录稳定 `parent_site_id`。只有 parent 已知、无活跃子代且仍 active 的点才称为 leaf；
parent 缺失的历史插点单独报告，不能猜成可删除点。NXP41 同一冻结代理复跑得到：10853 个自适应点中
6324 个是无保护边的 interior leaf；2359 个 40°–80° 越界角中，1437 个坏角直接落在 leaf 顶点上
（60.9%），2183 个违规三角形触及 leaf。leaf 度数为 d4/d5/d6/d7 = 19/1704/3302/1299，且物理与
平衡残留仍为 0/0。这给 reverse insertion 明确的 **Go**，但不等于生产删除已经安全。

最小 clone-only spike 只处理 degree-4 interior leaf：四边形 cavity 只有两条对角线，穷举重三角化，
再用现有 `MeshState::from_parts`、Lawson 合法化、闭合/degree<=7/25°、物理需求和 40°–80° 渐进改善
门筛选。合成球面 fixture 找到一个通过全部门的删除候选，同时原网格逐值不变、V-E+F 守恒。这证明
leaf retirement 不是不可行的特例修补；它确实能解除 split-only 的单向棘轮。

尚未加入生产路径：没有 `swap_remove`，没有复用只能撤销增长的 `MeshPatch`，也没有把 `active=false`
伪装成已删除。下一步是对实际 NXP41 的 19 个 d4 leaf 做同样的只读虚拟审计并报告通过率；只有实网格
样本也通过，才值得设计 stable SiteId 到活动 vertex row 的映射和 shrink-aware rollback。d5–d7 的一般
多边形 cavity 留到 d4 证据之后，避免先写一个没有收益证明的通用删除框架。

### 11.60 HARP-DV NXP41 d4 leaf 只读退役审计（2026-08-14）

用 `EARTHMESH_HARP_D4_RETIREMENT_AUDIT=1` 重跑 §11.59 的同一 NXP41 单圆代理。审计只构造
compact `MeshState` 克隆，不修改交付网格；每个 d4 interior leaf 枚举四边形 cavity 的两条对角线，
并检查闭合、Delaunay、25° 下限、degree≤7、十二个基础五边形、物理需求、尺度平衡以及全局
40°–80° 质量不退化。

19 个候选产生 38 个合法三角化；38/38 通过拓扑/硬门、尺度平衡和角度质量改善，22/38 不恢复任何
物理需求，最终有 **11/19** 个 leaf 至少存在一个同时满足全部门的退役方案。原网格仍为 384 个角低于
40°、1975 个角高于 80°，物理需求与平衡残留均为 0。结论是 **Go，但只够支持先实现严格门控的 d4
leaf 生产事务**；另外 8 个候选会恢复已满足的物理需求，不能删除，也不能据此泛化到 d5–d7 或边界点。

### 11.61 HARP-DV stable leaf retirement：真实事务与保守映射（2026-08-14）

在 §11.60 的只读审计之后，生产实验路径加入了稳定 SiteId tombstone、非压实的活动 vertex/triangle
slot、degree-4 interior leaf 的原位反插入事务，以及基于局地 Lambert 等面积投影的旧 Voronoi cell
到新 cell 的交叠权重。事务只在闭合、局地 Delaunay、25°、degree≤7、物理需求、尺度平衡、40°–80°
质量、eta 和 `degree<5` 数量全部改善或不退化时提交；失败候选保持原状态。映射逐旧 SiteId 归一为 1，
并写入 `result/harp_dv_conservative_remap.csv`，未受影响远场保持隐式恒等映射。

同一 NXP41 冻结代理用 `EARTHMESH_HARP_D4_RETIREMENT=1` 实测：19 个 d4 interior leaf 中提交
**11** 个，与 §11.60 的 11/19 只读预测一致。`degree<5` 顶点由 **25 降到 14**，低于 40° / 高于
80° 的角由 **384 / 1975 降到 379 / 1951**，窗口外总数 **2359 → 2330**；物理需求、尺度平衡和
未解决 cell 仍为 **0 / 0 / 0**。共输出 55 个旧 SiteId 映射行，CSV 含 159 个非零权重，最大行和误差
为 `2.22e-16`。最终最小/最大角仍为 **35.108701° / 95.206565°**，交付裁定继续诚实输出 `fail`。

结论：reverse insertion、稳定身份和守恒交付链已经在真实网格上闭合，但 d4 不是主要残留来源，单靠
这一层不能根治 40°–80°。下一步必须把同一严格事务推广到触及违规角的 d5–d7 interior leaf 多边形
cavity，并在每次退役后运行局部 relocation/legalization；不能再靠增加全局移动轮数，也不能把 d4
结果外推成已完成。

### 11.62 HARP-DV 通用 leaf 退役与局部质量比较器实测（2026-08-14）

把相同退役事务推广到触及坏角的 d4–d7 interior leaf 后，对最差 64 个候选做有界实测，仅提交
**3** 个，且全部仍是 d4；d5–d7 删除都会恢复物理需求，因此没有用“角度变好”换取需求缺失。最终
低于 40° / 高于 80° 为 **383 / 1968**（合计 2351），`degree<5` 为 **22**，物理、尺度平衡和未解决
需求仍为 **0 / 0 / 0**。结论：通用 reverse insertion 是必要的回退算子，但不是当前残留的主解；
继续扩大删除预算只会重复扫描被物理门拒绝的候选。

质量移动的真实缺陷在局部接受比较：旧的 leximin 允许“最差一项略好、其余窗口外角变多或平方惩罚
变坏”的移动，随后全局守卫再把整轮回滚。局部比较现同时要求窗口外计数和平方惩罚均不退化。NXP6
代理由 **282 → 72** 个 40°–80° 越界角，1727 次移动，且窗口阶段不再发生整轮回滚；此前为
**282 → 73**。该改动保留。

三条替代候选均经同一代理否定并已清理：简化的密度加权 Voronoi/Lloyd 候选把残留恶化到 144 或
175；最差三角形的 pair move 与 d5/d7 度数转移均提交 0 次；三角形三个角点的原子联合 eta 上升也
提交 0 次，结果仍为 **72**。因此不保留三点事务或多点抽象。当前仍未根治；下一步需要能改变
d5/d7 扇区拓扑、同时由局部窗口残差和物理门验收的受限 cavity 重三角化，而不是继续增加移动候选。

### 11.63 HARP-DV 入射 star 窗口梯度与残留归因（2026-08-14）

旧的几何方向只优化单个最差三角形，无法表达 d5 顶点五个扇区的角度分配。窗口阶段现增加整个入射
star 的 40°–80° 平方残差有限差分方向，仍由原有单点事务、Delaunay 合法化、25°、degree≤7、物理
需求和尺度门验收。NXP6 代理的越界角由局部比较器修复后的 **72** 进一步降到 **65**，总移动
1727→1736，耗时约 113→117 秒；测试以 `<=70` 锁住该增益。

65 个残留按角点拆分：d4 为 4 个高角，d5 为 39 个高角，d6 为 17 个高角和 1 个低角，d7 为 4 个
低角；全部可移动。说明主残留已经是 d5 扇区分配，不是 d7 leaf 删除。放宽局部 eta 门会让首轮提交
78 次移动却把 `eta_min` 由 0.861 降到 0.851，随后被全局守卫正确回滚，最终退化到 73；该实验已撤销。
退役后以原位、star-ascent 或自然长度重插，在最差 64 个 d5–d7 leaf 上仍为 0 个可接受候选，也已撤销。

同一 star 内允许多个硬门安全小步、最后统一验收的联合移动也只提交 6 步，窗口外角仍为 **65**；
它只降低平方惩罚而没有减少违规数，生产代码已删除。固定拓扑上的更多联合移动因此不再作为主线。
对残留可动点再做 16 个切向方向、5 档步长的只读扫描，虽找到 32 个可降低惩罚的合法单点移动，
连续四轮仍未减少违规数；该昂贵扫描也未进入生产路径。

另修复窗口调度早停：当冷却集让本轮只剩少量未试点且没有提交时，先清空冷却集完成真正的全量无收益
轮，而不是误把“这一小撮失败”当成全局停滞。实测下一全量轮仍提交 0，但停止理由现在与行为一致。
已在 NXP41 严格事务实测 11/19 成功、且数学上必然制造 ≥90° 角的 d4 leaf retirement 改为默认；
d5–d7 通用 retirement 仍只在 `EARTHMESH_HARP_LEAF_RETIREMENT` 实验开关下运行。

### 11.64 HARP-DV 残留拓扑诊断：关闭直接翻边和小步 relocation（2026-08-14）

正式 NXP6 代理继续以 refinement 前冻结的背景尺度为准，质量优化把 40°–80° 越界角由 **282 降到
65**。细化后重新取背景尺度会得到 57，但那会改变目标场定义，不能混作生产结果。

对正式终态做三项 clone-only 诊断：16 个切向方向×5 档步长能继续降低平方惩罚，但连续四轮不减少
违规数；195 条触及违规三角形的现有边中，34 条可翻且通过闭合、degree≤7 和 25° 门，**没有一条**
减少局部或全局违规数；围绕最坏三角形的 1/2-ring cavity 做 105 次硬门安全、Delaunay 合法化的小步
联合 relocation，同样没有一个正例。临时代码均已删除。

另对前 64 个触及残留的 d5–d7 interior leaf 做完整复合事务审计：退役中间态只检查拓扑，随后立即
在原位、cavity 球面质心或最长边中点重插一点，并允许 ring 做现有窗口小步移动；物理、尺度、eta 和
窗口门只在最终态检查。6 个候选完成退役和重插，3 个最终全门安全，但 **0 个**把 65 个违规角继续
降低。这排除了“中间态门太严”的假阴性，也说明单点退役–重插不是当前平台的突破口。

因此直接翻边、更多单点方向和最坏三角形周围的小步联合移动都不是剩余 65 个角的根治路径。继续时
必须先用 clone 证明真正的 cavity 重建/退役后重插能够减少违规数，再接生产事务；否则维持诚实
`fail`，不能用 degree 势函数或更宽的门伪装达标。

### 11.65 HARP-DV 插点路线：当前局部算子族的平台（2026-08-14）

§11.62–11.64 的有界位置算子族到达平台之后，本节审计增点方向。删除侧已由 §11.59–11.62 探过（前 64
个 d5–d7 候选只提交 3 个）；这一节做增点侧，用 clone-only 定向审计，生产路径不动。

审计对象是 NXP6 正式代理优化后的 **65** 个越界角（5 个 <40°、60 个 >80°）中，承载高角的 **d4 与
d5** 站点，共 29 个（d4 1 个、d5 28 个）。刻意不含 d7：d7 均值 51.43°、离 40° 只有 11.43° 余量，
插点抬到 d8 后均值 45°、余量剩 5°，方向是错的。

三个候选族，406 个候选：现有阶梯（`candidates_for_site`，witness 传 `None`，因此实际只有
farthest / off-centre / 最长边中点**三级**）、越界扇区的球面角平分线（径向 0.4/0.6/0.8/1.0/1.3 ×
平均入射边长）、以及在扇区两条边上抬起的近等边顶点（高度 0.55/0.866/1.15 × 边长）。第三族是
**shape-oriented 代理**，不是 Engwirda 的 shape-optimal point——后者落在 Voronoi 段上、界于自身
外心与前沿边直径球之间，本节没有实现它。

**原子插点：0/406，两种判据下都是 0。** 换成 JIGSAW-GEO 的 `SplitEdge` 判据（只要求局部 cavity
的排序 area-length 向量不退化，不要求任何全局计数下降）仍是 0/406。所以否决不是「我们自己的全局
单调测度过严」造成的。

逐条拒绝（`RetirementPostcondition` 的九条，硬门短路）：

| 拒绝原因 | 计数 |
|---|---:|
| 硬门（拓扑 / degree≤7 / 25° / 五边形） | 274 |
| **窗口违规数未严格下降** | **132（= 全部幸存者）** |
| 平方惩罚未严格下降 | 132 |
| **<40° 计数增加** | **131** |
| eta_min 下降 | 130 |
| >80° 计数增加 | 123 |
| degree<5 集合增大 | 82 |

真正的通杀项是**新增 <40° 的角**：132 个通过硬门的候选里 131 个如此，而且与新点度数无关——其中
52 个新点度数 ≥5（48 个 d5、4 个 d6）也全部如此。**「新点落在 d4」只解释了一半。**

径向阶梯给出「距离 → 新点预测度数」：

| 距离 | 预测 deg(p) 分布 |
|---|---|
| 0.4L | 3:1, 4:16, 5:8, 6:4 |
| 0.6L | 4:23, 5:6 |
| 0.8L | 4:23, 5:6 |
| 1.0L | 4:12, 5:17 |
| 1.3L | 4:8, 5:20, 6:1 |

**非单调**（0.4L 有四个 6，0.6/0.8L 一个都没有）。度数对距离分段常数，不能二分搜索。原因是恒等式
`deg(p) = t + 2`（t 为 cavity 三角形数）：由 `Σ(6−d) = 6V − 2E` 不变、`ΔV=+1`、`ΔE = k − L` 得
`k − L = 3`，而盘状 cavity 的 `L = t − 1`。放在扇区内部的点通常只吃掉那两个扇区三角形，于是
`deg(p) = 4`，算术上必带一个 ≥90° 角。放远才吃得多，但那时已经不在扇区里。

构造本身是有效的：406 个全部插入成功，358 个与目标顶点相邻，313 个真的删掉了越界三角形，**272 个
真的把那个高角劈小了**。几何对，账不平。

**复合插点+局部修复：1/96。** 允许插点这一步变差（它就是单点移动跨不过去的鞍点），随后在新点及其
一环上做最多 6 轮、每轮两个方向 × 五档步长的普通爬山重定位，最后整体一次验收。96 个结构安全种子
（无硬门 / 物理 / 尺度拒绝）中：

```
ACCEPT site 429 SectorBisector(1.0): 31 次重定位，局部 gap 0.1804 -> 0.0000，违规 65 -> 62，零拒绝
最接近的拒绝 site 426 LongestEdgeMidpoint: 30 次重定位，gap 0.2040 -> 0.0012，违规 65 -> 60
```

这是整条线上第一个正例，且不是伪改善——全局物理、尺度、角度九条和硬门全过，局部公共前缀向量未
退化，产物过闭合、度数与 25°。`site 426` 减了 5 个违规却差 0.0012 被拒；那 0.0012 不是浮点噪声，
是真实质量亏损，不应加 epsilon 放行。

**但它不成序列。** clone-only 贪心（每步重新审计全部候选，取第一个被接受者）：

```
trajectory [65, 62]      vertices [469, 470]      一步后停止
```

第二轮重新审计当前全部候选，无一被接受（第二轮的候选数未记录）。62 又是一个局部最优。

**结论按可证伪的粒度写**：当前候选族（3 族 406 个采样点）加当前局部重定位算子（2 个方向、5 档
步长、6 轮一环）在 NXP6 上到达 **62 个残留的平台**，不值得直接生产化。这**不是**「[40°,80°] 不可
达」的证明，也不是所有局部方法的数学穷尽——审计只覆盖 d4/d5 的最坏扇区、修复站点集固定为目标点
及其初始邻居、没有连续位置优化、没有更大 cavity、没有批量插点、没有生成–优化交替。同理
§11.59–11.62 的「删除」是前 64 个候选，§11.62–11.64 的「位置」是大量负实验而非连续可行域证明。
统一表述应为「当前有界算子族到达平台」。

两个换算上的更正，避免以后被引用错：`eta ≥ 0.890317`（最坏形状 40/60/80）是三角三角全部落在
[40°,80°] 的**必要不充分**条件——45/54/81 的 eta ≈ 0.9189 仍越界；且 eta 是三个角的函数，不能用
单一灵敏度把 eta 差折算成「差几度」。本仓库的 `triangle_eta` 还是球面弧长上的平面 Heron 指标，与
窗口用的球面切向角不严格等价。

下一项独立研究是**生成阶段的放点策略**，不是继续堆局部补丁。注意 JIGSAW-GEO 论文把它的空间填充
细化轨迹称为「贪心优先调度、前沿过滤与 off-centre 放点三者交互的涌现属性」，而 HARP-DV 的细化必须
以物理需求为第一优先级，因此只能移植放点、移植不了调度。那个混合体两篇论文都没描述过，若实测
No-Go，只能得出「**需求驱动调度下的** frontal 放点无效」，不能得出 frontal 放点无效。生成阶段的
Go/No-Go 必须同时约束顶点增长，否则「用更多单元换更少违规」可以无意义地满足任何残留门槛。

审计代码是一次性的，已随本节记录删除；完整逐候选输出见
`EarthMesh_NXP6_insertion_audit_2026-08-14.log`。

**残留归因（同一代理，只读诊断）**：优化器不插点，站点身份跨它不变，所以「越界角的所有者」两侧
可直接比较。所有者定义为**越界角实际所在的那个角点**，并与它落在窗口哪一侧配对——只要三角形有
一个坏角就把三个角点全记上，量到的是「残留是否留在同一邻域」，而且会报出比越界角还多的所有者
（第一版即如此：65 个角报出 100 个站点）；与侧别配对则是因为一个站点从低角问题变成高角问题不算
继承。

| | 细化后 | 优化后 |
|---|---:|---:|
| 越界角 | 282 | 65 |
| 越界角所有者 `(site, is_high)` | 184 | **47** |
| 其中继承自细化 | — | **42（89.4%）** |
| 其中由优化器新造 | — | **5（10.6%）** |

**残留以继承为主**，这是生成阶段放点原型值得写的前提；但 10.6% 由优化器自己产生，所以生成阶段
改动的收益上界不是 100%。诊断连同两条前置断言（跨优化器顶点数与活跃站点数不变；所有者数不得超过
越界角数）保留在 `cycle/tests.rs`，并断言 `inherited > manufactured`——否则前提不成立时必须失败，
而不是被读过去。

### 11.66 前沿放点与接受规则的交互：完整 2×2（2026-08-14）

§11.65 把局部修复探到平台之后，下一个可动的只有**生成阶段的放点**。本节记录 NXP6 上的
四格对照，以及一次被自己的实验设计推翻的中间结论。

移植的是 Engwirda 2017（GMD 10:2117–2140，CC-BY）Algorithm 1 与 §3.6 的 off-centre 级联：
前沿边取三角形的**最短**边；两个候选都落在该边的 Voronoi 弧上；shape-optimal 高度
`a_θ = |e₀| / (2 tan(θ̃/2))`、`θ̃ = arcsin(1/(2ρ̄))`，使新顶角**恰好**等于 θ̃；size-optimal 高度
`a_h = min( sqrt(ĥ² − |e₀|²/4), (√3/2)ĥ )`，`ĥ` 取**两条新边中点**的目标值平均，按论文的
predictor-corrector 迭代；取离中点更近且 `≥ |e₀|/2` 的那个，否则回退外心。
`ρ̄ = 1/(2 sin θ̄)`，所以"要 40° 下限"和"给放点 θ̄ = 40°"是同一句话。

**只应称 Eq. 6–7-corrected fan-wide frontal candidate，不是完整的 Frontal-Delaunay。**
五处偏离都由 HARP 的契约强制：为需求站点的整个 fan 生成候选（论文refine全局最差三角形）；
frontal 固定排在出厂阶梯之前；尺寸场是**未经梯度限制**的判据值（受限场在细化结束后才构造）；
长度用球面弧长；只有一个新边中点有目标值时由它单独定尺度。论文的质量是"贪心前沿调度 + 前沿
过滤 + off-centre 放点"三者交互的涌现属性，而 HARP 必须需求优先，只能移植第三样。

**完整 2×2，两个 frontal 格用同一个候选公式：**

```
production reference: sites 469, refined 282, final 65 (below 5 above 60)

cell                          sites  refined  final  below  above  owners  eta_min   maxdeg  deg<5  pending
first-survivor, shipped only    469      282     65      5     60      47  0.861076       7      1        0
first-survivor, + frontal       477      280     75     14     61      48  0.839396       7      2        1
Better+leximin, shipped only    456      256     68     10     58      47  0.859977       7      0        0
Better+leximin, + frontal       463      246     50      2     48      34  0.862923       7      1        0
```

控制格（first-survivor + 出厂阶梯）走的就是 `refine_cell` / `refine_cell_fallback`，测试
**断言它与生产网格逐位相同**（`state() == state()`），不是打印出来看着像。生产 ladder 以后
一改，这个测试就红，不会让控制格无声漂移。

**结论是交互，不是主效应。** 同一候选公式下：first-survivor 时 frontal 让 65 → **75**（有害，
且 `pending 0 → 1`，丢了一个物理需求）；Better+leximin 时 68 → **50**（有益）。而
Better+leximin 单独也略差于生产（68 > 65）。**两个主效应都是负的，只有组合为正。**
准确表述：*前沿候选与 Better+leximin 接受/排序策略之间存在交互*，不是"前沿放点有效"。

最好那格相对生产：越界角 65 → **50**（−23%），below-40 5 → **2**，above-80 60 → **48**，
owners 47 → **34**（−28%），站点 469 → **463**（**−1.28%**，更少而非更多），
物理/尺度/度数门全部保持。

**仍是 No-Go**：预注册门是 ≤ 32，50 高于它。

两处过程记录，因为它们各自推翻过一个结论：

- 第一版驱动同时改了三样（候选集、`Better` 局部过滤、leximin 排序），报出"frontal 每项都赢
  （61）"。做真单变量后，赢的是**选择规则**不是候选。**旧的 61 是混杂实验，已被完整矩阵取代，
  不得再作为"当前最好方案"引用。**
- 翻转时同步改了 size-optimal，又是一次归因风险。单独关掉 `(√3/2)ĥ` 上限对照：同一驱动下
  97（无上限）对 75（有上限）——**论文的上限是对的且帮了 22 个**，翻转来自驱动。修正在两个
  方向都改善：first-survivor 格 97 → 75，Better+leximin 格 61 → 50。

最佳格仍有 **1 个 `degree < 5` 顶点**。四个角和为 360° 意味着它**必然**带一个 ≥ 90° 的角，
所以它是最终达到 [40°,80°] 的明确结构障碍；但它不阻断"这条路是否值得继续研究"的代理判定。

**适用范围**：NXP6、这个驱动、这个尺寸场。不代表完整 Frontal-Delaunay，也不代表 NXP41。
度量口径见 `docs/angle_window_40_80_experiment_spec.md`。

**θ̄ 扫描：惰性，不是杠杆（预锁判读）。** 在唯一有效的那格（Better+leximin + frontal）扫
θ̄ ∈ {40°, 45°, 50°}，三档输出**逐位相同**：

```
theta_bar  sites  refined  final  below  above  owners  eta_min   deg<5  pending  growth
       40    463      246     50      2     48      34  0.862923      1        0  -1.28%
       45    463      246     50      2     48      34  0.862923      1        0  -1.28%
       50    463      246     50      2     48      34  0.862923      1        0  -1.28%
```

原因是测出来的，不是推出来的——**这里推过两次，两次都错**。第一次推"size-optimal 恒小于
shape-optimal，所以 shape 臂从不被选"；加计数后 shape 臂在 θ̄=40 被选了 **234** 次。第二次改推
"被夹到外心"，这次先加计数再看：

```
theta_bar   size_chosen   shape_chosen   其中被夹到外心   fallback
       40           229            234              234        286
       45           181            282              282        286
       50           157            306              306        286
```

**shape 臂确实会赢（而且 θ̄ 越大赢得越多），但它赢下来的每一次高度都超出 Voronoi 段、被
`min(ceiling)` 夹到外心** —— `shape_chosen == shape_clamped`，三档皆 100%。夹住之后落点与 θ̄
无关，所以最终结果逐位相同。测试断言的正是 `shape_chosen == shape_clamped`，不是"shape 从不被
选"。

按预锁判读（`≥ 50 → θ̄ 不是有效杠杆，停止调参`）：**停止参数调整。**

顺带一处诊断，措辞要准确：`target_angle_window_survey` 报出 `above_80 = 1`（超出 0.030°）。
尺寸场本身没有三角形也没有角度，不能称为"不可行"；准确说法是——**在当前拓扑上，由冻结顶点尺度
推导出的目标边长三元组中，有一个隐含角超过 80° 约 0.030°**。量级可忽略，记此备查。

### 11.67 离线 feasibility oracle：结构条件全过，两条角度门都不过（2026-08-14）

§11.66 之后，唯一还没试过的是**完全自由的点集**——不受 HARP 的需求调度、事务门和既有几何约束，
只按同一个冻结尺寸场布点。这一节记录该实验。它在 Rust 之外（`scripts/angle_window_oracle.py`），
因为仓库里没有任意点集的球面 Delaunay（`MeshState::from_parts` 要调用者自带拓扑），而球面上单位
向量的**三维凸包就是**它们的 Delaunay，离线用 `scipy.spatial.ConvexHull` 即可，不必为一次性实验
在 Rust 里新写一个三角化器。

**阶段一：先在已知答案上验工具。** 用冻结场导出的 469 个点重建，与 Rust 侧逐项对照，11 项断言
全过：`V/F/E = 469/934/1401`（即 `2V−4`、`3V−6`）、`below40/above80 = 146/136`、`owners = 184`、
`d4/d5/d6/d7 = 5/45/376/43`、`Σ(6−d) = 12`。七项拓扑校验全过（含 `len(hull.vertices) == V`——
那条最容易被计数自洽掩盖）。工具可信之后才用它判别的点集。

阶段一顺带抓到两件事：

- **顶点半径漂移最大 0.945 m**（相对 1.48e−7）。来源是插点候选按站点自身当前半径归一化
  （`projected_step` 用 `magnitude(here)` 而非名义球半径），所以一旦漂了不会自我纠正。
  0.945 m 是最近点距的 7.5e−6，对凸包判定无害；但检查的阈值必须绑住失效模式（"大到 qhull 会把点
  判为内点"），不能拍一个 epsilon——第一版拍 `1e-9` 直接误报。
- **NXP6 上平面比较角的误差是 max 0.379°**，比 NXP81 的 4.32e−3° 大两个量级（边长约 30 倍，球面
  盈余按平方走）。规格 §2.1 原来只写了 NXP81 的数字，等于把实际在用的 fixture 上的代理误差低估
  了 90 倍。已改为按 fixture 分列，并写明 **NXP6 上角度结论必须用球面角**。

**阶段二：变半径 Poisson-disk 播种 + 密度加权 Lloyd 松弛。** 如实命名——**这不是
Fornberg-Flyer**，那篇的推进前沿规则没有被复现，照记忆写会是这条线上第三次"论文式"误称。
它的意义在于 §11.62 记录的"简化密度加权 Lloyd 恶化到 144/175"是**在 HARP 贪心事务里逐站点**做
的（每步必须改善否则回滚），而这里是**自由点集上的整扫描**，是那条负结果没有覆盖的形态。
密度取 `ρ = ℓ*^{-4}`（二维 CVT 的 `h ∝ ρ^{-1/4}`）。点数按规格 §5 标定：缩放因子 0.82812 →
479 点，相对 469 为 +2.13%，落在 ±10% 内。

**收敛必须先测。** 第一次跑 60 轮得 83，若照此报数就是又一次"轮次用完当成平台"：

```
sweeps    outside   below40  above80    min      max     deg<5
    60         83        16       67   28.60   120.64       0
   150         60        13       47   31.50   110.07       0
   300         52        14       38   31.66   109.49       0
   600         48        13       35   31.93   100.07       0
  1200         43        14       29   31.41    98.01       0
  2400         41        13       28   31.74   102.88       0
  4800         41        13       28   31.57   102.87       0   <- 平台
```

**判定（规格 §6.1）：No-Go。**

| 条件 | 要求 | 实测 | |
|---|---|---:|:--:|
| 球面 min_angle | ≥ 40° | **31.74°** | ✗ |
| 球面 max_angle | ≤ 80° | **102.88°** | ✗ |
| max_degree | ≤ 7 | 7 | ✓ |
| degree < 5 | 0 | **0** | ✓ |
| 尺度违例数 | 0 | **0**（播种时 4，Lloyd 后归零） | ✓ |
| 顶点数 | 基线 ±10% | 479（+2.13%） | ✓ |
| 物理需求 | — | 未评估（oracle 复现不了判据语义） | — |

**六项可评估条件里五项通过，只有两条角度门不过——而且两条都不是差一点**：最小角差 8.3°，
最大角超 22.9°。

三个结构性发现：

**1. 两条路线在相反的一侧失败。**

| | outside | below40 | above80 | deg<5 |
|---|---:|---:|---:|---:|
| HARP 生产 | 65 | **5** | 60 | 1 |
| HARP 最佳 frontal 格（§11.66） | 50 | 2 | 48 | 1 |
| **oracle（收敛）** | **41** | **13** | **28** | **0** |

oracle 总数最少，但**高角侧好一倍以上（28 对 60）、低角侧差一倍以上（13 对 5）**。机制上一致：
CVT 驱动向六边形格，专治大角；而变密度场的过渡带必然留下拉长三角形，那是小角的来源，CVT 的目标
函数不惩罚它。这与 §11.62–11.64 记录的"HARP 侧主残留是高角"正好互补。

**2. `deg<5` 全程为 0。** 自由点集从播种到收敛没产生过一个 4 度顶点，而 HARP 各臂都有 1–5 个，
§11.66 记过那是"必然带 ≥90° 角"的结构障碍。这是点集路线的真实结构优势。

**3. 最大角非单调**：1200 轮 98.01°，2400 轮反弹到 102.88° 并稳住。**多跑 Lloyd 不是一致更好**
——它收敛到一个不动点，而那个不动点不是最大角的极小点。

**解读限制（规格 §7）**：本节只否定**这一个生成器与这组参数，在这个冻结场上**。不否定
meshfree / Riesz 谱系，也不证明该场不存在可行点集。而该场的梯度限制本身绑定在一张特定网格图上，
所以结论也不上升到产生它的那组判据。

### 11.68 循环兜底上界扫全网格：CI 停摆两周的单行病根（2026-08-17）

`12078c4` 之后 CI 一次都没绿过。三个作业里 `fast` 和 `heavy` 每次都被超时
杀掉,于是**排在超时之后的失败从未被任何人看见**——包括一个真实的断言失败。
把作业跑完之后才发现,病根是一行:

```rust
// mesh_voronoi/mod.rs:153
let limit = self.triangle_count() + 1;
```

`triangle_count()` 是 `active_triangle_slots().count()`,一次 O(F) 全槽位扫描。
而这个 `limit` 的唯一用途是循环跑飞时的兜底。**一个约六个三角形的扇形,为了
决定它最多能走几步,先把整张网格数一遍**;质量优化器每做一次目标函数求值就
读一次扇形。NXP=21 的 CLI 算例约 15,000 个槽位,即约 2,500 倍的纯开销。

采样(§11.3 的方法)把 3,637 个样本里的 **3,305 个,90.9%** 钉在
`triangle_count` 里面。

同样的写法还有两处:`mesh_insertion` 的定位走步、`mesh_flip` 的 Lawson 上界。
后者的注释自己写着 "Generous, and a bound rather than a guess" —— 作者本就
只要一个上界。三处都换成 `triangles().len()`:O(1),恒不小于活跃数,所以原先
能走完的循环仍然走得完,跑飞仍然被截住,只是截在更大的步数上。

| 算例 | 前 | 后 |
|---|---|---|
| CLI `harp_dv_output_passes_the_mesh_quality_gate`(NXP=21,debug) | CI 2 小时被杀 | **8m02s**,48 遍全跑完 |
| harp_dv 单元套件 | CI 上 500.06s | **23.56s** |
| NXP80 生产全路径(release) | 1935.2s | **207.1s** |
| CI `fast` / `heavy` | 均超时被杀 | 4m56s / 25m31s |

输出逐字节不变:NXP80 的 203 行日志全部相同,站点数、周期数、停止原因一致。

**没有动**的一处:`mesh_insertion` 里 `cavity.len() >= self.triangle_count()`。
那是对网格的断言("空腔吞掉了整张网格"),不是循环上界;放宽阈值是削弱安全网,
不是优化。

三条可迁移的教训:

- **兜底上界不需要精确值,只需要上界。** 一个"防跑飞"的计数若要扫全结构才能
  得出,它本身就是那个跑飞。这是继 §11.3 的邻域循环之后,同一缺陷类别的第二例
  ——两次都是"每元素一次全局扫描"藏在看似无害的辅助调用里。
- **超时会把它后面的一切藏起来。** `fast` 里有一个真实失败
  (`protected_segments_make_a_quality_target_terminate` 在 Linux 撞周期顶),
  它在超时背后待了两周。作业跑不完时,"没有报告失败"不等于"没有失败"。
- **先归因再动手。** 本例最初被归因为"优化器跑满 48 遍",据此写的改动会改变
  交付网格,而真实主因与遍数无关、量级大三个数量级。一次五秒的采样把归因
  纠正了过来。

### 11.69 真实数据 NXP80 全路径的阶段分摊（2026-08-17 实测）

§11.68 的加速是在 `the_full_production_path_on_the_nxp_proxy` 上量的,而那是
`sphere(80)` 加合成判据。代理不覆盖 NetCDF 读入、掩膜链、gridinit 和写出,
所以"内核快 9.3 倍"能兑现多少到端到端,需要真实数据才能回答。

配置取自 `examples/default/land_hex_global.nml`,只改四处:NXP 64→80、
`NL%refine_backend = 'harp_dv'`、landtype 换成 IGBP(125 MB 真实栅格)、
输出目录。其余保持生产原样。**NXP=80 当时需要 `--max-tris`**(已修,见本节末):默认上限 100,000,
而 NXP80 的 gridinit 要 128,000 个三角形,否则第 1 秒即报错退出。

479 秒跑完(exit=0),阶段分摊:

| 阶段 | 累计 | 占比 |
|---|---|---|
| gridinit + 125MB landtype 读入 + 掩膜链 | 0→1s | **0.2%** |
| HARP-DV 细化 12 个周期(至 101,622 单元) | 1→79s | 16% |
| 冻结目标场角度诊断 | 79→105s | 5% |
| 低度数修复(第一轮) | 105→241s | 28% |
| 质量优化 + 第二轮低度数修复 | 241→451s | 44% |
| leaf 退役 + 写出 | 451→479s | 6% |

**结论与预期相反**:I/O 和 gridinit 合计只占 0.2%,**约 99% 的墙钟在 HARP-DV
里面**。所以 §11.68 的加速基本全额兑现到端到端,不存在被 I/O 稀释的问题。
125 MB 栅格读入之所以可以忽略,是因为它只驱动陆海掩膜,不参与逐周期的循环。

收敛类指标干净:`balance_demands_remaining=0`、`quality_constrained_cells=0`、
`unbalanced_pairs=0`、`unresolved_cells=0`、`landtype_masked_cells=45079`。

#### 别把目标场的角度当成交付网格的角度

`harp_dv_target_triangle_angles_below_40_deg=1` / `above_80=6` / `count=610548`
读起来像"61 万个角里只有 7 个出窗",实际不是。字段注释写的是
**"measured from the frozen desired edge lengths rather than the realised
mesh"** —— 它量的是**冻结目标场自身的自洽性**:如果网格完美实现了目标尺寸场,
三角形会长成什么角度。目标场平滑,所以它几乎必然接近理想,7/610548 说明的是
目标场没有内在矛盾,**不是交付网格的质量**。

同一份日志里量交付网格的是另一组:`angles_below_40_at_leaf_vertices=5292`、
`angles_above_80_at_leaf_vertices=9613`、`violating_triangles_touching_leaf=21916`。

从 `tmpfile/gridfile_NXP0080_05_refine_raw_hex.nc4` 直接重算全部 203,500 个
三角形(球面面积合计 12.566371 = 4π,Euler V=101752 相符,可证连通性读对了):

| | 值 |
|---|---|
| 角度总数 | 610,500 |
| min / max | 28.22° / 108.11° |
| 中位数 / 均值 | 60.12° / 60.00° |
| p1 / p99 | 37.72° / 85.04° |
| < 40° | 11,022(1.81%) |
| > 80° | 17,297(2.83%) |
| 窗外合计 | 28,319(**4.64%**) |
| < 30° | 18 |
| > 90° | 638 |

即**交付网格是 4.64% 出窗,不是 0.001%**。分布单峰、以 60° 为中心、
79.8% 落在 [50,70)、88.2% 落在 [45,75),尾巴很短(最差 28.2°/108.1°,无退化三角形),
这是一个健康的结果——但和目标场那 7 个差三个数量级,两者**不可互相替代引用**。
`triangle_eta_min=0.702859`、`eta_p1=0.831945`、`triangles_below_eta_0_89=13723`
(占 6.7%)与之一致。

这是 §11.1 沉默失败的又一变体:**两个名字相近的指标,一个量意图一个量结果,
日志把它们并排打印而不区分**,引用时极易取到那个必然好看的。

**注意这不是 §11.68 那 207.1s 的可比对象**:代理跑满 100 周期且判据陡峭,
这里 12 个周期就收敛。两者工作量不同,倍数不可直接搬用。本节量的是分摊结构,
不是加速比。

#### gridinit 的弹性松弛确实在跑,只是跑在基网格上

`NL%niter = 5000` 在一秒内完成,看起来像被跳过,其实没有。
`method_c_gridinit_factorization_canonical(80)` 给出
**base_nxp = 40,expansion_factor = 2**:松弛跑在 NXP=40 的基网格(32,000 个
三角形)上,之后解析地 ×2 展开。`niter=0` 与 `niter=5000` 产出的 gridfile
校验和不同,可证它在做功。

#### 细化阶段的弹性松弛被丢弃,提示词曾指错对象

harp_dv 后端下 `effective_refinement_spring_iterations` 把细化弹簧归零——这是
`12078c4` 的有意设计,Laplacian 弹簧会和事务性移动的接受判据打架。丢弃时有提示,
但原文写的是 "the generic **regional** Laplacian spring",而到达这里最常见的
配置恰恰是 `RL%SpringRegional_type = 0` 加 `RL%SpringGlobal_type = 1` ——
读起来像"说的不是我",于是 5000 步被静默丢弃而无人察觉。

提示已改为指名实际设置与被丢弃的步数,并说明它不影响 `NL%niter`。这是
§11.1 那类沉默失败的一个变体:**提示存在,却描述了一个不匹配的对象,效果
等同于没有提示**。

#### 同一个字面量在两条路径上扮演相反角色

`--max-tris` 的默认值,namelist 路径写的是 `.unwrap_or(100_000)` —— 一个裸
字面量,无命名常量、无注释,而且**这条路径根本不读 NXP**。三角形数是
`20 × NXP²`,于是它把全球算例静默卡在 **NXP ≤ 70**(20×70²=98,000 过,
20×71²=100,820 不过)。

而 `--project` 路径上,同一个 100,000 是**下限**:

```rust
let base = 20 * nxp * nxp;              // 按分辨率精确算
base.checked_shl(passes * 2)            // 每轮细化 ×4
    .map(|budget| budget.max(100_000))  // 至少 100,000
```

同一个数字,一处是地板一处是天花板;同一个分辨率,走 project 有余量,走
namelist 直接退出。字面量由 `ebddef6` 引入。

`max_tris` 全仓**只被比较,从不用于分配**(`gridinit_voronoi_state_canonical`
里的一次上限校验),所以 100,000 不对应任何真实资源约束。namelist 路径已改为
同样从 NXP 推导,保留 100,000 作为地板——因此这个改动**只会抬高上限、不会
降低**,原先能过的运行仍然能过。NXP=80 现在无需 `--max-tris` 即可跑通,
cycle 1 的数字与显式给 4,000,000 时逐项一致。

顺带:`refine_gridfile.rs` 曾把 `max_tris` 传给 `from_icosahedron` 的第五个形参,
而那个形参是 `_diagnostic_every` ——带下划线前缀,完全被忽略。传错但无害(这条
路径的网格大小由 `nxp0` 决定,真正的上限校验在 gridinit 里),然而调用点在说谎。

**2026-08-19 已删掉这个形参。** 它在全仓 190 个调用点里没有一个被使用过:除这一处
外全部传 `0` 或 `100`,而函数体从不读它。**一个永远被忽略的形参就是陷阱**——恰好
有一个调用方以为它是三角形预算。删掉之后这个误用写不出来了。`MethodCMesh::from_icosahedron`
的同名转发形参和 `voronoi_gridinit` 里随之失效的 `METHOD_C_DIAGNOSTIC_EVERY` 常量
一并移除。
