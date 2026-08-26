# EarthMesh v3.0.0-alpha5 深度审计报告

审计日期：2026-08-26（Asia/Shanghai）

审计分支：`v3.0.0-alpha5`

范围：根 Rust workspace 的 13 个 crate；本轮未修改 `gui-tauri/`，因此 GUI 独立 workspace 四条闸不适用。

方法：先建立全绿基线；静态追踪索引、元数据、单元类型、判据路径和错误传播；每个逻辑修复先保留失败输出再最小修复；性能结论区分静态复杂度、采样证据和推断。

## 一、缺陷清单（按严重度排序）

### Critical —— 错误结果 / 崩溃 / 数据损坏

本轮没有通过失败测试确认新的 Critical 缺陷。

### High —— 静默错误行为（产物合法但不是被请求的）

| # | 位置 | 问题 | 为什么是 bug | 复现 | 修复 | 状态 |
|---|---|---|---|---|---|---|
| H-1 | `rust/earthmesh_cli/src/region_sources/bbox.rs:16-24`；`circle.rs:20-28`；`close.rs:25-33` | `refine_spc` 的 bbox/circle/close 指令若请求级别大于 `max_iter_spc`，旧路径把解析结果变成 `None` 后返回成功，指令无声消失。 | `refine_spc` 是用户指令，不是可按级过滤的计算判据；输出仍是合法网格，但没有达到指定级别。 | `rust/earthmesh_cli/src/refine_controls/tests.rs:587-627` 同时覆盖三种源。修复前失败为 `expected Err, got []`；修复后得到带源路径和 `requested level 2 exceeds max_iter_spc 1` 的 `InvalidInput`。 | 指定路径先完整解析，再由共享 `require_specified_region_level`（`shared.rs:121-136`）拒绝不可达级别；计算路径仍保留原过滤语义。 | **已修**，`2632015` |
| H-2 | `rust/earthmesh_cli/src/refine_pipeline/global_source.rs:391-410`；`rust/earthmesh_mesh/src/mesh_from_gridfile/mod.rs:59-91,207-242` | 全局多轮 Method-C 把上一轮 gridfile 重新建成 Delaunay 网格时读取了级别元数据，却丢弃 `earthmesh_m_lineage` / `earthmesh_w_lineage`，重建器又生成 identity lineage。 | 第二轮及以后输出的祖先编号可解析、质量检查也可通过，但血缘指向当前行而不是持久化祖先；M-cell 必须映射到 Delaunay W-face lineage，W-cell 必须映射到 Delaunay M-point lineage。 | `mesh_from_gridfile::tests::gridfile_rebuild_restores_persisted_lineage`（`mod.rs:398-430`）。修复前断言得到 identity `2` 而不是输入 lineage；修复后逐侧恢复并检查首行 lineage 为 1。 | 读取 gridfile lineage，校验长度、正值和首行占位语义，做 checked `i64 -> usize`，按 M/W 对偶关系写回内部 lineage。 | **已修**，`8cd4cdf` |
| H-3 | `rust/earthmesh_refine_harp_dv/src/state/mod.rs:245-258,413-418`；`cycle/mod.rs:4082-4092` | HARP-DV 的 `AdaptiveMesh::cycles_completed` 从未递增；所有自适应新站点都被标成 `birth_cycle = 1`，即使它实际产生于后续周期。 | 网格几何有效，但 lineage/诊断元数据错误；叶节点周期分布、追溯和后续审计得到假的单周期历史。 | `cycle::tests::refining_makes_the_demand_go_away`（`cycle/tests.rs:343-357`）固定执行 2 周期。修复前报告为 2 周期但最大 `birth_cycle` 为 1；修复后为 2。 | 每个真正执行完的周期只调用一次 `record_cycle_completed()`；站点仍由持久化 mesh 状态计算出生周期。 | **已修**，`e5e9215` |

### Medium —— 性能 / 健壮性

