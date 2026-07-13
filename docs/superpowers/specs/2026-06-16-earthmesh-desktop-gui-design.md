# EarthMesh 桌面 GUI — 设计规格

- 日期：2026-06-16
- 状态：已通过头脑风暴评审，待用户复核
- 范围：本规格详细定义 **MVP（阶段 P0 + P1）**，并附 P2–P5 路线图。
- 目标平台：Windows / macOS / Linux

> **Rebase notice (2026-07-09):** this proposal predates removal of the
> generic `refine_loop_*` executor stack. References below to those functions
> describe the original proposal and must be mapped to the current Method-C direct
> runner before implementation.

---

## 1. 目标与非目标

### 1.1 目标
把现有的 EarthMesh（Rust 网格生成引擎，CLI `mkgrd.x` + engine `.nml` 配置）做成一个**可双击安装的跨平台桌面软件**，满足：

1. **全部 Rust**：GUI 与引擎同栈，无 JS/web 前端。
2. **所有选项可在界面直接输入**：92 个 namelist 选项全部以合适控件暴露，带联动校验。
3. **跨平台、零依赖安装**：最终用户无需预装 NetCDF/HDF5 等任何库，双击安装即用。
4. **本地运行**：引擎在本机后台线程跑，界面实时显示进度/日志、可取消，不卡死。
5. **中英文切换**：界面语言可在设置中运行时切换。

### 1.2 非目标（MVP 阶段明确不做）
- 远程/HPC 作业提交、SSH（已确认"只在本地跑"）。
- 3D 网格可视化、地图交互选区（留 P3/P4）。
- 代码签名 / 公证、三平台 CI 全自动发布（留 P5；MVP 仅在开发者本机出未签名安装包）。
- MPI / 并行 NetCDF（引擎本身单线程，static 构建也不支持 MPI）。

### 1.3 成功判据（MVP 验收）
- 能加载 `examples/default/atmosphere_hex_global.nml`，在界面看到全部选项被正确填充。
- 能在界面修改选项、保存为 `.nml`、与现有 CLI 互通（往返一致）。
- 能点击"运行"，引擎在后台线程跑一个小算例（低 NXP），界面显示进度条 + 日志，可取消。
- 跑完能在文件管理器中打开输出目录。
- 能在设置里切换中/英文，界面即时刷新。
- 能在 macOS 上用 `cargo packager` 产出一个可双击打开的 `.app`/`.dmg`（未签名）。
- 现有 CLI 与 30 个集成测试不因引擎接缝改动而失败。

---

## 2. 背景：现有代码现状（已核实）

- 引擎已从 reference implementation 完整迁移到 Rust，分 4 个 crate：
  `earthmesh_cli (lib+bin)` → `earthmesh_mesh` → `earthmesh_geometry` → `earthmesh_core`。
- CLI `main.rs` 是薄壳：仅解析 argv + `println!` 打印最终报告；**全部引擎逻辑在 `earthmesh_cli` 库 crate**（`pub fn`，返回结构化 `…RunReport`）。
- 配置：两个 namelist 段 `&mkgrd`(`NL%`) 与 `&mkrefine`(`RL%`)，解析成 `EarthmeshConfig`（`earthmesh_core/src/lib.rs:700`）与 `RefineConfig`（`:1092`）。解析器：`EarthmeshConfig::from_mkgrd_namelist`（`:805`）、`RefineConfig::from_mkrefine_namelist`、校验 `validate_like_read_nl`（`:944` / `:1369`）。
- **引擎已是干净的库**（关键结论，已核实）：同步、阻塞、可安全跨线程；**无** `static mut`/`lazy_static`/`thread_local`/`OnceLock`/原子量；**无** `rayon`/`thread::spawn`/并行；**无** `process::exit`/stdin。所有 `println!` 在 `main.rs`，引擎内仅 5 处 env-gated `eprintln!` 调试。
- NetCDF：当前 `netcdf = "0.12"`（`netcdf-sys` 链接 C 库）。该 crate 提供 `static` 特性。

---

## 3. 架构

### 3.1 Crate 分层
```
earthmesh_gui   [新增 · eframe App]   ── 直接调库，不 shell-out ──┐
earthmesh_cli (lib)   [+ 引擎接缝]                               │
earthmesh_mesh        [+ 1 个循环钩子]                            │ 单一 cargo workspace
earthmesh_geometry · earthmesh_core                              │
netcdf (features=["static"])  → netCDF-c 4.9.3 + HDF5 2.0 + zlib ┘
```
- 新增 `rust/earthmesh_gui`（bin crate），依赖 `earthmesh_cli`（库）与 `earthmesh_core`。
- `mkgrd.x` CLI 二进制保留不变（脚本化/批处理仍可用）。

