# CMRC 算法全链条缺陷审计（2026-10-27）

**审计对象**：`/Users/zhongwangwei/Desktop/Github/EarthMesh`，分支 `v3.0.0-alpha12`，HEAD `018ffc3e`。
CMRC = Certified Mother-grid Reverse Coarsening（`refine_backend = 'certified'`）：
认证母网格 → 需求层级量化 → 受约束反向粗化（部件事务/弹性/拓扑搜索）→ 几何/物理/remap/最终单元认证 → gridfile/remap/证书/清单/模型发布，
另含合并判据（`&certified_merge`）与可选局部更新（`EARTHMESH_CMRC_LOCAL_UPDATE`）两条需求/变更路径。

**方法**：1 个主审 + 6 个并行独立审计线程（需求/配置与母网格族、粗化事务与回滚、几何/拓扑/弹性、证书层、
remap/交付/区域发布、合并判据与局部更新），全部只读；主审另做了 1 次真实的端到端 A3 算例运行与独立核对。
候选缺陷要求“可复现命令 + 输出”或“严格代码路径证明”，否则降级为未验证风险。
本报告不重复 `docs/bug_audit_20261026.md`（A1–A25）与指南 §11 已记录/已修项，只报新发现或“文档宣称已修而代码没修/修错”。

**基线证据**：
- `cargo test -p earthmesh_refine_certified --lib`：**336 passed / 0 failed / 7 ignored**（633 s，共享 target，高负载）。
- 真实算例 `examples/cmrc_atmosphere_a3.nml`（NXP64，2 级，38–82 契约）：debug CLI 56.3 s，交付 **63,810 个六边形单元 / 127,616 个三角形**；
  证书、gridfile、remap、MPAS 与独立 `--mesh-quality` 交叉核对全部自洽（见 §5/§7）。**没有发现交付网格本身错误。**

## 一、结论摘要

- **未发现 Critical / High 缺陷**；未发现新的“合法、通过质量检查、但不是被请求结果”的**已交付网格**错误路径。
- **确认 4 项 Medium、12 项 Low**（其中数项为当前生产不可达的 API/防御缺口）。最值得处理的是：
  1. **A1（Medium，算法数值）**：弹性能量的 Delaunay 软罚项 `1000·side²` 中 `side` 是单位球四点的 orient3d（O(h⁴)），
     产尺度（30–150 m 单元）罚项 1e-38…1e-42、梯度 ~1e-31…1e-34，远低于解算器 1e-14 停止阈值 → 该“几何修复”阶段在产尺度失活，
     可修复状态会被误报 `RequiresDifferentTopology`，触发不必要的拓扑提升/重试甚至整次运行失败（不产出错误网格）。
  2. **B1（Medium，fail-closed 误拒）**：`reverse_coarsening`/移动母网格模式下，小于需求栅格像元的命名区域必被
     `CriterionNotCertifiable` 拒绝，与需求合成的保守下界（`conservative_global_bound`）及指南 §11.100 的承诺矛盾；已复现
     （1 km 圆 exit 2，平移到像元中心 exit 0；regional 同理）。
  3. **C1（Medium，静默空判据）**：合并判据只有“所有层都读不到样本”才报错；**单层**读到 0 样本时该层判据恒通过，
     运行成功、按剩余判据合并到底——请求未被履行但退出码 0。
  4. **D2（Medium，潜在空过）**：`verify_geometry_within` 在 `edge_sites` 覆盖全部活跃站点时整体空过（开边/度数/Delaunay/对偶全豁免，
     `voronoi_cells=0`，`charge` 只记录不门禁）；已用探针复现，CLI 仅在极小区域/NXP=1 类极端配置可达。
- **已知限制的定量复核**：指南 §11.110 记录的“边交叉阈值 1e-18”在本轮被量化——产尺度交叉强度 O(h⁴)（90 m 约 2.5e-21），
  该阈值**在所有 ≤2 km 单元上永不触发**，因此 `provably_untangled` 失败时的回退扫描在产尺度同样失活；且最终几何证书没有嵌入性检查。
  这解释了为什么交付自交网格未被发现，但没有证明产线上出现过（本轮 A3 未触发）。
- 端到端 A3 运行显示主链是**自洽且 fail-closed** 的：层级、remap、证书、MPAS 交付逐项独立核对通过（§5）。

## 二、确认缺陷

严重度定义：**Critical/High** = 生产可达的错误网格/证书/层级；**Medium** = 生产可达的静默错误元数据/诊断、
可复现的合法配置误拒、或算法能力在产尺度失效；**Low** = 潜在/API/防御缺口、误导性诊断、成本问题。

### A1（Medium）弹性能量的 Delaunay 软罚项在产尺度数值失活

| 项 | 内容 |
|---|---|
| 位置 | `rust/earthmesh_refine_certified/src/coarsen/elastic_block.rs:3648-3662`（`dual_energy_in` 的连续 Delaunay 项）、`:3355-3361`（`energy += 1000·violation`）、`:1627`（梯度停止阈值 1e-14）、`:1691`（接受阈值 `1e-12·max(|E|,1)`）、`:2745-2766`（阶段判定）、`:1534`（`no_step` 归因 `RequiresDifferentTopology`） |
| 机制 | `side = dot(a-d, cross(b-d, c-d))` 是四个**单位球面点**的 orient3d（近似共面 ⇒ O(h⁴)，h 为单元角尺寸）；`violation += (-side).max(0)²` ⇒ 罚项 O(h⁸)。同函数中的 `center`（`acos²`）、`area`（`ln²`）、角度/边长项全是无量纲 O(1) 量，只有 Delaunay 项漏了尺度归一化。产尺度 h=`cell_m/R⊕`≈4.7e-6…2.4e-5，罚项 1e-42…1e-36，对梯度贡献 ~O(h⁷)≈1e-34…1e-30，低于停止阈值且远小于其它项。因此 `DelaunayVoronoiFeasibility` 阶段对 Delaunay 违例没有有效梯度；`no_step` 因 `final_phase==DelaunayVoronoiFeasibility` 把“能量对该违例无梯度”记成“需要不同拓扑”，触发拓扑提升/重试；焦点预算耗尽时可升级为整次失败。最终 `verify_geometry_region` 用精确 `in_circle` 复检，所以**不会交付错误网格**。 |
| 证据 | ① 上列逐行代码路径；② 主审独立用 python3 复现同一公式（`verification/t1_numeric_evidence.txt`）：30 m 罚项 2.9e-42、90 m 1.9e-38、150 m 1.1e-36；同尺度交叉强度/阈值见下文已知限制；③ 对照 n=12 fixture（h≈0.18）罚项 ~1e-5，测试只在粗尺度上“活”。 |
| 影响 | 产尺度上“Delaunay/Voronoi 几何修复”能力失效，代价是额外的拓扑状态/弹性迭代/失败升级；不改变交付网格正确性。触发该阶段需要“角度窗已通过但精确 `in_circle` 仍失败”的边（通常是与固定邻面的边、或 Voronoi 非凸/度数违例），真实数据上的触发频率未统计（本轮 A3 两组件均一次通过，未触发）。 |
| 修法 | 把罚项无量纲化：`side / 特征体积`（如两条边长的乘积或 `sin(h/2)^4`），或直接用球面 Delaunay 等价的“对角和 −π”正部；权重与接受阈值按同一尺度重标定（对照 §11.110 的相对 FD 步长先例）。补一个“产尺度（h≤1e-4）下可修的 Delaunay 违例确实被修复”的回归。 |