| # | 位置 | 问题 | 为什么是 bug | 复现 / 证据 | 建议修复 | 状态 |
|---|---|---|---|---|---|---|
| M-1 | `rust/earthmesh_refine_harp_dv/src/state/mod.rs:409-411`；`cycle/mod.rs:3639-3643,3706-3710,4030-4034` | `active_site_count()` 每次扫描全部 site，却被三个需求/重试循环当预算上限。复杂度从 O(D) 放大为 O(D·S)，当 D≈S 时为 O(S²)。 | 这是“用 O(n) 精确计数做防跑飞界”的同族复发；若工作集有 737,000 个站点，一次满需求遍历约检查 `5.43×10^11` 个 `active` 标志，小代理不会显现。 | 静态调用链已确认。两次 4 秒采样在约 500–1,000 站点代理上未把它提升为主热点，说明小算例不能否定生产风险。 | 在 `AdaptiveMesh` 内维护 O(1) 活跃计数，并用插入、成功退休、clone/restart 的不变量测试对拍现有扫描 oracle 后再替换。 | **待修**；未写出可靠红测前未改逻辑 |
| M-2 | `rust/earthmesh_cli/src/refinement_demand/threshold.rs:337-365`；半径来源 `refinement_demand/plan.rs:137-146` | 非周期经度分支对每个输出格重新扫描完整 `(2r+1)²` 窗口；周期分支已经使用逐行滑窗，两个分支复杂度不一致。 | 120 格/度、全球 `43200×21600`、`r=107` 时单判据约 `4.31×10^13` 次格值访问；240 格/度时约 `1.73×10^14`。即使仅 4°×4° 窗口也约 `1.07×10^10` 次。 | 复杂度由嵌套循环直接计数；本轮没有构造大型 NetCDF 性能夹具，因此没有把静态估算冒充实测。 | 复用周期分支的逐行滑窗/列统计；必须保留旧循环为 oracle，在含真实 halo、NaN 和边界裁剪的窗口逐格对拍。若浮点求和顺序改变，需先证明 bit-for-bit。 | **待修**；数值等价守卫缺失，未冒险改写 |
| M-3 | `rust/earthmesh_refine_harp_dv/src/cycle/mod.rs:3063-3117,3159-3173` | 质量优化器对每个站点、每个目标、每个线搜索步重复重建受影响 Voronoi cell并重算 balance/判据/角度/eta。 | 成本随候选数、线搜索步和局部环大小相乘；生产网格会把几何重评分放大，而不是一次局部更新。 | `sample` 两次各 4 秒：`optimise_mesh_quality` 占主测试线程 82.0% 和 99.0%，其中 `propose_move_cached` 占 42.0% 和 68.5%。代理日志：31,378 次线搜索尝试、2,777 个保留移动，优化器 22.0 秒/总测试 23.19 秒。采样文件：`/tmp/earthmesh-harp-sample-1787731382-{1,2}.txt`。 | 缓存候选不变的局部拓扑/尺度项，只对移动改变的三角形增量重算；旧 `QualityScore` 全量闭包必须作为逐候选 oracle。 | **待修**；已确认热点，尚无等价缓存实现或加速比 |

### Low —— 风格 / 边界卫生

| # | 位置 | 问题 | 证据 | 建议 | 状态 |
|---|---|---|---|---|---|
| L-1 | `rust/earthmesh_cli/src/area_judge_domain_builders/seaorland.rs:27,36-48` | `sum_land_grid` 为 `i32`，而函数维度是 `usize` 且无上限；240 格/度全球全陆面有 3,732,480,000 格，超过 `i32::MAX`。 | 120 格/度生产基线 933,120,000 仍安全；本轮没有为 3.7B 格分配测试夹具，因此只记录边界风险，不把它升级成经红测确认的缺陷。 | 输出计数改为 `usize`/`u64` 前，先补一个不需要巨型分配的 checked-counter 单元测试，并审计序列化边界。 | **待验证** |

## 二、性能热点分析

### P-1 HARP-DV 预算界中的 O(n) 精确计数

