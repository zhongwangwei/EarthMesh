# HARP-DV 全网格扫描性能优化实施计划

日期：2026-08-15（修订版）
状态：可交给 Claude Code 执行
范围：行为等价的性能修复；不改变网格算法、门禁、停止条件或输出格式

修订说明：本版相对初版的实质改动有四处——补入退休阶段的三处热点；重新定义
NXP80 checkpoint 门的适用范围；新增独立的退休性能门；删除不可比的基线数字。
理由见 §1.2 与 §2.4。

## 1. 目标与基线

目标：消除 HARP-DV 全网格统计中的意外近平方复杂度。

### 1.1 可用基线

| 场景 | 当前时间 | 当前结果 |
|---|---:|---|
| NXP40，最多 20 cycles | 16.22 s | cycle 11 停止，17,922 sites，pending=0 |

NXP40 的 15 秒 sample（11,428 个工作线程样本）：

- triangle_fan_from: 40.1%
- voronoi_cell: 23.5%
- balance_demands: 16.5%
- site_scale: 8.2%
- vertex_degree: 5.2%

证据文件：
`.omx/logs/harp-nxp40-perf-probe-1786771236.sample.txt`

NXP80 的采样（5 秒，4162 样本，`retire_quality_leaf_sites` 运行期间）：

- `balance_survey_state`：4161 / 4162 = 99.98%
  - 其中 `state_scales` → `voronoi_cell` → `triangle_fan_from` 约 67%
  - `triangle_fan` 与内层邻居比对约 33%

这次采样是在 `cycle/mod.rs` 加入 checkpoint 提前返回**之前**取的，当时
checkpoint 运行仍会执行退休阶段。它证明的是退休阶段本身的热点分布，与
checkpoint 语义无关。

### 1.2 作废的基线

初版列出的两个 NXP80 数字**不可用作对照**，本版删除：

- `NXP80，100 cycles，checkpoint 1532.61 s`
- `NXP80 旧完整路径 3892.96 s（包含 quality + retirement）`

原因是三次运行语义不同，且中间隔着未受控的代码变更：

| 运行 | 已确证 | 未确证 |
|---|---|---|
| 3892.96 s（`harp-natural-length-current-nxp80.log`） | cycle 100 后 checkpoint 断言失败；质量优化器输出 0 行，未进入 | 是否进入退休阶段。退休阶段不打任何日志，日志缺席不构成证据 |
| 1532.61 s（`harp-nxp80-pending-audit.log`） | 采样直接证明进入了 `retire_quality_leaf_sites` | — |
| 当前代码 | `cycle/mod.rs:3620` 的提前返回明确跳过 quality **和** retirement | — |

因此不得用这两个数相减来推算任何阶段的耗时，第二个数也不得作为改动后复测的
对照——它是含退休阶段测的，而当前代码的 checkpoint 路径不含退休阶段。

**所有 NXP80 基线必须在当前代码上重测**，见 §5 阶段 5。

## 2. 根因

### 2.1 每个站点重新全表寻找 fan seed

`rust/earthmesh_mesh/src/mesh_voronoi/mod.rs:180`：

```rust
let seed = self
    .active_triangle_slots()
    .find(|&triangle| self.triangles()[triangle].contains(&site))
```

- 对整个 mesh 线性；文件注释已明确写出这一点，并指向 `triangle_fan_from`；
- `vertex_degree` 和 `voronoi_cell` 都走这条路径。

`rust/earthmesh_refine_harp_dv/src/cycle/mod.rs:2964`：

- `evaluate` 对每个活动 site 调用 `voronoi_cell(site)`；
- 一次名义上的全站点扫描因此接近 O(V × F)。

所需的局部接口**已经存在且是 pub**：`triangle_fan_from`(131)、
`voronoi_cell_from`(230)、`vertex_degree_from`。本计划不新增 mesh API。

### 2.2 balance 在 evaluate 后再次逐站点重建 fan

`cycle/mod.rs:97-135`：

- `evaluate` 已计算全部 cell scales；
- `balance_demands` 随后又对每个 site 调用 `triangle_fan(site)`；
- 实际只需遍历活动三角形的六个有向 corner 对。

### 2.3 quality 重算相同 scales

`cycle/mod.rs:2445` `QualityGuardMetrics::read`：

- 先通过 `balance_survey` 计算一次 scales；
- `pending_site_count` → `evaluate` 随后再计算一次。

