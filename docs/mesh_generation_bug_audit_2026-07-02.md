# EarthMesh 核心网格生成代码深度审查报告

日期：2026-07-02（同日更新：修复状态 + OLAM r1095 对照结论，见第五、六节）
范围：`rust/earthmesh_mesh`（约 17.5k 行源码，170 文件）、`rust/earthmesh_geometry`（1.2k 行）、`rust/earthmesh_quality`（3.2k 行）、`rust/earthmesh_refine_planner`（0.9k 行）。
方法：12 个子系统分片并行逐行审查（重推导公式、手工模拟拓扑操作、与单测/集成测试交叉验证），随后对每项候选发现人工复核、剔除误报。仓库中无 Fortran 原始源码（仅迁移文档），涉及"与 Fortran 保真"的判断以此为限。

## 总体结论

代码质量整体很高。这是一次异常严谨的 Fortran→Rust 移植：1-based 占位约定全库一致，单测大量采用独立推导的期望值（而非回归自身实现），关键数值路径（弹簧松弛、球面面积、投影正反变换）均有 Fortran 参考数值夹具。**未发现会破坏当前主流水线输出的高危 bug**。确认了 3 项当前可触发的真实缺陷（均在新增 v3 功能面，非核心 Fortran 移植路径）、2 项潜在保真偏差（需对照 Fortran 源码定夺）、以及若干低危/健壮性问题。

## 一、确认的真实缺陷（当前可触发）

### 1. quality：`negative_area_cell_count` 门控永远不会触发
`rust/earthmesh_quality/src/lib.rs:112,502` + `rust/earthmesh_geometry/src/lib.rs:74-84`

`GeometryMetrics::negative_area_cell_count` 被声明、参与 `evaluate()` 的 Fail 门控（lib.rs:502）、写入 `quality_summary.json` 和 GUI，但全仓库没有任何一处对它做过递增（grep 仅见声明/门控/序列化三处）。唯一面积来源 `polygon_area` 返回 `total.abs() * 0.5`，数学上恒非负。该"灾难性错误"检查是永远显示通过的死门控——绕排/自交单元只能靠 `self_intersection_count`/`invalid_polygon_count` 兜底，标准不同，存在漏检面。
建议：用 `signed_area`（geometry crate 已有）判断绕向并递增该计数，或删除该字段与门控。

### 2. quality：NaN 顶点坐标会静默毒化 `cell_area` 统计
`rust/earthmesh_quality/src/lib.rs:315-327,58-79`

`validate_polygon` 能检出 `NonFiniteCoordinate` 标志，但调用点从不检查该标志；`polygon_area` 无有限性守卫，NaN 顶点 → `area = NaN`；IEEE 语义下 `NaN <= 1.0e-12` 为 false，NaN 被 push 进 `areas`。下游 `Stat5::from_slice` 的 mean/std/cv 全部变 NaN，而 min/max 用 `f64::min/max` fold 会跳过 NaN 保持有限——产生内部自相矛盾的统计块，且该单元不计入任何异常计数（`io.rs` 的 `num()` 还会把非有限值写成 JSON `null`，进一步掩盖）。入口 `grid_quality_inputs/gridfile.rs` 直接从 gridfile 原始数组构造 `Point`，无过滤，路径可达。
建议：在 flags 判断后追加 `NonFiniteCoordinate` 分支（计数并跳过），或给 area 分支加 `area.is_finite() &&`。

### 3. olam：Bbox/Polygon 区域在 Cartesian-XY 模式下校验放行但永不匹配
`rust/earthmesh_mesh/src/olam_region_selection/mod.rs:121,145` + `olam_region_validation/mod.rs:145`

`contains_cartesian_xy`/`close_to_cartesian_xy` 对 `Bbox`/`Polygon` 无条件返回 `false`，而 `validate_cartesian_xy()` 对这两个变体却放行（仅委托经纬度 `validate()`）。`spawn_nest_cartesian_xy_*` 三个公开入口无过滤直通。后果：种子锚点仍会被 `closest_m_point_to_region_anchor` 选中（只用 anchor 经纬度、与包含判定无关），BFS 扩张永不生长，`rad3` 足迹绕过"全未选中"守卫——**静默细化一小块错误区域**，而不是报错。
建议：要么为两变体实现 Cartesian 包含判定，要么让 `validate_cartesian_xy()` 显式拒绝（与 `Circle`/`Corridor` 的 `validate()` 拒绝模式对称）。