- **位置**：`AdaptiveMesh::active_site_count`（`state/mod.rs:409-411`）及三个循环调用点（`cycle/mod.rs:3640,3707,4031`）。
- **复杂度**：每次 O(S)，一轮 D 个需求为 O(D·S)；D≈S 时 O(S²)。
- **生产规模估算**：若 S=737k，约 `5.43×10^11` 次标志检查/满轮；S=10 亿时上界为 `10^18`，不可运行。
- **采样结论**：当前 NXP6 代理站点数太小，4 秒样本未显示该函数为主栈；这是生产规模复杂度证据，不是假装成小样本热点。
- **优化方案/预期收益**：维护 O(1) 活跃计数，可把预算检查从 O(D·S) 降为 O(D)，理论上单满轮减少约 S 倍。
- **实测加速比**：未修改，暂无；需要扫描 oracle 对拍插入/退休/restart 后再测。

### P-2 非周期标准差窗口重复扫描

- **位置**：`refinement_demand/threshold.rs:337-365`。
- **复杂度**：O(N·(2r+1)²)；周期路径为滑窗，非周期路径仍为朴素窗口。
- **生产规模估算**：120 格/度、r=107 为 `4.31×10^13` 次/判据；240 格/度为 `1.73×10^14` 次/判据。
- **优化方案**：按纬度行做列和/列平方和滑窗，或积分图 O(1) 查询；保留 NaN count/sum/sum_squares 的旧实现 oracle。
- **预期收益**：由每格 46,225 次访问降为常数次更新，理论数量级约四万倍；内存带宽和 NetCDF 读入后实际更低。
- **实测加速比**：未修改，暂无；bit-for-bit 求和顺序风险未解除。

### P-3 HARP-DV 质量候选重复重评分

- **位置**：`cycle/mod.rs:3092-3117,3159-3173`。
- **复杂度**：O(P·K·L·R)，P 为 pass，K 为候选站点，L 为线搜索步，R 为局部 Voronoi/判据重评分成本。
- **采样**：测试 `protected_segments_make_a_quality_target_terminate`；两个独立 4 秒样本均进入真实测试线程，优化阶段分别占 82.0%/99.0%，总运行 23.19 秒中优化器自报 22.0 秒。
- **优化方案**：缓存未变局部拓扑、只重算移动影响的三角形；不能删全量评分，先作为 oracle。
- **预期收益**：日志显示平均每个保留移动约 11.3 次线搜索评分，增量化上限可接近这一重复因子；实际收益需基准验证。
- **实测加速比**：未修改，暂无；本轮只确认热点，不在无逐候选对拍时改数值路径。

### 内存说明

`RefinementDemand` 已使用 `Vec<u64>`（`refinement_demand/mod.rs:61-118`）。Rust 的 `Vec<bool>` 本身也是位打包；Area_judge 的二维 `Vec<Vec<bool>>` 在 120 格/度约 111.24 MiB/完整矩阵，240 格/度约 444.95 MiB/矩阵，另有行分配开销。风险来自同时存在多张全域矩阵和全量扫描，不应按“一布尔一字节”误算。

## 三、模式总结

### 同族复发

1. **指令与判据共用 `Option` 过滤语义**：H-1 与 §11.1 同族。守卫应区分 `refine_spc`（不可达即错误）和 `refine_cal`（允许按级过滤/提升）。
2. **文件边界元数据只接了一半**：H-2 与“新建/迁移行元数据默认 0/identity”同族。几何 round-trip 通过不能代表 lineage round-trip 通过。
3. **报告有局部计数器，持久状态却不更新**：H-3 是“分支执行了但元数据没走同一路径”的变体。
4. **精确 O(n) 计数进入热循环**：M-1 与已知 `triangle_count()` 事故完全同族，只是对象从 triangle 换成 site。
5. **同一判据的周期/非周期路径复杂度不一致**：M-2 属于多路径一致性问题；语义相同不代表成本相同。

### 最值得新增的守卫

