# R9 — GUI Workflow + Quality Dashboard + Visual Polish (MVP) 报告

> 阶段：R9（GUI 工作流平台过渡 + 质量仪表盘 + 视觉打磨 MVP）· 配套 [10_gui_redesign_proposal.md](../10_gui_redesign_proposal.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)
> 边界（严守"不允许修改"）：未改 mesh generation / refinement 核心 / production geometry / CLI workflow / output schema（仅**只读** quality/run manifest）/ 无大规模 GUI 架构重写。只动 `gui/src/main.rs`(additive) + `i18n.rs` + 3 个新模块。

## 1. Visual changes summary
- **Design system**(`theme.rs`):`EarthMeshTheme`(light/dark)+ spacing scale(xs..xl)+ 状态色(PASS/WARN/FAIL/INFO/NEUTRAL)+ map-layer 色(land/ocean/river/coast/estuary/wetland/urban)+ card_fill/accent;`apply()` 设 visuals+spacing(仅在用户切换主题时调用,启动保留既有 `configure_style` 外观→零回归)。
- **组件**(`components.rs`):`status_badge`(彩色状态药丸)、`card`(分组填充框+标题)、`status_message`(info/success/warning/error + 图标)、`section_header`、`empty_state`。
- **Toolbar 打磨**:新增 target-template 下拉、主题切换(🌓)、Expert 开关,均带 tooltip;语言选择保留。
- **Quality dashboard**(results dock 折叠区):verdict 徽章 + run status + headline 指标 + top warnings + manifest warnings + next steps + worst cells 路径 + "打开质量报告"按钮。
- **更好空态**:无输出时 dashboard 显示 `dash.empty` 提示而非空白。
- **中英文**:36+ 新 i18n 键(模板/仪表盘/步骤/expert/主题)全部 en/zh 双语。

## 2. Workflow changes
- **Target templates**(`ui_helpers::target_templates`,9 个):global atmosphere / regional land / regional ocean / land-ocean coupled / MERIT-Hydro river-coast / coastal estuary / hydrology land / urban land / orographic atmosphere。选择即 `apply_template`(预设 mesh_type/mode_grid/output_format/nxp/global/refine 到 config,用户仍可改)。
- **Workflow steps skeleton**(`WorkflowStep` + `workflow_steps()`,7 步):New Project/Target/Domain/Data/Strategy/Quality/Run-Results — 作为命名步骤模型 + i18n(可视化导航的基础;**未**重排既有 3-tab 布局,避免架构重写)。
- **Expert mode**:`expert_mode` 开关(默认关)——为"高级参数(NXP/HALO/spring/manual masks)仅 expert 可见"提供开关基础(逐字段折叠为后续,见 §7)。

## 3. Quality dashboard（只读现有产物，无 schema 改动）
`ui_helpers::QualityDashboard::from_dir(dir)` 读 run 输出目录的 `quality_summary.json`(R4)+`run_manifest.json`(R2)+ 探测 `worst_cells.geojson`/`quality_report.md`,解析出:verdict、headline 指标、top warnings(gates warn/fail + topology issues)、manifest status/warnings、actionable next steps。GUI 在 results dock 渲染 verdict 徽章 + 警告 + 下一步 + 报告按钮。

## 4. Theme/design system
见 §1。light/dark 低风险(默认 light,启动不强制 apply);status/layer 色集中管理;spacing scale 统一。

## 5. Files changed / New components
| 文件 | 改动 |
|------|------|
| `rust/earthmesh_gui/src/theme.rs` | **新增** EarthMeshTheme/Spacing/状态色/layer 色/apply（4 测试） |
| `rust/earthmesh_gui/src/ui_helpers.rs` | **新增** QualityDashboard(JSON 解析)/9 templates/workflow steps/tooltips（6 测试） |
| `rust/earthmesh_gui/src/components.rs` | **新增** status_badge/card/status_message/section_header/empty_state（1 测试） |
| `rust/earthmesh_gui/src/main.rs` | additive:声明 3 模块；EarthMeshApp +4 字段(expert_mode/theme_dark/project_name/target_template)；toolbar 加模板选择/主题切换/Expert 开关；results dock 加 dashboard 折叠；新增 `impl` 块(theme/apply_template/render_quality_dashboard) |
| `rust/earthmesh_gui/src/i18n.rs` | +36 键(模板/仪表盘/步骤/expert/主题,en/zh) |

## 6. Tests
- `cargo test -p earthmesh_gui --all-targets` → **45 passed / 0 failed**（34 既有 + 11 新:theme 4 + ui_helpers 6 + components 1）。
- `cargo check -p earthmesh_gui` → 见 §验证（exit 0，无 warning）。
- 新文件 `rustfmt --check` → PASS。
- 新增测试覆盖:status_color 映射、light/dark、spacing 单调、layer 色区分;JSON 字段提取、dashboard 解析(verdict/warnings/next_steps)、缺 quality 时建议、9 模板、7 步、tooltips 齐全。

## 7. Manual QA checklist（需人工在 GUI 跑;代码已就位）
| 场景 | 预期 | 状态 |
|------|------|------|
| no project | dashboard 空态提示 `dash.empty` | 代码就位（output_files 空→empty_state） |
| loaded quickstart | 载入 nml,toolbar/模板可用 | 既有 load 保留 |
| missing basemap | wireframe fallback(既有行为) | 既有 |
| missing quality report | dashboard 显示 "run mesh-quality" next step | 代码就位 |
| run success | verdict 徽章 + next steps(若有 quality) | 代码就位 |
| run failure | manifest status=failed + warnings 显示 | 代码就位 |
| Chinese UI | 所有新键 zh 显示 | i18n zh 已填 |
| English UI | en 显示 | i18n en 已填 |
| small window | results dock 上限半屏(既有 clamp) | 既有 |
| large window | 布局自适应 | 既有 |
> 自动化 headless GUI 测试受 egui/eframe 限制;以上为手动 QA 清单（单元测试已覆盖纯逻辑）。

## 8. Remaining GUI issues
1. **Workflow 步骤未替换 3-tab 布局**:仅提供步骤模型 + 导航基础;完整向导式重排是更大改动（10 §G-3/G-4，避免本期架构重写）。
2. **Expert mode 逐字段折叠**:开关已加,但把 NXP/HALO/spring/manual-mask 等具体控件按 expert 折叠需逐个改既有 tab 渲染（4356 行）——本期未逐字段接,作后续。
3. **tooltips**:已为新控件 + dashboard 加 tooltip 与 `ui_helpers::tooltip(key)` 文案库（NXP/strategy/threshold/quality/merit/target/score）;接到既有 NXP/threshold 控件需逐控件 `.on_hover_text`（后续）。
4. **Project save/load**:沿用既有 namelist save/load;真正的 ProjectConfig（R2/03）GUI save/load 待接。
5. **theme.apply 全局**:默认不强制(保留 configure_style);若要默认深色/统一视觉需评估对既有自定义 style 的影响。
6. **dashboard 每帧读盘**:`from_dir` 每帧读 JSON(小文件,MVP 可接受);可加缓存。
7. **CLI command preview**:未做(低优先)。
