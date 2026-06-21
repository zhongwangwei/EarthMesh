# OLAM Method-C Line Mapping (v1)

## Scope

- Fortran source: `/Users/zhongwangwei/Desktop/olam-model-code-r1095-trunk/omodel/spawn_nest.f90`
- Rust source: `/Users/zhongwangwei/Desktop/Github/EarthMesh/rust/earthmesh_mesh/src/lib.rs`
- Test coverage: `/Users/zhongwangwei/Desktop/Github/EarthMesh/rust/earthmesh_mesh/tests/olam_spawn_nest.rs`
- CLI handoff: `/Users/zhongwangwei/Desktop/Github/EarthMesh/rust/earthmesh_cli/src/lib.rs`

## Method-C migration map (核心)

- Fortran `subroutine spawn_nest` (`spawn_nest.f90:1-1126`)  ↔  Rust
  - `OlamDelaunayMesh::spawn_nest` (`lib.rs:1941`)
  - `OlamDelaunayMesh::spawn_nest_as_atmosmesh` / `spawn_nest_as_atmosmesh_with_max_mrows`
    (`lib.rs:1953-1987`, `2022-2059`)
  - `OlamDelaunayMesh::spawn_nest_cartesian_xy`（及 `with_max_mrows`/`with_spring` 变体）
    (`lib.rs:1991-2099`)
  - `OlamDelaunayMesh::spawn_nest_internal` (`lib.rs:2106-2198`)

  语义对照:
  - `mdomain<2`/`mdomain>=2` 分支与 Fortran 在 `spawn_nest` 里 `iatmgrid` 与几何入口的差异，已体现在 rust 的两类分派器（地理球面/投影平面）里。
  - Fortran 的“同级 region 逐 pass 处理、`MAX_ITER_SP` 驱动”行为，对应 rust 的 pass 循环和 `max_level` 调度。
  - Fortran 的 `method_c` 专用路径切换点已内嵌于 `spawn_nest_internal` 分支选择。

- Fortran `perim_map2` + `perim_ngr` + 与 W/M 闭合逻辑 (`spawn_nest.f90:1569-1775`)  ↔  Rust
  - `perim_map2_method_c` (`lib.rs:3016-3072`)
  - `perim_map2_method_c_from` (`lib.rs:3046-3069`)
  - `perim_ngr_method_c` (`lib.rs:3109-3142`)
  - `emit_method_c_tables` (`lib.rs:3142-3341`) 接收 perimeter 三元组后重建局部拓扑

  语义对照:
  - `perim_map2` 的 perimeter 起点筛选（`nwdiv == 2` 的凸角）→ `perim_map2_method_c`
  - `perim_ngr` 的 CCW 步进与 inside/outside `nest_wd(iw).iw(3)` 标志规则 → `perim_ngr_method_c`
  - perimeter 长度非 3 的倍数直接报错 → rust error message 包含 `Method-C perimeter length invalid`，并在测试中断言。
  - `emit_method_c_tables` 对应在 rust 内联执行的 M/U/W 标号表重建 + 逐点验证流程。

- Fortran `perim_mrow` (`spawn_nest.f90:1779-1902`)  ↔  Rust
  - `apply_olam_perimeter_mrows` (`lib.rs:3795-3874`)

  语义对照:
  - 外围行按 `max_mrows` 层扩散并传播符号（边界向外/向内）
  - 对于已经存在的兼容旧行保留、冲突行报错/重写规则均有等价测试覆盖。
  - `olam_perim_mrow_uses_fortran_half_step_row_growth` 锁定 Fortran `mod(irow,2)` 的隔步行号增长规则。

- Fortran `fill_rad3` (`spawn_nest.f90:2215-2287`)  ↔  Rust
  - `mark_fill_rad3_faces_with_neighbors` (`lib.rs:2754-2769`)
  - `mark_olam_fill_rad3_faces`（若干内部调用点）

  语义对照:
  - 以中心 M 点为起点沿相邻环展开 3/6 邻接选择
  - 在 cart_hex 周期副本场景下排除副本 W face 的标记。
  - `olam_fill_rad3_marks_six_neighbors_of_three_distant_m_points_like_fortran` 锁定 Fortran `im1/im2/im3` 远端 M 点的 6 邻接 W 标记规则。