### B1（Medium）亚像元命名区域在 reverse/moved 模式下必被误拒

| 项 | 内容 |
|---|---|
| 位置 | 守卫：`rust/earthmesh_cli/src/refine_pipeline/certified_pipeline.rs:766-787`；保守下界：同文件 `:2378-2400`（`raise` + `conservative_global_bound`）；文档承诺：指南 §11.100“落在采样中心之间的小区域仍保留声明的层级” |
| 机制 | 守卫在 `options.mode != SafeMotherOnly` 时逐命名区域要求“需求栅格里存在一个像元中心落在区域内部且 `effective_level ≥ region.level`”。亚像元区域按定义不含像元中心 ⇒ 恒不可满足 ⇒ 报 `CMRC CriterionNotCertifiable`。但需求合成恰好为这种区域计算了保守下界：全局域内 `/` 区域内所有样本被抬到请求级并置 `conservative_global_bound=true`；守卫完全不看该字段。于是同一配置 `safe_mother_only` 可交付，`reverse_coarsening`（生产默认模式）必失败。 |
| 证据 | 复现（debug CLI 3.0.0-alpha12，NXP=3，720×360 栅格）：`/tmp/cmrc_repro_a` 单个 1 km 圆 reverse → `exit 2 CMRC CriterionNotCertifiable`；同圆平移到像元中心 `/tmp/cmrc_repro_c` → exit 0；regional 版 bbox ±10° + 20 km 圆（level 2）→ exit 2，100 km 圆 → exit 0。日志：`verification/f1_subraster_global_rejected.log`、`f1_subraster_center_ok.log`。 |
| 影响 | 合法配置被 fail-closed 误拒（可用性缺陷）；用户按文档使用小区域（1–20 km）时无法用 CMRC 默认模式。不会交付错误网格。 |
| 修法 | 守卫改为校验“该区域声明的层级确实体现在 `required_levels` 中”（例如区域覆盖的样本最大值 ≥ requested，或 `conservative_global_bound` 时跳过），而不是“区域内部必须有像元中心”；安全母网格/移动母网格模式共用同一判据。 |

### C1（Medium）合并判据单层 0 样本时静默失效

| 项 | 内容 |
|---|---|
| 位置 | `rust/earthmesh_cli/src/refine_pipeline/certified_merge.rs:147-153`（只有 `samples.iter().all(Vec::is_empty)` 才报错）；`rust/earthmesh_refine_certified/src/requirement/heterogeneity.rs:209-214`（`face.counts[layer] < minimum_samples` ⇒ 该判据返回 true） |
| 机制 | 每个数据层单独读取并汇总全区域样本。若某一层读到 0 个样本（瓦片目录无匹配瓦片、数据不覆盖区域、全为缺测），它不会触发错误；父面可成形性判定对“样本不足”一律放行，于是该层上的**所有判据恒通过**，网格按剩余判据合并到底。`merge` 记录里只留下 `samples_read: 0`。 |
| 证据 | 严格代码路径证明（上列行号）；`heterogeneity` 的“样本少于下限不构成反对证据”是 §11.111 的设计语义，但“整层 0 样本”是数据/配置错误，不应等同“无反对证据”。可复现：配置两条判据，其中一条指向域外瓦片目录或全缺测文件，运行成功且证书 `layers[].samples_read` 为 0。 |
| 影响 | 请求的合并判据未被执行但退出码 0；网格合法、合并范围比请求更粗——§11.1“合法但不是请求结果”类。 |
| 修法 | 逐层检查 `samples_read == 0` ⇒ `InvalidInput`（点名层/变量/窗口）；若确需“该层在域内无数据”的合法用法，加显式开关并把判据标为 disabled 写入证书。 |

### D2（Medium，潜在）区域几何证书在 `edge_sites` 覆盖全部站点时整体空过

| 项 | 内容 |
|---|---|
| 位置 | `rust/earthmesh_refine_certified/src/certificate/mod.rs:215-287`（`verify_geometry_within`） |
| 机制 | 当 `edge_sites ⊇ 全部活跃站点`：开边检查只数“两端不都在 edge_sites 的开放边”（0 条）；`bad_degrees` 被全体 retain 掉；Delaunay 只查“内部站点参与的边”而集合为空；`seeds` 为空使 `verify_dual_sites` 迭代 0 次（`voronoi_cells=0`）；`charge` 只记录不门禁。于是任意合法三角形（甚至单三角形、Euler=1、charge=15）都能通过“区域几何证书”。 |
| 证据 | 探针（`.omx/artifacts/cmrc-audit-20261027/scratch/degenerate_region`，输出见 §7）：单三角形 + 3 站点全为边界 + `expected_euler=1` → `ALL-EDGE ACCEPTED: voronoi_cells=0 … degree_bad=0`；留 1 个内部站点时正确拒绝 `mesh has 2 open directed edges`。 |
| 影响 | 公共证书 API 可被退化输入空过；CLI 生产路径只有在“构建出的区域网格所有站点都在外边界上”时才可达（如 NXP=1 + 极小交付域这类极端配置），本轮未构造出现网命中。属潜在门禁缺口。 |
| 修法 | `verify_geometry_within` 要求 `!seeds.is_empty()`（至少一个内部站点/单元），并要求 `voronoi_cells` 与区域发布单元数一致；`charge` 与 Euler 的关系按开放区域特征数校验或显式标注“仅记录”。 |

### D1（Low-Medium）区域证书的 delivered 层级统计包含无单元的外边界站点

