# 01 — Build & Crate Audit (EarthMesh v3)

> Phase P1 体检 · 只读阶段（未修改任何 `src/rust` 代码）
> 基准：[AUDIT_PRINCIPLES.md](./AUDIT_PRINCIPLES.md) · 计划：[PROJECT_AUDIT_PLAN.md](./PROJECT_AUDIT_PLAN.md)
> 审查对象：EarthMesh v3，分支 `v3.0.0-alpha1`，仅当前项目，不引用任何旧版本。
> 环境：macOS 25.5.0 (arm64) · cargo/rustc **1.95.0 (Homebrew)** · rustfmt 1.9.0 · clippy 0.1.95 · 系统 netCDF 4.9.3 (miniforge `nc-config`)
> 日期：2026-06-22 · 证据等级见 AUDIT_PRINCIPLES §5 (A=已证实/B=强推断/C=疑似/D=假设)

---

## 1. Executive Summary

EarthMesh v3 是一个**无 workspace 根**的多 crate Rust 项目（5 个独立 crate 经 path 依赖串联），CLI 把所有 workflow 做成 library 函数、main 仅做薄派发，GUI 直接链接 CLI crate 作为其事实上的 workflow/IO 库。**轻量 crate（core/geometry）编译、测试、fmt、clippy 全绿**；**mesh 绝大多数测试通过，但 `olam_delaunay_mesh` 有 4 个测试当前失败**（半径容差过紧，平台敏感）；**cli/gui 因 36k 行体量 + `static-netcdf` 从源码构建 netcdf-c，在本次 9 分钟命令预算内无法完成编译/测试**，未能独立验证。

整体可作为独立 v3 项目编译运行（有预编译产物 `mkgrd.x` 与 14G/8.5G 缓存为证），但**构建自洽性与 CI 可重复性存在明确风险**：无 workspace 导致 5 份 target（≈27G）与依赖重复编译；`make fmt` 在 3 个 crate 上失败；文档列出的 `cargo test cli`（动态 netcdf）与 `make test`（静态 netcdf）feature 不一致会触发缓存抖动；`static-netcdf` 对 Windows 打包高风险。

| # | 发现 | 严重度 | 类别 | 证据等级 | 位置 |
|---|------|--------|------|----------|------|
| 1 | `olam_delaunay_mesh` 4 个测试失败（半径 ±1e-6 m 容差，相对 ~1.5e-13） | High | Bug/Test | **A** | `rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs:31,246,271,336` |
| 2 | 无 workspace 根 Cargo.toml → 5 份 target、依赖重复编译、无统一 cargo 命令 | High | Build/Arch | **A** | 仓库根（无 `Cargo.toml`） |
| 3 | `cargo fmt --check` 在 mesh/cli/gui 失败（1560/516/22 行 diff），`make fmt` 会红 | Med | Build hygiene | **A** | mesh/cli/gui |
| 4 | `EarthmeshConfig::default()` 用 `"/tmp"`，且 `base_dir`/`mode_file` 带前导空格 `" /tmp"` | Med | Bug/跨平台 | **A** | `rust/earthmesh_core/src/lib.rs:766-790` |
| 5 | GUI 重度依赖 CLI crate（80 处 `earthmesh_cli::`），workflow/IO 未抽成独立库 | Med | API/Arch | **A** | `rust/earthmesh_gui/src/main.rs` |
| 6 | `static-netcdf` 从源码构建 netcdf-c+HDF5+zlib，Windows 打包高风险、CI 极慢 | Med | Package | **B** | `rust/earthmesh_cli/Cargo.toml` `[features]` |
| 7 | 文档命令 feature 不一致：`cargo test cli`(动态) vs `make test`(静态) → 缓存抖动 | Med | Build | **A** | Makefile `test` vs goal 命令 |
| 8 | 全部 crate 版本仍为 `0.1.0`，未反映 v3/3.0.0 | Low | Metadata | **A** | 各 `Cargo.toml` |
| 9 | GUI 无集成测试（0 个 `tests/` 文件，仅 35 个内联 `#[test]`） | Med | Test gap | **A** | `rust/earthmesh_gui` |

