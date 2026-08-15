# HARP-DV 事务目标函数去扫描实施计划

日期：2026-08-15
状态：**已执行（2026-08-15）——结果见 §11**
范围：质量优化器与 low-degree repair 的**局部**路径；前置计划
`.omx/plans/harp-dv-voronoi-performance-plan.md` 已完成全网格路径

## 1. 目标

消除事务目标函数里逐站点重新扫描 fan seed 的开销。

前置计划把**全网格**扫描改成了一次性 seed 表（NXP40 端到端 15.54 s → 6.79 s）。
本计划处理它明确推迟的另一半：

> 局部事务如果已经持有 seed 可以顺手改为 `*_from`；不要在本阶段全面重构局部调用。

### 1.1 采样证据

NXP80 完整路径运行中采样（5024 样本，质量优化器 window pass 11–12）：

| 帧 | 样本 | 占比 |
|---|---:|---:|
| `voronoi_cell` 内联的 `find` 扫描（`mesh_voronoi/mod.rs:245`） | 1882 | 37.5% |
| `triangle_fan_from` 走环（`mesh_voronoi/mod.rs:153`） | 1412 | 28.1% |
| `triangle_fan_ids`（`cycle/mod.rs:1032`，扫描与走环一并内联） | ~1026 | ~20% |
| 角度计算、eta 计算与排序 | 3 | 0.06% |

目标函数闭包合计约占 **95%** 的运行时间。

两点必须写清楚，避免高估收益：

1. **走环那 28.1% 是不可消除的**——绕顶点走一圈是真实工作，换 seed 一点省不掉。
   本计划能消除的只有"找起点"的扫描。
2. `triangle_fan_ids` 那 ~20% 里扫描与走环内联在同一行，这份采样**分不开**。
   已确认的是：角度/eta 计算与排序只占 3 个样本，所以那条链的成本几乎全在
   `triangle_fan_ids` 内，而不在数值计算上。

因此预期收益是"消除 37.5% 中的大部分，加上 ~20% 中扫描的那部分"，而不是两者相加。
**阶段 5 重新采样才是唯一可信的收益判据。**

### 1.2 基线

**必须在动任何生产代码之前采集**（前一份计划在此处失手：退休阶段改完才想起没有改动前的
端到端基线，只能靠分量对照补救）。

阶段 0 必须记录并写回本节：

| 场景 | 命令 | 基线 |
|---|---|---|
| NXP40 checkpoint | 见 §5 阶段 0 | 6.79 s（2026-08-15 实测） |
| NXP40 退休门 | `leaf_retirement_on_the_production_checkpoint` | 3.70 s（同上） |
| NXP40 完整路径 | `the_full_production_path_on_the_nxp_proxy` | **723.7 s**（同上，优化器 716.7 s = 99.0%） |
| NXP80 checkpoint | `natural_length_ab_on_the_nxp_proxy` + `LOCATE_PENDING` | 422.0 s（同上） |
| NXP80 完整路径 | 同上，`EARTHMESH_TEST_NXP=80` | **5887.3 s**（同上） |

### 1.3 NXP80 完整路径的分阶段耗时（2026-08-15，前置计划完成后）

| 阶段 | 耗时 | 占比 |
|---|---:|---:|
| 细化（100 周期） | ~412 s | 7.0% |
| **质量优化器** | **5461.9 s** | **92.8%** |
| 叶退休 | 13.2 s | 0.2% |
| 最终评估等 | 余下 | <1% |

总计 5887.3 s，70,685 sites，stop `MaximumCyclesReached`，peak RSS 137 MB。
优化器提交 19,923 次移动，角度窗违规 12,475 → 1,574。

**这份数据决定了本计划的优先级**：本计划针对的目标函数闭包位于质量优化器内，而质量
优化器占生产路径的 **92.8%**。相比之下退休阶段只剩 13.2 s（前置计划完成前它曾是首要
嫌疑），已不值得再投入。

