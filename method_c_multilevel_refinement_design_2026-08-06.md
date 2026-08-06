# EarthMesh v3 多级细化重构设计：嵌套编译器与事务化增量细化

日期：2026-08-06
状态：设计提案（未实现）
基线：v3.0.0-alpha3（4dfbdca），生产细化路径 = Method-C（`spawn_nest` 谱系）
问题：多级细化在多数构型下失败，只有单级稳定可用。
目标：在保留 Method-C 内核的前提下，同时满足七条需求——阈值触发自动细化（土地利用/地形/LAI/土壤/海洋动力）；指定点、半径、走廊、闭合区域细化；连续多次局部细化；未变化区域保持原坐标、原单元 id、原连接关系；新增单元具有父子谱系并可生成局地守恒重映射；同时输出球面 Delaunay 三角网与 Voronoi 多边形网；兼容 MPAS、FVCOM、CoLM 与 EarthMesh 既有质量检查、掩膜和耦合流程。

---

## 一、结论（先说答案）

多级细化不稳不是若干孤立 bug，是结构性失配。Method-C 的嵌套合法性是**格点算术性质**——边界须落在五边形锚定的 thirdm（隔二取三）子格上、成 3 的倍数直段、种子行走全程同代；而现有全部多级入口（半径阶梯、h 场栅格、掩膜退火、eroded parent retry）都在**度量空间**里逼近这个格点条件。度量到格点的映射不连续，所以"再调一点参数"永远存在失败窝。`ladder.rs` 实测合法域**非上闭**（halo 1.5 行通过、2.0 失败、2.5/3.0 通过）是这一判断的直接证据：净距更大反而失败，说明失败根源不是净距不足，是对齐算术。

方案分三层，不更换 1→4 细分 + 3 过渡行内核：

1. **嵌套编译器**（新）：需求统一落为当前网格 W 面上的整数目标层场，在格点空间做洋葱化、格点覆盖、连通修复，产出按构造合法的逐层面掩膜；内核新增 `spawn_nest_from_face_masks` 直接消费。合法性从"运行时碰运气"变为"编译期保证"。
2. **事务语义**（新）：细化定义为幂等操作 `refine_to(mesh, L_target)`，id 只追加、旧点冻结、受影响集之外逐位不变；破坏性掩膜裁剪移到导出视图。这一层承担"连续多次局部细化"与"未变化区域完全保持"。
3. **谱系与守恒重映射**（补全）：血缘表补齐 U 边与过渡面两侧，受影响集内用现有 Lambert 等积求交机器产出一阶守恒权重，跨事务稀疏复合。

OLAM `spawn_nest` 的设计域是网格生成期少量手工同心嵌套——这也是"同心圆阶梯 + 最细层统一块心"是目前唯一稳定多级构型的原因。v3 把任意数据驱动需求塞进这门语言而翻译层缺位，失败就以下节五种形态在内核深处出现。修法是补上翻译层，不是继续在内核里加特判。

---

## 二、失败机理：五类根源与证据

| # | 类别 | 机理 | 证据 |
|---|---|---|---|
| 1 | 同代行走 | `selected_region_thirdm_seed_points_with_neighbors` 对每个弹出种子检查全部邻边 `mrlu == mrlo`，碰到跨代边即报错。子层需求必须离父层补丁边界足够远，且"远"以格点行数计 | `method_c_selection/mod.rs`（"crosses the parent boundary"，`NonTripletPerimeter`）；实测 NXP 21 外 300 km + 内 150 km 失败、外 1200 km + 内 150 km 通过（`tests/point_radius_coastal_demand.rs`） |
| 2 | 周界合法性 | 边界须为 3 的倍数长的粗边直段、3 过渡行跨 2 粗行、顶点度 {5,6,7}。锯齿或碎片掩膜要到周界/补丁代码深处才暴露 | "transition patch has no solid split edge"（`method_c_table_helpers`、`method_c_patch`）；"exceeds 7-edge ring"（周界行走器） |
| 3 | 合法域非上闭 | 同一构型 halo 1.5 行通过、2.0 失败、2.5/3.0 通过；`MEASURED_PARENT_HALO_ROWS = 3.0` 注明 "measured rather than argued"。合法性是边界落点相对锚定子格的对齐算术，不是净距的单调函数 | `refinement_demand/ladder.rs` 模块注释与 18 构型 sweep。**这一条在数学上宣判了"调半径、调 halo、调栅格"路线不可能调稳** |
| 4 | 栅格窗口 | 默认 720×360 欠采样出锯齿掩膜；过细则需求碎成小于一个 rad3 足迹的斑点而静默少细化（曾 exit 0 少交付 82%）；可用窗口 h_base/间距 ≈ 7–12 至今找不到决定变量 | technical guide §8。guide 自己点破根因：分辨率相关判据被绑在与网格无关的栅格上，本来就没有正确答案 |
| 5 | 修复链单调破坏 + 身份断裂 | annealing（≤32 轮，全消或全保即停）与 eroded parent retry 都在削需求；同级多子区域硬错 "multiple child regions require explicit parent-level halo"；细化之后的破坏性掩膜清理与重索引打断 cell id，连续两刀之间没有稳定 parent | `spawn_nest_internal/mod.rs` 的 fallback 链；guide §9 "第二遍必须以第一遍被测量的精确 production gridfile 作为 parent" |