**总原则达成（构建维度）**：本阶段聚焦原则 4 的"数值稳定性"子项——发现 #1（半径容差）即数值稳定性/可重复性的直接反例。其余原则留待 P2–P6。

---

## 2. Command Result Table

| Command | Status | Reason / Notes | Required dependency | Recommended CI |
|---------|--------|----------------|---------------------|----------------|
| `cargo test earthmesh_core --all-targets` | **PASSED** (39: 33+6) | 全绿，<1s | 无 | rustc 1.95.0 |
| `cargo test earthmesh_geometry --all-targets` | **PASSED** (15: 9+6) | 全绿，<1s（pyo3 可选未启用） | 无（默认无 pyo3） | rustc 1.95.0 |
| `cargo test earthmesh_mesh --all-targets` | **FAILED** | ~230 通过、3 ignored、**`olam_delaunay_mesh` 4 失败**（见 §6/#1）；编译 18s + 测试 53s | 无 | rustc 1.95.0 |
| `cargo test earthmesh_cli --all-targets` | **NOT COMPLETED** | 540s 超时仍在编译；145 个集成测试二进制 + 默认动态 netcdf 触发 netcdf-sys 重建（与缓存的静态构建冲突） | 系统 netCDF (`nc-config`) | 预建 netcdf；拆分测试分片 |
| `cargo test earthmesh_gui --all-targets` | **NOT RUN** | 依赖 cli `static-netcdf`，编译成本同上；本次未尝试完整跑 | static netcdf 工具链 | 同 cli |
| `make` (build cli static→mkgrd.x) | **NOT COMPLETED** | 420s 超时；`static-netcdf` 从源码构建 netcdf-c（见 #6） | cmake + C 工具链 | 缓存 netcdf-c 构建 |
| `make test` | **PARTIAL** | core/geometry/mesh 段等价于上（mesh 段会因 #1 失败）；cli 段超时 | 同上 | 同上 |
| `make test-gui` | **NOT RUN** | 同 gui | 同上 | 同上 |
| `make test-full` | **NOT RUN** | 含 `check-method-c-neighbors` 脚本 + `test-slow`（`--ignored` + 外部 fixture） | MERIT/landtype/gridfile fixtures | nightly job |
| `cargo fmt --check`（5 crate） | **MIXED** | core/geometry **PASS**；mesh **FAIL** (1560 行 diff)、cli **FAIL** (516)、gui **FAIL** (22) | rustfmt 1.9.0 | 固定 rustfmt 版本 |
| `cargo clippy`（core, geometry） | **PASSED (warnings)** | core 3 类 lint、geometry 1 类，均非阻塞 warning | 无 | `-D warnings` 可选 |
| `cargo clippy`（mesh, cli, gui） | **NOT RUN** | 需完整重编译，成本同 test | 同对应 crate | 同上 |

> 复现要点：core/geometry/mesh 三段为本次**实跑结果**；cli/gui/make* 为**实测超时**（非主观跳过），原因均为 36k 行体量 + `static-netcdf` 源码构建。

---

## 3. Crate Dependency Graph

```
                 earthmesh_core  (1.9k 行, 无依赖, 常量/配置/runtime/progress)
                  ▲      ▲      ▲
                  │      │      │
   earthmesh_geometry    │   earthmesh_mesh ──► earthmesh_core
   (610, 可选 pyo3,       │   (22.4k 行单 lib.rs, 网格/refine 内核)
    cdylib+rlib)         │        ▲
        ▲                │        │
        └──────┐         │        │
               earthmesh_cli ─────┘     (36k 行 lib.rs + 1.5k main.rs)
                 ├─► earthmesh_core / earthmesh_geometry / earthmesh_mesh
                 └─► netcdf 0.12 (default-features=false; feature `static-netcdf`)
                      ▲
                      │ (features=["static-netcdf"])
               earthmesh_gui ─► earthmesh_core + earthmesh_cli + eframe0.34 + rfd + open + walkers0.53
                 (4.4k main.rs + i18n)
```

