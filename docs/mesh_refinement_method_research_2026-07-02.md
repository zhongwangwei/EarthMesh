# 网格细化方法选型深度研究报告

日期：2026-07-02
问题：是否存在更好的网格细化方法，能统一陆面/大气/海洋三域，并同时支持阈值细化（refine_cal：LAI/坡度/土壤/SST/SSH/EKE/台风路径）与指定细化（bbox/circle/polygon/corridor）？
约束：最终输出必须保持 MPAS 六边形 C-grid 与 FVCOM 三角网格兼容（下游模式不变），候选方法仅作为上游细化/生成内核。
方法：5 路并行检索（JIGSAW/MPAS 密度场路线、OceanMesh2D/梯度限制、SCVT/Lloyd 对比弹簧、FESOM/FVCOM/多域实践、Rust 集成与反面论证），一手来源直接抓取（LICENSE 文件、GMD/JAMES/JCP 论文原文、代码库），关键论断交叉验证并标注置信度。

---

## 一、结论（先说答案）

**存在，而且方向明确：连续网格密度场（cell-width field）范式。** 把"阈值细化"和"指定细化"统一表示为一个标量场 h(x)（目标单元尺寸），各判据独立生成各自的 h 场，逐点取 min 合成，再做梯度限制（|∇h| ≤ g），最后交给一个以 h(x) 为驱动的生成/优化内核。这是 E3SM/MPAS-Ocean 生产网格（JIGSAW + compass）、OceanMesh2D（ADCIRC 社区）、FESOM 的共同做法，工程上完全成熟。

**但不建议推倒重来。** 推荐分阶段混合架构：先在 EarthMesh 现有管线上游加一个"统一密度场层"（纯 Rust 自研，中等工作量，立刻改善现有离散细化的多判据组合问题），生成内核的替换放到第二阶段且保留现有引擎作为 compat 模式。理由与证据见下文，特别是第六节（现有离散细化仍占优的场景）。

---

## 二、范式对比：连续密度场 vs 离散分层细化

### EarthMesh 现状（离散分层）
标记三角形（阈值判据或区域判定逐个单元打标）→ 1→4 细分 + 1→2 过渡三角形 → LOP 翻边 → 弱凹清理 → 弹簧平滑。过渡质量靠 7 边帽、弱凹特判、mrow 过渡行等离散规则手工维护——这正是本项目审查中最复杂、特判最多的代码。

### 密度场范式（连续）
1. **每个判据一个场**：阈值判据（LAI、坡度、SST 梯度…）直接把栅格数据映射为 h 场；指定区域（bbox/polygon/corridor）用带符号距离函数（signed distance）+ tanh 过渡带生成 h 场——E3SM compass 教程中的标准做法就是 `weights = 0.5*(1+tanh(signed_distance/trans_width)); h = h_fine*(1-w) + h_base*w`，且可嵌套（30–60km 背景 → 20km 南大西洋 → 10km 亚马逊河口三层嵌套的实例）[MPAS-Dev compass RRM 教程，高置信]。
2. **合成**：逐点取 min。OceanMesh2D 原文（GMD 12, 1847–1868, Eq. 13）："h = min[(h_dis or h_lfs), h_wl, h_slp, h_ch]"[高置信，原文逐字]。
3. **梯度限制替代过渡行**：Persson (2004/2006) 证明限制 |∇h| ≤ g 即保证相邻单元尺寸比 ≤ 1+g（原文："This corresponds to a limit on the gradient |∇h(x)| ≤ g with g = G − 1"），且最优解有闭式（点源锥 min），用 fast marching O(n log n) 求解[高置信，原文逐字]。实践取值：OceanMesh2D 各案例 g = 0.15–0.35，作者指导"g > 0.25 显著降低顶点数，g < 0.20 妨碍特征场扩展"[高置信]。**这一条数学保证吃掉了 EarthMesh 里 7 边帽/弱凹/过渡行的全部离散特判**——平滑性在场的层面一次性解决，而不是在拓扑操作层面逐例修补。

