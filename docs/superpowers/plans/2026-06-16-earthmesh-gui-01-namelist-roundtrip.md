# EarthMesh GUI — Plan 01: Namelist 往返写入器 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 `EarthmeshConfig` 增加一个 `to_mkgrd_namelist()` 写入器，使 `parse → write → parse` 完全往返一致；这是 GUI 保存/加载 `.nml` 与现有 CLI 互通的地基。

**Architecture:** 写入器与现有解析器 `EarthmeshConfig::from_mkgrd_namelist`（`earthmesh_core/src/lib.rs:805`）对称放在同一 crate、同一 `impl` 里（"一起变的代码放一起"）。CLI 与未来的 `earthmesh_gui` 都复用它。本计划是纯库改动，TDD，不涉及 UI 或引擎运行路径。

**Tech Stack:** Rust（`earthmesh_core` crate）；engine namelist 文本；`cargo test` 集成测试。

---

## 本计划在 MVP 计划序列中的位置

MVP（规格 `docs/superpowers/specs/2026-06-16-earthmesh-desktop-gui-design.md`）被拆成独立可测试的计划序列。**本文件 = Plan 01**。后续计划（到达时各自展开为完整 bite-sized 计划）：

- **Plan 01（本计划）** — `&mkgrd`(`EarthmeshConfig`) namelist 往返写入器。
- **Plan 02** — `&mkrefine`(`RefineConfig`) namelist 往返写入器（重复 01 的模式，覆盖 70 个键/数组槽）。
- **Plan 03** — 引擎接缝（P0）：`ProgressSink` trait + `Cancelled` 错误 + 细化步循环非破坏式 `_and_progress` 变体（`earthmesh_cli/src/lib.rs:13191/13200`）；spring 迭代进度（`earthmesh_mesh/src/lib.rs:1358`）作为可选后续。
- **Plan 04** — `earthmesh_gui` 壳 + 后台运行：eframe 三栏脚手架；点击运行=把配置写成 `.nml` 到独立 workdir，worker 线程调 `run_mkgrd_top_level_namelist_with_default_restart_refine_handoff`（`lib.rs:14484`）；状态/进度/取消；加载/保存；打开输出目录。
- **Plan 05** — 配置表单（5 标签页 92 选项）+ 结构性联动校验 + `rust-i18n` 中英文。
- **Plan 06** — 本地打包：`netcdf` 的 `static` 特性 + `[profile.release]` LTO/strip + `cargo-packager` 出 macOS `.dmg`。

> 每个计划交付可独立测试的软件。先做 Plan 01 → 02（数据地基，纯 TDD），再做 03（引擎），随后 04–06（应用与打包）。

---

## File Structure

- 修改：`rust/earthmesh_core/src/lib.rs` — 在 `impl EarthmeshConfig`（含 `from_mkgrd_namelist`，约 `:805`–`:869`）内新增 `pub fn to_mkgrd_namelist(&self) -> String`。职责：把配置序列化回 `&mkgrd` 块。
- 新建：`rust/earthmesh_core/tests/namelist_roundtrip.rs` — 往返集成测试（仅用 `pub` API）。

无新依赖。

---

## Task 1: `to_mkgrd_namelist` 写入器 + 内联往返测试

**Files:**
- Modify: `rust/earthmesh_core/src/lib.rs`（在 `EarthmeshConfig` 的 `impl` 块内，紧接 `from_mkgrd_namelist` 之后）
- Test: `rust/earthmesh_core/tests/namelist_roundtrip.rs`（新建）

- [ ] **Step 1: 写失败测试**

新建 `rust/earthmesh_core/tests/namelist_roundtrip.rs`：