### 2.4 退休阶段的三处全网格扫描（初版遗漏）

NXP80 的实测热点全部在这里，而初版阶段 1 的改造清单只覆盖细化与质量路径。

**(a) 候选收集**（`cycle/mod.rs:1512-1531`）

```rust
.filter(|&site| {
    let degree = mesh.state().vertex_degree(site).ok();   // 无条件求值
    leaves.interior_leaf.get(site).copied().unwrap_or(false)
        && degree.is_some_and(|degree| (4..=maximum_degree).contains(&degree))
        && mesh.state().triangle_fan(site).is_ok_and(...)  // 第二次线性扫描
```

`vertex_degree` 是 `let` 绑定，在廉价的 `interior_leaf` 查表**之前**对每个活动
site 求值，因此非叶子站点也全额付费。合格候选随后再付一次 `triangle_fan`。

紧接着的 `sort_by`（1518-1531）在比较器内重复调用 `worst(site)`，每次比较都重
新取 fan 并重算球面角，且发生在 `truncate(64)` **之前**，即对全部候选排序。

**(b) `RetirementPostcondition::accepts`**（`cycle/mod.rs:1475-1478`）

```rust
let after_vertices_below_degree_5 = state
    .active_vertex_slots()
    .filter(|&site| state.vertex_degree(site).is_ok_and(|degree| degree < 5))
    .collect::<BTreeSet<_>>();
```

逐站点 `vertex_degree` → O(V × F)，且 `accepts` 按环的三角剖分数被反复调用
（Catalan(度数−2)：度 4 为 2，度 7 为 42）。

**(c) `read_baseline`**（`cycle/mod.rs:1550-1553`）

与 (b) 逐字相同的那段扫描。只修 (b) 等于只修一半。

单遍实现**已经存在**：`cycle/mod.rs:1854` 的 `vertex_degrees(state)` 一次遍历活动
三角形即得到全部度数，O(T)；`cycle/mod.rs:1271` 的 `vertices_below_degree_5`
已在使用它。

### 2.5 checkpoint 提前返回改变了可测范围

`cycle/mod.rs:3619-3628`：

```rust
#[cfg(test)]
if capture_quality_checkpoint(mesh) {
    // Checkpoint-only tests inspect the saved clone. Running optimisation
    // and leaf-retirement audits on the throwaway working mesh is pure
    // cost, and becomes dominant on large NXP fixtures.
    return Ok(CycleOutcome { ... });
}
```

后果有二：

1. `natural_length_ab_on_the_nxp_proxy` **无论是否设置 `EARTHMESH_TEST_LOCATE_PENDING`**
   都会先调用 `production_quality_checkpoint()`（`cycle/mod.rs:3071`），因而必然
   走这个提前返回。**仅删除该环境变量不会得到完整路径**，只会在 checkpoint 断言
   处失败。
2. 该测试因此**不覆盖退休阶段**。若把它当作 §2.4 三项修复的性能门，会得到一个
   与本计划任何优化都无关的"提速"。

因此需要独立的退休门（阶段 5）和独立的完整路径测试（阶段 6）。

## 3. 非目标

本计划不做：

1. 不修改 max_cycles 或停止条件；
2. 不新增 stagnation/no-progress 退出；
3. 不改变 candidate ladder、r-adaptation 或 quality pass 数；
4. 不引入 Rayon 或新依赖；
5. 不给 MeshState 增加长期 vertex-to-triangle 索引；
6. 不改变 balance 当前重复计数口径；
7. 不做近似统计或放宽浮点容差；
8. 不修改 `capture_quality_checkpoint` 的提前返回语义——它是既有的正确优化，
   本计划只是不再拿它当性能门。

先完成可逐位验证的纯性能优化，再单独评估算法级提前停止。

## 4. 验收标准

### 正确性

- 所有现有测试通过；
- NXP40 的 cycle 数、最终 sites、pending 逐项不变；
- 新旧 `state_scales` 的每个 `Option<f64>` 逐位相等；
- 新旧 `balance_demands` 的完整列表相等，包括顺序、site、ratio；
- 新旧 `balance_survey` 的 `(over, worst)` 逐位相等；
- 新旧退休**候选列表逐位相等，包括顺序**（见 §7 风险）；
- 退休提交数与最终网格指纹逐位相等；
- retired/tombstone vertex 继续得到 None seed 和 None scale；
- `a_run_is_deterministic` 继续通过。

### 性能

预注册门槛：