### 3.2 引擎接缝（P0）——最小改造，不重写
目标：新增 `run_job(config, &progress, &cancel) -> Result`，不引入全局状态、不强制引擎依赖线程原语。

1. **进度/取消契约**（定义在 `earthmesh_cli`，无外部依赖）：
   ```rust
   pub trait ProgressSink {
       fn report(&self, phase: &str, done: usize, total: usize); // 如 ("refine", k, N) / ("spring", i, niter)
       fn cancelled(&self) -> bool;                              // 在循环顶部检查
   }
   ```
   提供一个 no-op 默认实现，保证现有所有调用方（CLI + 30 个测试）零改动编译通过。
2. **接受 config 的入口**：把 `run_mkgrd_top_level_namelist_with_default_restart_refine_handoff`（`lib.rs:14484`）在 `fs::read_to_string` + `from_mkgrd_namelist` 之后的主体抽出为 `..._with_config(config, contents, …, sink)`；原路径函数变成读文件后转调新函数的薄壳。GUI 把编辑器里的 `.nml` 文本 + 已解析 config 传进去，避免临时文件。
3. **两处循环钩子**：
   - 细化步循环 `run_mkgrd_refine_loop_execution_with_final_domain_contain`，`lib.rs:13200` 的 `for step in &plan.steps` 顶部：`sink.report("refine", k, plan.steps.len())` + `if sink.cancelled() { return Err(Cancelled) }`。
   - spring 迭代 `gridinit_voronoi_state_reference` 的 `for iteration in 1..=niter`，`earthmesh_mesh/src/lib.rs:1358` 顶部：复用已有 `diagnostic_every` 节奏 `sink.report("spring", iteration, niter)` + 查 `cancelled()`。
4. 取消语义：协作式。两次检查之间无法中断；若用户在长块中途取消，GUI 可弃用该线程结果（无共享状态，安全）。

> MVP 兜底：即使 P0 未完成，GUI 也可今天就用现有"路径版"入口在 worker 线程跑（已确认线程安全），只是没有进度与中途取消。P0 让其升级为有 `iter N/5000` 进度 + 可取消。

### 3.3 运行时线程模型
- UI 线程（egui 即时模式）只做绘制与事件。
- 点击"运行" → spawn 一个 `std::thread`，传入 `ProgressSink`（内部持 `Sender<ProgressMsg>` + `Arc<AtomicBool>` 取消位）。
- worker 调 `run_job(...)`（同步）。进度/日志/完成事件经 channel 回传；UI 侧每帧 drain channel 并 `ctx.request_repaint()`。
- 同一时刻只允许一个运行任务，且每个任务用独立 `workdir`（避免输出目录冲突）。

### 3.4 零依赖安装（NetCDF 静态链接，已核实）
- `netcdf = { version = "0.12", features = ["static"] }`：捆绑 netCDF-c 4.9.3 + HDF5 2.0.0 + zlib，全部 vendored 源码编译，**运行时零依赖**，无需 `NETCDF_DIR`/系统库/conda。
- 构建机要求：CMake ≥ 3.20、C 编译器、**C++ 编译器**、Rust ≥ 1.77；首次编译 HDF5 较慢，CI 要缓存 `target/`。
- 明确**保持关闭** `dap`（会拉 curl/OpenSSL）与 `mpi`（static 下直接 panic）；二者默认即关闭。
- 体积：netcdf/hdf5 静态部分约单位数 MB；用 `lto=true`+`codegen-units=1`+`strip=true` 缩减。
- Windows 用 **MSVC**（`x86_64-pc-windows-msvc`），不用 MinGW（上游已知 MinGW 静态构建坏）。

### 3.5 打包（MVP 含本地出包；CI/签名留 P5）
- 工具：**`cargo-packager`**（CrabNebula）——单工具产出三平台 GUI 安装包：macOS `.app`+`.dmg`、Windows NSIS `.exe`+`.msi`、Linux `.deb`+`.AppImage`，内置签名/公证钩子。
- MVP：在开发者本机（macOS）`cargo packager` 出一个可双击的 `.app`/`.dmg`（**未签名**；首次打开走右键 Open 或清隔离属性）。
- 配置：`[package.metadata.packager]`（bundle id、版本、图标、分类）。
- P5 再补：GitHub Actions 三平台矩阵（ubuntu-22.04 / macos-14+macos-13 / windows-msvc）、Apple Developer ID 签名+`notarytool` 公证、Windows Authenticode 签名。

---

## 4. MVP 详细设计（P0 + P1）

### 4.1 P0 — 引擎接缝
见 §3.2。交付物：`ProgressSink` trait + `run_*_with_config` 入口 + 两处循环钩子 + `Cancelled` 错误类型。
验收：`cargo test`（含 30 个集成测试）与 `mkgrd.x` 行为不变。

### 4.2 P1 — GUI App（`earthmesh_gui`）