## 二、潜在 Fortran 保真偏差（需对照原始 .F90 定夺，仓库内无源码）

### 4. `refine_iter_c/mod.rs:112`：`mrl_in[neighbor] = 2` vs iterB 的 `+= 2`
iterB（`refine_iter/mod.rs:64`）对已细化三角形的邻居做 `+= 2` 累加，iterC 做 `= 2` 覆盖。复核后注意：两内核的传播结构本就刻意不同（iterB 的 `mrl_bk` 从 `mrl_in.clone()` 起步、iterC 每轮 `fill(0)` 重置；终局用途也不同），"同一模式移植两次出现抄写错"的推断不成立；且现有单测（`iter_c_marks_single_refined_hex_neighbors_...`）编码的是当前 `=` 行为。当一个未细化三角形同时邻接 ≥2 个已细化三角形时两种语义产生不同的 `ref_sjx` 标记。**建议对照 `MOD_refine.F90:iterC_judge` 原文确认**；端到端 Fortran 对比样例（default/merit 五例）曾通过，倾向当前实现正确，但多重邻接构型未必被样例覆盖。

### 5. 弹簧平滑中同一条边"自身/邻居"两种角色的 f32 截断不一致
`spring_dynamics/mod.rs:35-56`（邻居距离，全 f64）vs `spring_edge_dynamics/mod.rs:19-27`（自身边向量 `as f32 as f64` 截断）

若 Fortran 只有一个 `dist()` 数组同时供两种角色使用，则 Rust 两径精度不一致会造成逐次迭代的数值漂移（地球半径量级坐标下 f32 截断损失约亚米级，与弹簧修正同量级）。姊妹移植 `icosahedron_spring_grid` 用单一数组、两角色一致。但也可能 Fortran 原文就在子程序内局部重算自身边长（混合精度）——Rust 有专门单测断言这一截断"like_fortran"，说明移植者细读过该处。**需对照 grid_preprocess 弹簧子程序原文**。现有端到端坐标对比（非极区 1e-3°容差）通过，实际影响未证实。

## 三、低危 / 健壮性 / 显示类问题

6. **`get_sort_new/mod.rs:76`** — 邻接行走循环 `for j in 1..num_inter` 跳过 0 号索引（同函数度数统计用 `0..num_inter`）。复核结论：对合法输入（单链/闭环）被 fallback「取第一个未用元素」精确补偿，输出与全范围扫描一致，多链情形亦验证一致；仅在畸形（度≥3）输入下可能偏离 Fortran。属"侥幸正确"的脆弱代码，建议改为 `0..num_inter` 以消除隐患。

7. **`refine_onedivide_two/mod.rs:95`** — 用 `rfind`（最后命中）选 split 邻居，5 个姊妹场景用 `find`（首命中）。注意 `rfind` 恰是 Fortran 无 `EXIT` 赋值循环（后写覆盖）的忠实等价物，很可能有意为之；当三角形有 ≥2 个满足态邻居时二者选边不同，影响子三角形顶点选取。现有单测全部用无差别构型（邻居行如 `[3,3,3]`），未覆盖多候选。建议补一个多候选单测并对照 Fortran `OnedivideTwo` 原文。

8. **`mesh_cell_vertex_shared_edges/mod.rs:47-68`** — 纯拓扑 fallback 顶点环仅按顶点 ID 大小决定行走方向，无几何定向校正；消费方 `connect_on_cell` 只验证相邻共享边，CW 环可静默通过，下游按 CCW 假设算面积/角度会得到镜像多边形。仅在几何排序器失败的 fallback 路径触发。建议环成后用叉积对单元中心做定向校验，负则反转。