### 对 EarthMesh 两种细化模式的映射

| EarthMesh 现有概念 | 密度场范式中的对应物 |
|---|---|
| 阈值细化 refine_cal（LAI/slope/SST/EKE…） | 判据栅格 → h 场（FESOM 的 SSHV 线性映射 r = clip(s/st, 1, rmax)、半 Rossby 半径规则；OceanMesh2D 的波长/坡度/岸距/特征宽度函数——均有可直接移植的公式）|
| 指定细化 bbox/circle/polygon/corridor | signed-distance 场 + tanh 过渡（compass 标准做法）；corridor = 折线距离场（OceanMesh2D 的 thalweg/polyline 函数同型）|
| set_dis / halo / 过渡行 | 梯度限制参数 g（单一标量，数学保证）|
| refine_planner 的 WeightedSum/WeightedMax | 已经是雏形——升级为完整 h 场合成层即可 |

---

## 三、生成内核选型（关键证据）

### JIGSAW / JIGSAW-GEO（Engwirda）
- **算法**：restricted frontal-Delaunay 细化 + hill-climbing 优化，Delaunay/Voronoi 对偶直接满足 MPAS C-grid 要求（GMD 10, 2117–2140）[高置信]。
- **质量**：三个全球测试例角度界 40°–80°，area-length 指标 0.90–0.94；作者自报告单核 i7 用时 12s / 1.5min / 10min（uniform sphere / regional Atlantic / Southern Ocean 156 万胞）[高置信，作者自报]。
- **生产采用**：E3SM 生产网格全家桶（EC30to60、SORRM 南大洋 12km、WC14 北美 14km、IcoswISC30E3r5 等，E3SM Confluence 有带 PR 号的正式表格）；NCAR MPAS-A 用户论坛有 60→3km/60→1km 实例与官方答疑[高置信，直接抓取]。
- **管线**：MPAS-Tools `build_spherical_mesh`（cellWidth 栅格进）→ jigsawpy → `jigsaw_to_netcdf` → `MpasMeshConverter.x`（算 Voronoi 对偶、产 nCells/verticesOnCell）→ `MpasCellCuller.x`（陆地剔除）[高置信]。**注意：这条链的后半段与 EarthMesh 现有 writers/mask postproc 职能重叠，接入点在 cellWidth→三角网格这一步。**
- **⚠️ 许可证（本次研究最重要的工程发现之一）**：自定义非 OSI 许可——"Private, research, and institutional use is free... Distribution of this code as part of a commercial system is permissible ONLY BY DIRECT ARRANGEMENT WITH THE AUTHOR"[两路独立抓取 LICENSE 原文，逐字一致，高置信]。科研使用无碍；若 EarthMesh 未来有任何商业分发路径需与作者达成协议。conda 包停在 2020 v0.3.3，但 GitHub master/jigsaw-python 活跃到 2025-08（v1.1.0）——需从源码构建[高置信]。
- **集成**：有干净的 C API（lib_jigsaw.h，extern "C" 单入口 + alloc/free），bindgen 绑定约数日工作量；或走 MPAS-Tools 模式以外部进程 + .msh 文件对接（不把许可证链进自己二进制，但捆绑分发仍需注意）[高置信]。无现成 Rust 绑定。

### 密度加权 SCVT / Lloyd（Ringler/Ju 路线）
- 理论干净：h ∝ ρ^(−1/4)（球面 d̃=2，指数 1/(d̃+2)；原文标注为"猜想+数值验证"而非定理）[高置信]。
- **但工程上不推荐作为首选内核**：Lloyd 收敛率随生成点数按 O(1/k²) 退化（Du/Emelianenko/Ju, SIAM 2006 证明）；参考实现 MPI-SCVT 是个人仓库、无 release、无维护[高置信]；Engwirda 论文称其需"days or even weeks"且产出少量钝角三角形破坏 well-centered 约束——此说来自竞争工具作者，两路代理均独立标注利益相关，仅中等置信。加速方案（Lloyd 预条件 LBFGS，Yang/Gunzburger/Ju）把 65 万点 SCVT 降到 128 核 22–32 分钟，但无维护良好的现成实现。
- 有趣的反证：Hoch et al. 2020 (JAMES) 表明 MPAS-Ocean 对网格质量缺陷相当宽容（故意劣化网格后南大洋输运误差 <0.4%、湾流 <12%）——网格质量焦虑不应主导选型[中置信]。

