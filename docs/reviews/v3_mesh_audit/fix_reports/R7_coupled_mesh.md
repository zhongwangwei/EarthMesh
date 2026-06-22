# R7 — Land-Ocean Coupling / LOCmesh Consistency (MVP) 报告

> 阶段：R7（coupled mesh 分类/fraction/coupling/质量检查 MVP）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [05_coupled_mesh_audit.md](../05_coupled_mesh_audit.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)
> 放置：`earthmesh_quality::coupling`（纯、无 netcdf、可即时 `cargo test`；复用 `geometry::safety::validate_fraction_partition` + `QualityLevel`）。
> 边界（遵守"不要做"）：**不大规模重写耦合算法、不改全部输出格式、不强求所有模式细节**。`coupling.nc` 仍是 cli 既有 CoLM writer（不动）。

## 1. CoupledCellClass（8 类，全实现）
Land · Ocean · MixedCoast · Estuary · RiverMouth · WetlandDelta · Island · Unknown。
- `classify_by_fractions` 按 fraction + CaMa 标志分类;`classify_all` 两遍:第二遍把"被 ocean 全包围的纯 Land"重分类为 **Island**。

## 2. CoupledCellFractions（字段齐全）
land_fraction · ocean_fraction · river_fraction · wetland_fraction · estuary_fraction · source_features(Vec) · quality_flags(Vec)。`fraction_quality_flags` 复用 `validate_fraction_partition([land,ocean],tol)` 标记 `unresolved_fraction_sum_error`。

## 3. CouplingMap（字段齐全）
land_cell_id · ocean_cell_id · overlap_fraction · exchange_weight · coupling_type(`CouplingType`: Coastline/RiverOutlet/Estuary/Other) · source_reason。
- `build_coupling_map`:RiverMouth/Estuary → outlet ocean cell(RiverOutlet/Estuary);MixedCoast ↔ 相邻 ocean(Coastline)。
- `max_ocean_oversubscription(maps)`:ocean cell 入向 exchange 权重和 >1 的超额(守恒检查)。

## 4. CouplingQualityReport（15 指标，全实现）
total_land_cells · total_ocean_cells · mixed_coastline_cells · coast_overlap_cells · river_mouth_cells · estuary_cells · unresolved_fractional_area · land_fraction_error · sea_fraction_error · coupling_row_count · orphan_land_cells · orphan_ocean_cells · mass_conservation_residual · outlet_matching_error · coastline_preservation_score · river_ocean_connectivity_score + verdict。
- mass_conservation_residual = max_c |1-(land+ocean)|;land/sea_fraction_error = 超出 [0,1] 量;orphan = 无邻居的 cell（按类分 land/ocean）;coastline_preservation = mixed 有 ocean 邻居比例;connectivity = river-mouth 匹配 outlet 比例。
- **verdict**:守恒残差>tol / fraction 越界 / orphan>0 / ocean 超额 → **Fail**;outlet 未匹配 / coast 保真<1 / 未解析混合面积>0 → **Warn**。

## 5. Output（MVP）
- `to_coupling_csv`:每 cell 一行(class + 5 fractions)。
- `to_coupling_quality_json`:`kind=earthmesh_coupling_quality` + 15 指标 + verdict。
- `to_coupling_manifest_json`:`kind=earthmesh_coupling_manifest` + products + verdict。
- `coupling.nc`:**沿用 cli 既有 CoLM netcdf writer**（不改输出格式，不重写）。

## 6. Tests（`cargo test -p earthmesh_quality` 全绿：36 测试 0 failed;fmt PASS）
| 要求测试 | 实现(coupling::tests) | 结果 |
|----------|------------------------|------|
| pure land/ocean/mixed classification | `pure_land_ocean_mixed_classification` | ✅ |
| fraction sum tolerance | `fraction_sum_tolerance_flagged` | ✅ |
| orphan land cell | `orphan_land_cell_detected` | ✅ |
| orphan ocean cell | `orphan_ocean_cell_detected` | ✅ |
| simple coupling map mass conservation | `simple_coupling_map_mass_conservation`(oversubscription 0) | ✅ |
| river mouth to ocean matching smoke | `river_mouth_to_ocean_matching_smoke` | ✅ |
| (附加) island / mass-conservation fail / unmatched river-mouth / outputs fields | 4 测试 | ✅ |

## 7. Files changed
| 文件 | 改动 |
|------|------|
| `rust/earthmesh_quality/src/coupling.rs` | 新增:8 类 + fractions + CouplingMap + 15-指标 report + classify_all/build_coupling_map/build_coupling_quality + CSV/JSON/manifest + 10 测试 |
| `rust/earthmesh_quality/src/lib.rs` | `+pub mod coupling`;`QualityLevel` 加 `#[derive(Default)]`(默认 Pass) |

## 8. 验证
- `cargo test -p earthmesh_quality --all-targets` → **36 passed / 0 failed**;fmt PASS。
- `cargo build -p earthmesh_cli --features static-netcdf`（quality 依赖变更后）→ 见后台日志（additive，预期绿）。

## 9. cli 集成 + Remaining / next（API 已就绪）
1. **cli 接线**:从 LOCmesh gridfile + landtype/MERIT/CaMa 构建 `CoupledCellInput`（fraction 用 R3 `geometry::overlay_cell` 守恒面积;is_estuary/is_river_mouth 从 CaMa;outlet 从 river-mouth→相邻 ocean），调 `build_coupling_quality` + 写 `coupling_quality.json`/`coupling_manifest.json`。建议 `--coupling-validate <gridfile> <landtype>` 子命令或在 CoLM 输出后自动写。**本期未接线**（需 netcdf + overlay 守恒 fraction 的真实数据管线）——纯模块 + 36 测试为可验证 MVP。
2. **守恒 fraction 数据源**:当前 fraction 由调用方提供;真正的守恒分类（点采样→overlay 面积，[05](../05_coupled_mesh_audit.md) W1/W2）是 cli 侧数据管线工作。
3. **score / planner**:coupling_priority 等占位接入 [09](../09_score_based_refinement_design.md)（R8+）。
