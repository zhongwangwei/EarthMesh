# PROJECT_AUDIT_PLAN — EarthMesh v3 审查计划

> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md)（总原则 + v3 目标）。
> 报告模板：[REPORT_TEMPLATE.md](./REPORT_TEMPLATE.md)。进度追踪：[TASK_STATUS.md](./TASK_STATUS.md)。
> 审查对象：仅 EarthMesh v3 当前项目（分支 `v3.0.0-alpha1`），作为独立 Rust-native + GUI + GIS workflow 项目。
> **不引用、不比较、不依赖任何旧版本实现。**

---

## 1. 审查范围（In Scope）

- `rust/` 下 5 个 crate 的源码、模块组织、数据流与算法：
  - `earthmesh_core`、`earthmesh_geometry`、`earthmesh_mesh`、`earthmesh_cli`、`earthmesh_gui`。
- 端到端 **workflow**：从 gridinit → 阈值/掩膜 → refine → 质量 → 模式耦合导出。
- **refinement 决策逻辑**：触发字段、阈值、area_judge、Method-C 细化与邻接一致性。
- **网格质量**：几何 / 拓扑 / 数值稳定性 / 耦合一致性度量（现状与缺口）。
- **MERIT-Hydro / Hydro-Coast** 工作流：close 掩膜、河网/岸线处理、陆海耦合一致性。
- **GUI workflow**：配置 → 运行 → 预览 → 结果追溯，及其对总原则第 5 条的满足程度。
- **测试与可复现性**：测试覆盖、慢测试（`--ignored`）、Method-C 邻接检查脚本。
- 输出格式与模式契约：CoLM2024 / FVCOM / MPAS / OLAM / IAP 导出的一致性。

## 2. 不审查内容（Out of Scope）

- 任何旧版本 / Fortran 历史实现的正确性对比（仅在涉及当前 v3 兼容契约时记录，不作为判据）。
- 第三方依赖内部实现（`eframe`/`egui`、`walkers`、`netcdf`、`pyo3` 等）——只审查我们的用法。
- 根目录已编译产物（`mkgrd.x`）、构建脚本细节（`make.sh`/`make_gnu.sh`/`switch_compiler.sh`）的正确性，仅确认其指向 Rust 构建。
- 输入大数据本体（MERIT-Hydro / land type 等原始数据集）的科学正确性——只审查读取/解释逻辑。
- CI / 部署 / 打包签名（GUI bundle 的 icon/identifier 等）超出网格质量范畴的部分。
- 性能微优化（除非触及总原则第 3 条"收益 vs 成本"或数值稳定性）。

## 3. Crate 列表（审查单元）

| Crate | 源码规模 | 角色 | 审查重点 |
|-------|---------|------|----------|
| `earthmesh_core` | ~1.9k 行 (`lib.rs` + `progress`) | 常量、配置、运行时状态、进度 | 配置语义、单位、状态机一致性 |
| `earthmesh_geometry` | ~610 行（含可选 `pyo3`） | 几何后端 + Python 扩展 | 几何基元正确性、Python 绑定边界 |
| `earthmesh_mesh` | **~22.4k 行（单 `lib.rs`）** | 网格与 refinement 内核 | refinement/Method-C/邻接/质量内核（核心战场） |
| `earthmesh_cli` | **~36k 行（`lib.rs`+`main.rs`）** | mkgrd 兼容 CLI、所有 workflow 编排 | workflow 一致性、参数语义、I/O 契约 |
| `earthmesh_gui` | ~4.6k 行（`main.rs`+`i18n`，eframe/egui+walkers） | 桌面 GUI / GIS workflow | 可解释性（原则 5）、与 CLI 行为一致性 |

> 架构观察（待 Refactoring Roadmap 处理）：`earthmesh_mesh` 与 `earthmesh_cli` 为超大单文件 `lib.rs`，
> 不利于审查与维护，是重构候选（见 [REPORT_TEMPLATE.md](./REPORT_TEMPLATE.md) 第 8 节）。

## 4. Workflow 列表（端到端审查路径）

| # | Workflow | 作用 | 关联测试（示例） |
|---|----------|------|------------------|
| 1 | `gridinit` | 全球/区域基础网格生成 | `mkgrd_gridinit`、`area_judge_grid_*` |
| 2 | `mask_make` | bbox/circle/close/lambert 细化掩膜 | `bbox_mask_make`、`circle_close_mask_make`、`lambert_mask_make` |
| 3 | `getref` | 细化判据阈值（land/ocean/atmos/loc） | `getref_land_*`、`getref_ocean_atmos_*`、`getref_loc_*` |
| 4 | `getcontain` | 包含矩阵 / 区域选择 | `getcontain_*` |
| 5 | `refine` (Method-C) | 端到端细化 + OLAM parity | `refine_end_to_end_topology`、`area_judge_refine_*` |
| 6 | `area_judge` | 细化决策 / 激活 | `area_judge_calculated_refine`、`area_judge_specified_refine` |
| 7 | `colm_coupling` | CoLM2024 NetCDF/CSV 耦合导出 | `colm_coupling_csv_from_mesh`、`colm_coupling_netcdf_cli` |
| 8 | `hydro_close` | MERIT-Hydro 河网/海岸 close 掩膜 / recipe / composite | `hydro_close_*`、`hydro_composite_close_mask_cli` |
| 9 | `fvcom_mesh_save` | FVCOM 网格导出 | `fvcom_mesh_save_writer` |
| 10 | `mask_postproc` | 边界写出 / MPAS atmos / IAP mesh | `mask_postproc_*`、`iap_mesh_read` |
| 11 | `data_preprocess` | MERIT NetCDF / landtype / cama binary / geojson | `data_preprocess_*` |
| 12 | `grid_quality` | 网格质量度量 | `grid_quality_global_adapter` |
| 13 | `earthmesh_info` | 运行信息 / 元数据写出 | `earthmesh_info_builder`、`earthmesh_info_writer` |
| 14 | `cama_reach` | CaMa-Flood reach 导出 | `cama_reach_jsonl_cli` |