## 2. 根因

`MeshState::triangle_fan(site)`（`mesh_voronoi/mod.rs:180`）用线性 `find` 扫描整张
活动三角形表来找起点；找到之后的走环是 O(度数)。带 seed 的
`triangle_fan_from` / `voronoi_cell_from` / `vertex_degree_from` 全是现成的 pub 接口，
文档注释也明确写着"quadratic for a caller measuring a neighbourhood per change"。

事务目标函数正是"per change 测量邻域"，却走了扫描版：

| 位置 | 调用 |
|---|---|
| `cycle/mod.rs:2834` | `balance_objective` → `site_scale` → `state.voronoi_cell(site)` |
| `cycle/mod.rs:2837` | `state.voronoi_cell(site)`，逐 affected site |
| `cycle/mod.rs:1032` | `triangle_fan_ids` → `state.triangle_fan(site)` |
| `cycle/mod.rs:2842`、`site_scale` 内部 | `state.sphere_radius()`，本身 O(V) |

而 seed 本来就在手上——`propose_move_cached`（`transaction/mod.rs:559`）算了
`sites_touching(&reach)`（返回 `BTreeMap<站点, 三角形>`）然后用 `.keys()` 把值扔掉。
同一文件的 `check()`（`transaction/mod.rs:411`）没扔，并写明了理由。

## 3. 两条硬约束

本计划的全部难度在这两条，实施时不得绕过。

### 3.1 移动前的 seed 在合法化后可能失效

`propose_move_cached` 目前把 `affected_sites` 算一次，供**移动前**和**合法化后**两次目标
函数调用共用。`legalize_within` 的边翻转会重写三角形槽位的角点数组，翻转前含站点 X 的
槽位翻转后可能不含 X——直接沿用移动前的 seed 会读到错误的 fan。

`check()` 之所以安全，是因为它用的 `sites_touching(touched)` 里 `touched` 已是合法化后的
区域，每次都重新取 seed。

**要求：**

1. 移动前生成一次 seeded map，供 before 目标函数使用；
2. 合法化后**重新生成**，供 after 目标函数使用；
3. 两次的 **affected 站点集合必须相同**——目标函数比较的是前后同一批站点的分数，
   集合变了比较就失去意义；
4. 若某站点在合法化后取不到 seed，回滚为 `Rejection::Unmeasurable`。

几何上第 3 条应当自动成立：一次翻转把 `(a,b,c)+(a,c,d)` 换成 `(a,b,d)+(b,c,d)`，四个角点
在翻转后仍各自被至少一个新三角形包含。但这必须**断言**而非假定。

### 3.2 seed 不同会改变浮点结果——只对 cell 路径

`sites_touching` 给的是 *reach 范围内*编号最小的三角形；`triangle_fan` 扫出来的是
*全网格*编号最小的。两者一般不同 → fan 起点不同 → 旋转。

| 路径 | 是否受影响 | 原因 |
|---|---|---|
| `triangle_fan_ids` → margins / eta | **否** | 输出进 `BTreeSet`，三角形集合与起点无关；随后 `sorted_triangle_values` 又按 `total_cmp` 排序 |
| `vertex_degree_from` | 否 | 只取 `.len()` |
| `voronoi_cell` → `effective_scale_m` | **是** | corner 顺序变 → 球面三角形面积求和顺序变 → 末位不同 |

**解法：只在 `voronoi_cell_from` 内部规范化**——取到 fan 后旋转到编号最小的三角形起步，
再算 circumcentres。

**不得规范化 `triangle_fan_from` 原语。** `mesh_retirement/mod.rs:115,156` 依赖 fan 起点
决定环的起点，进而决定 `triangulations()` 的枚举顺序和"第一个被接受的剖分"。动原语会改变
退休结果，验证成本没有必要。

规范化之后：任何有效 seed 得到的 cell 与现行扫描版 `voronoi_cell(site)` 逐位相同——因为
现行扫描版本来就是从全网格编号最小的入射三角形起步的。

