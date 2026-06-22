# R11 — 自审跟进:把"加并行模块"改成"真修+真接线" 报告

> 缘起：用户要求"深度检查 R1–R10 是否有偷懒跳过"。自审(基于 grep/commit/实跑证据)发现系统性问题:R6/R7/R8 是"加了没人调用的纯库",R9 仪表盘空壳,R10 quickstart smoke 用文档代替实现。本阶段按优先级真修。
> 日期：2026-06-22 · 全部改动**真跑验证**(不只编译)。

## 修复 1 — GUI 质量仪表盘真接线（R9 空壳 → 工作）
**问题**：GUI 不依赖 `earthmesh_quality`,正常出图后不产 `quality_summary.json`,R9 仪表盘恒空。
**修**：
- cli lib 暴露 `pub fn quality_input_from_gridfile(&GridfileMeshPoints)`(从 main.rs 私有提出,两处共用)。
- gui 依赖 `earthmesh_quality`;`poll_run` 成功臂在载入网格后 `compute` + `io::write_all` 写入 run 目录。
**验证**：gui 45/0;现在每次 GUI run 产 `quality_summary.json` + worst_cells/report,仪表盘有真数据。

## 修复 2 — MERIT 反子午线 tile 选择真 bug（R6 只诊断 → 真修 reader）
**问题**：`merit_bbox_intersects` 是朴素 AABB,查询跨 ±180°(west>east)时漏选反子午线两侧 tile(审计 H-B1)。R6 当初只在 `quality::hydro_coast` 加了个没人调用的校验器,reader 一行没改。
**修**：`merit_bbox_intersects` 处理跨界查询(分 [west,180]∪[-180,east]);非跨界路径行为不变。
**验证**：新测试 `merit_tile_selection.rs`(只读文件名,无 netcdf)——跨界查询选中 `n10e175`+`n10w180`、不误选远处;普通查询回归不变。✓

## 修复 3 — quickstart smoke 真做成测试（R10 文档 → 真跑二进制）
**问题**：R10 声称的 "quickstart smoke test" 根本没做成测试,只在文档里说"GUI 会载入"。
**修**：`quickstart_smoke.rs` 真起 cli 二进制跑 `00_quickstart_n16.nml`,断言产出 gridfile + run_manifest。
**意外收获(真 bug)**：smoke 一跑就 exit 2 失败,暴露**裸文件名 namelist 崩溃**:`apply_read_nl_workspace_plan` 里 `namelist_source.parent()` 对裸名(如 `mkgrd.x case.nml`)返回 `Some("")`,`"".canonicalize()` → ENOENT。
**修**：空 parent 视作当前目录(`Path::new(".")`)。这是真 cli bug(在 case 目录里跑 `mkgrd.x case.nml` 是最自然用法)。
**验证**：quickstart 现 0.93s 跑通出图;smoke 用裸名调用,回归守护此修复。✓

## 修复 5 — R5 拓扑校验"接了但没牙" → 派生真实邻接
**问题**：`quality_input_from_gridfile` 给每个 cell 的 `neighbors` 设空 Vec。于是我之前声称"已接线"的 R5 拓扑校验器,以及报告的 neighbor-reciprocity / transition / max_adjacent_ratio 指标,在真实 gridfile 上**全是空转**(遍历空邻接)。(报告的 `orphan_cell_count` 例外——它自算边,所以 R4 当时没误判,掩盖了这个洞。)
**修**：在 `quality_input_from_gridfile` 里按**共享边**派生 cell 邻接(两 cell 共享 2 顶点即互为邻居,reciprocal by construction;非流形边>2 不连,另有 duplicate-edge 标记)。
**验证**：新测试 `quality_input_neighbors.rs`——共享边的两三角互为邻居、孤立三角无邻居、`compute()` 的 `orphan_cell_count==1`。现在 GUI/cli 写出的 quality 报告里拓扑段是**真有数据**的。✓

## 修复 4 — 诚实标注未接线的实验模块（R6/R7/R8）
`refine_planner`(整 crate 孤岛,cli/gui 都不依赖)、`quality::coupling`、`quality::hydro_coast` 顶部加 **`INTEGRATION STATUS: EXPERIMENTAL / NOT WIRED`**,说明产品无代码路径执行它们、仅单测覆盖,避免文档夸大。

## 仍未做（诚实保留）
- **R7 coupling / R8 refine_planner 仍未接线**:真实耦合路径 `write_colm_coupling_csv_from_mesh` 未用 `quality::coupling`;refine planner 无 feature 来源。接线是更大工作,本阶段只如实标注,未接。
- **R6 hydro_coast validator 未接线**:MERIT reader 的其余 P0/P1(stride 抽样、buffer 单位)仍只在审计文档里,reader 未改(本阶段只修了 dateline 这一个真 bug)。
- CI 仍未在 GitHub runner 实跑。

## 验证总览（本阶段实跑)
- `cargo test -p earthmesh_cli --features static-netcdf` 全套 **0 failed**(含 2 新测试,merit+empty-parent 改动无回归)。
- gui 45/0、quality 32/0、refine 6/0、core(examples 分类)绿;全改动 crate `cargo fmt --check` PASS。