#### 4.2.1 配置数据模型
- 定义 `GuiConfig`：扁平、可 `serde` 序列化，**镜像** `EarthmeshConfig` + `RefineConfig` 的全部 92 个用户可见键（含数组槽展开为单键，如 `RL%refine_lai_m` → 一个布尔字段 + 配对阈值字段）。
- **`.nml` 往返**：
  - 读：复用现有 `from_mkgrd_namelist` / `from_mkrefine_namelist` → 填充 `GuiConfig`。
  - 写：新增 namelist writer（`GuiConfig` → `&mkgrd`/`&mkrefine` 文本），保证与示例 `.nml` 往返语义一致。
  - 内部工程格式可用 `.toml`/`.json`（serde）做"算例存档"，但**对引擎始终经 `.nml`/config 结构**，保证与 CLI 互通。
- 默认值取 namelist/示例默认（不是 Rust `Default` 里的 `"/tmp"`/`0` 占位哨兵）。

#### 4.2.2 主窗口：三栏工作台（方案 A + 全局搜索）
- **顶部工具条**：▶ 运行 / ⏹ 取消、💾 保存、📂 加载 `.nml`、🔍 全局搜索（跳转到 92 项中任意一项）、语言切换入口、打开输出目录。
- **左栏**：算例列表（MVP：最近打开的 `.nml` + 内置示例作为模板起点）。
- **中栏**：5 个标签页的配置表单（见 §4.2.3）。
- **右栏**：运行面板——进度条（spring `i/niter`、refine `k/N`）、滚动日志、运行/取消按钮、状态（成功/失败/已取消）。

#### 4.2.3 配置表单：5 个标签页（92 键）
控件映射与默认值依据已枚举的字段表（见附录 A 概要）：
1. **基础**（`&mkgrd`）：experiment_name(text)、base_dir(dir)、mesh_type(下拉)、mode_grid(下拉)、NXP(数字)、output_format(下拉)、openmp(数字)、gridnum_perdegree(下拉 120/240)、landtype_file(文件)、mode_file/mode_file_description。
2. **初始网格 / Spring**：niter、beta、relax、niter_refine、SpringGlobal_type(下拉)、num_rc、set_dis_type(下拉)、SpringRegional_type(下拉)、vertex_pretect_layers。
3. **掩膜 / 区域**：mask_domain_global、mask_domain_type(下拉)、mask_domain_fprefix(文件)、mask_restart、mask_sea_ratio、mask_patch_on、mask_patch_type、mask_patch_fprefix、isolated_ocean。
4. **细化 - 总体**（`refine=true` 时启用）：refine、weak_concav_eliminate、Istransition、HALO(多值)、max_transition_row(多值)、iterD。
5. **细化 - 指定 & 阈值**：指定区域（refine_spc、max_iter_spc、mask_refine_spc_type/fprefix）；阈值计算（refine_cal、max_iter_cal、mask_refine_cal_type/fprefix、threshold_dir）；按 mesh_type 出现的陆/海/气判据开关+阈值对（5c–5f，共 54 项）。

#### 4.2.4 校验与联动（MVP 实现"结构性"规则，穷尽打磨留 P2）
GUI 侧实现以下 enable/disable 与过滤（其余 15 条规则在 P2 完善）：
- `output_format` 下拉按 `mesh_type` 过滤（land/earth/LOC→CoLM；ocean→FVCOM；atmos→MPAS/MPAS-Simple）。
- `gridnum_perdegree` 仅 120/240。
- `mask_domain_type`/`fprefix` 仅当 `mask_domain_global=false` 可用；`mask_patch_*` 仅当 `mask_patch_on=true`。
- 标签页 4/5 整体仅当 `refine=true` 可用。
- `refine_cal` 在 `mesh_type=atmosmesh` 时禁用；`refine=true` 时要求 `refine_spc`/`refine_cal` 至少一个为真。
- 指定/阈值各自子区仅当对应开关为真可用；判据开关勾选时其配对阈值必填（哨兵 999.0 视为未填）。
- **运行前**：调用现有 `validate_like_read_nl` 做最终校验，把错误信息在界面友好呈现（即便 GUI 漏判，引擎也会拒绝非法配置）。

#### 4.2.5 国际化（i18n）
- `rust-i18n 3.1`：编译期内嵌 `zh-CN` / `en` 翻译文件，`t!("key")` 取词，运行时 `set_locale(...)` 全局切换 + `request_repaint()`。
- MVP：搭好框架 + 覆盖所有界面字符串（标签、按钮、错误、92 项的显示名与提示）。设置里提供语言下拉。

#### 4.2.6 打包（MVP 本地）
- 给 `earthmesh_gui` 接上 `netcdf` 的 `static` 特性、`[profile.release]` LTO/strip、`[package.metadata.packager]`、应用图标。
- 文档化 `cargo packager` 本地出包步骤（macOS `.dmg`）。