| 项 | 内容 |
|---|---|
| 位置 | `rust/earthmesh_refine_certified/src/construction/mod.rs:1228-1236`（对**全部** active 站点压 `delivered_levels`，含 `outer_sites`）、`:1349`（`delivered_level` 取全站点最大）、`rust/earthmesh_cli/src/refine_pipeline/certified_pipeline.rs:868-871, 916-923, 1222-1223, 1251-1266`（fulfillment/证书字段） |
| 机制 | 区域构造的 delivered 层级向量长度 = active 站点数 = 有单元站点数 + 外边界无单元站点数；`AdaptivityFulfillmentReport` 对其做直方图，因此 `sum(delivered_histogram) = 已发布父网格 W 单元数 + outer_boundary_sites`（再相对 `region.certified_cells` 多出 F 圈未逐单元认证的单元）。同一证书的 `region` 块把外边界站点单独计为 `outer_boundary_sites`、把单元数记在 `certified_cells`，口径不一致。`mixed_levels_delivered` 也由该直方图决定（诊断/分类输入）。 |
| 证据 | 真实历史区域证书（`.omx/artifacts/cmrc-boundary-acceptance-20261019/*/new/certified_certificate.json`，`verification/d1_regional_histogram_evidence.txt`）：100km `sum=5,334,318` vs `certified_cells=5,333,072`、`outer=626`；heihe `4,684,911` vs `4,682,483`、`outer=1216`；vertex3/vertex5 同型。差值 = outer + F 圈未认证单元。 |
| 影响 | 不改交付网格/层级数组（gridfile 用 `region.published()` 过滤后的向量，已核正确）；证书/清单的计数口径不一致，误导读者与下游脚本。物理残差仍会拦截真正的欠细化，所以不会静默交付错层级。 |
| 修法 | 在区域路径用 `cell_sites` 过滤后统计 fulfillment/`delivered_level`（与 `RegionPublication::published` 同口径），或把字段改名为 `site_level_*` 并在证书中显式写口径。 |

### D3（Low，潜在）区域 remap 行覆盖检查单向；`finalize_region_geometry` 行数校验自证

| 项 | 内容 |
|---|---|
| 位置 | `construction/mod.rs:1309-1318`（只查“remap 行 → 已发布单元”）、`:1325-1328`；`rust/earthmesh_refine_certified/src/api.rs:166-183`（只比 `evidence.remap_rows` 与调用方传入的 `covered.len()`）；`certified_pipeline.rs:902-907` |
| 机制 | `finalize_region_geometry` 的行数来自与调用方**同一个** remap（`covered.len()`），几何证书的 `voronoi_cells` 从不参与校验（全局变体用 `geometry.voronoi_cells` 独立对比）。若将来 extent/R 规则变化导致某个已发布单元没有 remap 行，它会“无行发布”且所有门禁通过（`certified_cells` 相应偏小）。当前构造下交付域 ⊆ R，不可达。 |
| 证据 | 探针（`scratch/region_finalize_probe`）：0-Voronoi-cell 几何证书 + 3 行 remap → `REGION FINALIZED`。 |
| 修法 | 构造里断言 `certified_cells == cell_sites.count()`（或显式例外并写入证书）；`finalize_region_geometry` 增加“区域已发布单元数”参数与几何证书独立比对。 |

### 其余 Low（逐条）

| ID | 位置 | 问题 | 证据/影响 |
|---|---|---|---|
| **B2** | `rust/earthmesh_cli/src/cli_mkgrd_run/run.rs:81, 952-984`；`construction/mod.rs:185-186` | CMRC 母网格预算 = `min(NL%maximum_cells, --max-tris)`，而 CLI 的 `max_tris` 派生只按**基网格**（namelist：`20·NXP²` 与 100,000 取大）或 `max_passes`（project：`20·NXP²·4^max_passes`），不含 CMRC 的 `maximum_level`。于是 ≥level 1 的常规 NXP 会在合法配置下报 `CellBudgetInsufficient`，且 `NL%maximum_cells` 无法抬高它。 | **已复现**：`examples/cmrc_atmosphere_a3.nml` 按文档直接运行（NXP64、2 级）→ `exit 2 requires 1310720, budget is 100000`；加 `--max-tris 20000000` → `exit 0`，56.3 s，63,810 单元（`verification/c1_*.log`）。project 路径同因（`max_passes` ≠ `maximum_level` 时）仅代码证明。修法：certified 后端用 `maximum_cells`（或按 `NXP·2^maximum_level` 派生 budget），并让两个入口的预算语义一致。 |
| **B3** | `certified_pipeline.rs:2175-2205` | `region_reaches_domain` 对 Circle 只看圆心、Bbox 只看中点，忽略区域实际形状；与域确有重叠但圆心/中点在域外的区域被判“域外”并从需求中删除，警告理由不实（exit 0）。 | 已复现：bbox ±10°，圆心 lon=10.1、半径 20 km（西缘伸入域内）→ `warning: CMRC ignores 1 of 2 refinement region(s): they lie outside the regional domain`，`verification/f2_region_center_outside_dropped.log`。该例中域内无像元中心，故无实际需求损失；但判据错误，配合 B1 的亚像元语义会丢真实需求。修法：用区域的经纬包围盒/球冠与域求交，或直接复用 `region_reaches_domain` 的采样点集合判据。 |
| **C2** | `certified_merge.rs:239-257` vs `certified_pipeline.rs:710-727` | `--cmrc-merge-preview` 的准入比实际运行宽松（不校验 `refine=.true.`、`mode=reverse_coarsening`、需求源互斥）：预览成功、同一 namelist 实际运行被拒。记录字段本身无漂移。 | 代码路径证明；影响预先校验失效。修法：预览复用运行期准入函数。 |
| **C3** | `certified_merge.rs:103-122, 203-211`；`final_delivery.rs:255-262` | 合并判据层数据无 provenance 绑定（证书只存 namelist 原字符串；相对路径按进程 cwd 解析），也无输入别名保护（A2 的 local-update 做法只覆盖局部更新输入）。 | 审计追溯风险，不产生错误网格。修法：记录路径 + 实际读取字节 SHA-256，并复用 A2 的输入别名检查。 |
| **C4** | 合并判据 `levels` 校验（仅 `>12` 拒绝） | `levels=12` 时每基面 `Moments` 为 `4^12` 量级（约 2 GB），合法配置可 OOM（默认 `maximum_level=8` 安全）。 | 代码路径 + 量级估算；属资源守卫过松。修法：按内存预算收紧上限或提前估算拒绝。 |
| **C5** | `cmrc_local_updates.rs:470-487` | `read_tail_levels` 对 `source_gridfile` 的层级向量采取“行数多于 cells 就取尾段”，无 w 点坐标/顺序交叉核对。 | 防御缺口；当前 CLI 生成的 gridfile 行序稳定。修法：按 cell 坐标/层级哈希校验后再取尾段。 |
| **D4（已知限制的定量复核 + 防御缺口）** | `elastic_block.rs:2751-2754, 2986, 3349`；`full_polygon.rs:366-413`；`certificate/mod.rs`（`verify_geometry*`） | 指南 §11.110 已把“边交叉阈值 1e-18”记为已知绝对常数；本轮量化：`minor_arc_crossing_strength` 也是 O(h⁴)，90 m 交叉强度 2.5e-21、300 m 3.1e-19、2 km 6.1e-16 ⇒ **≤2 km 单元上永远进不了 `Untangle`**；且最终几何证书没有嵌入性/自交检查，`provably_untangled` 证明失败后的回退扫描在产尺度失活。产线是否出现过自交网格本轮未复现。 | 数值表见 `verification/t1_numeric_evidence.txt`。建议：交叉强度按边长归一化并把阈值/权重同尺度重标；或在提交阶段加入精确谓词级嵌入性证书。 |
| **D5** | `construction/mod.rs:133-143`（`RegionPublication::published`） | `levels.iter().zip(&cell_sites)` 长度不一致时静默截断，不报错。 | 防御缺口；两个调用点长度均已被 `TargetLevelField` 校验，生产不可达。改用 `assert`/`Result`。 |
| **E1** | `coarsen/mod.rs:1564-1575`；`construction/mod.rs:520-567` | 层级重建预算耗尽分支的 `snapshot = grid.clone(); snapshot == grid` **恒真**（clone 后无任何写），construction 的 `snapshot_unchanged` 守卫不可达；同时该分支把 `attempted_patches = search_budget`（实际一次补丁都没尝试）且 `components_exhausted=1 / promoted=全部` 自相矛盾地写进报告。 | 仅错误诊断路径，不影响网格/预算决策；测试 `rust/earthmesh_refine_certified/tests/certified.rs:298` 把恒真值当断言。修法：删除死自检或改成真正的快照指纹；修正计数字段。 |
| **E2** | `construction/mod.rs:1841-1846, 1917-1961`（局部更新候选） | `LocalUpdate` 是公共 API 回调，若改变了活跃站点集合（等长），`delivered_levels` 只经长度校验就按候选活跃顺序重关联，可能交付“几何/证书全过但 cell→level 元数据错位”的网格。CLI 的 `apply` 只 `move_vertex`，生产不可达。 | 代码路径证明。修法：候选路径先断言结构相等（顶点数/active 站点/三角形表），否则 `InvalidData` 回退 control。 |