- Fortran `thirdm` (`spawn_nest.f90:2103-2211`)  ↔  Rust
  - `olam_thirdm_neighbors_fortran_with_neighbors` (`lib.rs:2641-2695`)
  - `opposite_ring_u_edge_with_neighbors` (`lib.rs:2703-2732`)

  语义对照:
  - 从当前 M 点沿选中 U 边走到相邻 M 点，再按 Fortran `mod(jj+2,6)+1` 规则连续选择两次对边。
  - `olam_thirdm_walks_straight_opposite_edges_and_marks_reciprocal_done_like_fortran` 锁定第三邻点结果和远端 reciprocal `jdone` 标记规则。

- Fortran `perim_fill3` (`spawn_nest.f90:1130-1565`)  ↔  Rust
  - `perim_fill3_method_c` (`lib.rs:3539-3794`)
  - `fill_method_c_full_subdivision` (`lib.rs:3426-3520`)

  语义对照:
  - 以 3-边组处理 perimeter 分段，建立子三角片段并更新中间 U/W 表
  - `fill_method_c_full_subdivision` 在 rust 执行 child-face 全量细分，调用 `perim_fill3_method_c` 衔接几何构造。
  - `method_c_split_outer_edges` (`lib.rs:4225`) 与 Fortran outer-edge 拆分逻辑一致，作为边界拼接断言点。
  - `olam_method_c_keeps_fortran_linear_coordinates_before_projection` 锁定 Fortran `perim_fill3` 先写未投影线性 M 坐标，随后由 `spawn_nest` 半径投影阶段处理。
  - `olam_perim_fill3_writes_fortran_weighted_transition_coordinates` 锁定 Fortran `im19/im18/im17/im20/im12/im13` 六个显式加权坐标公式、`im17/im20/im18/im19` 的局部 `mrlm_orig` 赋值规则、`im22..im26`/`iw20,iw26..iw32` 的 `ngr` ownership 写入、`im22..im26` 不改 `mrlm/mrlm_orig` 的过渡邻点 ownership 规则、transition W/U 表的 edge、endpoint、adjacent-face 写入、`iw8/iw19/iw20/iw27/iw29/iw31` exact W-face `iu` 槽位顺序、关键 transition U-edge exact endpoint/adjacent-W 槽位顺序，以及 `iu33` special-case endpoint 写入。
  - `olam_method_c_projection_matches_fortran_radius_expansion` 逐点锁定 Fortran `expansion = erad / sqrt(x^2+y^2+z^2)` 投影公式。

- Fortran `spawn_nest` 后处理中的 mrow/spring/mlevel 标记逻辑  ↔  Rust
  - `spawn_nest_pass_method_c` (`lib.rs:2868-3405`)
  - `spawn_nest_pass_with_max_mrows` (`lib.rs:2858`)
  - `spawn_nest_with_spring_*` 系列（`1999-2099`）

  语义对照:
  - 逐 pass 只生成一个新网格号（grid-number）
  - 每个 face 的 `ngr/mrlw/mrow` 映射保留，父级 halo 行为一致
  - atmosphere 与 surface 的 `max_mrows` 默认与常量 `METHOD_C_MAX_MROWS_*` 对齐

## CLI handoff map

- Legacy `run_mkgrd_*_namelist` 分派链（global / specified refine / regional）逐步引入 Olam 直达入口
  - `run_mkgrd_olam_specified_refine_global_source_namelist` (`lib.rs:8916`)
  - `run_mkgrd_top_level_namelist` 及各个 migrated executor 分支（约 `10904`, `11510`, `12053`）
  - Method-C 运行参数封装/校验：
    - `olam_method_c_spring_iterations` (`lib.rs:9535`)
    - `olam_native_method_c_spring_iterations` (`lib.rs:9548`)
    - `validate_olam_native_method_c_spawn_mdomain` (`lib.rs:9570`)

  语义对照:
  - 非重启路径已进入 Olam 指定区域细化主干；`method_c` 的边界宽度/step 分支由 mesh 层函数负责。
  - 本地 `method_c` 风格测试位于 CLI `olam_mkgrd_pipeline`（新建文件，未全部标记为 [ ] 里程碑）中。

## 测试覆盖（可复用清单）