9. **`mask_postproc_boundary/mod.rs:122-128`** — "第二长边界曲线"跟踪逻辑无降位（新最长者不把旧最长值挪入 `[1]`）且永久排除 curve_id 1，构造序（4,6,2）下得 2 而非 4。已核实 `num_bdy_long[1]` 目前全仓库只写不读（消费者仅读 `[0]`/`[2]`），无现实影响；且该写法形似 Fortran 直译，改前需对照原文。

10. **`mesh_triangle_topology/mod.rs:66-80`** — `neighbor_count == 3` 提前退出判断放在外层循环，实际从不提前退出；合法流形输入下因写幂等而无害，畸形输入下可能覆盖已正确的槽位。建议将判断移入内层循环。

11. **quality `lib.rs:521-541`** — `min_angle` Fail 用严格 `<`、`aspect_ratio` Fail 用 `>=`，两门控对"恰在阈值"处理不一致（边界值罕见，影响仅表述一致性）。

12. **quality `lib.rs:201-210`** — `WorstCell.centroid` 用朴素 (lon,lat) 平均，跨日界线单元的 GeoJSON 标记会落在地球另一侧（仅诊断显示，不影响判定；同文件 `interior_angles_deg` 已为同一原因改用 3D 弦法，此处遗漏）。

13. **`olam_emit/mod.rs`** — `emit_method_c_tables`（全库最密集的手写父→子重映射）返回前不做 `validate_topology()`，而 `olam_spring`/`olam_nest_spring` 均自检输出。防御纵深建议：`Ok(mesh)` 前加一行校验。

14. **`olam_dump/mod.rs:15-27`** — 拓扑转储的 M 邻居 " im" 列写死 `[1;7]` 占位，未从 `u_edges` 推导，与文档所称"供 Fortran 对拍工具比对完整表"不符（仅诊断工具，不影响网格）。

15. **`olam_cart_hex_outer_pair/mod.rs:18,61`** — 外环邻居排序失败时静默回退原始顺序而不报错；`validate_topology` 不检查 `iw[3..9]`，错误排序可无声下行。建议改为报错或 debug 断言。

## 四、审查后判定为干净的分片

icosahedron 构建/菱形/邻居派生、gridinit 因子分解、Voronoi 对偶与 PCVT、坐标转换（分片 1）；球面投影/外心/质心/l'Huilier 面积/测地距离/层距（分片 2，公式全部独立重推导，正反投影矩阵互为转置逐项验证）；LOP 翻边绕向与镜像折叠、1→4 细分连通性（分片 4 除上述第 7 条外）；OLAM spawn 计数簿记、五/六/七邻环遍历、重建重编号（分片 7 除第 15 条外）；OLAM 区域/周界/走廊（分片 8 除第 3 条外，日界线中点选择经多例数值验证）；method_c 密集重映射逐分支对照独立推导的测试预言全部吻合（分片 9 除 13/14 条外）；geometry/refine_planner crate（haversine 用 atan2 形式天然免疫反对径 NaN；地球半径常量 6_371_229 m 全库一致；WeightedMax 冗余分支代数上惰性无影响）。

## 五、修复状态（2026-07-02 当日）

