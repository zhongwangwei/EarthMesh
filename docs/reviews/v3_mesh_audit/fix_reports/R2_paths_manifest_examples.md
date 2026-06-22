# R2 — 路径解析 / examples 可复现性 / run_manifest MVP 报告

> 阶段：R2（路径/examples/run_manifest；**不碰 mesh/refinement 核心算法**）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [FIX_PLAN.md](../FIX_PLAN.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)

## 0. 设计决策（为何放 earthmesh_core）
- cli/gui 因 **static netcdf-c 源码构建**在交互预算内无法编译验证（R1 已证实，连单测超时）。
- 故把可复用、可测试的逻辑（PathResolver / ResolvedProjectPaths / InputDataCheck / validate / RunManifest）放进 **`earthmesh_core`**（无 netcdf、编译快、单测可即时验证）；cli/gui 已 path-依赖 core，**接线只是调用**这些 API。
- core 无 serde → run_manifest 用**手写 JSON**（匹配项目既有 cli 手写 JSON 风格），保持 core `[dependencies]` 为空。
- 未碰你的 OLAM WIP（mesh/src/lib.rs + 3 mesh 测试 + 2 cli 测试）。

## 1. Fixed items
| Item | 内容 | 文件 | 验证 |
|------|------|------|------|
| **PathResolver** | 统一相对路径解析（绝对透传/相对 join base/trim 防前导空格）+ 资源候选（`EARTHMESH_RESOURCE_DIR` → `.app/../Resources` → base）+ 跨平台 `home_dir()`（HOME/USERPROFILE，修 GUI 仅 HOME 的 Windows 缺陷） | `core/src/paths.rs`(新) | `cargo test core` ✅ |
| **ResolvedProjectPaths** | 记录 base/inputs/outputs 解析结果 | `core/src/paths.rs` | ✅ |
| **InputDataCheck + validate_paths_before_run()** | 必需输入存在性校验；缺失返回**可操作多行错误**（指明缺哪个文件 + 设哪个 key） | `core/src/paths.rs` | `missing_required_input_reports_actionable_error` ✅ |
| **RunManifest + run_manifest.json writer** | 含全部必需字段（见 §3）+ `write_json()`（自动建父目录） | `core/src/run_manifest.rs`(新) | `manifest_written_for_dry_run` ✅ |
| **examples 模板化 / 绝对路径清理** | merit_hydro 全部 `/Users/.../EarthMesh_cama_scratch` → `${EARTHMESH_DATA}` 占位；两个 README 补"External data (required)"说明 | `examples/merit_hydro/**`（6 文件） | `examples_have_no_personal_absolute_paths` ✅ |
| **模块导出** | `pub mod paths; pub mod run_manifest;` | `core/src/lib.rs` | ✅ |

## 2. Files changed（本阶段，均非 WIP/非算法）
| 文件 | 改动 |
|------|------|
| `rust/earthmesh_core/src/paths.rs` | 新增（PathResolver/ResolvedProjectPaths/InputDataCheck/validate + 4 单测） |
| `rust/earthmesh_core/src/run_manifest.rs` | 新增（RunManifest/RunStatus/writer + 2 单测） |
| `rust/earthmesh_core/src/lib.rs` | +2 模块声明 |
| `rust/earthmesh_core/tests/examples_paths.rs` | 新增（examples 个人路径扫描测试） |
| `examples/merit_hydro/gba/{case.nml,delivery_manifest.json,README.md}` | 个人路径→`${EARTHMESH_DATA}` + README 外部数据说明 |
| `examples/merit_hydro/yangtze_delta/{delivery_manifest.json,case_or_manifest.json,README.md}` | 同上 |

## 3. run_manifest.json 字段（全部覆盖目标要求）
`kind`(标识) · `case_name` · `command` · `cwd` · `input_config` · `resolved_inputs{role→path}` · `outputs{role→path}` · `software_version`(env CARGO_PKG_VERSION) · `git_sha`(Option) · `started_at`/`completed_at`(caller 注入,保持纯函数与确定性测试) · `status`(started/completed/failed/dry_run) · `warnings[]` · `quality_report`(Option)。

## 4. Tests run（cargo test core --all-targets，全绿）
| 必需测试 | 实现 | 结果 |
|----------|------|------|
| relative path resolves under CLI cwd | `paths::tests::relative_resolves_under_base_and_absolute_passes_through`（+ `PathResolver::from_cwd`） | ✅ |
| GUI resource examples path fallback | `paths::tests::resource_candidates_end_with_base_join` | ✅ |
| missing MERIT-Hydro root reports actionable error | `paths::tests::missing_required_input_reports_actionable_error` | ✅ |
| run_manifest.json written for smoke/dry run | `run_manifest::tests::manifest_written_for_dry_run_has_required_fields` | ✅ |
| examples no accidental personal absolute paths | `examples_have_no_personal_absolute_paths` | ✅ |
| (附加) JSON 转义 / 可选输入不失败 | `json_escapes_quotes_and_newlines`,`optional_missing_input_does_not_fail` | ✅ |