## 4. 非目标

1. 不修改 `triangle_fan_from` 的语义；
2. 不引入 Rayon 或任何新依赖（`rayon` 是别的 crate 的依赖，对
   `earthmesh_refine_harp_dv` 仍是新增）；
3. 不修改 `max_cycles`、停止条件或任何运行预算——那会改变输出，不是性能修复；
4. 不改变候选顺序、pass 数、线搜索或门禁；
5. 不给 `MeshState` 增加长期索引。

并行化只在阶段 5 重新采样后、且只读全量统计重新占主导时才评估。

## 5. 实施步骤

### 阶段 0：基线与 reference oracle

**先测基线**（§1.2 表格），写回计划文件。

reference 实现（test-only，生产不得调用）：

1. `reference_voronoi_cell_scanned(state, site)`：现行 `state.voronoi_cell(site)`；
2. `reference_triangle_fan_ids(state, sites)`：现行逐站点 `triangle_fan` 版本；
3. `reference_site_scale(state, site)`：现行含内部 `sphere_radius()` 的版本。

fixture 复用前一份计划的 `full_sweep_fixtures()`：`sphere(6)`、插入后、移动并 legalize 后、
存在 tombstone 后。

**关键回归测试**（阶段 4 的验收基础）：

```text
对每个 fixture、每个活动站点、该站点的每一个入射三角形 seed：
    voronoi_cell_from(site, seed) 与 reference_voronoi_cell_scanned(site) 逐位相同
```

不使用 epsilon。这条测试在规范化落地前应当**失败**——若一开始就通过，说明测试没有真正
覆盖非最小 seed，必须先修测试。

### 阶段 1：`sphere_radius` 提出循环

`state.sphere_radius()` 是 O(V)（对全部活动顶点求模长再平均），却被放在逐 affected site
的循环内，并且 `site_scale` 内部又调一次。

1. 目标闭包内提升到循环外；
2. `site_scale` 增加 `radius_m` 参数，由调用方传入；
3. `balance_objective` 同样接收并透传。

闭包执行期间 state 不变，返回值逐位相同，纯等价。

### 阶段 2：目标函数接收新鲜的 seeded map

1. 目标函数签名 `&BTreeSet<usize>` → `&BTreeMap<usize, usize>`（站点 → seed）；
2. `propose_move_cached`：移动前生成一次，合法化后重新生成；
3. 断言两次的键集合相同，不同则 `Unmeasurable` 回滚；
4. `propose_pair_move_cached`、`score_before_move` 同步改造。

本阶段**不改任何 cell 读取**，只铺管道。改完 NXP40 输出必须逐项不变。

### 阶段 3：`triangle_fan_ids` 用 seed

改为 `triangle_fan_from(site, seed)`。逐位等价，不需要 §3.2 的规范化。

同时覆盖两条路径——两个目标函数用的是同一条链：

- `optimise_mesh_quality_with_natural_length`（`cycle/mod.rs:2856-2857`）
- `repair_low_degree_stars`（`cycle/mod.rs:2455-2456`）

验收：`reference_triangle_fan_ids` 逐位相等；NXP40 输出不变；单独记录时间。

### 阶段 4：`voronoi_cell_from` 规范化 + cell 链用 seed

1. `voronoi_cell_from` 内部把 fan 旋转到编号最小的三角形起步；
2. `site_scale`、`balance_objective`、目标闭包的 `2837` 行改用 `voronoi_cell_from`；
3. 阶段 0 那条"任意入射 seed 逐位相同"的回归测试此时必须转为通过。

验收：全部 reference 对照逐位相等；**`mesh_retirement` 的退休提交数与网格指纹不变**
（原语未改，但必须证明）；NXP40 输出不变。

### 阶段 5：重新采样

与 §1.1 相同方法，NXP80 质量优化器运行期间采样。

判定：