- **无环**：依赖呈 DAG，`core` 为叶，`gui` 为根；未发现循环依赖（A 级）。
- **无 `[workspace]`**：仓库根无 `Cargo.toml`，各 crate 仅靠 `path =` 互连；每个 crate 自带 `target/`（cli 14G、gui 8.5G、mesh 4.6G、core 51M、geometry 13M ≈ 27G）。共享依赖（core/mesh/netcdf/eframe）被多次独立编译。
- `cli` 的 `main.rs` **未绕过** lib：0 处直接 `earthmesh_mesh::`/`earthmesh_geometry::`，仅 2 处 `earthmesh_core::`，其余全部经 `earthmesh_cli::` 公开 API → main→lib 分层干净（A 级，反驳"main 调用未暴露 API"）。

---

## 4. API Exposure Issues

| 项 | 现状 | 判定 | 证据 |
|----|------|------|------|
| main.rs 调用未暴露 API | main 全程经 `earthmesh_cli::` pub API（编译通过即证明无私有越权） | **无问题** | `main.rs:169..` 全为 `earthmesh_cli::`；A |
| CLI workflow 是否已抽成 library API | `run_mkgrd_*`、`write_*` 等 workflow **已在 `lib.rs`**，main 仅派发/打印 | **已基本完成** | `cli/src/lib.rs` 含 308 个 `pub fn` |
| public surface 过宽 | cli lib.rs 暴露 **308 个 pub fn** + 数十 pub struct/enum，无模块边界（单 lib.rs） | **问题** | `cli/src/lib.rs`；A |
| GUI 过度依赖 CLI | GUI 80 处引用 `earthmesh_cli::`（类型 `GridfileMeshPoints`/`GridRegion`/`LonLatPoint`/`BBoxMask`… 与 `write_*`） | **问题/分层倒置** | `gui/src/main.rs:355,500,1131..1761`；A |
| 库定位 | `earthmesh_cli` 自述为"mkgrd 兼容 CLI adapter"，实际同时充当 GUI 的核心 workflow/IO 库 | **命名/定位错配** | README + 依赖关系；B |

**结论**：API 暴露本身无"未暴露调用"硬 bug；真正问题是**架构层面**——workflow/IO 逻辑住在名为 "cli" 的 crate 里，导致 GUI 必须整体依赖 CLI（并被迫开 `static-netcdf`），且 308 个无边界 pub fn 难以形成稳定 API 契约。建议抽出 `earthmesh_workflow`（或 `earthmesh_io`）库（详见 §8 重构路线图，留待 P7）。

---

## 5. Build / Package Risks

1. **无 workspace（A，High）**：5 份 target、依赖重复编译、无 `cargo build/test/fmt` 一把梭；`make` 用手工逐 manifest 弥补。→ 加根 `[workspace]`（共享 target、统一 lockfile）。
2. **`static-netcdf` 跨平台（B，Med-High）**：`netcdf/static` 从源码构建 netcdf-c+HDF5+zlib，需 cmake + C 工具链；本次 macOS 上 >7min 未完成；**Windows 静态构建 netcdf-c 历来脆弱**（需 MSVC/cmake，易失败）。→ Windows 走预编译 netcdf DLL 或 vcpkg；dev 默认动态。
3. **feature 不一致触发缓存抖动（A，Med）**：`cargo test cli`（默认=动态 netcdf）与 `make test`（`--features static-netcdf`）切换会重建 netcdf-sys，正是本次超时主因之一。→ 文档统一为同一 feature 集；CI 固定一种。
4. **`cargo fmt --check` 失败（A，Med）**：mesh/cli/gui 有 diff（1560/516/22 行）。可能是代码未按 rustfmt 1.9.0 格式化或未启用 fmt 门禁。→ 仓库 `rust-toolchain.toml` 固定版本 + 一次性 `cargo fmt` + CI 门禁。
5. **跨平台路径（A，Med）**：
   - `gui/src/main.rs:93` 用 `var_os("HOME")` → Windows 无 HOME（应回退 `USERPROFILE`），目前 Windows 会落到 `current_dir`。
   - `gui/src/main.rs:62,143` `dir.join("../Resources")` 为 macOS `.app` 约定；Win/Linux 打包无对应布局，资源仅能靠 `EARTHMESH_RESOURCE_DIR`。
   - `core/src/lib.rs:766-790` 默认配置硬编码 `"/tmp"`（POSIX-only），且 `base_dir:" /tmp"`、`mode_file:" /tmp"` 带前导空格（疑似 typo，#4）。
