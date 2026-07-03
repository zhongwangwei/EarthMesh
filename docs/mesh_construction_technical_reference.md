# EarthMesh v3 网格构建技术参考：计算细节

版本：2026-07-02（对应 h 场层 M1–M4 合入后的代码状态）
性质：实现级技术文档。所有公式、常量与索引约定均直接对照 Rust 源码（及 OLAM r1095 / EarthMesh-2.0.0 Fortran 参考）核验；模块名即 `rust/earthmesh_mesh/src/` 下的目录名，便于对照阅读。

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
              │  (area_judge*, getref*, earthmesh_hfield)    │
              └──────────────┬───────────────┬───────────────┘
                             ↓               ↓
   ┌── grid_preprocess 三角细化管线 ──┐   ┌── OLAM Method-C 嵌套 ──┐
   │ 打标 → iterA..G 过渡判定 →       │   │ 选面(thirdm/rad3) →     │
   │ 1→4/1→2 细分 → LOP 翻边 →        │   │ 周界 mrow → 表重映射 →  │
   │ 弱凹清理 → 重编号/排序 →         │   │ emit → nest 弹簧        │
   │ MPAS 式弹簧平滑                  │   │ (olam_*)                │
   └──────────────┬───────────────────┘   └───────────┬────────────┘
                  ↓                                   ↓
        掩膜后处理（海陆域、孤立单元、边界曲线、水道、重索引）
        (mask_postproc_*)
                  ↓
        输出：MPAS 六边形 C-grid / FVCOM 三角网格 / CoLM 耦合文件
        (earthmesh_cli writers)          质量报告 (earthmesh_quality)