- 扫描帧（`mesh_voronoi:245` 与 `cycle/mod.rs:1032`）应显著下降；
- `triangle_fan_from` 走环占比上升是**预期**的，不是回归；
- 若扫描帧未降，检查 seeded map 是否真的传到了每个调用点。

### 阶段 6：NXP40 / NXP80 验证

四项都与阶段 0 基线比对，输出必须逐项一致：

- NXP40 checkpoint（sites / cycles / pending / audit）
- NXP40 退休门（候选顺序 / 提交数 / 网格指纹 / 耗时）
- NXP40 完整路径
- NXP80 完整路径（分阶段耗时：refinement / quality / retirement / final evaluation）

## 6. 完整门禁

```bash
cargo test -p earthmesh_mesh --release
cargo test -p earthmesh_refine_harp_dv --release
cargo fmt --all --check
cargo clippy -p earthmesh_mesh -p earthmesh_refine_harp_dv \
  --all-targets -- -D warnings
git diff --check
```

## 7. 风险与缓解

### 合法化后 seed 失效（最高）

- 合法化后重新生成，不得沿用；
- 断言前后键集合相同；
- 缺失即 `Unmeasurable` 回滚；
- fixture 必须包含真实发生翻转的移动。

### 前后 affected 集合不一致

比较的是前后同一批站点的分数。集合变了，`partial_cmp` 比较的就是两个不同的东西，而它
返回 `None` 时调用方读作"没有改进"——**失败会表现为静默的接受率下降，不是崩溃**。必须
断言而非依赖几何论证。

### cell 路径的浮点顺序

- 只规范化 `voronoi_cell_from`，不动原语；
- 阶段 0 的"任意入射 seed"测试是唯一能抓住它的断言；
- 该测试在阶段 4 之前必须是失败的。

### 误伤 `mesh_retirement`

- 不改 `triangle_fan_from`，所以理论上不受影响；
- 仍需在阶段 4 断言退休提交数与网格指纹不变。

### 高估收益

走环那 28.1% 不可消除；`triangle_fan_ids` 内扫描与走环无法从当前采样分离。以阶段 5 的
重新采样为准，不以预估为准。

## 8. 提交拆分

1. Hoist the sphere radius out of the objective's per-site loop
2. Give transaction objectives a freshly seeded neighbourhood map
3. Seed the fan-id collection behind the margin and eta scores
4. Seed the cell reads and pin the Voronoi fan to its lowest triangle

每个提交附 reference 回归与独立 NXP40 时间。提交信息遵循 Lore protocol。

## 9. 给 Claude Code 的指令

> 严格按阶段执行，**先采集 §1.2 的全部基线再动生产代码**。阶段 0 那条"任意入射 seed 逐位
> 相同"的测试在阶段 4 之前必须是失败的；若一开始就通过，先修测试再继续。不得规范化
> `triangle_fan_from` 原语。不得沿用移动前的 seed。不得引入 Rayon、修改 `max_cycles`、
> 停止条件或任何门禁。不要覆盖工作区里与本计划无关的现有修改。

## 10. 完成定义

- 阶段 0 的全部基线已记录在案；
- 任意入射 seed 的 cell 与扫描版逐位相同；
- `triangle_fan_ids`、`site_scale`、`balance_objective` 的 reference 对照逐位相等；
- 退休提交数与网格指纹不变；
- NXP40 / NXP80 输出逐项不变；
- 阶段 5 采样显示扫描帧显著下降；
- 全部门禁通过；
- 没有新依赖，没有原语语义、预算或 schema 变化。


## 11. 执行结果（2026-08-15）

### 11.1 性能