- **Gridfile 元数据 round-trip 矩阵**：对 M/W 两侧的 lineage、refine level、original level、ngr，覆盖首行占位、一个继承行、一个新建行，写文件→读文件→重建→再次写文件，逐行相等。
- **指定区域不可达级别表驱动测试**：bbox/circle/close × NML/NetCDF，统一断言错误包含源路径和请求/最大级别。
- **活跃计数不变量**：任何插入、回滚、成功退休、失败退休、clone/restart 后，O(1) 计数必须等于扫描 oracle；预算循环只准调用 O(1) 值。
- **标准差双实现 oracle**：含 halo、跨经线、NaN、边界截断和真实窗口；优化路径逐格、逐 bit 对拍旧实现。
- **HARP 多周期血缘**：至少一处第二周期插入，断言 `report.cycles_completed` 与 `birth_cycle` 分布一致。

### A–H 清单结论

- **A 索引/边界**：复核 one-based 转换、halo 和经线周期路径；未通过新红测确认额外 off-by-one。现有 dateline/pole/halo 测试继续全绿。
- **B 元数据完整性**：确认并修复 H-2、H-3。
- **C 单元种类**：H-2 的 M-cell/W-face、W-cell/M-point 对偶映射已用双侧断言锁定；未确认新的 tri/hex 混用。
- **D 多路径一致性**：确认 H-1 和 M-2；计算区域过滤语义未被指定区域修复改变。
- **E 静默吞错**：确认 H-1；其余 `.ok()` 使用只有在失败测试证明会吞掉用户指令后才应升级，未凭代码外观修改。
- **F 数值/资源**：记录 L-1 和三个性能项；Fortran 对拍所需的 f32 截断未“现代化”。
- **G 并发**：rayon 路径按独立行写 bit；现有确定性测试全绿，未确认共享写或浮点归约顺序新缺陷。
- **H 死代码/误配置**：确认 H-3；`cycles_completed` 过去只有读没有写。

## 四、基线记录

### 审计前

| 命令 | 结果 | 墙钟时间 | 警告/备注 |
|---|---:|---:|---|
| `cargo build --release` | 通过 | 34.74 s | rustc 警告 0 |
| `make fmt` | 通过 | 1.43 s | 13 crate 均 `--check` |
| `make clippy` | 通过 | 254.20 s | `-D warnings`，clippy 警告 0 |
| `make test-fast` | 通过 | 972.78 s | 69 个 test-result 分组，全绿 |
| `make test` | 通过 | 2133.94 s | 210 个 test-result 分组，全绿；Method-C/HARP/CLI 慢套件均执行 |
| `./mkgrd.x examples/00_quickstart_n16.nml` | 通过 | 0.43 s | 输出 `cases/quickstart_n16/gridfile/gridfile_NXP0016_01_hex.nc4`；`sjx=5121`、`lbx=2563` |

### 修复级验证

| 修复 | 红测证据 | 绿测/回归 |
|---|---|---|
| H-1 | 指定区域测试修复前返回空区域而非错误 | 定向 CLI 单测通过；结束 `make test-fast` / `make test` 覆盖 |
| H-2 | lineage 重建修复前得到 identity `2` | `cargo test -p earthmesh_mesh`：113 单测及全部 integration/doc tests 通过，625.69 s |
| H-3 | 2 周期 fixture 修复前最大 `birth_cycle=1` | 定向红绿；`cargo test -p earthmesh_refine_harp_dv`：110 passed、10 ignored，159.67 s |

### 审计结束

| 命令 | 结果 | 墙钟时间 | 警告/备注 |
|---|---:|---:|---|
| `make fmt` | 通过 | 1.37 s | 无格式差异 |
| `make clippy` | 通过 | 29.10 s（增量） | `-D warnings`，0 警告 |
| `make test-fast` | 通过 | 450.44 s（增量） | 全绿 |
| `make test` | 通过 | 2052.08 s | 210 个 test-result 分组，全绿；失败分组 0 |
| `./mkgrd.x examples/00_quickstart_n16.nml` | 通过 | 0.04 s（增量） | 再次输出 `sjx=5121`、`lbx=2563` |

工作树中审计开始前已存在的 `.omx/` 运行态改动、`.workbuddy/`、`docs/audit_2026-08-26.md` 等未纳入任何提交；本轮未触碰 `.workbuddy/`。