---

## 5. 测试策略
- **P0 引擎接缝**：单元测试 no-op sink 不改变行为；新增一个调用 `run_*_with_config` 的测试，断言与路径版结果一致；用一个会汇报进度的假 sink 断言 `report`/`cancelled` 在两处循环被调用；`cargo test` 全绿。
- **配置往返**：对每个 `examples/**/*.nml` 做 `nml → GuiConfig → nml` 往返，断言语义等价（解析回来字段一致）。
- **校验规则**：对 §4.2.4 每条规则写表驱动单元测试。
- **后台运行/取消**：集成测试用低 NXP 小算例跑通；测试取消位在循环顶部生效。
- **手动验收**：按 §1.3 成功判据逐条走查（含本地出包双击打开）。

## 6. 风险与缓解
| 风险 | 缓解 |
|---|---|
| 首次 static 构建慢（编译 HDF5） | CI/本地缓存 `target/`；文档说明首次较慢。 |
| egui 家族跨小版本破坏 API；walkers/egui_plot 可能滞后 | 全家锁定同一 minor（0.34.x）；`cargo tree` 校验 egui 单一版本。 |
| 92 选项 + 15 联动规则易错 | 规则表驱动测试；运行前兜底调 `validate_like_read_nl`。 |
| 取消只能协作式 | 文档说明；长块中途取消采用"弃用线程结果"兜底。 |
| 未签名包在 macOS/Win 触发 Gatekeeper/SmartScreen | MVP 文档说明绕过方式；P5 补签名。 |

## 7. 路线图（P2–P5，MVP 之后）
- **P2 校验+模板**：穷尽 15 条联动规则的 UI 联动；全局搜索完善；模板/算例库（示例作起点、复制/重命名）；设置（默认 base_dir、默认线程数、语言）。
- **P3 可视化**：`egui-wgpu` `CallbackTrait` 自绘 3D 地球 + 非结构网格（线框/按格着色/相机控制），读取输出 `gridfile_*.nc4`；`egui_plot` 出网格质量/收敛统计；跑完即预览。
- **P4 地图选区**：`walkers 0.53` slippy 地图 + `Plugin` 叠加层，画 bbox/圆/多边形 → 写成引擎所需的 `mask_*_fprefix` 区域文件，免去手编。
- **P5 发布打磨**：GitHub Actions 三平台矩阵出包；Apple Developer ID 签名+`notarytool` 公证；Windows Authenticode 签名；（可选）Linux Flatpak/Flathub。

## 8. 待确认问题
- 暂无阻塞项。`earthmesh_gui` 是否复用 `earthmesh_cli` 库名（直接依赖）还是后续抽 `earthmesh_engine` facade，留实现计划阶段按编译边界决定（MVP 直接依赖 `earthmesh_cli` 库）。

---

## 附录 A — 配置字段枚举（概要）
完整逐字段枚举（Rust 字段名、namelist 键、类型、默认值、可选枚举、控件、含义）见头脑风暴研究产物；要点：
- `&mkgrd` 暴露 22 键；`&mkrefine` 暴露 70 键（16 个通用/spring/指定/阈值 + 54 个陆/海/气"开关+阈值"对）= **92 键**。
- 固定枚举：`mesh_type∈{landmesh,oceanmesh,atmosmesh,LOCmesh}`、`mode_grid∈{lonlat,lambert,cubical,tri,hex,dbx}`、`output_format∈{CoLM,FVCOM,MPAS,MPAS-Simple}`、`mask_*_type∈{bbox,lambert,close,circle}`、`set_dis_type∈{linear,nonlinear1..3}`、`SpringGlobal_type∈{0,1}`、`SpringRegional_type∈{0,1,2}`、`gridnum_perdegree∈{120,240}`。
- 只读派生（不作输入）：`refine_setting`、`max_iter`、`mask_refine_ndm`、`exit_loop_step`。
- `LonLatMeshConfig` / `FvcomMeshConfig`：无 namelist 解析器（仅测试引用），MVP 不暴露。

## 附录 B — 关键 file:line 锚点
- CLI 入口：`earthmesh_cli/src/main.rs:7,17,40,165,594`
- 顶层派发：`lib.rs:14484`（默认）/ `lib.rs:14127`
- gridinit：`lib.rs:14604`，内核调用 `lib.rs:14689`
- 细化步循环（进度钩子）：`lib.rs:13200`
- spring 迭代循环（进度钩子）：`earthmesh_mesh/src/lib.rs:1358`，内核签名 `:395`
- 配置结构：`earthmesh_core/src/lib.rs:700`(`EarthmeshConfig`) / `:1092`(`RefineConfig`)
- 解析器/校验：`:805`、`:944`、`:1369`