- NXP40：16.22 s → 不高于 9.0 s；
- 退休门（阶段 5）：以当前代码实测为基线，不低于 10× 改善；
- NXP80 checkpoint 与完整路径：**先在当前代码取基线，取得后再设门**（§1.2）；
- 若 NXP40 改善不足 20%，停止扩展，重新采样；
- 不得以减少 cycles、sites 或 criteria 评估次数换速度。

只比较同一机器、release 模式和相同环境变量。

## 5. 实施步骤

### 阶段 0：锁住旧语义

文件：

- `rust/earthmesh_refine_harp_dv/src/cycle/tests.rs`
- 必要时 `rust/earthmesh_mesh/src/mesh_voronoi/tests.rs`

新增最小 test-only reference 实现，复制当前旧逻辑，不供生产调用：

1. `reference_state_scales`：逐活动 site 调 `voronoi_cell(site)`；
2. `reference_balance_demands`：保留逐 site + fan 逻辑；
3. `reference_balance_survey`：保留当前重复计数；
4. `reference_retirement_candidates`：保留当前的求值顺序与比较器排序，输出候选
   列表（截断前与截断后各一份）。

fixture 至少覆盖：

- `sphere(6)`；
- 插入若干 site 后；
- 移动并 legalize 后；
- 存在 tombstone 后。

断言新旧结果逐位相等，不使用 epsilon。

停止条件：reference 测试不能稳定复现生产结果时，不进入阶段 1。

### 阶段 1：一次构建全站点 fan seeds

文件：`rust/earthmesh_refine_harp_dv/src/cycle/mod.rs`

新增私有 helper：

```rust
fn active_site_triangle_seeds(state: &MeshState) -> Vec<Option<usize>>
```

要求：

1. 分配 `state.vertices().len()` 长度；
2. 按 `active_triangle_slots()` 的现有升序遍历；
3. 三个 corner 只记录第一次遇到的 triangle；
4. reserved、retired、无活动三角形的 vertex 保持 None。

必须选择第一个活动 triangle，使其与旧 `triangle_fan(site)` 的 `find` 结果一致，
从而保持 fan 起点、Voronoi corner 顺序、面积求和顺序和浮点结果逐位一致。

改用 `voronoi_cell_from(site, seed)` 的全量扫描：

1. `state_scales`
2. `evaluate`
3. `demanded_cells_in_state`
4. `median_cell_scale`（复用一次 `state_scales`，不再逐 site 调 `site_scale`）

局部事务如果已经持有 seed 可以顺手改为 `*_from`；不要在本阶段全面重构局部调用。

验收：

- reference 对照逐位通过；
- NXP40 输出不变；
- 单独记录阶段 1 时间。

### 阶段 2：balance 改为直接三角形遍历

文件：

- `rust/earthmesh_refine_harp_dv/src/cycle/mod.rs`
- `rust/earthmesh_refine_harp_dv/src/cycle/tests.rs`

修改：

1. `balance_demands`
2. `balance_survey_state`
3. 新增 `balance_survey_from_scales(state, scales, limits)`

实现要求：

- 遍历每个活动 triangle `[a,b,c]`；
- 每个 triangle 检查六个有向关系：`a→b`、`a→c`、`b→a`、`b→c`、`c→a`、`c→b`；
- **不得对 edge 去重**。

不去重是必要条件：旧实现中，同一无向边因两侧三角形和两个方向重复计数。字段名
虽写 `pairs`，本计划必须保留当前数值语义。

`balance_demands` 继续：

- 只对较粗端建立 demand；
- 保留 `minimum_cell_width_m` 判断；
- 同一 site 只保留最大 ratio；
- BTreeMap 输出顺序不变。

验收：

- reference demand 完整列表相等；
- reference survey tuple 逐位相等；
- NXP40 输出与基线相等。

### 阶段 3A：复用同一轮 evaluate 的 scales

文件：`rust/earthmesh_refine_harp_dv/src/cycle/mod.rs`

#### QualityGuardMetrics

改为：

1. 只调用一次 `evaluate`；
2. 从返回的 demands + scales 计算 pending、balance demands 和 balance survey；
3. angle、eta、margin 的计算和顺序保持不变；
4. 删除或收窄重复的 `pending_site_count`，避免留下两份等价实现。

#### 最终报告

`run_cycles` 已持有 `final_scales`。`unbalanced_pairs_remaining` 必须调用
`balance_survey_from_scales`，不得再次调用 `state_scales`。