| 场景 | 改前 | 改后 | 变化 |
|---|---:|---:|---|
| NXP40 checkpoint | 6.79 s | **5.73 s** | −16% |
| **NXP40 完整路径** | **723.7 s** | **241.2 s** | **−67%** |
| ├ 质量优化器 | 716.7 s | **235.4 s** | −67% |
| └ 叶退休 | 0.5 s | 0.6 s | 持平 |
| crate 测试套件 | 325.7 s | 169.9 s | −48% |
| **NXP80 完整路径** | **5887.3 s** | **2164.8 s** | **−63%** |
| ├ 质量优化器 | 5461.9 s | **1784.7 s** | −67% |
| ├ 细化 | ~412 s | ~367 s | −11% |
| └ 叶退休 | 13.2 s | 13.3 s | 持平 |

分阶段：阶段 1（半径提升）把 checkpoint 从 6.79 降到 6.46；阶段 2–4（seed 管道 + fan
规范化）再降到 5.73，并把优化器砍掉三分之二。

### 11.2 等价性

NXP40 完整路径改前改后**逐项相同**：13,381 次移动、margin_min −63.522602 → −12.710471、
eta_min 0.157919 → 0.838451、角度窗违规 3723 → 245、17,921 sites、stop `AllSatisfied`、
below40 22 / above80 221 / min 36.7917 / max 92.7105、unresolved 0、unbalanced 0。

NXP80 完整路径同样逐项相同：19,923 次移动、margin_min −92.577117、eta_min 0.000077 →
0.005841、违规 12,475 → 1,574、70,685 sites、stop `MaximumCyclesReached`、
below40 554 / above80 1017 / min 0.1933 / max 172.5771、unresolved 49 / unbalanced 456，
连停滞升级审计行都一字不差。peak RSS 137 MB → 140 MB。

其中 NXP40 的 below40 22 / above80 221 / min 36.7917 / max 92.7105 还**逐项等于**
`harp-natural-length-ab.md` §9 记录的 NXP40 生产基线——那份基线记于本日全部改动之前。

对照测试：
- `every_incident_seed_builds_the_same_cell_as_the_scan`：9,210 对 (站点, seed)，其中
  7,667 对为非最小 seed，全部与扫描版逐位相同。**该测试在规范化落地前确实失败过**
  （`site 2 seeded from 290 produced a differently ordered fan`），符合 §5 阶段 0 的要求。
- `triangle_fan_ids_are_independent_of_the_seed`：落地前即通过，证实该链无需规范化。
- `full_cell_sweeps_match_the_scanned_reference` 增测了 `site_scale_from` 对
  `reference_site_scale` 的逐位相等。

### 11.3 阶段 5 采样（NXP80 优化器 eta pass 3/16）

| 帧 | 改前 | 改后 |
|---|---:|---:|
| `voronoi_cell` 内联 `find` 扫描（`mesh_voronoi:245`） | 1882 | **0** |
| `triangle_fan_from` 走环（`mesh_voronoi:153`） | 1412 | 6146 |
| `triangle_fan_ids` | ~1026（含扫描） | 1721（纯走环） |

扫描帧归零；走环占比上升是 §5 阶段 5 预注册的预期，不是回归。**剩余开销已是绕顶点一圈
这件不可约的工作**，进一步提速需要复用 `triangle_fan_from` 的分配缓冲区，或按 §4 的非目标
重新评估并行。

### 11.4 与计划的偏离

1. **`voronoi_cell_from` 的规范化提前到阶段 0 之后立刻做**，而非留到阶段 4。原因是阶段 0
   的红测试一旦观察到失败，把套件留红会挡住后续每一次门禁；而规范化本身在当时不改变任何
   行为（彼时没有调用方传非最小 seed）。红测试已按要求先观察到失败再转绿。
2. **阶段 2/3/4 合并实施**。签名从 `&BTreeSet<usize>` 改为 `&AffectedSites` 会让所有调用点
   同时失效，分三次提交需要人为造中间态，收益为零。
3. `balance_objective` 需要为**环外的邻居**也提供 seed（`edges` 会引用不在 `sites` 里的
   角点）。做法是建边时顺手记下每个角点所在的三角形——任何含该角点的三角形都是合法 seed，
   而规范化保证了选哪个都不影响结果。