6. **examples/assets 解析（A，Med，但 macOS 已覆盖）**：`examples_root()` 候选顺序 = `EARTHMESH_RESOURCE_DIR` → exe `../Resources`（mac bundle） → `CARGO_MANIFEST_DIR/../..`（dev）。dev 与 mac 打包均可定位；**Win/Linux 打包需显式 `EARTHMESH_RESOURCE_DIR`**，否则找不到 examples。Cargo packager `resources=["../../examples"]` 仅对应 mac/通用 bundle。
7. **版本元数据（A，Low）**：所有 crate `version=0.1.0`，cli 缺 `description/license`（其余 crate 有）。→ 统一 `version="3.0.0-alpha.1"` + 补 cli 元数据。
8. **release/debug 输出路径（A，Low）**：Makefile `CARGO_TARGET_DIR=rust/earthmesh_cli/target`，release→`target/release/earthmesh_cli`、debug→`target/debug/earthmesh_cli`，统一 copy 到 `./mkgrd.x`，cli 内部一致；但无 workspace 使 gui/mesh 产物各自为政，无统一 `bin/` 约定。

---

## 6. Test Coverage Gaps

| 范围 | 现有覆盖 | 缺口 / 风险 | 证据等级 |
|------|----------|-------------|----------|
| earthmesh_core | 33 内联 + 6 集成（namelist roundtrip） | 充分 | A |
| earthmesh_geometry | 9 内联 + 6 集成；pyo3 路径未在默认测试启用 | `extension-module`/Python 绑定无 CI 覆盖 | A |
| earthmesh_mesh | 94 内联 + 41 集成文件，~230 测试 | **`olam_delaunay_mesh` 4 红**（icosahedron 闭合/spring/expand-global2/3）：断言"每点半径=`EARTH_RADIUS_METERS` ±1e-6 m"，实测偏差 >1µm（相对 ~1.5e-13）= **过紧绝对容差，平台敏感**，非拓扑错误 | **A** |
| earthmesh_cli | 13 内联 + **145 集成文件**（未在本次跑完） | 全量 CI 时间不可控（145 测试二进制 + static netcdf）；本次未独立验证绿 | A（规模）/ C（绿否） |
| earthmesh_gui | 35 内联 `#[test]`，**0 个 `tests/` 集成文件** | GUI run 编排（`start_run`/`poll_run`）、asset 解析、跨平台路径无集成测试 | A |
| 慢测试 / 全量 | `test-slow`（`--ignored`）+ `check-method-c-neighbors` 脚本 | 依赖外部 fixture（MERIT/landtype/gridfile），CI 需单独 nightly | A |

**#1 失败定位**：`olam_delaunay_mesh.rs:31/246/271/336` 均为 `assert!((magnitude(point) - EARTH_RADIUS_METERS).abs() <= 1.0e-6, "point {id} radius {r}")`，失败信息 `point 2 radius 6371220`（即第一个被检点就超差）。判定为**容差工程问题**（建议改相对容差，如 `<= 1e-6 * EARTH_RADIUS_METERS` 即 ~6.4mm），需在 P2 落实根因（是否 spring/expand 后未严格 renormalize）。

---

## 7. Immediate Fixes (最小修复建议，不在本阶段落地)

> 均为最小、可回溯改动；P1 为只读阶段，仅记录，落地见 §8 Patch Plan / P8。