## 5. 需要跑的测试命令

```bash
# 格式检查（read-only 验证，不改代码）
make fmt

# 全 crate 单元 + 集成测试（core / geometry / mesh / cli）
make test

# GUI crate 测试
make test-gui

# 慢测试 / --ignored（mask restart、CoLM、refine 拓扑、gridinit 全球 fixture）
make test-slow

# Method-C 邻接一致性脚本（拓扑质量门禁）
make check-method-c-neighbors

# 全量（= check-method-c-neighbors + test + test-gui + test-slow）
make test-full
```

> 审查约定：每个 phase 开始与结束时各跑一次相关命令并记录原始输出到对应报告，作为证据（参见 AUDIT_PRINCIPLES 证据等级）。

## 6. 需要生成的报告（交付物清单）

所有报告遵循 [REPORT_TEMPLATE.md](./REPORT_TEMPLATE.md) 的 9 个章节模板：

1. Executive Summary
2. Bug Table
3. Workflow Consistency Table
4. Physical Consistency Table
5. MERIT-Hydro / Hydro-Coast Review
6. Mesh Quality Metric Proposal
7. GUI Redesign Proposal
8. Refactoring Roadmap
9. Patch Plan

最终汇总文件建议命名：`docs/reviews/v3_mesh_audit/AUDIT_REPORT.md`（由各 phase 输出聚合）。

## 7. 阶段计划（Phases）

| Phase | 名称 | 目标 | 交付物 | 代码权限 |
|-------|------|------|--------|----------|
| **P0** | 工作区初始化 | 建立审查工作区与四个基准文件 | 本目录四文件 + git status | **只读**（仅写 docs/） |
| **P1** | 架构与 workflow 测绘 | 摸清 5 crate 模块边界、14 workflow 数据流；建立基线测试输出 | Workflow Consistency Table 初稿 + 基线测试日志 | **只读** |
| **P2** | 正确性 / Bug 审查 | 定位算法、I/O、参数语义、边界条件 bug（含 Method-C / 拓扑） | Bug Table（A/B 级带行号） | **只读**（可提 patch 草案） |
| **P3** | 物理一致性审查 | 按总原则五条核对每个 refinement 触发与阈值 | Physical Consistency Table | **只读** |
| **P4** | MERIT-Hydro / Hydro-Coast 审查 | 河网/海岸/陆海耦合一致性、close 掩膜正确性 | MERIT-Hydro / Hydro-Coast Review | **只读** |
| **P5** | 网格质量度量提案 | 定义几何/拓扑/数值/耦合质量指标与门禁 | Mesh Quality Metric Proposal | **可提 patch** |
| **P6** | GUI 重设计提案 | 让 GUI 满足总原则第 5 条（可解释 + before/after） | GUI Redesign Proposal | **可提 patch** |
| **P7** | 重构路线图 | 拆分超大 `lib.rs`、明确模块边界与接缝 | Refactoring Roadmap | **可提 patch** |
| **P8** | 补丁实施 | 在用户批准后实施已确认补丁，绿测后交付 | Patch Plan + 实际改动 + 测试结果 | **允许直接修改代码** |

## 8. 阶段权限矩阵（明确边界）

### 8.1 只能读代码的阶段（read-only）
- **P0、P1、P2、P3、P4**。
- 这些阶段**禁止修改任何 `src/rust` 代码**；只允许在 `docs/reviews/v3_mesh_audit/` 下写报告。
- 允许运行测试 / 构建 / 脚本（不产生源码改动）。

### 8.2 允许提出 patch（提案，不落地）的阶段
- **P5、P6、P7**（以及 P2 可附 bug 修复草案）。
- 产出形式为 **Patch Plan 中的 diff 草案 / 伪代码 / 改动说明**，但**不直接写入源码树**。
- 任何 patch 草案须经用户审阅批准，方可进入 P8。

### 8.3 允许直接修改代码的阶段
- **仅 P8**，且必须满足：
  1. 对应 bug/提案已记入报告并获用户批准；
  2. 改动遵循 surgical change 原则（每行可回溯到具体结论）；
  3. 改动前后跑通 §5 相关测试命令，记录证据；
  4. 在 `v3.0.0-alpha1` 或派生分支上操作，不直接污染 `master`。

## 9. 退出标准（Definition of Done）

- 四个基准文件齐备且章节完整（P0 完成判据）。
- 每个 workflow 至少有一条 Workflow Consistency Table 记录。
- 每个 refinement 触发点至少有一条 Physical Consistency Table 记录并对照总原则五条。
- 所有 A/B 级 bug 带文件:行号；所有 patch 提案有验证计划。
- 最终 `AUDIT_REPORT.md` 聚合九节内容，TASK_STATUS 全部 phase 状态收敛。