## 三、未验证风险

| ID | 内容 |
|---|---|
| R1 | `provably_untangled` 只证明 guard 面内部映射单射，不证明其像不越过 patch 边界覆盖固定邻面；产尺度交叉扫描失活（D4）后，这是“无自交”唯一实质防线，未构造出反例。 |
| R2 | 拓扑/弹性区域还有一组绝对常数（`1e-12·max(|E|,1)` 接受、`1e-14` 梯度停止、`1e-10` Untangle det 下限、`1e-12` 方向守卫、`-1e-12` Voronoi 凸性松弛），fixture 尺度比产尺度粗 10²–10⁴ 倍；除 A1/D4 外未证明会造成错误，但“产尺度行为与 fixture 相同”缺少测试覆盖。 |
| R3 | `transition_topology::preflight` 要求母 `subdivision` 为偶数；生产 derivation 是否恒偶数未在本次范围核实（奇数会显式 `InvalidBoundary`，非静默错误）。 |
| R4 | 区域 tri 发布的 remap↔谱系逐行对照、regional+landtype 掩膜+MPAS 写出的数值级回归、`--cmrc-merge-preview`↔实际运行逐字段对照（除 C2 的准入口径）均无测试；区域 frame 单元在 Lattice 路径不参与最终需求（仅 pre-export parent，现网不构成交付）。 |
| R5 | 移动母网格交付层级可能超过 requested/`maximum_level` 的上界门（未在真实数据上复现）；`certified_mother_pentagons` 的顺序不是 icosahedron 顶点序（当前无读者）；`hfield.max_level` 与 `raise` 量化上限不一致。 |
| R6 | 局部更新（opt-in）仍缺真实多案例成本与“注入后晚期失败回滚”的真实数据回归（A23/A25 之外的剩余项）；本轮只做静态审计，未启用。 |

## 四、已核查干净（分链路，含验证方式）

1. **入口/配置/需求合成**（req 线程）：NXP→基面→级数换算单实现（`earthmesh_core::resolution` 被 project/CLI 共用；GUI JS 镜像公式与取整一致）；
   `CellBudgetInsufficient`/`maximum_level`/2 的幂边界无 off-by-one；`mother_grid` 生成/子父/区域地址有逐位 oracle 测试；
   零需求规则（`sourced && chosen_level==0 → safe_mother_only`）与 §11.98/11.100 一致；moved-mother 候选/预算/回退与 §11.87/101/102 一致；
   `on_demand` extent 环数公式与整网版本 oracle 一致；`&certified_merge`/`&adaptive`/regional 的 fail-closed 与 project 校验一致。
   验证：`cargo test -p earthmesh_refine_certified --lib -- mother_grid:: on_demand:: requirement::` → 32 passed / 1 ignored。
2. **粗化编排/事务/回滚/计数**（transaction 线程）：失败路径不改 state（仅成功臂 `work.commit` 写），window commit 唯一可失败步在 Work 副本先失败；
   级末/最终认证失败走 `Err` 不发布；需求覆盖（每 parent 恰属一组件，planner duplicate/missing 显式报错）；
   计数不变量 `total=committed+promoted`、`exhausted⊆promoted`；预算公平分与 retry/focus 循环终止；HashMap 用法顺序无关；
   A23/A24 门禁全路径（增量 remap 与全量逐位相等、`validate_final_delivery` 与 `finalize_*` 同判据，均有测试实证）。验证：3 个定向测试通过。
3. **几何/拓扑/弹性**（topology 线程）：生产可达核心 = `elastic_block`/`transition_topology`/`core_condensation`（其余 30+ 模块 research-only，已用调用图证明）；
   拓扑硬门禁（流形/重复面/度数 5–7/五边形/Euler/开放边位置/正定向）逐项可靠；门禁窗 38.4/81.6 ⊂ 证书窗 38.2/81.8 ⊂ 交付窗 38.0/82.0，
   不存在“门禁比证书宽”；角度区间阈值方向经核对为保守；能量与中心差分梯度同一函数、并行逐位相同；L-BFGS 递推正确；
   失败绝不当作候选接受；搜索→提交的 changed 面集与后续证书失败重试语义正确；Rayon 无竞争/顺序相关；质量缓存是未接线死 API（按内容签名失效）。
   验证：`coarsen::transition_topology` 26 passed；elastic 并行/契约/untangle 证明 5 passed。
