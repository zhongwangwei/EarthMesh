# R5 — Topology Validator + Repair Hooks 报告

> 阶段：R5（topology validator + repair hooks）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [08_mesh_quality_metrics_design.md](../08_mesh_quality_metrics_design.md) / [R4](./R4_mesh_quality_report.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)
> 放置：`earthmesh_quality::topology`（与 R4 同 crate，复用 `QualityMeshInput`；无 netcdf、可即时 `cargo test` 验证）。
> 边界（遵守"不要做"）：**不做大规模 topology repair 算法、不重写 mesh 结构、不改用户 workflow**——repair hooks 只作用于**细化 target levels**（`Vec<u32>` + 邻接表），是未来 repair 的稳定入口。

## 1. MeshTopologyValidator（10 个 validate_*，全部实现）
`validate_indices` · `validate_edges`(=dangling) · `validate_neighbors` · `validate_cell_vertex_incidence` · `validate_duplicate_edges` · `validate_dangling_edges` · `validate_orphan_cells` · `validate_polygon_edge_counts` · `validate_refinement_levels` · `validate_transition_continuity` · `validate_all`（汇总，每类 cap `MAX_ISSUES_PER_TYPE=100` 防爆）。
- `max_adjacent_level_jump`（默认 1）控制 transition 判定。
- `worst_severity(&[issue]) -> Option<Severity>` 供 verdict 折叠。

## 2. TopologyIssue（结构化，字段齐全）
`issue_type`(`TopologyIssueType`，10 类) · `severity`(`Severity::Fail|Warn`) · `cell_id: Option<usize>` · `edge_id: Option<(usize,usize)>` · `vertex_id: Option<usize>` · `message: String` · `suggested_fix: String`。
- 严重度策略：**catastrophic 连通性 = Fail**（invalid index/cell-vertex incidence/duplicate/dangling/orphan/nonreciprocal/abnormal-polygon/invalid-refinement）；**退化 = Warn**（transition discontinuity）。

## 3. Repair hooks（只动 levels，不改 mesh）
- `remove_isolated_refined_cells(levels, neighbors)`：把"邻居全更粗"的 refined cell 降级到最大邻居级。
- `smooth_target_levels(levels, neighbors)`：一遍平滑，level ≤ max(邻居)+1。
- `enforce_max_adjacent_level_jump(levels, neighbors, max_jump)`：迭代下调,相邻级差 ≤ max_jump。
- `mark_unrepairable(issues)`：筛出 level-repair 无法修的灾难性连通性问题（需 mesh surgery）。
- `run_repair_hooks(...) -> RepairReport` / `emit`：依次跑上述 hooks + 标记 unrepairable，产出 `RepairReport`（actions+cells_changed / unrepairable / `to_json` / `to_md`）。

## 4. Integration
- **接入 `MeshQualityReport`**：`compute()` 跑 `MeshTopologyValidator::validate_all()`，结果存 `report.topology_issues`；**verdict 折叠** validator 最坏严重度（Fail/Warn）与既有 gate verdict 取最坏。
- **after mesh generation / after refinement**：二者共用 `compute()` 入口（cli `--mesh-quality` 已调用）；逐 run() 分支的自动 post-run 接线沿用 R4 的同一入口（下一步）。
- **输出**：`quality_summary.json` 新增 `topology_issues` 数组（type/severity/cell_id/vertex_id/message/suggested_fix）；`quality_report.md` 新增 "Topology issues" 表。
- **fail only for catastrophic / warn for degradation**：由 `default_severity()` 保证。

## 5. Tests（`cargo test -p earthmesh_quality` 全绿：17 测试 0 failed；fmt PASS）
| 要求测试 | 实现（topology::tests） | 结果 |
|----------|--------------------------|------|
| invalid vertex index | `invalid_vertex_index_detected_as_fail` | ✅ |
| duplicate edge | `duplicate_edge_detected` | ✅ |
| nonreciprocal neighbor | `nonreciprocal_neighbor_detected` | ✅ |
| isolated refined cell | `isolated_refined_cell_repaired_and_reported` | ✅ |
| level jump too large | `level_jump_too_large_warns_and_enforced`（warn + enforce） | ✅ |
| repair hook emits report | `repair_hook_emits_report`（3 actions + unrepairable + JSON/MD） | ✅ |
| (附加) 有效网格无 issue | `valid_mesh_has_no_issues` | ✅ |
| 集成 + 输出（R4 4 测试仍绿） | quality.rs 集成测试 | ✅ |

## 6. Files changed
| 文件 | 改动 |
|------|------|
| `rust/earthmesh_quality/src/topology.rs` | 新增：validator(10) + TopologyIssue + repair hooks + RepairReport + 7 单测 |
| `rust/earthmesh_quality/src/lib.rs` | `+pub mod topology`；`MeshQualityReport.topology_issues` 字段；`compute()` 跑 validator + 折叠 verdict |
| `rust/earthmesh_quality/src/io.rs` | JSON + MD 加 topology_issues 输出 |

## 7. 验证（含真实 gridfile 集成）
- `cargo test -p earthmesh_quality --all-targets` → **17 passed / 0 failed**；`fmt --check` PASS。
- `cargo build -p earthmesh_cli --features static-netcdf`（依赖 quality）→ 见 §（后台）；真实全球 gridfile `--mesh-quality` → JSON 含 `topology_issues`（该 quasi-uniform 网格连通性干净 → 0 catastrophic issue，verdict 仍为 R4 的 area_cv warn）。

## 8. Remaining / next
1. **自动 post-run 接线**：在 cli run() 各 gridinit/refine 产物分支自动调用 validator + 写报告 + 把 verdict/issues 写进 run_manifest（R4/R5 共同的下一步）。
2. **refine_level/neighbors 来源**：cli 当前从 gridfile 三角视图构建时未填 refine_level/neighbors → refinement/transition validator 在 cli 路径暂为空；需从 gridfile 读细化级与邻接后启用 transition/isolated 检测与 repair。
3. **repair 落地**：当前 repair hooks 改 target levels；把"重生成被降级 cell 的网格"接到引擎是更大的工作（本期明确不做）。
4. **接 QualityConstraintConfig**：Fail 严重度 + `on_violation=Block` 可阻断 run（[03](../03_config_schema_audit.md)）。