反面对照也要记录：几何区域路径从不构造栅格，同心五级阶梯（NXP 21，半径 2000/1200/700/400/200 km）稳定跑到 12 km、1.1 s、面数 +96%。**内核本身没有问题，问题全部出在"需求 → 合法嵌套"的翻译上。**

---

## 三、总体架构

```
判据(现有 14 项 + getref_mean_std 逐单元移植) ┐
显式区域(点/半径/走廊/闭合域/shapefile)        ├→ 目标层场 L(face) ∈ {0..5}
既有网格层级(max 合并, 幂等基础)               ┘        ↓
                              ┌── 嵌套编译器（格点空间） ──┐
                              │ ①洋葱化  ②格点覆盖  ③连通修复 │→ 逐层 Vec<bool> + 对账报告
                              └──────────────┬──────────────┘
                     spawn_nest_from_face_masks（内核新入口）
                      逐 pass: emit(1→4 + 过渡) → nest spring(仅本代)
                                             ↓
              RefineTransaction{ mesh', 受影响集, id 区间, 谱系Δ, 守恒权重 }
                                             ↓
         规范网格(未裁·全球, 下一事务的 parent) ──导出视图──> mask/cull + writers
                                                              (MPAS / FVCOM / CoLM)
```

半径阶梯、annealing、eroded retry 全部降级为兼容路径保留，不再是多级的承重墙。h 场按 guide 既有称呼正式归入 legacy。

---

## 四、第 1 层：嵌套编译器（格点空间合法化）

### 4.1 需求归一：目标层场

目标层场定义在**当前网格**的活跃 W 面上，三类来源合并取 max：

- **阈值判据**。分辨率无关判据（`sst > 28`、`slope > 15`、离岸距离）一次求值缓存；分辨率相关判据（landcover 异质度、子网格方差、`getref_mean_std` 系）逐层在真实单元上重算——`spawn_nest_adaptive` 的逐层循环已经在，缺的只是把"等尺度方块近似"换成真实单元求值（§九 P2）。
- **显式区域**。点/半径、走廊（河流）、闭合曲线、shapefile、bbox 全部现存；它们是指令不是判据，直接置层，够不到即报错（既有规则保持）。
- **既有网格层级**。`mrlw − 1` 参与 max，是幂等语义的基础（§5.1）。

### 4.2 洋葱化：净距约束（图上整数梯度限制）

记 D_j 为"需要 ≥ j 级"的面集合。自深向浅构造每 pass 的支撑集：

```
S_k = ⋃_{j ≥ k} dilate^(j−k)·W ( D_j )        （dilate 在该级面邻接图上，按行数）
```

保证任意 k+1 级补丁严格套在 k 级补丁内部且净距 ≥ W 行。这是 Persson 梯度限制的整数图上版本：|ΔL| ≤ 1 每 W 行。W 的构造下界由三项相加——种子内部性 1 子环（同代行走要求）+ rad3 足迹半径 3 子面 + 过渡 3 子行跨 2 粗行——折合父层约 4 行，与 `ladder.rs` 实测的 3.0 行/级同数量级。**最终取值不靠推导，由 §八的性质测试钉死**（非上闭的教训：不能"取大保平安"），surface（max_mrows=7）与 atmosphere（13）分别标定。

### 4.3 格点覆盖：对齐约束（关键步）