### 弹簧平滑（现状，Tomita 谱系）
- Peixoto & Barros (2013, JCP) 直接证伪了 Tomita 自称的二阶收敛："our results show that in fact only first order convergence in the maximum norm is guaranteed"[高置信，原文逐字]。
- 无密度函数机制（自然弹簧长是全局参数），做变分辨率天然别扭；且自然弹簧长超临界值时无稳定平衡。
- 但 Peixoto (2016) 同时表明"SCVT 优于弹簧"是算子相关的：标准 TRiSK 散度算子下两者同样不一致（截断误差不收敛），HR95 才救得回来；SCVT 只在 Coriolis 项占优。**"换掉弹簧"的收益别高估**[高置信]。

### 许可证友好备选
- **geogram（Lévy）**：BSD-3[抓取 LICENSE 确认]，活跃维护，CVT/RVD/鲁棒谓词齐全，但球面尺寸场生成需自己在其原语上搭。C++ FFI。
- **纯 Rust 生态**：`robust`（Shewchuk 自适应精度谓词的 Rust 移植，MIT/Apache）+ `spade`（2D Delaunay/CDT，MIT/Apache，成熟）是可用地基；**不存在现成的球面尺寸场驱动 Delaunay/CVT crate**——自研属实打实的数月级工程[高置信]。
- CGAL（Mesh 包 GPL）、gmsh（GPL）、TetGen（AGPL）对本项目许可证不友好；Triangle 与 JIGSAW 同类（科研免费、商业需协议）。

---

## 四、三域统一：已有实践与我们的位置

- **E3SM 路线（最成体系）**：MOSART 河流路由已原生跑在 MPAS Voronoi 网格上（Liao et al. 2025, JAMES），明确目标"unified mesh framework for coupled land, river, and ocean simulation"；Engwirda & Liao (IMR 2021) 的 "Unified Laguerre-Power Meshes" 提出单一多尺度网格 + 嵌入边界免插值耦合——LANL/PNNL 的多年计划[高置信存在，内部算法细节未验证（PDF 抓取失败）]。
- **FESOM（海洋判据公式可直接移植）**：SSHV 阈值/线性法（r = max(1, min(s/st, rmax)) + 扩散平滑迭代 rk+1 = rk + R²Δrk）、半 Rossby 半径规则（上限 4/7km）；并有重要负结果——**局地解析 Rossby 半径不充分**，上游粗网格会耗散传入涡旋（Danilov & Wang 2015；Sein 2017 XR 网格在巴西-马尔维纳斯汇流区 SSH 变率反而低于 HR）[高置信/二手高置信]。
- **陆面判据（Mesher 模式可直接移植）**：逐三角形对 DEM/土地覆盖栅格算 RMSE/Tol/MD 误差阈值 + 多约束加权 W = Σ αr·wr + 河网作为约束边（PSLG）嵌入；自报告可在保留地形异质性下削减 50%–99.9% 单元数（Marsh et al. 2018, GPLv3——只能学思路不能抄代码）[高置信]。
- **我们自己**：检索路径 4 独立找到了 Fan et al. 2024 (GRL, e2023GL107059)——EarthMesh 的多目标判据（elevation/slope/land cover/LAI）+ CoLM 耦合已是该方向的已发表先行工作之一。本报告的建议本质上是：把这套已发表的多判据能力从"离散标记"升级为"连续密度场"，与 E3SM/OceanMesh2D 的范式合流，同时保住我们独有的 CoLM/FVCOM/MPAS/Method-C 四模式输出面。

### 过渡区证据对比（重要且诚实：文献是矛盾的）