- 第 1 条（负面积死门控）：**已修复**。`compute()` 用 `signed_ring_area`（经度展开后）检测 CW 绕向并递增计数；新增 `non_finite_cell_count` 字段与门控；io.rs JSON 同步；新增单测含跨日界线 CCW 不误报用例。
- 第 2 条（NaN 毒化统计）：**已修复**。`NonFiniteCoordinate` 单元计数并整体跳过几何统计；边长循环加有限性守卫；worst_cells 跳过 NaN 环；邻居索引校验移到几何块之前保持行为。
- 第 3 条（Bbox/Polygon Cartesian-XY）：**已修复**。`validate_cartesian_xy()` 显式拒绝并报错；新增单测。
- 第 5 条（弹簧 f32 口径不一致）：**已确认为真 bug 并修复**（OLAM `spring_dynamics_globe` 745-757 行证实参考实现用单一截断 `dist` 数组同时服务自身/邻居角色）；`spring_dynamics` 的邻边距离改为与 `spring_edge_adjustment_fortran` 相同的分量截断口径。
- 第 6 条（get_sort_new 索引 0）：**已修复**（`1..` → `0..`）。
- 第 8 条（fallback 顶点环 CW）：**已修复**。函数签名新增顶点/单元坐标参数，环闭合后按几何排序器同一判据（`cross·外向法线`）定向，负则保持起点反转其余；3 个调用点与测试同步更新，新增 CW 反转用例。
- 第 10 条（triangle_topology 提前退出守卫）：**已修复**（只计首次填槽、判断移入内层循环；合法输入行为不变）。
- 第 11 条（阈值比较不一致）：**已修复**（aspect 改严格 `>`，与 min_angle 口径一致并注释）。
- 第 12 条（跨日界线质心）：**已修复**（经度展开平均后回绕）。
- 第 13 条（olam_emit 缺输出校验）：**已修复**（`Ok(mesh)` 前加 `validate_topology()?`）。
- 第 14 条（olam_dump 占位列）：**已修复**（由 `u_edges` 推导真实 M 邻居）。
- 第 15 条（outer_pair 静默回退）：**行为面经 OLAM 证实无误**（`spawn_nest.f90` 1443-1506 行两种镜像改写模式与 Rust `_after`/`_before` 一一对应）；静默回退处加 `EARTHMESH_OLAM_DEBUG` 警告。
- 第 4 条（iterC `=` vs `+=`）：**已定案，非 bug**。EarthMesh-2.0.0 `MOD_refine.F90:760` 即为 `mrl_in(ngrmm(j,i)) = 2`，注释明言"只存在 0 和 2 两种情况，因为大于 2 的情况在 iterB 中已被细化"；iterB（641 行）确为 `+2` 累加。两内核不对称是原版设计。Rust 未改动。（注：v2 的 iterC 无 set_dis 传播核，Rust 移植自更新版本，此为旁证而非逐行对照。）
- 第 7 条（rfind）：**已定案，非 bug**。`MOD_refine.F90:1600-1608` 的选邻循环无 `EXIT`——后写覆盖、最后命中生效，`rfind` 是忠实移植。未改动。
- 第 9 条（num_bdy_long 次长曲线）：**已定案并双侧修复**。缺陷源自 Fortran 原版 `MOD_mask_postproc.F90:1294-1305`（bdy_connection_closed_curve），Rust 为逐行忠实移植。三处错误：新最长出现时旧最长不降位、`num_closed_curve /= 1` 永久排除第 1 条曲线、`< num_bdy_long(1)` 排除等长并列。两边该值原本均只写不读（Fortran 仅进日志、Rust 无生产消费者），故修复不影响任何网格输出。已按两槽位最大值跟踪同步修正 Fortran（EarthMesh-2.0.0）与 Rust（mask_postproc_boundary），并更新受影响单测断言（`[5,1,2]`→`[5,4,2]`）。
- 第 5 条补充定案：EarthMesh-2.0.0 `MOD_grid_preprocess.F90:762,816-819` 证实 `dist` 声明为 `real(r8)`、分量经无 kind 的 `real()` 截断后以 r8 求模——与本次修复的"截断分量 + f64 模"口径逐行一致，且自身/邻居共用同一 `dist` 数组。修复方向精确无误。
- shard-9 遗留疑点（olam_spring 迭代后整体 f32 截断）：**经 OLAM 906-908 行证实为忠实模拟，非 bug**。

## 六、OLAM r1095 原始 Fortran 网格生成代码审查（omodel/）

全文逐行审查 icosahedron.f90、hex_grid.f90、fill_itabs.f90、spawn_nest.f90、cart_hex.f90、triangle_utils.f90、spring_dynamics.f90（globe 部分本人复核）。icosahedron、fill_itabs、cart_hex 干净。发现（均属"对畸形/退化输入无防御"的潜伏缺陷，正常网格不触发）：