#### RetirementPostcondition

新增或改造为一次 cell sweep 同时返回：

```rust
fn demanded_cells_and_scales(state, criteria) -> Option<(usize, Vec<Option<f64>>)>
```

physical postcondition 使用 count，balance postcondition 使用同一份 scales。
保留已经实现的 baseline 规则：首次计算一次，仅提交成功后刷新。

### 阶段 3B：退休阶段三处热点（初版遗漏）

文件：`rust/earthmesh_refine_harp_dv/src/cycle/mod.rs`

**(a) 候选收集**（`1512-1531`）改为：

1. 先检查 `leaves.interior_leaf`，短路掉全部非叶子站点；
2. 循环外一次性调用 `vertex_degrees(state)`，替代逐站点 `vertex_degree`；
3. 每个存活候选只计算一次 fan 与 worst margin，收集为 `Vec<(usize, f64)>`；
4. 用 `worst_margin.total_cmp(...)` 排序后 `truncate(64)`，再取出 site。

**不得使用 `sort_by_cached_key`**：它要求 `K: Ord`，而 `f64` 不实现 `Ord`，
无法编译。也不得用 `to_bits` 排序——margin 可以为负，`to_bits` 对负数的序是反的。

**(b) `accepts`**（`1475-1478`）与 **(c) `read_baseline`**（`1550-1553`）：
两处共用同一份 `vertex_degrees(state)` 单遍结果构造
`BTreeSet<usize>`，不得只修其中一处。

验收：

- 候选列表（截断前与截断后）与 reference 逐位相等，**包括顺序**；
- retirement 提交数不变；
- 退休后网格指纹逐位相等；
- 完整 HARP 测试通过；
- NXP40 输出逐项不变。

### 阶段 4：重新采样

使用与基线相同的 NXP40 命令和 sample 方法。

关注：

- `voronoi_cell` 顶栈占比；
- `balance_demands`；
- `triangle_fan_from`；
- `site_scale`；
- `vertex_degree`。

判定：

- NXP40 ≤ 9.0 s：进入阶段 5；
- 改善 ≥20% 但未达 9.0 s：只处理新的第一热点；
- 改善 <20%：停止，检查 seed helper 是否覆盖 evaluate 和 balance；
- 不根据猜测修改 quality pass 数。

### 阶段 5：退休性能门（新增）

初版没有任何门覆盖退休阶段，而 NXP80 的实测热点全在那里（§1.1）。

新增 `#[ignore]` 测试，直接调用 `retire_quality_leaf_sites`，**绕开 checkpoint
提前返回**：

1. 用 `production_quality_checkpoint` 返回的克隆作输入——它正是质量边界处的
   状态，也就是退休阶段真实的输入。这样只付一次细化代价即可反复验证退休本身，
   不必每次跑完整路径。
2. `retire_quality_leaf_sites` 是私有的，但 `tests.rs` 是 `cycle` 的子模块，可
   直接调用。
3. 记录并断言：候选列表与顺序、提交数、`committed_d4`、最终网格指纹、耗时。

先在当前代码取基线，再在阶段 3B 后复测。规模用 NXP40 与 NXP80 各一次。

### 阶段 6：完整路径验证（新增）

`natural_length_ab_on_the_nxp_proxy` 不能用于此目的（§2.5）。新增最小
`#[ignore]` 测试，**直接调用 `run_cycles`**，生产代码无需改动；可照抄既有的
`transaction_floor_0_vs_25_vs_40_on_the_nxp6_proxy` 写法。

先在当前代码取一次基线，再在全部改动后复测一次。分别记录：

- refinement（cycle 循环）；
- quality optimisation；
- retirement；
- final evaluation；
- 总耗时、peak RSS。

命令模板：

```bash
EARTHMESH_TEST_NXP=80 \
EARTHMESH_TEST_MAX_CYCLES=100 \
/usr/bin/time -lp \
cargo test -p earthmesh_refine_harp_dv --release \
  <新增的完整路径测试名> -- --ignored --nocapture
```

### 阶段 7：NXP80 checkpoint 验证（范围收窄）

保留此项，但**只用于验证 refinement + r-adaptation + pending audit 的正确性**，
不再承担性能门职责。