```rust
use earthmesh_core::EarthmeshConfig;

// 一个最小但通过 validate_like_read_nl 的 &mkgrd 块：
// atmosmesh → output_format 必须是 MPAS/MPAS-Simple；gridnum_perdegree 必须是 120/240。
const SAMPLE_MKGRD: &str = "\
&mkgrd
  NL%EXPNME = 'ATMOS_hex_N64_refine2_global'
  NL%base_dir = './cases/'
  NL%mesh_type = 'atmosmesh'
  NL%mode_grid = 'hex'
  NL%mode_file = 'none'
  NL%mode_file_description = 'none'
  NL%NXP = 64
  NL%refine = .TRUE.
  NL%gridnum_perdegree = 120
  NL%niter = 5000
  NL%beta = 1.0
  NL%relax = 0.035
  NL%openmp = 8
  NL%landtype_file = './input/landtype_usgs_update.nc'
  NL%mask_domain_global = .TRUE.
  NL%mask_domain_type = 'circle'
  NL%mask_domain_fprefix = 'none'
  NL%mask_restart = .FALSE.
  NL%mask_sea_ratio = 0.5
  NL%mask_patch_on = .FALSE.
  NL%mask_patch_type = 'close'
  NL%mask_patch_fprefix = 'none'
  NL%output_format = 'MPAS'
/
";

#[test]
fn mkgrd_namelist_round_trips_through_writer() {
    let original = EarthmeshConfig::from_mkgrd_namelist(SAMPLE_MKGRD).expect("sample parses");
    let rendered = original.to_mkgrd_namelist();
    let reparsed =
        EarthmeshConfig::from_mkgrd_namelist(&rendered).expect("rendered output re-parses");
    assert_eq!(original, reparsed, "parse → write → parse must be identity");
}
```

- [ ] **Step 2: 运行测试，确认编译失败**

Run: `cargo test -p earthmesh_core --test namelist_roundtrip`
Expected: 编译失败 —— `no method named to_mkgrd_namelist found for struct EarthmeshConfig`。

- [ ] **Step 3: 实现写入器（最小代码）**

在 `rust/earthmesh_core/src/lib.rs` 中，`impl EarthmeshConfig` 块内、`from_mkgrd_namelist` 函数之后插入：

```rust
    /// Serialize the configuration back into the `&mkgrd` namelist block that
    /// `from_mkgrd_namelist` consumes. The round-trip
    /// `from_mkgrd_namelist(&x.to_mkgrd_namelist())` reproduces `x`.
    pub fn to_mkgrd_namelist(&self) -> String {
        fn flag(value: bool) -> &'static str {
            if value {
                ".TRUE."
            } else {
                ".FALSE."
            }
        }

        let mut out = String::new();
        out.push_str("&mkgrd\n");
        out.push_str(&format!("  NL%EXPNME = '{}'\n", self.experiment_name));
        out.push_str(&format!("  NL%base_dir = '{}'\n", self.base_dir));
        out.push_str(&format!("  NL%mesh_type = '{}'\n", self.mesh_type));
        out.push_str(&format!("  NL%mode_grid = '{}'\n", self.mode_grid));
        out.push_str(&format!("  NL%mode_file = '{}'\n", self.mode_file));
        out.push_str(&format!(
            "  NL%mode_file_description = '{}'\n",
            self.mode_file_description
        ));
        out.push_str(&format!("  NL%NXP = {}\n", self.nxp));
        out.push_str(&format!("  NL%refine = {}\n", flag(self.refine)));
        out.push_str(&format!(
            "  NL%gridnum_perdegree = {}\n",
            self.gridnum_perdegree
        ));
        out.push_str(&format!("  NL%niter = {}\n", self.niter));
        out.push_str(&format!("  NL%beta = {}\n", self.beta));
        out.push_str(&format!("  NL%relax = {}\n", self.relax));
        out.push_str(&format!("  NL%openmp = {}\n", self.openmp));
        out.push_str(&format!("  NL%landtype_file = '{}'\n", self.landtype_file));
        out.push_str(&format!(
            "  NL%mask_domain_global = {}\n",
            flag(self.mask_domain_global)
        ));
        out.push_str(&format!("  NL%mask_domain_type = '{}'\n", self.mask_domain_type));
        out.push_str(&format!(
            "  NL%mask_domain_fprefix = '{}'\n",
            self.mask_domain_fprefix
        ));
        out.push_str(&format!("  NL%mask_restart = {}\n", flag(self.mask_restart)));
        out.push_str(&format!("  NL%mask_sea_ratio = {}\n", self.mask_sea_ratio));
        out.push_str(&format!("  NL%mask_patch_on = {}\n", flag(self.mask_patch_on)));
        out.push_str(&format!("  NL%mask_patch_type = '{}'\n", self.mask_patch_type));
        out.push_str(&format!(
            "  NL%mask_patch_fprefix = '{}'\n",
            self.mask_patch_fprefix
        ));
        out.push_str(&format!(
            "  NL%isolated_ocean = {}\n",
            flag(self.isolated_ocean)
        ));
        out.push_str(&format!("  NL%output_format = '{}'\n", self.output_format));
        out.push_str("/\n");
        out
    }
```