4. **证书层**（certificate 线程）：角度区间算术经 40 位十进制高精度对拍确认保守（不存在“证明通过、浮点重算越界”）；
   契约窗与 `earthmesh_quality`(35–85) 方向正确；闭球空/退化网格被拒；Delaunay/Voronoi 逐站点/逐边无退化；
   完整 remap `rows==target_cells` 且 `row.target==index`；`from_final_cells`/`validate_final_delivery` 与 `finalize_*` 判据逐条一致、指纹 fail-closed；
   交付对象就是被认证 primal（published 双网格逐站点/逐面回校 ≤1e-12）。
   验证：两个 rustc 探针（链接 warm rlib）运行（见 §7）。
5. **remap/交付/区域发布**（delivery 线程）：alpha12 增量 remap 的合并语义**精确**（改动行与全量共用 `overlap_prepared` 同一路径 → 逐位相同；
   未改动行逐位复用已认证 base；`coverage_error` 取合并后 residual 最大值，精确等于全量 max；changed 集 = incident triangle 三顶点对应 cell，是恰好集合；
   结构变化返回 `None`；base 身份用 `joins(initial, control.primal())` 复核）；
   closed/open 两种 CMRC gridfile 的 placeholder、canonical id=row+1、remap target=canonical id−2、W/M 行序与 MPAS 宽度/context 压缩均核对一致；
   域裁片/掩膜经 `compact_mpas_context` 重新压缩并 validate；原子发布 staging/rollback 正确。
6. **合并判据/局部更新**（merge-local 线程）：alpha12 四个 local-update 提交（`9bad545d`/`dbd5552f`/`b30135a6`/`018ffc3e`）新增分支逐行核对，
   失败回退、no-op 复用（`MeshState` 手写 PartialEq 逐字段无容差）、changed_cells 推导、增量 remap 均成立；
   合并判据的数据读取（窗口分箱/0–360/全球栅格/MERIT 半像元/缺测/日界线/边缘像元）、四叉树统计（shift/minimum_samples/purity/递归/截断）、
   `finest_m→level`、preview↔证书记录同源（除 C2 准入口径）均核对干净。验证：`merge_layer_samples` 5 passed。
7. **端到端真实算例（主审）**：`examples/cmrc_atmosphere_a3.nml --max-tris 20000000`，debug CLI 56.3 s。
   独立核对全部通过（`verification/e2e_delivery_checks.txt`，`FAILURES: none`）：
   - gridfile W 层级直方图 = 证书 `delivered_histogram`（39,235/2,947/21,628，合计 63,810）；
   - M 层级逐行 = 三角形 W 层级的 max（127,616 行 0 失配）；cellwidth = 120/2^level km；
   - remap 1,025,213 行、63,810 个 target（与证书 vertices 一致）、权重和全部为 1（1e-12 内）、target ∈ [0,63809]；
     每层单元的 source 数中位数 25/9/1（约 4×/级，符合 dual 单元面积比）；
   - 证书：Euler=2、charge=12、开边/拓扑/度数/Delaunay/Voronoi/对偶/物理/平衡/remap 残差全 0，角度 38.267–81.706° 在 38–82 契约内；
   - 独立 `--mesh-quality`：verdict pass、26 门全 pass、角度与证书一致（38.26746559°/81.70610434°）、Euler 2、无自交/非法多边形；
   - MPAS：63,810 cells、面积和 = 4π、`meshDensity=(30/width)^4` 逐格成立、`nominalMinDc=29.999 km`。

## 五、测试与环境说明

- 本机 16 核、并行 6 线程审计 + cargo 构建，load average 一度 60+；`cargo test -p earthmesh_refine_certified --lib` 336 passed / 7 ignored / 633 s。
- 未运行：release 级 ignored/真实 MERIT/黑河/云南案例、`make test`/`test-fast`/`heavy` 全量、GUI 四闸、`make regression`（时间盒 + 外部数据缺失）。
  真实数据上的 A1/D4 触发频率与区域发布数值级回归仍需后续验证（R1–R6）。
- 所有审计线程只读；`git status` 显示本分支相对审计开始的 tracked 改动仍只有审计前已脏的 `.omx/metrics.json`、`.omx/state/update-check.json`，
  以及审计前已存在的未跟踪 PDF/`docs/redgreen_*`/`mer`/`target_lepp`；**审计阶段没有源码被修改、没有 commit**（后续修复批次见 §8/§9，提交在分支 `cmrc-elastic-waste-20261027`）。
- 复现脚本/日志在 `.omx/artifacts/cmrc-audit-20261027/`：`brief.md`、6 份分报告、`verification/`（A3 端到端独立核对、B1/B3 复现日志、
  A1 数值表、D1 区域证书计数证据、`verify_delivery.py`）、`scratch/`（证书探针源码与二进制、e2e 工作目录）。

## 六、建议修复顺序

1. **先修低风险、高确定性**：B1（守卫判据）、C1（逐层 0 样本拒绝）、D1（区域统计口径）、E1（死自检/计数）、B3（区域求交判据）、C2（预览准入复用运行准入）。
2. **再修算法数值**：A1（Delaunay 罚项无量纲化 + 产尺度回归）、D4（交叉强度归一化或精确嵌入性证书）；两者都建议配一个“产尺度 h≈1e-5 下确实能修复”的回归，
   并附真实案例上的触发频率统计（A1 修完后重跑 A3/黑河/云南，比较拓扑状态数与失败数）。
3. **契约/防御补强**：D2/D3/E2/D5（空过/单向/自证/截断）、C3/C4/C5（provenance、资源守卫、行对齐）、B2（CMRC 预算从 `maximum_cells`/`maximum_level` 派生，
   并让 namelist/project 两入口一致）。
4. 修复 A1 会改变弹性求解轨迹，需要按“算法改动”流程做 A/B（同配置多种子对比失败数/迭代数/最终网格），不能把交付网格变化当成纯加速宣称。

## 七、优化空间（2026-10-27 补充，按“收益/风险/工作量”排序）

### 7.0 基线测量（先有数，再优化）

A3 算例（`examples/cmrc_atmosphere_a3.nml --max-tris 20000000`，debug CLI，引擎 crate `opt-level=2`，16 核，
warm cache）：**9.50 s real / 1.93 GB peak RSS**；其中构造阶段 6.94 s，其余约 2.5 s 是 gridfile/remap CSV/MPAS 写出。
`EARTHMESH_CMRC_TIMING=1` 的阶段耗时（`verification/a3_phase_timing.log`）：