- `rust/earthmesh_mesh/tests/olam_spawn_nest.rs` 覆盖项（按语义）
  - 指定区域/同级与异级 pass 行为
  - `max_mrows`/`max_iter_spc` 与 `LEVEL` 语义
  - `overlapping region`, `tiny region`, `parent halo`, `perimeter` 报错条件
  - cart_hex 周期副本稳定性
  - cartesian_xy 下坐标投影与 `deltax` 行为一致性
  - `perim_mrow` 与 `fill_rad3` 与 Fortran 分支对比
  - mrow 标记、mrlm/mrlw 语义、边界行一致性、拓扑闭合验证

- `rust/earthmesh_cli/tests/olam_mkgrd_pipeline.rs`
  - 全球、区域、land/ocean 与 coupled 场景的端到端路径（`run_mkgrd_olam_*` 分支）
  - `default_atmos_global_specified_refine_uses_olam_spawn_nest` 锁定 namelist 指定细化进入 `OlamRefineGlobalSource`，`RL%SpringGlobal_type=1` + `RL%niter_refine=2` 触发 Method-C nest spring，输出 gridfile 可读回且拓扑一致。
- `rust/earthmesh_cli/tests/olam_method_c_gridfile_handoff.rs`
  - Method-C one-based `itab_w%npoly` → compact `n_ngrwm` gridfile handoff；锁定 7-corner/3-corner W face valence 不被 fallback 5/6 逻辑覆盖，并通过 NetCDF 写读回归。

## 当前未决（需要继续的精确差异）

1. 最终 Fortran OLAM `.h5` gridfile tolerance 对比仍未完成。
   - 已确认 Fortran gridfile 写入口在 `/Users/zhongwangwei/Desktop/olam-model-code-r1095-trunk/omodel/olam_grid.f90` 的 `gridfile_write()`，测试输入位于 `/Users/zhongwangwei/Desktop/olam-model-code-r1095-trunk/build_olam_test/OLAMIN`。
   - 当前本机直接构建受环境阻塞：默认 Makefile 需要 `h5pfc` + Intel flags；改用 miniforge `h5fc` 时 wrapper 指向缺失的 `arm64-apple-darwin20.0.0-gfortran`；改用 Homebrew `gfortran` + HDF5/NetCDF flags 后编译推进到 OLAM 源文件，但停在 `mem_lp.f90` 的非标准 `IMPORT` 语句（gfortran 报 `IMPORT statement ... only permitted in an INTERFACE body`）。除非提供 Intel/parallel-HDF5 环境或允许修补 OLAM 源码/构建配置，否则无法在本机生成 authoritative Fortran `.h5` fixture。
2. `fill_rad3` 与 Method-C 过渡边界行的极端几何路径（少数测试未覆盖的边界几何）

## 里程碑状态（当前会话快照）