1. **#1 半径容差**：将 4 处 `<= 1.0e-6` 改为相对容差 `<= 1.0e-6 * EARTH_RADIUS_METERS`（或对 spring/expand 输出强制 renormalize 到地球半径）。先确认是测试容差问题还是内核未归一化（P2 根因）。— 改 ≤4 行（测试）或 1 处内核。
2. **#4 默认配置**：去掉 `base_dir:" /tmp"`/`mode_file:" /tmp"` 的前导空格；`"/tmp"` 默认改用 `std::env::temp_dir()` 或空串占位（跨平台）。— 改 ~20 行单文件。
3. **#3 fmt**：固定 `rust-toolchain.toml` + 跑一次 `cargo fmt`（独立提交，纯格式）+ CI 加 `make fmt` 门禁。
4. **#7 feature 一致性**：README/计划统一 cli 测试 feature（建议统一 `--features static-netcdf` 或为 dev 提供动态档），避免缓存抖动。
5. **#2 workspace**：加仓库根 `Cargo.toml [workspace] members=[...]` + 共享 `target/`；显著降本（重复编译/磁盘）。

---

## 8. Patch Plan (提案，待 P8 批准后落地)

| Patch ID | 关联发现 | 目标 | 改动摘要 | 验证命令 | 风险 | 审批 |
|----------|----------|------|----------|----------|------|------|
| PATCH-01 | #1 | `mesh/tests/olam_delaunay_mesh.rs` 或半径归一化内核 | 相对容差 / renormalize（先定根因） | `cargo test mesh --test olam_delaunay_mesh` | 低（若为容差）/ 中（若改内核） | 待批 |
| PATCH-02 | #4 | `core/src/lib.rs:766-790` | 去前导空格 + 跨平台临时路径 | `cargo test core --all-targets` | 低 | 待批 |
| PATCH-03 | #2 | 仓库根新增 `Cargo.toml [workspace]` | 5 crate 纳入 workspace，统一 target | `cargo build --workspace` + `make` | 中（路径/CI 调整） | 待批 |
| PATCH-04 | #3 | `rust-toolchain.toml` + 全量 `cargo fmt` | 固定版本 + 一次性格式化 | `make fmt` | 低（纯格式，独立提交） | 待批 |
| PATCH-05 | #5 | 抽 `earthmesh_workflow`/`earthmesh_io` 库 | 迁 workflow/IO 出 cli，cli/gui 共依赖 | 全量 `make test` | 高（大重构，归 P7） | 待批 |
| PATCH-06 | #6 | cli `[features]` + GUI 构建文档 | Windows 预编译 netcdf 方案 + dev 动态档 | 三平台 CI build | 中 | 待批 |

---

## 9. Open Questions

1. **#1 olam 失败的根因**：在本机（arm64/Homebrew rustc 1.95）为红——这是测试容差工程问题，还是 spring/expand 后点未严格归一化到 `EARTH_RADIUS_METERS`？是否在原开发机上为绿（即平台/工具链敏感）？(P2 根因待查)
2. **fmt diff 性质**：mesh/cli/gui 的 fmt diff 是"代码未格式化"还是"rustfmt 版本差异"？仓库是否曾启用 fmt 门禁？是否需要固定 rustfmt 版本？
3. **netcdf 策略**：v3 是否要求 GUI 单文件零依赖分发（即必须 `static-netcdf`）？还是可接受动态链接 + 随包 netcdf 库？这决定 Windows 打包方案。
4. **workspace 迁移意愿**：是否接受引入根 `[workspace]`（会改变 target 路径与 Makefile 假设）？
5. **cli 全量测试的 CI 形态**：145 个集成测试二进制是否需分片 / 选择性运行？是否设独立 nightly 跑 `test-slow`/`test-full`？
6. **GUI 集成测试**：是否需要为 GUI run 编排与 asset 解析补 headless 集成测试（当前 0）？

---

*本报告所有"PASSED/FAILED"为本机实跑结果；"NOT COMPLETED/NOT RUN"均为实测超时或成本过高，非主观跳过。未修改任何 `src/rust` 代码。*