| 阶段 | ms | 占构造 |
|---|---:|---:|
| requirement_planning / mother_grid / initial cert | 64 / 63 / 147 | 4% |
| **initial_requirement_projection** | **1,264** | **18%** |
| graded_envelope | 169 | 2% |
| **elastic_component_epochs** | **4,823** | **70%** |
| ├ 2→1 级（1 组件；topology_search 1,117、remap 478、elastic_solve 120） | 3,110 | |
| ├ 1→0 级（1 组件） | 970 | |
| └ elastic_final_cells | 317 | |
| **final_requirement_projection** | **313** | **5%** |
| voronoi_remap / final cert | 79 / 15 | 1% |

本机没有真实 MERIT；真实硬案例的参考 profile（`.omx/notepad.md` 2026-09-08 的 30 m 组件 0）：
**169.4 s = 梯度 78.1 s（46%）+ 能量 51.0 s（30%）+ 阶段判断 29.5 s（17%）+ 认证 0.5 s**，可移动顶点 92,492、guard 218,717、172 次迭代。
这说明“难案例”与 A3 的耗时结构完全不同：A3 是拓扑搜索/投影为主，真实案例是弹性梯度/能量为主。

### 7.1 性能优化机会

| 优先级 | 机会 | 位置/证据 | 预期收益 | 风险/工作量 |
|---|---|---|---|---|
| **P0** | **失败路径的诊断/见证全是“算了就丢”**：每次弹性失败都做 ① `geometry_failure_reason` 对**整张 level 网格**做一次完整 `verify_scoped`（角度区间证明 + Delaunay + 对偶）；② `geometry_failure_requires_different_topology` 再verify一次；③ `angle_range` 全网格三角扫描两遍；④ `geometry_failure_diagnostics` 再做 2 次全网格扫描，其中 `face_graph_distance_to_any` 为全网格建 **BTreeMap<edge, Vec<face>>**；⑤ `geometry_failure_witness` **clone 整张 HierarchyLeafMesh + patch**。而生产 `component_transaction` 只消费 `failed_guard_face`、`global_angle_degrees` 和 witness 里的局部顶点/guard 面；`GeometryFailureDiagnostics` 只被 research-only 的 `full_polygon_merge.rs` 读取。 | `coarsen/elastic_block.rs:1524,1546,1559,1750`（诊断/见证构造）、`:1735-1756`（SearchBudgetExhausted 同款）、`:1755-1762`（witness clone）、`:2358-2498`（全网格扫描 + BTreeMap BFS）；`coarsen/component_transaction.rs:1640-1725`（只取三项）；`elastic_block.rs:238-241`（witness 结构） | 难案例（指南 §11.134：单级 20 次失败；heihe 4 级/100 km 5 级）每次失败可能省数秒到数十秒，整跑分钟级；A3 这类一次通过案例无收益 | 极低：verify 只算一次、诊断/见证惰性化（只存 movable 顶点坐标 + guard 三角），保留日志字段；改完输出应逐字节不变 |
| **P1** | **非角度阶段的梯度是串行有限差分**：`AngleFeasibility` 已用“位置覆盖 + 逐顶点局部能量”的并行 FD；`Interior`/`DelaunayVoronoiFeasibility`/`Untangle` 仍走 `serial_finite_difference_gradient`（每可移动顶点 5 次局部能量评估，串行）。真实组件 profile 里梯度占 46%。 | `coarsen/elastic_block.rs:4590-4660`（并行角度路径）与 `:4662-4730`（串行路径）；profile 见 `.omx/notepad.md` 2026-09-08 | 若难案例迭代主要落在这几个阶段，16 核并行可把梯度部分最多降一个量级（整体弹性时间 −30~40%）。**先测阶段占比再决定** | 中：把 `voronoi_cell_from`/`in_circle`/crossing 改成“单顶点位置覆盖”版本可保持逐位不变；解析梯度工作量大且会改搜索轨迹，需 A/B |
| **P1** | **remap 行存储**：A3 的 remap 1,025,213 行 / ~1,600 万对 `(source, weight)`，`Vec<RemapRow{ Vec<(usize,f64)> }>` 约占 1.93 GB 峰值里的 20–30%（每行 Vec 头 + 堆分配 + 16 B/对）；90 m 区域案例峰值 3.3 GB。 | `remap/mod.rs` 的 `RemapRow`/`ConservativeRemap`；A3 证据 `verification/e2e_delivery_checks.txt`（rows/targets/每层 source 数） | CSR（offsets + 扁平 source/weight）+ u32 id：内存 −20~30%、证书遍历/CSV 写出更快 | 中：API 面较大（remap/证书/增量/区域 renumber 全要改），算法不变 |
| **P1** | **两次需求投影**：A3 的 initial+final projection 共 1.58 s（23% 构造）。两次都构造“259,200 栅格单元 → Voronoi 单元”的完整球面 overlap remap（`certify_final_cell_requirements_from_raster`）。 | `requirement/mod.rs:338-400`、`:478+`；timing 表 | ① 缓存 raster 多边形/cap 索引可省重复构造（小）；② final 投影改走已认证的 initial→final remap 的最大层级（类似 `certify_final_cell_requirements_with_remap`），把 313 ms 级操作降为 O(remap 行数)；③ 栅格最大层级金字塔 + 精确边界过滤（中） | ②是**语义变化**（保守上界，可能略强于直接投影），需要证明或接受“更严”；③工作量中 |
| **P2** | **全局运行也按需物化**：global 运行总是建整张最细母网格（A3：1,310,720 个三角形，最终只交付 63,810 单元）；regional 已有 `on_demand::materialization_extent*`（指南 11.106–11.109），可作蓝图，把“需求/渐变到不了的基面”直接当 settled 的 0 级，只建需要的细母网格 | `construction/mod.rs:952-1402`（区域路径）vs `:1690-1993`（整球 mixed 路径）；`on_demand.rs` | 对“局部需求/全球交付”的案例（单圆 `cmrc_global_*`、真实 90 m 局部加密）可同时降母网格内存与构造时间；A3 型全亚洲 2 级需求收益小 | 高：闭合球面的 remap/层级/Euler/证书语义都要重新论证；建议先做真实案例的可行性研究 |
| P2 | 组件级并行：scheduler 目前按组件串行（每个组件独立 parent 集，但共享 remap source/epoch/计数，提交要确定性顺序） | `coarsen/scheduler.rs`、`component_transaction.rs` | 多组件难案例（heihe/100 km）理论可近核数加速 | 高：确定性、回滚、计数、热启动语义都要重审 |
| P2 | 交付写出：A3 的 remap CSV 33 MB、MPAS 89 MB，写出约 2 s；可提供可选二进/压缩 remap | `certified_pipeline.rs` 交付段、`remap` 序列化 | 省几秒 I/O、省磁盘 | 低收益；下游契约需改 |