```bash
EARTHMESH_TEST_NXP=80 \
EARTHMESH_TEST_MAX_CYCLES=100 \
EARTHMESH_TEST_LOCATE_PENDING=1 \
EARTHMESH_TEST_SKIP_REPEAT=1 \
/usr/bin/time -lp \
cargo test -p earthmesh_refine_harp_dv --release \
  natural_length_ab_on_the_nxp_proxy -- --ignored --nocapture
```

输出必须与在**当前代码上重取的**基线一致：

- 70,686 active sites；
- 0 physical demands；
- 54 balance demands；
- audit ordinary=0、fallback=28、blocked=26。

这四项在提前返回前后都成立——checkpoint 是在质量边界处捕获的克隆，quality 与
retirement 都在它之后，因此不受提前返回影响。任何一项不同，性能结果作废。

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

### seed 不同导致浮点顺序变化

- 必须选择每个 site 第一个活动 triangle；
- 新旧 scales 逐位相等。

### balance 去重导致统计变化

- 每 triangle 保留六个有向比较；
- 不使用 edge set；
- reference survey 精确相等。

### `vertex_degrees` 与 `vertex_degree` 在退化情形下语义不同（新增，最高风险）

这是阶段 3B 最容易出错的地方：

- `vertex_degree(site)` 走 fan，遇到 `FanIsOpen`、`FanDidNotClose`、
  `SiteIsInNoTriangle` 会返回 `Err`，因而被 `is_ok_and` 过滤掉；
- `vertex_degrees(state)` 只统计入射计数，破损扇的顶点仍会得到非零度数，
  于是**可能被计入**。

在闭合三角剖分上二者一致，HARP 也断言 `open_edge_count == 0`——但这必须**断言**
而非假定。阶段 0 的 fixture 必须包含 tombstone 与 legalize 后的状态，并对
`before_vertices_below_degree_5` 的集合本身（不只是大小）做逐位比对。

### 候选顺序变化导致 truncate 截到另一批候选（新增，最高风险）

阶段 3B 的 (a) 同时改动候选集合的构造与排序。只要顺序变了，`truncate(64)` 截到
的就是另一批候选，最终网格必然不同，而**总耗时和提交数可能看起来正常**。

因此等价性断言必须包含候选顺序本身，且要在截断前后各比一次。这是整组修改中唯一
能抓出该错误的断言。

### tombstone 索引错误

- seeds 与 vertices 等长；
- 只给活动 triangle 的活动 corner 写 seed；
- tombstone fixture 必须覆盖。

### 长期 cache 失效

- 只用单次 sweep 生命周期内的临时 Vec；
- 不向 MeshState 添加 cache；
- 每次 topology/geometry 改变后下一 sweep 自动重建。

### 混入提前停止

- 禁止修改 stop reason 和 max cycles；
- 后续如需 stagnation stopping，另做质量 A/B 计划。

### 拿 checkpoint 路径当性能门（新增）

- checkpoint 运行跳过 quality 与 retirement（§2.5）；
- 退休相关改动一律用阶段 5 的退休门验证，不得用 `LOCATE_PENDING` 运行验证；
- 任何 NXP80 性能数字必须注明是 checkpoint 路径还是完整路径。

## 8. 提交拆分

建议四个独立提交：

1. Make full-cell sweeps linear by seeding Voronoi walks once
2. Derive balance metrics directly from triangle edges
3. Reuse evaluated scales across quality and retirement guards
4. Take the retirement guards off the per-site fan scan

每个提交必须包含 reference 回归测试和独立 NXP40 时间。提交信息遵循 Lore protocol。

## 9. 给 Claude Code 的指令

> 严格按阶段执行。先写 reference 回归，再改生产代码。每阶段跑 NXP40；输出不逐项
> 一致就停止。退休阶段的改动必须用阶段 5 的退休门验证，不得用带
> `EARTHMESH_TEST_LOCATE_PENDING` 的运行验证——那条路径跳过退休阶段。不要实现
> 提前停止、并行化、MeshState 长期缓存或任何新依赖。不要覆盖工作区里与本计划无关
> 的现有修改。

## 10. 完成定义

只有同时满足以下条件才完成：

- 行为对照逐位一致，包含退休候选顺序；
- 所有门禁通过；
- NXP40 ≤ 9.0 s；
- 退休门相对当前代码基线 ≥10× 改善；
- NXP80 checkpoint 的 sites/pending/audit 与当前代码基线一致；
- 完整路径已实测一次，且分阶段耗时已记录；
- 没有新依赖；
- 没有算法、停止条件或 schema 变化。