- [ ] **Step 4: 运行测试，确认通过**

Run: `cargo test -p earthmesh_core --test namelist_roundtrip`
Expected: PASS（`mkgrd_namelist_round_trips_through_writer ... ok`）。

- [ ] **Step 5: 确认无回归**

Run: `cargo test -p earthmesh_core`
Expected: 全部既有测试 + 新测试 PASS。

- [ ] **Step 6: 提交**

```bash
git add rust/earthmesh_core/src/lib.rs rust/earthmesh_core/tests/namelist_roundtrip.rs
git commit -m "feat(core): add EarthmeshConfig::to_mkgrd_namelist writer with round-trip test"
```

---

## Task 2: 用真实示例文件验证往返

**Files:**
- Test: `rust/earthmesh_core/tests/namelist_roundtrip.rs`（追加）

- [ ] **Step 1: 追加失败测试**

在 `rust/earthmesh_core/tests/namelist_roundtrip.rs` 末尾追加（覆盖仓库内全部默认示例，确保真实数据也往返）：

```rust
use std::path::Path;

fn assert_example_round_trips(relative: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let original = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    let reparsed = EarthmeshConfig::from_mkgrd_namelist(&original.to_mkgrd_namelist())
        .unwrap_or_else(|e| panic!("re-parse {}: {e}", path.display()));
    assert_eq!(original, reparsed, "round-trip mismatch for {}", relative);
}

#[test]
fn default_example_namelists_round_trip() {
    assert_example_round_trips("examples/default/atmosphere_hex_global.nml");
    assert_example_round_trips("examples/default/land_hex_global.nml");
    assert_example_round_trips("examples/default/ocean_hex_global.nml");
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p earthmesh_core --test namelist_roundtrip`
Expected: 两个测试函数均 PASS。若某个示例 FAIL，说明该 `.nml` 含写入器未覆盖的 `NL%` 键 —— 回到 Task 1 Step 3，按 `from_mkgrd_namelist` 的 `match`（`lib.rs:834`–`:863`）补齐缺失键后重跑。

- [ ] **Step 3: 提交**

```bash
git add rust/earthmesh_core/tests/namelist_roundtrip.rs
git commit -m "test(core): round-trip &mkgrd writer against bundled example namelists"
```

---

## Self-Review

**1. Spec coverage（针对本计划范围）：** 本计划实现规格 §4.2.1「`.nml` 往返 — 写」的 `&mkgrd` 部分。`&mkrefine` 部分由 Plan 02 覆盖（已在计划序列中列明，非占位）。✅

**2. Placeholder scan：** 无 TBD/TODO；每个代码步骤均含完整代码与确切命令/预期输出。Task 2 Step 2 的"补齐缺失键"是真实的失败处理指引并指向具体行号，非占位。✅

**3. Type consistency：** 测试与实现统一使用 `EarthmeshConfig::from_mkgrd_namelist`（既有 `pub`，`lib.rs:805`）与新增 `to_mkgrd_namelist`；`EarthmeshConfig` derive `PartialEq`（`lib.rs:699`），`assert_eq!` 可用；写入器仅引用结构体既有字段（`lib.rs:700`–`:725`），键名与解析器 `match`（`lib.rs:834`–`:863`）一一对应。✅

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-16-earthmesh-gui-01-namelist-roundtrip.md`.