| 模式系 | 机制 | 结论 |
|---|---|---|
| FESOM（海洋，涡旋物理） | 连续尺寸场，定性指导 | 陡过渡**实测有害**（涡旋跨粗区被耗散）|
| ICON（大气，嵌套） | 固定 2:1 四分 + 4 行边界带 | 伪波反射**存在但有界**，"对实际应用可忽略" |
| CAM-SE/MPAS-A（大气，台风 RRM） | 静态变分辨率 | 台风穿越过渡区"**无数值畸变、无波反射**" |

没有任何一篇给出普适的邻胞尺寸比上界——g 是模式/物理相关的调参量。对 EarthMesh 的含义：**梯度限制参数 g 应按输出目标模式暴露为 namelist 选项**（海洋网格取小 g、大气可放宽），而不是写死。台风路径细化在文献中就是"气候学走廊 + 静态 RRM"（Zarzycki & Jablonowski），与 corridor 指定细化同构，无需特殊机制。

---

## 五、推荐架构（分三阶段，兼容优先）

```
阈值判据(LAI/slope/SST/EKE/台风走廊…) ─┐
                                        ├→ 各自 h_i(x) 场 → h = min_i(h_i)
指定区域(bbox/circle/polygon/corridor) ─┘        ↓
                                    梯度限制 |∇h| ≤ g (fast marching, 按域配 g)
                                                 ↓
                     ┌── compat 模式：h 场驱动现有离散标记(阈值=h场采样) ──┐
                     │                                                      │
                     └── fast 模式：密度场生成内核(外部 JIGSAW / 自研)  ──┤
                                                 ↓                          ↓
                          现有 Voronoi 对偶/PCVT → mask postproc → MPAS/FVCOM/CoLM writers（不变）
```

**阶段 1（低风险，立即有收益，纯 Rust 自研，约数周量级）：统一密度场层。**
在 `earthmesh_refine_planner` 基础上扩成完整的 h 场管线：判据栅格→h 场、区域→signed-distance→tanh 场、min 合成、Persson fast-marching 梯度限制。然后**现有离散细化引擎改为从 h 场采样打标**（h(cell) < 当前尺寸 → 标记细化）。收益：多判据组合从各处 ad hoc 代码收拢为一处可测试的数学层；过渡宽度获得 g 的统一控制；阈值+指定两模式在输入层就统一了；**完全不动生成内核与对拍**。

**阶段 2（中风险，可选）：密度场生成内核，feature-gate 双轨。**
- 短平快：外部进程对接 JIGSAW（MPAS-Tools 模式，.msh 文件交换），仅科研构建启用；商业分发前需与 Engwirda 达成协议或禁用该 feature。
- 长线：在 `spade`+`robust` 上自研球面 frontal-Delaunay（立体投影分片或直接 S² 谓词），或 geogram FFI（BSD 干净）。数月级工程，等阶段 1 证明密度场价值后再启动。
- 验收标准用已建好的 `check_mpas_mesh_topology`（χ=2）+ quality 报告，而非位级对拍——这正是此前性能讨论中 compat/fast 双模式的落点。

**阶段 3：三域一场。**
同一 h 场分域实例化（海洋通道叠 FESOM 类判据 + 小 g；陆面通道叠 Mesher 类误差判据 + 河网 corridor；大气通道叠区域 RRM），MOSART-on-MPAS (2025) 证明统一网格耦合在下游是可行方向。

**保留不动的**：Method-C 嵌套（需要精确 2:1/父子唯一映射与逐级时间步配对的场景，ICON 论文给出了保留它的最好论据——连续变分辨率网格全域受最小胞 CFL 限制，而离散嵌套天然按层配 0.5× 时间步）；mask postproc；全部 writers；位级对拍测试体系（compat 模式专属资产，fast 模式换质量验收）。

---

## 六、诚实的反面：现有离散细化仍占优的场景