每层掩膜改写为该层 thirdm 子格种子的 rad3 足迹并集：

```
seeds_k = { p ∈ Lattice_k : footprint_rad3(p) ∩ S_k ≠ ∅ }
mask_k  = ⋃_{p ∈ seeds_k} footprint_rad3(p)
```

Lattice_k 由现有 selection 行走器**反向使用**枚举：现在是"区域包含判定驱动行走"，编译器改为"掩膜相交判定驱动行走"，五边形锚定规则（含五边形的组件必须以该五边形为行走起点）原样内建。由构造直接获得三条性质：边界自动为 mod-3 直段；种子内部性由 4.2 的净距保证；不存在任何"度量半径吸附到格点"的步骤——第二节第 3 条的非上闭失效模式整体消失。覆盖只增不减（Grow-only）：需求零丢失，对账报告记录每层扩张了多少面、把哪些需求搬到了哪里。

### 4.4 连通修复

小于一个足迹的组件并入邻近组件或升到父层；两组件间距小于分离下限（周界行走器可分辨的最小间隔）则合并。全部操作确定性（BTreeSet 序、最小 canonical id 破平局，沿用全库约定）。同级多窝限制随之在 API 层取消：编译器逐组件发父补丁，`spawn_nest_internal` 里那条硬错误退役。

### 4.5 与 pass 循环的交织；deferred 语义

Lattice_k 只在 pass k−1 完成后存在，所以编译器逐 pass 运行，与 `spawn_nest_internal` 既有的 per-pass 结构同构：

```
pass k:  在当前网格上重估 D_j (∀ j ≥ k；深层判据用等尺度块预估)
         → 洋葱化得 S_k → 格点覆盖得 mask_k → spawn pass k → 记录事务增量
```

深层需求在细网格上重估后可能溢出预留围裙（细单元上发现了粗尺度看不见的异质性）。处理规则：裁到合法支撑，**记入对账报告的 deferred 项**，绝不静默丢弃；因为事务幂等（§5.1），补一刀廉价——下一次 `refine_to` 先扩父层围裙再下去。这是诚实处理，与现有"逐层物化检查 + 请求-结果对账"两道网的精神一致，只是从"事后报错"前移为"事前分类"。

### 4.6 内核入口与旧路径处置

```rust
pub struct CompiledNest {
    /// 每 pass 一张与 selected_regions_faces 同形的 W 面掩膜
    pub per_pass_masks: Vec<Vec<bool>>,
    pub report: NestCompileReport,   // 扩张量、搬移、deferred，全部可机读
}

impl MethodCDelaunayMesh {
    pub fn spawn_nest_from_face_masks(
        &self,
        plan: &CompiledNest,
        opts: &NestOptions,          // max_mrows、spring(nxp,niter)、cartesian
    ) -> io::Result<RefineTransaction>;
}
```

消费点就是 `spawn_nest_pass_*` 已经在吃的 `selected: Vec<bool>`，绕开圆链、栅格与 `level_at` 点采样。selection 里的跨代检查在此路径降级为 `debug_assert`：触发即编译器缺陷，不再是用户配置问题。`next_grid_number = max(ngr)+1` 的代数簿记原样沿用。

---

## 五、第 2 层：事务语义（连续多次局部细化）

### 5.1 refine_to 与四条合同

细化定义为幂等函数：`refine_to(mesh, L_target) → RefineTransaction`，其中 `L_target = max(网格现有层级, 新需求)`，只对 diff 动作。于是"在别处再开新窝""把旧窝再加深一级""同一需求重跑"是同一个操作的三种输入。四条合同各配位级回归测试：

1. **append-only id**。新 M/U/W 行只追加；被替换父面保留占位行（现约定连通行 `[1,1,1]`），事务之间不压缩。重索引只发生在导出视图，并落一张 old→new 映射文件随产物走。
2. **冻结**。nest spring 可动点 = 本代（ngr）中邻接 `mrow ≠ 0` 面的 M 点——现状已近似如此（guide §4.4），写死为合同，`move_interior` 默认 false。所有 `ngr <` 本代的点坐标逐位不变。
3. **受影响集显式化**。分裂面 ∪ 过渡补丁 ∪ 弹簧 stencil，随事务输出。集外坐标、连接、元数据逐位不变，可直接 byte-diff 断言。
4. **幂等**。空需求 → 输出与输入逐位恒等；同 L_target 二跑 → 第二次 no-op。这两条测试能抓住绝大多数多 pass 状态泄漏。