1. **[中危] spawn_nest.f90:1255-1264 与 1284-1293** — `perim_fill3` 中两处 if/elseif 链无 else 兜底，`iu15/iu25/iu16/iu26` 未初始化即可能被用于 `itab_ud(...)` 下标（1266、1272、1341-1363 行）。三条候选边均不满足"两端点为真实 M 点"时读栈上垃圾值，静默破坏过渡区拓扑。该文件其他分支全部有 `olam_stop` 兜底，唯此两处缺失。建议加 else + `olam_stop`。
2. **[中危] hex_grid.f90:536-560** — `dnu/dnv` 作除数先于任何退化检查（564 行的 `iw1<2` 检查在除法之后且不查 `im1==im2`）；零长边 → Inf/NaN 静默流入 `arw0/arm0/quarter_kite`。
3. **[低危] spawn_nest.f90:1643-1692** — `perim_map2` 周界行走无迭代上限，且写入 `npts=10000` 定长数组前不查界；行走不闭合时越界写而非报错。
4. **[低危] spawn_nest.f90 `thirdm`（2155-2185）** — 六边形假设仅对起点 clamp（`min(npoly,6)`），对行走中途的 `imm/immm` 不验证；邻近五边形时 `iuu/iuuu` 可能未赋值。
5. **[低危] hex_grid.f90:360-371、779** — 外心/叉积分母对共线退化无守卫（PCVT 的 303 行 skip 只查邻居索引有效性，不查几何退化）。
6. **[低危] triangle_utils.f90:504、532** — `matinv3x3/2x2` 奇异阈值 `1.e-12` 对单精度无意义（机器 ε≈1.2e-7），阈值与 ~1e-6 之间的病态矩阵静默给出不可靠解；且失败走裸 `stop` 无恢复路径。
7. **[低危] spring_dynamics.f90:770-771** — `twocosphi` 除以 `dist(iu1)*dist(iu2)` 无零守卫（Rust 移植版返回 None，更稳健）。
8. **[极低] hex_grid.f90:2175/2190/2280** — 地形切割插值用 `1.e-25` 作单精度近零判据，等同于"非位相等"。

这些不影响"正确输入 → 正确输出"，Rust 移植也不应逐位复刻其脆弱性（EarthMesh 现有防御性校验普遍优于原版）；但若你仍维护/运行 OLAM 本体，第 1、2 条值得在 Fortran 侧修补。

**上述 8 条在 Rust 移植中的状态（逐条核实）**：第 1 条 → `method_c_split_outer_edges`（olam_table_helpers:40-68）三轮候选耗尽后返回带诊断的显式 Err，且 Rust 语言层面不存在"读未初始化变量"，已防御；第 3 条 → `olam_selection_topology` 对"无对边/不在环内"均显式报错并按各点实际 npoly 遍历，已防御；第 6 条 → `spherical_circumcenter` 对共线/退化有守卫（回退重心、择优分支），已防御；第 7 条 → `spring_edge_adjustment_fortran` 对任意零距离返回 None，已防御；OLAM 第 2 条对应的 `perim_map2_method_c_from` 用 Vec 动态增长 + BTreeSet 访问集（重访即报错）+ `perim_ngr` 无法前进即报错，已防御。第 4 条（matinv）、第 5 条（ctrlvols 的 dnu/dnv）、第 8 条（地形切割 1e-25）在 EarthMesh 中无对应移植代码，不适用。结论：8 条中 5 条有 Rust 对应物且全部已有防御，3 条不适用——OLAM 的这些缺陷没有一条泄漏进 Rust。

## 七、限制与后续建议

- 沙箱无 Rust 工具链且仓库挂载不稳定，本次未能运行 `cargo test`/`cargo clippy`；上述发现全部来自静态逐行审查。建议本地跑一遍 `cargo test --workspace` 与 `cargo clippy --workspace -- -W clippy::correctness` 作交叉验证。
- ~~第 4、5、7、9 条的最终定夺需要原始 `.F90` 源码~~ 已用 `/Users/zhongwangwei/Desktop/EarthMesh-2.0.0/src` 全部定夺（见第五节）。注意 v2 与 v3 所移植版本存在差异（如 iterC 传播核、GetSortNew 的 degree-1 起点与 fallback 均为更新版特性），v2 仅作旁证时已注明。建议把移植时实际参照的 Fortran 版本检入仓库或在迁移文档附摘录，避免以后再遇到无法对照的情况。
- 建议补测试：iterC 多重邻接构型、onedivide_two 多候选邻居、shared_edges fallback 的 CW 输入、quality 的 NaN 顶点输入。