### 7.2 算法能力/质量优化

| 优先级 | 机会 | 说明 |
|---|---|---|
| **P1** | **修 A1（Delaunay 罚项无量纲化）+ 产尺度回归** | 让“Delaunay/Voronoi 几何修复”在 30–150 m 恢复工作；可能直接减少难案例的拓扑重试/失败数（成本与成功率双收益），但会改变弹性搜索轨迹，需要真实案例 A/B（失败数/迭代数/最终网格/质量）。 |
| **P1** | **把可选局部质量优化器接入生产** | `EARTHMESH_CMRC_LOCAL_UPDATE`（指南 11.153–11.164）已能把真实 90 m MERIT 的发布域最大相邻尺度比 2.02416→2.01435，且 A23/A24 已把它的额外认证成本降下来；还差多案例成本、真实注入后失败回滚与安全边界验收。收益：真实数据质量软告警减少，而不是只做性能。 |
| P2 | 自适应过渡宽度/gradation rings | A3 的 5 圈是逐案选择（5 vs 3：纵横比 −1.71%、边长 CV −3.62%、单元 +1.7%，指南 `cmrc_atmos_delivery_quality_20260909.md`）；可按需求形状自动选圈数/渐变，减少“逐案调参”，收益小且需质量 A/B。 |
| P2 | 碎片化需求的搜索效率 | 指南 §11.110 记录同一图案在细基面上要 189–570 个拓扑状态、粗基面只要 1 个；更好的候选排序/预测/跨级热启动可能显著降搜索成本（该案例 8.4 h→?）。先量化，再改。 |
| P2 | 精确嵌入性证书（D4） | 用精确谓词给出“提交网格无自交”的证书，替代/兜底产尺度失活的 crossing 扫描；既是安全加固，也可能让 `Untangle` 阶段的启发式开销变小。 |

### 7.3 工程/门禁优化

- **生产可达面收敛**：`coarsen` 约 8.1 万行，生产可达核心只有 `elastic_block`/`transition_topology`/`core_condensation`（审计 3/6 的调用图证据）；`annular_*`、`face_band*`、`sdce_*`、`cec_find_one`、`anchor_ear`、`transition_cell_*`、`transition_feasibility`、`collar`、`sector_recovery`、`combined_recovery`、`quality_cache`、`domain_quality` 等只被 research_runner/tests 引用。用 `#[cfg(feature = "research")]` 或独立 crate 隔离，可减少编译时间、二进制体积、误用面与审计成本。
- **死 API/死配置清理**：`ElasticCmrcConfig::allow_safe_fallback`（scheduler 从不读）、`quality_cache`/`domain_quality`（生产未接线）、`snapshot_unchanged` 恒真自检（E1）、失败诊断里生产不用的字段。
- **性能可观测性**：把 `EARTHMESH_CMRC_TIMING` 的 phase/迭代/失败/重试计数写进 `certified_resources.json`，并加一个规范小案例（A3 或 `00_quickstart` 级）的 `make perf-cmrc` 回归（墙钟/峰值 RSS/阶段耗时阈值）。没有这个，上面 P1 的收益只能靠猜。
- **测试缺口**（审计各报告）：产尺度（h≈1e-5）弹性回归（A1/D4 只有粗 fixture）、区域证书内部计数一致性（D1）、区域 tri remap↔谱系逐行、`--cmrc-merge-preview`↔运行准入一致（C2）、增量构造对比漏 `delivered_levels`/`elastic_report`。把 7 个 ignored + 75 个 crate ignored 测试中可廉价化的移进 `make test-slow` 或 CI 分片。
- **CI**：heavy job 的 NetCDF 冷编译约 23 分钟，可缓存 netcdf-c/HDF5 构建产物或按 nextest 分片；fast/heavy 的 CMRC 案例可用 nextest 分组并行。

### 7.4 建议的落地顺序（每步都可独立验收）

1. **P0 性能快改**：失败诊断/见证惰性化 + verify 只算一次 + 免整网 clone；输出逐字节不变，用 A3/现有测试回归。预期难案例分钟级收益，工作量 1–2 天。
2. **可观测性**：阶段/迭代/失败计数进 resources + 一个 perf 回归门；用真实 MERIT/黑河案例测一次“梯度 vs 能量 vs 拓扑搜索”占比。
3. **审计快改批**：B1、C1、D1、B2、E1（以及 D3 断言）——确定性强、风险低，同时去掉 `--max-tris` 摩擦。
4. **P1 性能批**：按第 2 步数据决定先做“非角度相位并行梯度（逐位不变）”还是 remap CSR；两项都不改算法结果。
5. **A1 + D4 + 局部质量优化器**：算法改动批，需要真实案例 A/B 与质量验收，不能与纯性能改动混在同一个提交里。
6. **P2 研究**：全局按需物化、组件并行，先做设计/小案例可行性，再谈实现。

（本节数字来自本机 warm-cache debug CLI 与历史 profile；真实 MERIT/黑河案例未在本轮运行，P1/P2 的预期收益是量级估计，需按第 2 步实测后再承诺。）

## 八、P0 修复实施与 7920 实测（2026-10-27）

§7 的 P0（失败路径诊断/见证）已实施，改动仅 `rust/earthmesh_refine_certified/src/coarsen/elastic_block.rs`
（+350/−97 行），细节、patch 与全部数据见 `.omx/artifacts/cmrc-audit-20261027/fix-failure-path/`。

- **改动**：一次 `verify_scoped` 同时给出失败原因与 `RequiresDifferentTopology` 分类（旧路径多数失败会做两次整网认证）；
  witness 由 `mesh.clone()` 改为把停机状态按值移入；`GeometryFailureDiagnostics` 默认不计算（research 用
  `EARTHMESH_CMRC_FAILURE_DIAGNOSTICS=1` 打开）；失败路径的全网格角度范围改为并行 chunk min/max（与串行逐位相同，有单测）。
- **本机验证**：`cargo test -p earthmesh_refine_certified --lib` **338 passed / 7 ignored**（旧 336 + 2 新回归）；
  fmt、`clippy --all-targets -D warnings`、定向集成测试全过；A3 端到端 6 个交付产物 SHA256 与修前 **完全相同**。