```

两条细化路线共享初始网格与输出端，方法论差异见 `docs/mesh_refinement_method_research_2026-07-02.md`；h 场层（第 8 节）是二者共同的上游。

---

## 1. 数据模型与索引约定

### 1.1 Fortran 保真索引

全库刻意保留 Fortran 约定：数组长度 n+1，**槽位 0（部分表还有 1）为占位**，有效 id 从 2 起；id 以 `usize` 存储，0/1 或负值作哨兵。循环样式 `for i in 2..=n`。删除的三角形连通行写 `[1,1,1]` 占位。此约定使整数拓扑表可与 Fortran 参考**逐位对拍**。

### 1.2 OLAM Delaunay 三表（`icosahedron_types`）

| 实体 | 字段 | 含义 |
|---|---|---|
| M 点（顶点） | `m_points: CartesianPoint(x,y,z)` | 球面（或平面）坐标，f64 |
| | `m_neighbors: { npoly, iu[7], iw[7] }` | 顶点度 npoly ∈ {5,6,7}；环序邻接边/面 |
| | `m_metadata: { mrlm, mrlm_orig, ngr }` | 网格细化级、原始级、代数 |
| U 边 | `{ im[2], iw[..], iu[..], mrlu }` | 两端点、两侧面、邻边、边细化级 |
| W 面（三角形） | `{ im[3], iu[3], iw[9], npoly, mrlw, mrlw_orig, mrow, ngr }` | 顶点/边/邻面、面细化级、过渡行号(isize)、代数 |

不变量（`validate_topology` 强制）：`face.iu` 与 `edge.iw` 互逆互指；活跃三元组唯一；全球网格 Euler 示性数 χ=2。**12 个五边形顶点**（`impent[12]`）是二十面体拓扑的必然（Σ(6−npoly)=12），永不移动、永不消除。

### 1.3 grid_preprocess 侧结构

三角网格用平行数组表达：`mp/wp`（三角形中心/多边形中心经纬度，`LonLatDegrees`）、`ngrmm[i][3]`（三角形三邻接，槽位语义 = `IsNgrmm` 的对顶点编码 1/2/3）、`ngrwm[cell][..]` + `n_ngrwm`（多边形的三角形环，CCW）、`mrl_new`（1=未细化，4=已 1→4 细化）、`ref_sjx/ref_lbx`（细化标记）。

`IsNgrmm(a,b)`：两三角形共享两个顶点时返回 a 中**不在共享边上的顶点槽位**（1/2/3），否则 None——它同时充当邻接判定与对顶定位，贯穿细分/翻边/排序。

### 1.4 Voronoi 对偶映射

角色互换、**索引恒等**（无偏移）：`nma=nwd, nwa=nmd, nua=nud`；三角形 id 直接成为对偶 M 点 id，Delaunay 顶点 id 直接成为六边形 W 胞 id（与 OLAM `hex_grid.f90:voronoi()` 相同约定）。

---

## 2. 初始网格构建

### 2.1 NXP 与菱形展开（`gridinit`, `icosahedron_initial/diamonds/grid`）

二十面体 10 个菱形（diamond），每个按 NXP×NXP 划分。`olam_gridinit_factorization` 从 NXP 分解出基础尺寸与倍增次数（选"阈值之下最大"的候选）。菱形角点经纬由黄金分割三角学给出；内部点按 Fortran `fill_diamond` 的权重混合公式插值，权重分母 `i+j−1` 与 `2·NXP+1−i−j` 恒 ≥1。南北极行有专门覆盖。填充后派生 M/U/W 邻接（两遍法避免别名，`derive_icosahedron_*_neighbors_fortran`）。

计数关系（全球）：`nwd = 20·NXP²`，`nud = 30·NXP²`，`nmd = 10·NXP² + 2`。

### 2.2 全球弹簧松弛（`icosahedron_spring_grid`，OLAM `spring_dynamics_globe` 谱系）

目标：把菱形展开的非均匀三角边长驱向准均匀。**逐迭代计算**：

1. 名义边长 `dist00 = β · 2πR / (5·NXP)`，其中 β=1.25（namelist beta），R 为网格半径；`disto12 = dist00 / 1.2`。平面模式（mdomain≥2）`dist00 = Δx·√(2/√3)`。
2. 每条边：`dx = f32(x₂−x₁)`（**刻意单精度截断**，模拟 Fortran 无 kind 的 `real()`；下同），`dist = √(dx²+dy²+dz²)`。
3. 对边的四条邻边（`EdgesOnedge_tri`/`iuun` 给出 iu1..iu4，即共享该边的两三角形的另四条边），用余弦定理算两对顶角的 2cos：
   `twocosphi₃ = (d₁²+d₂²−d²)/(d₁d₂)`，`twocosphi₄ = (d₃²+d₄²−d²)/(d₃d₄)`。
4. `ratio = clamp(twocosphi₃+twocosphi₄, 0.15, 1.2)`；目标长 `distm = disto12 · ratio`（等边三角形时 2cos60°×2 = 2→clamp 到 1.2→distm=dist00，即上限恢复名义长；钝角/退化则收缩到最低 0.15/1.2·dist00≈0.125·dist00）。
5. `frac_change = (distm − dist)/dist`；位移分量 `dx ← dx·frac_change`。
6. 每个 M 点累加 `x += Σⱼ dirs(j)·dx(iuⱼ)`，`dirs = ±relax`（relax=0.035，符号取决于该点是边的 im[1] 还是 im[2]——两端点反向受力）。**Jacobi 结构**：位移全部来自迭代开始时的快照。
7. 球面模式逐点投影回半径：`expansion = R/‖x‖`。
8. 12 个五边形点（impent）**钉死不动**。
9. 坐标以 f64 累加（r8 模拟），每迭代的 delta 经 f32 截断；全部迭代结束后整体 `x = f64(f32(x))` 一次（对应 Fortran `xem(:) = real(xem8(:))`）。

迭代次数 namelist 控制（典型数千次）。已知理论性质：该法仅一阶最大范数收敛（Peixoto & Barros 2013 证伪了原论文的二阶声明）；自然弹簧长超临界值无稳定平衡——本实现的 clamp 与固定 β 避开该区。

### 2.3 Voronoi 对偶与 PCVT（`voronoi_grid/voronoi_gridinit/voronoi_pcvt`）

对偶 M 点（六边形网格顶点）初值 = 所环绕三角形三顶点的**重心**（算术平均/3）；随后 `pcvt` 用**外心**替换：仅当该点的三个 `iw` 邻接全部有效（`≥2`）才替换，否则保留重心（边界/占位防护，与 OLAM `pcvt()` 的 skip 条件一致）。

外心求解在**极平面立体投影**（polar stereographic）切平面内进行：以三点重心方向为投影极点，正/反投影矩阵互为转置；平面内用垂直平分线联立（Cramer），并按 |dx12| 与 |dx13| 大小选择条件更好的方程回代 xc（数值稳健分支）。退化（共线/零轴）回退重心。

球面多边形面积/定向用 **l'Huilier** 公式：半周长 s，`tan²(E/4) = tan(s/2)tan((s−a)/2)tan((s−b)/2)tan((s−c)/2)`，`sqrt` 前 `max(0,·)` 防负；跨日界线先做 ±360 单步经度校正。CCW 判定失败即反转顶点序（`GetSortNew`、`orderVerticesOnCell` 谱系）。

---

## 3. grid_preprocess 三角细化管线

驱动循环按"轮"（iter/level）推进：打标 → 过渡判定 → 细分 → 翻边 → 清理 → 重编号 → （最终）弹簧。

### 3.1 打标（三种来源，产物统一为 `ref_sjx ∈ {0,1}`）

- **阈值细化**（`area_judge*`, `getref*`）：对判据栅格（LAI/坡度/土壤/SST/SSH/EKE…）逐三角形取均值/标准差与阈值比较；数据经 2D/3D 归约（`getref_mean_std_*`）。
- **指定细化**：bbox（跨日界线感知）、circle（大圆距离）、closed curve（射线交点奇偶，含共享顶点退化的容差处理）、Lambert 投影域。
- **h 场细化**（默认路径；legacy 硬掩膜仅专家模式启用）：`ref_sjx[i] = 1 ⟺ mrl_new[i]==1 且 level_at(中心) ≥ 当前轮次`。梯度限制过的场保证逐轮标记集为嵌套收缩环。

### 3.2 过渡判定链 iterA..G（`refine_iter*`）

细分只允许 1→4；为避免非法拓扑，一串"judge"内核把初始标记扩张成**可行标记集**。关键计算（已与 EarthMesh-2.0.0 Fortran 逐行对照）：

- **iterB**（`+=` 累加语义）：每个已细化三角形向三个未细化邻居注入 `mrl_in += 2`；随后 `set_dis` 轮传播：对 `transition_sum == 4` 且存在相邻的两个 `mrl_in==2` 邻居（HHH=[0,1,2,0,1] 环序判两连）者，`mrl_bk[自身] += 2, mrl_bk[对顶] += 2`（mrl_bk 从 mrl_in 克隆起步、逐轮回写）；最终 `mrl_in ≥ 4` 者标记。
- **iterC**（`=` 覆盖语义，**与 iterB 的不对称是原版设计**，Fortran 注释明言"此处只有 0/2 两种取值"）：
  - 五边形胞：邻接三角形 `Σmrl_new > 10`（≥2 个已细化）→ 其余未细化邻居全部标记（五边形无弱凹容忍）。
  - 六边形胞 `Σ==12`（恰两个已细化）：若二者相对（槽位 j 与 j+3），把中间两个未细化者标记（对角细化 → 视作四连）。
  - 射线传播（每轮 `mrl_bk.fill(0)` 重置，与 iterB 不同）后构造 `ref_lbx_in[cell][槽位]`；**七边帽**：对不含细化三角形的 5/6 边形，相邻两条"射入"合并计 0.5+0.5，`Σ + num_edges > 7` 则把射入三角形标记（细化后边数不超 7 的约束）。
- **iterE**：`state_sum == num_edges + 6` 识别"恰两个已细化邻接"构型并回写 `lbx_refine`（写幂等，覆盖顺序无关）。
- **iterF/G**：保护单元（`impent` 及 `edge_counts < 5` 者）的标记回收，防止五边形顶点被过渡链波及。

### 3.3 细分几何

- **1→4**（`refine_onedivide_four*`）：三边中点为新 M 点（经纬平均后如跨日界线先 `CheckCrossing` ±360 校正，再回绕）；父三角形连通行清为 `[1,1,1]`，四子行填入（先 [1]/[2] 槽后补 [0] 槽的 Fortran 次序）；`sjx_child` 记父→子。
- **1→2 过渡**（`refine_onedivide_two`）：被标记的过渡三角形找邻域中**唯一**满足态邻居（正向找 `mrl_new==4`，反向找 `==1`）——扫描用 `rfind`，等价 Fortran 无 EXIT 的后写覆盖循环（多候选时取"最后命中"，正常构型唯一候选）；对顶点 w1 与公共边两端 w2/w3 确定：公共边中点 `tempc=(w2+w3)/2` 为新 W 点，两子三角形中心 = `(w1+tempc+w2)/3`、`(w1+tempc+w3)/3`。新点编号 `m₁,m₂ = num_mp[iter−1] + 2k+1, 2k+2`；`w₄ = num_wp[iter−1] + k+1`（Fortran 先加后用的计数惯例）。
- 全程日界线规则：三点极差 >180° 触发 `CheckCrossing`（±360 单步），落点再校正回 [−180,180]。

### 3.4 LOP 翻边与弱凹（`refine_lop*`, `refine_isreverse_judge`, `refine_boundary*`）

Lawson 式对角交换：共享边三角形对 (a,b,c)/(a,b,d) → (c,d,a)/(c,d,b)，同时更新两三角形与四外邻居的 `ngrmm`（槽位相对、非几何绕向——`IsNgrmm` 编码保证一致性）。变体：`_sharp/_weak/_pair/_weak_pair` 分别处理锐角、弱凹、成对镜像折叠（`num_end−k` 折叠索引、`step_by(4)`，两端向中间收）。弱凹段构造含偶/奇配对 `k%2==0→k−1 else k+1`。方向判定 `isreverse` 用段压实游标（无匹配不推进，避免空洞）。

### 3.5 重编号与排序（`refine_renewal*`, `get_sort_new`）

`ngr_renew` 重建 `ngrwm/n_ngrwm`（`.skip(2)`——槽位 1 为 Fortran 空行约定）；`GetSortNew` 对每个多边形的三角形环做邻接行走排序（起点取首个度 1 三角形，闭环取槽位 0；断链回退取第一个未用），`robust_spherical_area < 0` 则整环反转为 CCW。

### 3.6 MPAS 式六边形弹簧（`spring_dynamics/spring_edge_dynamics`）

结构同 2.2，但作用于六边形 C-grid：边邻居由 `EdgesOnedge_tri(4,·)` 给出，目标长来自 `distsOnEdge`（`target = distsOnEdge/1.2 · ratio`），方向符号 `CellsOnEdge(2,iu)==iw → +relax`。**精度口径**（对照 `MOD_grid_preprocess.F90:816-819` 修正后）：坐标差分量 f32 截断、r8 求模，自身边与邻边共用同一 `dist` 数组。全零距守卫返回 None（比 Fortran 稳健）。

---

## 4. OLAM Method-C 嵌套细化

### 4.1 选面（`olam_selection*`, `olam_spawn_hfield`）

选择的产物是 `selected: Vec<bool>`（W 面掩膜）。合法掩膜的三要素（缺一则周界行走器报 "exceeds 7-edge ring"）：

1. **thirdm 步进种子**：从起点沿"隔二取三"的格行走（stride-3 lattice）扩张 M 点种子集；每个弹出点检查其所有边 `mrlu == mrlo`（越代即报 "crosses the parent boundary"）；邻居入栈条件 `jdone 遍历数 < 2 且 被需求包含`。
2. **五边形格锚定**：若任一五边形被需求集包含，行走**必须**从该五边形起步（把 stride-3 子格钉在二十面体框架上；这是边界 3 对齐的来源）。区域路径还有"五边形仅邻近 → 从其行军至区域"的细化分支。
3. **rad3 足迹**：每个种子标记其半径 3 环内的 W 面，按种子的 `mrlm` 过滤 `mrlw == mrlo`（只选同代面）。掩膜 = 足迹并集——天然肥厚平滑。

h 场模式（M4）以 `level_at(质心经纬) ≥ pass` 替换几何包含，其余机制同源；逐 pass 1..max_level 推进，空选择即干净停止。

### 4.2 周界与过渡行（`olam_perimeter*`, mrow）

细化边界强制为**3 的倍数条粗边的直线段**；跨原 2 粗行的空隙精确布 **3 条过渡行**（Fortran `spawn_nest.f90` 注释原文语义）。`perim_mrow` 从边界行（mrlw 失配处）向两侧交替扩散行号：`mrow_temp2 = mrow ± jrow`，`jrow = mod(irow,2)`，循环 `2..=2·max_mrows`。顶点度全程限 {5,6,7}。掩膜不合法时 `olam_mask_annealing` 单调侵蚀修复（上限 32 轮，全消或全保即停）。

### 4.3 表重映射与发射（`olam_method_c_*`, `olam_emit`）

每个被选面 1→4：分裂边（split-U）中点 = 端点加权平均，**先累加、最后统一投影半径**（`project_to_radius` 门控，与测试预言一致）；`perim_fill3` 处理过渡带的 iu 槽位改写，两种镜像模式——匹配槽 j 时写 (j−1) mod 3（"after"）或 (j+1) mod 3（"before"），与 OLAM `spawn_nest.f90:1443-1506` 的 if/elseif 链逐一对应。`emit_method_c_tables` 三条 id 分配环（iwnew/iunew/imnew 首见门控）重建全表，子面 `mrlw = mrlo+1`，出口强制 `validate_topology`。

### 4.4 nest 弹簧（`olam_nest_spring*`）

在 2.2 公式上叠三项：

- **级缩放**：`target_base = (dist00/1.2)/2^(mrlu−1)`（每细化级目标减半）。
- **mrow 乘子**（过渡行几何渐变，即"级内伸缩"的原始雏形）：按边两侧面的 mrow 对查表
  `{(−2,−2):7/6, (−1,−2):8/6, (−1,−1):9/6, (1,−1):10/6, (1,1):11/12, 其余:1}`。
- **面积防退化**：`dmin = dist00/2^(max_mrlu−1)`，`minA² = 0.1875·dmin⁴`（0.1875=3/16，边长 a 等边三角形面积平方 = 3a⁴/16）；局部两三角形 Heron 面积平方 `s(s−d)(s−d₁)(s−d₂)` 取小者，`area_ratio = max(minA²/localA², 1)` 只放大不缩小目标长——防止过渡区三角形被压塌。

可动点 = 目标代（ngr）中邻接 `mrow ≠ 0` 面的 M 点（`move_interior` 可扩为全代）。仅对 `moveu/compu` 掩膜内的边计算（选择性 stencil）。h 场变体（M2）以 `h(边中点)/1.2` 直供 `target_base`、乘子恒 1、`dmin = min h`。

---

## 5. 掩膜后处理（`mask_postproc_*`）

- **域标记**：`IsInDmArea ∈ {0 占位, −1 陆, 1 海}`，landtype 栅格采样按 1024² 瓦片缓存或 ≤256MB 整读（逐点读法已废除——曾占 ocean 案例 30% 耗时）。
- **孤立海剥离**：顶点邻接计数新旧对照逐层内收（`num_add==0∨1` 停）。
- **边界闭合曲线**：海陆界 vertex-vertex 双邻接链 `bdy_ngr[2]` 行走成环（`num_points<3` 报错）；`num_bdy_long = [最长长度+1, 次长+1, 最长曲线 id]`（两槽位最大值跟踪，2026-07 修正版）。
- **水道加宽 / 顶点仅触海填充**：模板见 `mask_postproc_waterway`（首边即断的 Fortran 惯用扫描）。
- **重索引**：old→new 映射一次生成、施加到所有引用表，占位槽 0/1 保留。
- 产物流向 FVCOM/OBC/CoLM writers 与最终 gridfile。

---

## 6. 质量度量与验收（`earthmesh_quality`, `check_mpas_mesh_topology`）

几何：平面 shoelace 面积（负绕向经经度展开后判定并 Fail）、haversine 边长、3D 弦角内角（acos 前 clamp[−1,1]）、aspect = 最长/最短边（大圆）、compactness = 4πA/P²；NaN 顶点单独计数并整体隔离出统计。拓扑：索引越界、非流形边（>2 面共享）、孤儿胞、邻接互惠、χ 校验（全球 2/区域盘 1）。门控分级 Pass/Warn/Fail，阈值默认 min_angle 5/20°、aspect 4/10（严格比较口径统一）。

验收层级：**compat 模式** = 整数拓扑逐位对拍（对 Fortran 参考）；**fast/h 场模式** = validate_topology + 质量报告 + 行为断言（本仓库 M1–M4 测试即范本）。

---

## 7. 数值约定（全库统一）

| 约定 | 细节 |
|---|---|
| 地球半径 | `EARTH_RADIUS_METERS = 6_371_229.0`（Fortran `erad`），全库单一来源；hfield 本地常量与之相等 |
| 混合精度 | 存储 f64；凡 Fortran 写 `real(expr)`（无 kind）处，Rust 以 `as f32 as f64` 精确复刻（弹簧坐标差、投影 `_f32` 变体、迭代尾整体截断）。这是对拍的一部分，**不是**可随手"修复"的精度损失 |
| 日界线 | 统一 ±360 单步校正（`CheckCrossing`/`unwrap_lon_around`/锚点展开）；跨界判据 = 极差 >180° |
| 极点/反对径 | 所有 `acos/asin` 入参 clamp[−1,1]；测地距优先 atan2 形式（hfield/quality 的 haversine 天然免疫反对径 NaN） |
| 定向 | CCW 基准 = `cross(v_i−c, v_{i+1}−c)·ĉ > 0`（球外视角），负则反转；退化面积 `max(0,·)` |
| 确定性 | 无线程、固定遍历序、BTreeSet/稳定序打破平手；同输入逐位同输出是显式测试项 |
| 除零守卫 | 距离/面积/模长为零一律显式返回 None/Err（比两份 Fortran 参考都严格） |

---

## 8. h 场层（`earthmesh_hfield`，2026-07 新增）

统一"阈值细化 + 指定细化"的连续目标尺寸场：

1. **合成**：`h(x) = min_i h_i(x)`。判据栅格 → h 场（推荐 FESOM 线性式 `r = clip(s/s_t, 1, r_max)`, `h = h_base/r`）；区域（bbox/circle/polygon/corridor，全部日界线安全）→ 域内钉 `h_inside`（硬边界，交给限制器造坡）。
2. **梯度限制**：解 `|∇h| ≤ g` 的最大下界场 `h*(x) = min_y (h₀(y) + g·d(x,y))`（Persson 定理）。实现为球面 fast sweeping：4 序确定性扫描，双轴上风 eikonal 局部解（1D 候选 `a+g·Δx` 与二次联立 `((h−a)/Δx)² + ((h−b)/Δy)² = g²` 取小），经度周期、逐行 `cosφ` 度量。**推论**：邻胞尺寸比 ≤ 1+g；量化后每级环带宽 ≈ `h_level/g`（≈0.7/g 行），Method-C 套娃净距在 g ≤ 0.22 时构造性满足。栅格须解析局地 h（间距 ≤ h，理想 ≤ h/2）。
3. **量化**：`level = ceil(log₂(h_base/h))`（含 1e−9 防浮点毛刺），clamp 到 max_level。
4. **三个消费口**：`level_at → spawn_nest_from_target_levels`（Method-C，第 4.1 节）；`level_at → refine_marks_from_target_levels`（3.1 节）；`sample(边中点) → spring_nest_with_edge_targets`（4.4 节，级内伸缩）。

参数建议：海洋网格 g=0.12–0.2（涡旋对陡过渡敏感），大气 0.2–0.3；`h_base` = 未细化名义尺寸 `dist00`。

---

## 9. 计算参数速查

| 参数 | 值 | 出处 |
|---|---|---|
| 弹簧 relax | 0.035 | OLAM/mkgrd 同 |
| 弹簧 β（globe） | 1.25 | dist00 系数 |
| twocosphi clamp | [0.15, 1.2] | 目标长比例窗 |
| 目标长除数 | 1.2 | `disto12 = dist00/1.2` |
| mrow 乘子 | 7/6, 8/6, 9/6, 10/6, 11/12 | 过渡行对 (−2,−2)…(1,1) |
| 面积底系数 | 0.1875 (=3/16) | 等边三角形 A² = 3a⁴/16 |
| 顶点度约束 | 5/6/7 | Method-C 拓扑规则 |
| 过渡行结构 | 3 行跨 2 粗行；边界段长 ≡ 0 (mod 3) | spawn_nest 注释语义 |
| 细化级上限 | 5 | `OlamRefinementRegion` 校验 |
| 最小网格间距 | 0.001 m | `OLAM_METHOD_C_MIN_GRID_SPACING_METERS` |
| 退火轮上限 | 32 | mask annealing |
| h 场 g 推荐 | 0.15–0.3（海洋取低） | 第 8 节 |
| 地球半径 | 6 371 229 m | 全库 |

---

## 10. 已知边界与提示

- 12 个五边形导致的 grid imprinting 是拓扑必然，任何优化只能缓解（Peixoto 系列）。
- 过渡行的 5/7 边胞是质量下限所在；h 场弹簧（级内伸缩）显著改善其形状但不消除其拓扑。
- `rfind`/`=`覆盖 等"怪癖"是 Fortran 忠实移植（已逐处对照原文定案），修改前先读 `docs/mesh_generation_bug_audit_2026-07-02.md` 第五节。
- writers 的 NetCDF 变量布局本文未展开（属 `earthmesh_cli`，见各 `*_writer/*_io` 模块与对应测试）。
- h 场 CLI 接线（namelist → HField 构建）在本文档撰写时尚未合入（M5 待做）。