### 5.2 规范网格与导出视图

破坏性掩膜清理（最大连通水体、扇区剪枝）与重索引移入 writer 阶段作为**视图**——MPAS 生态里 `MpasCellCuller` 所处的位置。`cases/<case>/canonical/` 保存未裁全球网格，作为下一事务唯一合法 parent，把 guide §9 的教训（第二遍必须以第一遍精确 gridfile 为 parent）从纪律升级为制度。PCVT 与外心重算限制在受影响集内，旧 Voronoi 顶点位稳。

### 5.3 诚实边界

过渡环上冻结生成点的 Voronoi 胞**形状会变**——它们的邻居变了。这不可避免，也正是守恒权重矩阵要记录的内容；"未变化区域"的严格定义是"其 Delaunay 邻域全部冻结的胞"，这些胞位级不变。文档与对账报告按此口径表述，不做更强的承诺。

---

## 六、第 3 层：谱系与局地守恒重映射

**谱系补全**。W 子面 → 父面、M 中点 → 父 U 边两端已在（新建行血缘为 0 的缺陷 2026-08 已修，回归测试 `tests/method_c_lineage.rs`）；补齐新 U 边 → 父边/父面、过渡面 → 父面。占位行 1 指向自身的既定约定不动。每行带 `birth_ngr`。

**守恒权重**。hex 视角下新胞面积由父胞局部割出，受影响集内逐胞计算：

- 求交机器照搬 `hydro_delivery_intersections` 的结构：逐胞 Lambert 等积平面、大圆弧 0.1° 加密、同类先 union 再按胞球面面积归一——这套已经在生产里证明了守恒性与日界线安全。
- `w_ij = A(child_i ∩ parent_j) / A(child_i)`，行和 = 1（容差 1e-12）；受影响集外恒等映射，不出现在稀疏矩阵里。
- 每事务写 `remap_gen{g}.nc`（CSR：child_id, n_parents, parent_ids, weights + 逐补丁面积守恒校验和）；多次细化的总权重 = 各事务稀疏矩阵按序复合。
- 用途：CoLM `restart_expand` 状态搬运、耦合器一阶守恒插值；二阶精度留待在权重上叠梯度重构，不进本期。

---

## 七、七条需求对照

| 需求 | 满足于 | 备注 |
|---|---|---|
| 阈值触发自动细化 | §4.1 + P2 | 分辨率相关判据逐层在真实单元求值，取代等尺度块近似 |
| 点/半径/走廊/闭合区域 | §4.1 | 选择器现存，改为落层场；走廊仍是河流的形状，圆链仍是海岸的形状 |
| 连续多次局部细化 | §5.1 | refine_to 幂等 + canonical parent；加深与开新窝同构 |
| 未变化区域保持坐标/id/连接 | §5.1–5.3 | 四条合同 + byte-diff 回归；口径见 §5.3 |
| 父子谱系 + 局地守恒重映射 | §6 | 谱系已大半在场，权重复用水文求交机器 |
| Delaunay + Voronoi 双输出 | 现存 | tri/hex 双视角与 PCVT 不动；仅限定受影响集重算 |
| MPAS/FVCOM/CoLM 兼容 | §5.2 + 现存 | validate_topology、质量门、writers 不动；破坏性清理移到导出视图 |

---

## 八、验证清单

| 测试 | 断言 |
|---|---|
| no-op 恒等 | 空需求事务输出与输入逐位相同 |
| 幂等 | 同 L_target 连跑两次，第二次零改动 |
| 冻结区 | 受影响集外坐标/连接/id byte-diff 为空 |
| 洋葱不变量 | ∀ k 级面，其 W 行邻域内层级 ≥ k−1（并入 validate_topology） |
| 谱系闭合 | 每行祖先存在；Σw = 1（1e-12）；逐补丁面积守恒 |
| 覆盖零丢失 | demanded ⊆ realized ∪ deferred，deferred 显式可机读 |
| 随机 sweep | 把 `ladder.rs` 的 18 构型 sweep 换成随机需求性质测试（层数 2–5、NXP ∈ {21,40,81}、含碎片/走廊/跨五边形/双窝），期望全绿——合法性按构造成立后，该测试从"标定"变成"守恒律" |
| 确定性 | 1/2/4 线程逐位一致（沿用既有模式） |