- **T7920（7920l）真实数据 A/B**：基线 = `git archive HEAD` 构建，候选 = HEAD+patch，同一 release 配置、
  16 线程 affinity、`EARTHMESH_CMRC_TIMING=1`。
  - `vertex5`（真实 MERIT，2.5 km→150 m，40 min/次）：两臂 acceptance `errors=0`；证书与两个 gridfile SHA 相同，
    失败日志逐行相同，失败/认证计数完全一致（771 certified / 202 NoImprovement / 145 BudgetExhausted）。
    **347 次弹性失败的 solve 合计 881.5 s → 762.0 s（−13.6%，平均 −344 ms/次）**；整跑墙钟 39:58.34 → 38:48.37
    （−70 s，−2.9%）。注意同机其它 agent 作业导致噪声：候选臂 user CPU time 反而 +12%、certified solve 合计 +36 s，
    因此墙钟差被压缩；可直接归因的是失败 solve 的 −119.5 s。
  - `merit90m`（真实 MERIT ~90 m，4 min/次）：接受门通过；唯一不同的稳定产物字段是含绝对路径的 manifest/quality JSON
    （path-scrub 后相等）；失败分量单次 solve 25,176→22,088 ms（−3.1 s，−12.3%）。
- **结论/优先级修正**：修复安全且产物逐字节不变，但失败路径只占硬案例墙钟约 5%；本轮实测显示真正的大头仍是弹性迭代本身
  （vertex5：1118 次 solve 合计 960–1043 s，占 40 min 的 40–43%），其次是 `component_install_delta`（233 s）、
  `component_topology_search`（251 s）、`component_remap`（73 s）。§7 的 P1（非角度相位梯度并行/解析化）优先级应提前；
  P0 本身在没有弹性失败的运行（A3、多数普通案例）上无收益，但代码路径更干净、无整网 clone。
- 本批改动提交于分支 `cmrc-elastic-waste-20261027`（基于 `v3.0.0-alpha12` 的 `018ffc3e`），按仓库流程走 fmt/clippy/CI 三闸与 release 分支 PR。

## 九、第二批优化：弹性热循环（2026-10-27）

§7/§8 之后实测确认真正的成本在弹性迭代本身（vertex5：1118 次 solve 合计 960–1043 s，失败 solve 占 76–84%，
且全部停在 AngleFeasibility），因此对热循环做两处**逐位不变**的优化（仍只改 `elastic_block.rs`）：

1. 有限差分基值惰性化（三处梯度函数）：两侧都有限时不再计算基值，能量评估 5→4 次/顶点（−20%）；
2. 迭代内能量复用：相位不变时复用上一轮已算出的 `(phase, energy)`，省掉每轮一次全 patch 能量评估。

**逐位不变证据**：A3 端到端 6/6 产物 SHA256 与 HEAD 基线相同；真实案例的证书/gridfile/remap、失败 reason 行、
outcome 计数逐字相同。

**受控交替 A/B（T7920，缩减案例 `vertex5_2l`：75 s/次、仍有 32 次弹性失败，p0→c2 交替 3 组、同一批 16 核）**：

| 指标 | P0 三次 | P0+第二批 三次 | 变化 |
|---|---|---|---|
| 整跑墙钟 | 80.39 / 79.59 / 78.37 s | 74.55 / 72.58 / 73.58 s | **−7.4%（−5.88 s）** |
| 弹性 solve 合计 | 24,454 ms | 18,259 ms | **−25.3%** |
| 失败 solve 合计 | 17,005 ms | 12,413 ms | **−27.0%** |
| 平均每次失败 solve | 531 ms | 388 ms | −143 ms |
| certified solve 合计 | 7,448 ms | 5,846 ms | −21.5% |
| 峰值 RSS | 1,044,577 KB | 1,044,347 KB | 不变 |

三次 p0 的最慢仍慢于三次 c2 的最快，交替消除了漂移，结论干净。完整 vertex5 复跑（非同时段）为
solve 合计 −37.0%（959.9 s→604.9 s）、墙钟 38:48→28:43（−26.0%），含机器负载漂移，仅作旁证。

**结论/下一步**：P0（失败路径）+ 第二批（热循环）都已落地并提交于分支 `cmrc-elastic-waste-20261027`；按受控数据，两批合计在
`vertex5_2l` 这类失败偏多的案例上把整跑墙钟降约 10–15%（P0 约 −5% + 第二批约 −7.4%）。剩余大头：
vertex5 完整案例里 elastic solve 仍约 600 s / 29 min 墙钟（~35%），其后是组件级固定开销
（`component_install_delta`、`component_topology_search`、`component_remap` 等，合计约 10 min）。
若继续优化，两条路：① 组件级开销（Window/Whole 刷新与 install 的增量更新）；② 角度梯度的解析化
（把 4 次差分评估再降为一次解析遍历，预计还能再砍弹性求解的 30–50%，但会改变搜索轨迹，需要真实案例质量 A/B）。

## 十、弹性求解分相定标与下一个优化（2026-10-27）

§11.166 之后剩余成本集中在弹性迭代。新增 env-gated 探针 `EARTHMESH_CMRC_ELASTIC_PROFILE=1`
（分支 `cmrc-elastic-profile-20261027`，PR #134；guide 11.167），每次 solve 记录
phase/energy/gradient/certify 四类墙钟与每相位迭代数；探针关闭时只有一次缓存 env 读取与分支，A3 6/6 产物 SHA256 不变。

T7920 真实 MERIT 定标（占比；绝对耗时受共享机器负载影响，不作横向比较）：

| 案例 | solve 合计 | 迭代 | phase | energy | gradient | certify | 四项和 |
|---|---:|---:|---:|---:|---:|---:|---:|
| vertex5_2l（缩减，101 solve/32 失败） | 32.5 s | 6,855 | 20.5% | 27.4% | 30.1% | 5.4% | 83.4% |
| merit90m（真实 90 m，31 solve/2 失败） | 57.3 s | 1,188 | 11.3% | 19.1% | 42.6% | 2.7% | 75.7% |
| vertex5（完整，1118 solve/347 失败） | 915.8 s | 87,929 | 18.6% | 24.0% | 32.4% | 3.9% | 78.9% |

迭代几乎全部落在 AngleFeasibility（完整 vertex5：87,912/87,929），失败 solve 的梯度/phase 占比更高
（梯度 41.6%、phase 24.4%）。因此下一项定为 **AngleFeasibility 角度/边长罚项的解析梯度**：预计梯度时间降至约 1/3、
弹性 solve 再降 20–30%；它会改变搜索轨迹，按 §11.165 的 A/B harness 做真实案例质量验收（不要求逐字节一致）。
其后是**相位判断增量缓存**（只更新被移动顶点 incident 的 guard faces/扇形，理论上再省 11–20% 的 phase），
组件级 `install_delta`/窗口重建（完整 vertex5 约 208 s）不在 elastic solve 内，作为独立设计项。