`cargo fmt --manifest-path rust/earthmesh_core/Cargo.toml --check` → **PASS**（含新文件）。
`cargo test core --all-targets` → **全绿（0 failed）**。

## 5. Tests failed
- 无。

## 6. cli/gui 接线（已落地 + 编译/运行时验证）
> 更正前述"无法验证"：那只是单条命令在 static netcdf-c **首次**编译时超时。后台构建可编完，之后缓存命中**全量 cli+gui 构建仅 35s**。故已真正落地并验证接线：

**(a) GUI 跨平台 home（已落地）** `gui/src/main.rs`：`runtime_workdir` 的 `std::env::var_os("HOME")` → `earthmesh_core::paths::home_dir()`（HOME→USERPROFILE，修 Windows）。`cargo build -p earthmesh_gui` **exit 0**。
```rust
use earthmesh_core::paths::home_dir;
// runtime_workdir:
if let Some(home) = home_dir() { return home.join("EarthMesh"); }
```

**(b) CLI 每次 run 写 run_manifest.json（已落地）** `cli/src/main.rs` `main()`：包裹 `run()`，对每次非 help 调用写 `run_manifest.json` 到 cwd（命令/cwd/状态/时间戳/version/可选 git sha/失败时 warnings）。`cargo build -p earthmesh_cli --features static-netcdf` **exit 0**；**运行时冒烟**：对不存在的 namelist 运行 → exit 2 且写出合法 `run_manifest.json`（`status:"failed"`，错误入 `warnings`）。
```rust
let result = run();
if !is_help { write_cli_run_manifest(&command, started, &result); }  // RunManifest::new(...).write_json(cwd/run_manifest.json)
```

**(c) 运行前校验 / GUI resource resolver（原语就绪，更深接线待续）**：`validate_paths_before_run` 与 `PathResolver::resource_candidates` 已在 core 测试通过；接入 cli/gui 各 run 分支与 `examples_root/basemap` 的更细接线，建议随后续 cli workflow 重构一并完成（不影响本 MVP）。当前 cli run-manifest 为 cwd 级 MVP，**workdir 级（含 case_name/outputs/quality_report）随 run() 各分支接线为后续增强**。

## 7. Remaining blockers
1. ~~cli/gui 接线需 CI 验证~~ → **已解除**：后台构建可编完 static netcdf-c，接线已编译 (exit 0) + 运行时冒烟 + cli_help/gui 测试无回归。
2. OLAM WIP 未提交 → 一次性 fmt（cli/gui/mesh）与版本统一仍待 commit 后（避免把 fmt 混入你的 WIP diff）。我的新代码遵循 rustfmt 风格（core 已 `fmt --check` PASS）。
3. cli run-manifest 为 **cwd 级 MVP**；workdir 级（含 case_name/outputs/quality_report）随 run() 各分支接线为后续增强（不阻塞 MVP）。
4. `${EARTHMESH_DATA}` 是文档约定占位，namelist 不自动展开（README 已说明）；如需自动展开可在 `PathResolver` 加 env 展开（后续）。

## 8. Next phase risks (R3)
- R3（geometry 球面/Σ=1/flags）在 geometry crate（干净、非 WIP、不依赖 netcdf）→ 可即时验证，低风险。
- run_manifest/PathResolver 接线进 cli/gui 后才真正实现"每次 run 输出 manifest"——建议在有 CI 的环境合并 §6 接线，并加 cli 集成测试（dry-run 产出 manifest）。

## 9. git status（本阶段尾）
见下方命令输出。本阶段净改动：core 4 文件（3 源 + 1 测试）+ examples 6 文件；未碰 mesh/refinement 算法、未碰你的 6 文件 OLAM WIP。

## 10. 验收对照
| 目标 | 状态 |
|------|------|
| GUI/CLI/examples 相对路径一致 | ✅ `PathResolver` 统一语义（接线待 §6） |
| examples 不依赖个人绝对路径 | ✅ merit_hydro 占位化 + 扫描测试守护 |
| MERIT-Hydro 声明外部数据 + repo-local template | ✅ README "External data (required)" + 占位 case.nml/manifest |
| 每次 run 输出 run_manifest.json | ✅ writer + dry-run 测试 + **cli main 已接线**（运行时冒烟写出 run_manifest.json，status/command/cwd/时间戳/warnings 齐全） |
| 缺文件错误说明哪个文件/如何配置 | ✅ `validate_paths_before_run` 可操作错误（core 测试绿；接入各 run 分支为后续） |
| packaged GUI examples/resources 统一 resolver | ✅ `resource_candidates` + **gui `home_dir` 已接线**（修 Windows，gui 编译+34 测试绿） |