随机 sweep 是本设计的成败判据：若它做不到全绿，说明 W 标定或格点覆盖还有漏，回到 §4.2/4.3 修，而不是加运行时 fallback。

---

## 九、落地计划

- **P0（数周）**：`TargetLevelField` + 洋葱化 + 格点覆盖（selection 行走器反向化）+ `spawn_nest_from_face_masks` + 随机性质测试；W 对 surface/atmos 分别标定。落点 `earthmesh_refine_planner`（现在只有 lib.rs，正好做编译器的家）与 `earthmesh_mesh` 各一半。
- **P1**：事务合同四件套 + 位级回归 + canonical 未裁网格与导出视图重排。
- **P2**：守恒权重 + 谱系补全 + `getref_mean_std` 移植为逐单元判据（landtype 瓦片缓存已证明访问模式可行；必要时按受影响集增量求值）。
- **P3**：h 场标记 legacy（guide 已如此称呼）；ladder/annealing/eroded retry 归档为兼容路径；文档与 GUI 三选一改为"编译器（默认）/ 点+半径（兼容）/ 离散区域（兼容）"。

---

## 十、风险与开放问题

- **W 只能实测钉死**。非上闭的教训是"不能取大保平安"；标定矩阵要覆盖 NXP、层深、五边形距离三个轴。
- **deferred 频率未知**。若细网格上判据重估经常溢出围裙，就在 D_j 预估时对分辨率相关判据加保守膨胀，代价是多细化一圈。
- **五边形近旁深层补丁**。`march_from_nearby_pentagon` 分支在掩膜路径下的等价规则需要显式写出：含五边形的组件其种子集必含该五边形。
- **既有工程输出会变**。旧多级网格建立在欠采样栅格或半径阶梯上，与 h 场栅格修复同类代价（当时实测 ±0.3% 量级）；需要一次对照运行给出本次的量级。
- **对拍资产迁移**。compat 左支（`extends/earthmesh_grid_preprocess`）不受影响；Method-C 参考对拍（nmd/nud/nwd、逐级 W 面数、mrow 包络）在掩膜路径下按"同输入同输出"迁移，输入从区域换成掩膜。
- **本设计不做的事**：不换生成内核（JIGSAW/密度场路线维持 2026-07-02 报告的阶段 2 定位）；不做粗化（coarsen）——p4est 语义里有它，本期需求没有，但 append-only id 与谱系表为将来留了位。

---

## 主要参考

- Burstedde, Wilcox & Ghattas 2011, *p4est*, SIAM J. Sci. Comput. 33:1103 —— 2:1 平衡森林、Morton 唯一 id、可无限重复 refine/coarsen：事务语义的对标物。本设计即其语义嫁接到二十面体菱形森林 + Method-C 过渡。
- Zängl, Reinert & Prill, ICON grid refinement, GMD —— 补丁式嵌套、父子唯一映射、逐层 0.5× 时间步：保留离散嵌套内核的最好论据。
- Walko & Avissar 2011, OLAM, MWR 139:4045 —— `spawn_nest` 出处；其设计域即少量手工同心嵌套，解释了现状。
- Rivara 1984（最长边二分）；Bank 红绿细化 —— 若将来换通用内核的候选；"green 不得直接再分"对应"过渡行面不得直接选为子层"，编译器洋葱化已内建该规则。
- Persson 2004/2006 —— 梯度限制数学；本设计改为面邻接图上的整数版（|ΔL| ≤ 1 每 W 行）。
- Engwirda 2017 (JIGSAW-GEO, GMD 10:2117)；Roberts et al. 2019 (OceanMesh2D, GMD 12:1847)；FESOM 判据系 —— 连续密度场范式：收其数学，弃其栅格。
- TempestRemap / SCRIP / YAC —— 守恒权重的格式与验证参照。
- 本仓库：`docs/mesh_refinement_method_research_2026-07-02.md`；`docs/mesh_construction_technical_guide.md` §4/§8/§9；`rust/earthmesh_cli/src/refinement_demand/ladder.rs`；`rust/earthmesh_mesh/src/method_c_selection/mod.rs`、`method_c_spawn_internal/mod.rs`、`method_c_emit/mod.rs`。