1. **位级可验证性**：EarthMesh 对 Method-C reference implementation 的表级精确对拍（nmd/nud/nwd、逐级 W 面数、mrow 包络）是离散整数拓扑才做得到的回归保证；连续优化内核只能做统计/拓扑级验收。
2. **cell-id 稳定性与增量细化**：restart_expand、掩膜、landtype 表都挂在 cell id 上；密度场全量重生成会打碎 id 映射，而 1→4 细分的父子关系是可追溯的（ICON："unique relationship between parent and child cells"）。
3. **时间步经济性**：见上，ICON 原文明确指出连续局部加密网格"time step is restricted by the smallest cell in the domain unless specific measures like sub-stepping are taken"。
4. **确定性**：JIGSAW/geogram 均无跨平台"同输入→同网格"的文档保证（JIGSAW 有线程并行路径，需实证检验）；现有整数拓扑管线是天然确定的。**若采用阶段 2，须先做 N 次重跑 byte-diff 实证**。
5. **许可证自由**：现有代码 100% 自有；引入 JIGSAW 即引入商业分发约束。
6. Hoch 2020 的宽容性结果提醒：网格质量提升对模拟结果的边际收益可能有限，重写的动机应主要来自**架构收益**（判据组合、过渡控制、三域统一）而非单纯质量数字。

---

## 七、未决问题清单

- JIGSAW/geogram 的实证确定性（重跑 diff 实验，采用前必做）。
- Engwirda & Liao Laguerre-Power 统一网格的算法细节（PDF 未能抓取，值得人工下载精读——与我们目标最接近的架构）。
- Danilov & Wang 2015 原文（ScienceDirect 拦截，二手引用高置信）。
- oceanmesh Python v1.0.0（2026-01）刚发布，其球面立体投影全球网格路径值得跟踪。
- MPAS-A OpenMP 线程数不可复现的传闻（模式运行层面，非网格生成，低-中置信）。

## 主要来源

- Engwirda 2017, JIGSAW-GEO, GMD 10:2117 — https://gmd.copernicus.org/articles/10/2117/2017/ ；LICENSE — https://github.com/dengwirda/jigsaw
- MPAS-Tools 网格创建/转换文档 — https://mpas-dev.github.io/MPAS-Tools/ ；compass RRM 教程 — https://mpas-dev.github.io/compass/latest/tutorials/dev_add_rrm.html
- E3SM 网格表 — https://e3sm.atlassian.net/wiki/spaces/DOC/pages/3310649615 ；变分辨率设计 — https://e3sm.org/variable-resolution-mesh-design/
- Roberts/Pringle/Westerink 2019, OceanMesh2D, GMD 12:1847 — https://gmd.copernicus.org/articles/12/1847/2019/
- Persson 2004/2006 梯度限制 — https://persson.berkeley.edu/pub/persson04gradlim.pdf ；DistMesh — https://persson.berkeley.edu/distmesh/persson04mesh.pdf
- Ringler/Ju/Gunzburger 2008, Ocean Dynamics 58:475；Jacobsen et al. 2013, GMD 6:1353；Yang/Gunzburger/Ju, arXiv:1709.06924；Du/Emelianenko/Ju 2006, SINUM 44:102
- Peixoto & Barros 2013, JCP 237:61；Peixoto 2016, JCP 310:127；Hoch et al. 2020, JAMES — doi:10.1029/2019MS001848
- Sein et al. 2016/2017, JAMES（AWI EPIC 全文）；Danilov & Wang 2015, Ocean Modelling 93:75
- Liao et al. 2025, JAMES — doi:10.1029/2024MS004737；Engwirda & Liao 2021, IMR29 — doi:10.5281/zenodo.5558988
- Fan et al. 2024, GRL — doi:10.1029/2023GL107059（EarthMesh）
- Zängl/Reinert/Prill, ICON v2.6.4 grid refinement, GMD；Zarzycki & Jablonowski 2014, MWR（台风 RRM）
- Marsh et al. 2018, Mesher, Comput. Geosci. 119:49；TINerator — https://github.com/lanl/tinerator
- geogram（BSD-3）— https://github.com/BrunoLevy/geogram ；spade/robust — crates.io