- ✅ `spawn_nest` Method-C pass 分支已激活，不再走旧的非 Method-C 通用闭合路径。
- ✅ `perim_map2/perim_ngr/emit/apply_olam_perimeter_mrows/fill_rad3/thirdm` 的主干等价实现已存在。
- ✅ Method-C 直接表编号路径已有回归测试：`olam_method_c_pass_uses_fortran_table_numbering_counts` 锁定 Fortran 的 `nmd/nud/nwd` 分配公式。
- ✅ Method-C split-U midpoint 编号已有回归测试：`olam_method_c_midpoint_m_ids_follow_fortran_first_seen_edge_order` 锁定 Fortran `im = 2..nmd` 加 `ltab_md(im)%iu` first-seen 顺序产生的 midpoint M id。
- ✅ Method-C split-U midpoint 坐标已有回归测试：`olam_method_c_split_u_midpoint_coordinates_match_fortran_edge_average_projection` 锁定 full-interior non-suppressed split-U midpoint 等于 Fortran edge-average 坐标并经过 active-radius 投影；`olam_method_c_cartesian_split_u_midpoint_coordinates_match_native_edge_average` 锁定 Cartesian/native-XY midpoint 保持原生 edge-average、不走球面投影；过渡带 midpoint 坐标继续由 `perim_fill3` 显式加权坐标测试覆盖。
- ✅ Method-C W-face child 编号与几何已有回归测试：`olam_method_c_child_w_ids_follow_fortran_parent_then_three_children_order` 锁定 Fortran `iwnew` 将三个 child W ids 紧跟在 remapped parent W id 后，锁定 remapped parent W 的 `mrlw` 提升但 `mrlw_orig` 保留、child W 的 `mrlw/mrlw_orig` 同步提升，并确认 child W 最终 M vertices 与其 U-edge endpoints 一致；`olam_method_c_full_subdivision_child_w_vertices_match_fortran_geometry` 锁定 full-interior remapped parent W 为三个 split-U midpoint 组成的 central triangle，child W 由 1 个旧 M 顶点和 2 个 split-U midpoint 组成，所有 midpoint 坐标为 edge-average 后 active-radius 投影。
- ✅ Method-C internal U 编号已有回归测试：`olam_method_c_internal_u_ids_follow_fortran_first_seen_w_order` 锁定 Fortran `iunew/nest_wd(iw)%iu(1:3)` 在 first-seen W face 上分配内部 U ids、写入 parent/child W face、连接三条 split-edge midpoint M ids、中心 remapped parent W face 由三 midpoint 组成，并确认 split-U half-edge 的 adjacent W face 回写落在 full-subdivision W face family 中、匹配 Fortran 三个 edge-slot 分支指定的 child W，且 child W `iu` 槽位顺序保持 Fortran `ltab_wd(iw*)%iu(*)` 写入顺序。
- ✅ Method-C split-U second-half 编号已有回归测试：`olam_method_c_split_u_second_half_ids_follow_fortran_iunew_order` 锁定 Fortran `nest_ud(iu)%iu` 对非 suppressed split-U 分配第二半边 U id，并确认两条 half-edge 共享同一 midpoint M id。
- ✅ Method-C split-U M metadata ownership 已有回归测试：`olam_method_c_split_u_m_metadata_marks_child_ownership` 锁定非 suppressed split-U 旧端点提升到 child `mrlm/ngr`，新 midpoint 写入 child `mrlm/mrlm_orig/ngr`；旧端点 `mrlm_orig` 不作为全局不变量，因为 perim_fill3 过渡点规则可进一步提升局部 original ownership。
- ✅ Method-C suppressed split-U 编号已有回归测试：`olam_method_c_suppressed_split_u_reuses_original_u_and_skips_midpoint_like_fortran` 锁定 Fortran suppression 下 `nest_ud(iu)%iu = iunew(iu)` 且 `nest_ud(iu)%im = 1`。
- ✅ Method-C `impent` remap 已有回归测试：`olam_method_c_remaps_impent_through_fortran_imnew_table` 锁定 Fortran `impent(im) = imnew(impent(im))`。
- ✅ Method-C prognostic partner remap 已有回归测试：`olam_method_c_remaps_prognostic_partners_through_fortran_tables` 锁定 Fortran atmospheric-style `imp/iup/iwp` remap 语义在 Rust `m/u/w_prognostic` 中通过 `imnew/iunew/iwnew` 保持。
- ✅ Method-C final topology closure 已有回归测试：`olam_method_c_emits_closed_topology_without_placeholder_neighbor_ids` 锁定 Method-C 输出通过 topology validation，且 U/W tables 与 M-neighbor rings 不含 placeholder ids。
- ✅ Method-C multi-shape/multi-region output closure 已有回归测试：`olam_method_c_multiple_regions_emit_projected_closed_outputs` 锁定 Circle/Bbox/Corridor/Polygon 多个指定区域输出的 Fortran allocation counts (`nmd/nud/nwd`) 一致、均通过 topology validation、无 U/W/M placeholder ids，且 final projected M points 位于 Fortran active radius。
- ✅ Method-C public spawn entrypoints 已有回归测试：`olam_method_c_public_spawn_entrypoints_use_same_table_path` 锁定 `spawn_nest`/surface alias/atmosmesh/explicit-width/spring entrypoints 以及 Cartesian/native-XY spawn/spring entrypoints 均使用同一 Method-C table path，而不是退回旧的 generic local subdivision。
- ✅ Method-C spring iteration entrypoint 已有回归测试：`olam_method_c_spring_niter_keeps_table_path_and_closed_topology` 锁定 `spawn_nest_with_spring(... niter=1)` 在实际执行 nest spring pass 后仍保持 Method-C Fortran allocation counts、闭合拓扑与 active-radius 投影。
- ✅ Method-C Cartesian/native-XY spring iteration entrypoint 已有回归测试：`olam_method_c_cartesian_spring_niter_keeps_table_path_and_closed_topology` 锁定 `spawn_nest_cartesian_xy_with_spring_and_max_mrows(... niter=1)` 在实际执行 nest spring pass 后仍保持 Method-C Fortran allocation counts、闭合拓扑与 finite 坐标；`olam_method_c_cartesian_deltax_spring_niter_keeps_table_path_and_closed_topology` 锁定使用 Fortran `deltax * sqrt(2/sqrt(3))` 目标间距的 native-XY deltax spring 路径。
- ✅ Method-C OLAMIN-style multilevel corridor 已有回归测试：`olam_method_c_olamin_style_multilevel_corridor_outputs_closed_mesh` 使用 `build_olam_test/OLAMIN` 风格的 `NXP=66`、两点 corridor、三层递减 `GRDRAD`，通过 atmosphere-style `spawn_nest_as_atmosmesh` 锁定最终 table sizes `(nmd,nud,nwd)=(84015,252037,168025)`、per-grid W-face counts `{1:76480,2:11234,3:15048,4:65262}`、atmosphere mrow envelope `[-13,13]` with `50294` nonzero rows、每层生成 grid number 2/3/4、最终 topology validation、无 placeholder、旧 M-point valence 不超过 OLAM 7-slot 限制，并验证 Voronoi/gridfile handoff 保留 Method-C `itab_m%npoly/ngr` 与 `itab_w%npoly`。
- ✅ Method-C CLI compact gridfile handoff 已有回归测试：`fortran_indexed_gridfile_handoff_preserves_explicit_method_c_w_npoly` 锁定 one-based Fortran `itab_w%npoly` 优先级、compact mesh `n_w_to_m`、NetCDF `n_ngrwm` 写出与读回，防止 7-corner transition W face 被旧 5/6 fallback 逻辑压扁。
- ✅ 对应测试套件持续可运行（`cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib -- --nocapture` 本轮以 78 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c -- --nocapture` 本轮以 28 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_cartesian_deltax_spring_niter_keeps_table_path_and_closed_topology -- --nocapture` 以 1 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_cartesian_spring_niter_keeps_table_path_and_closed_topology -- --nocapture` 以 1 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_split_u_midpoint_coordinates_match_fortran_edge_average_projection -- --nocapture` 以 1 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_split_u_m_metadata_marks_child_ownership -- --nocapture` 以 1 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_spring_niter_keeps_table_path_and_closed_topology -- --nocapture` 以 1 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_cli/Cargo.toml --test olam_method_c_gridfile_handoff -- --nocapture` 以 1 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_cli/Cargo.toml --test olam_mkgrd_pipeline default_atmos_global_specified_refine_uses_olam_spawn_nest -- --nocapture` 以 1 passed / 0 failed 通过；`cargo test --manifest-path rust/earthmesh_cli/Cargo.toml --test olam_mkgrd_pipeline -- --nocapture` 本轮以 62 passed / 0 failed 通过；`olam_spawn_nest`、calculated-refine level selector unit test、LOCmesh calculated threshold ordering test、GetRef_Lnd mainland-fraction maxlc denominator test、Method-C OLAMIN-style multilevel corridor test、Method-C public entrypoint table-path test、Method-C spring-niter table-path/topology test、Method-C Cartesian spring-niter table-path/topology test、Method-C Cartesian deltax spring-niter table-path/topology test、Method-C CLI spring-enabled pipeline test、Method-C multi-shape/multi-region projected closure test、Method-C final topology closure test、Method-C prognostic remap test、Method-C impent imnew-remap test、Method-C suppressed split-U reuse/no-midpoint test、Method-C split-U M metadata ownership test、Method-C split-U midpoint sphere/native coordinate projection test、Method-C split-U second-half numbering/shared-midpoint test、Method-C internal U first-seen numbering/endpoint/central-face/face-rewrite/exact child-W adjacency test、Method-C W-face child numbering/metadata/vertex-topology/geometry test、Method-C split-U midpoint first-seen numbering test、Method-C impen marched-IMBEG parent-ownership test、Method-C thirdm straight-path reciprocal-jdone test、Method-C perim_fill3 weighted-coordinate/mrlm_orig/ngr ownership/neighbor ownership preservation/exact W-U slot topology/iu33-special-case test、Method-C fill_rad3 distant-M expansion test、Method-C mrow half-step growth test、Method-C edge-coordinate staging/projection unit tests 与 focused Method-C regressions 在本轮前序步骤通过）。
- ⚠️ 更大范围 per-point edge-geometry fixture 与最终 Fortran `.h5` gridfile tolerance 对照仍需 authoritative Fortran 输出。
