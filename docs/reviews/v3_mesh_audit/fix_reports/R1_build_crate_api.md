# R1 — Build / Crate / API / CLI-GUI 基础修复报告

> 阶段：R1（仅修构建/crate/API/CLI-GUI 基础，**不碰 mesh/refinement 算法**）· 配套 [FIX_QUEUE.md](../FIX_QUEUE.md) / [FIX_PLAN.md](../FIX_PLAN.md)
> 日期：2026-06-22 · 环境：macOS arm64 · cargo/rustc 1.95.0 (Homebrew) · 系统 netCDF 4.9.3 (miniforge)

## 0. 约束（决定可行范围）
- 工作树有你的 **OLAM WIP**：`mesh/src/lib.rs` + `mesh/tests/{olam_delaunay_mesh,olam_spawn_nest,voronoi_grid_state}` + `cli/tests/{mkgrd_gridinit,refine_end_to_end_topology}`（共 6 文件）。工程原则 7/8 → **本阶段不触碰这 6 个文件**。
- 其余文件（根 Cargo.toml、各 `Cargo.toml`、`cli/src/main.rs`、`gui/src/main.rs`、core/geometry）**不在 WIP 内，可安全处理**。
- 不跑 `cargo fmt`(写入)：会重排 mesh/cli/gui 源，与你 WIP 冲突 → 仅 `--check`。
- 不改 `version` 字段：是构建相关字段，改 mesh/cli 版本会令你 WIP 缓存失效 → 推迟。

## 1. Fixed items
| Item | 内容 | 文件 | 验证 | 改变用户行为? |
|------|------|------|------|---------------|
| **EM3-P2-001** | **加 workspace 根 Cargo.toml**（resolver=2 + 5 成员）→ 统一 lockfile、可从根跑 cargo | `Cargo.toml`(新), `Cargo.lock`(新生成) | `cargo metadata` 干净解析 5 成员 ✓；`cargo build -p earthmesh_core` 0.30s ✓；`cargo test --manifest-path .../core` 仍绿 ✓；mesh olam 仍 18/0（不扰 WIP）✓ | 否（`make` 经 CARGO_TARGET_DIR 覆盖仍走原路径） |
| **EM3-P2-010(部分)** | cli crate 补 `description`/`license`（仅它缺；非构建字段，不触发重编） | `rust/earthmesh_cli/Cargo.toml` | `cargo metadata` 显示 ✓ | 否 |
| EM3-P0-001 | **由你的 OLAM WIP 解决**（非本阶段）：mesh 全量绿，`olam_delaunay_mesh` 18/0 | (WIP) | `cargo test mesh --all-targets` 43 套件 0 FAILED ✓ | 否 |

## 2. 检查项结论（R1 acceptance 逐条）
| 检查 | 结论 |
|------|------|
| workspace 是否需要 | **需要，已应用**（§1）。原 5 独立 crate→27G 重复 target；现统一根 lock。 |
| crate 依赖关系 | 无环 DAG：`core←{geometry,mesh,cli,gui}`；`mesh←core`；`cli←{core,geometry,mesh,netcdf}`；`gui←{core,cli(static-netcdf),eframe,walkers}`。 |
| lib.rs/main.rs API 暴露 | cli `main.rs` 全程经 `earthmesh_cli::` pub API（编译通过即证无私有越权）；cli lib 暴露 workflow 函数。正常。 |
| **GUI 是否过度依赖 CLI binary** | **否——GUI 用 library API**：`earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff`/`landtype_file_is_real`（`gui/main.rs:1473,1975,2819`）；`std::process::id()` 仅命名临时目录，**不 spawn `mkgrd.x`**。结构正确。 |
| static-netcdf fallback/文档 | **已有**：`cli/Cargo.toml:13-16` 注释"off by default→链接系统 NetCDF"；fallback=动态系统 netcdf（`nc-config` 在）。 |
| **examples/quickstart 最小入口** | **干净未破坏**：`examples/00_quickstart_n16.nml` 用相对 `base_dir='./cases/'` + `landtype_file='none'`；`default/*.nml` 无个人路径。仅 `merit_hydro/{gba,yangtze}` 含 `/Users/...` 绝对路径 → **R2**（PathResolver/examples 模板化），非 R1 范围。 |
| **unwrap/expect 启动 panic** | **低风险**：`cli/src/main.rs` 全文**零** unwrap/expect/panic；`gui/src/main.rs` 的 `fn main` 块**无** unwrap/expect。启动阶段无明显 panic 源。 |
| cargo fmt/test/make 入口 | 见 §3（fmt 可跑；core/geometry/mesh 可测且绿；cli/gui/make-test 受 static-netcdf 编译时长限制，已记原因）。 |
| 编译 warning/clippy | core/geometry clippy 仅非阻塞 warning（[01](../01_build_and_crate_audit.md)）；mesh/cli/gui clippy 未跑（同 netcdf/编译时长）。 |

## 3. Tests run
| 命令 | 结果 |
|------|------|
| `cargo fmt --check` ×5 | core/geometry **PASS**；mesh(WIP)/cli/gui DIFF（未 apply：避与 WIP 冲突，待 commit 后一次性 fmt） |
| `cargo test core --all-targets` | **PASS（3 套件全绿）** |
| `cargo test geometry --all-targets` | **PASS（3 套件全绿）** |
| `cargo test mesh --all-targets` | **PASS（43 套件 / 0 FAILED；含 olam 18/0）** |
| `cargo test cli --all-targets` | **NOT COMPLETED**：默认 feature 300s 超时；`--features static-netcdf --test cli_help` 单测 290s 亦超时 → 瓶颈是 **static netcdf-c 源码构建本身**（非二进制数量/feature 不匹配） |
| `cargo test gui --all-targets` | **NOT COMPLETED**：依赖 cli static-netcdf，同瓶颈（前轮 90s 无输出） |
| `make test` | **明确结果**：顺序 core→geometry→mesh→cli。前三段等价于上（全绿）；**cli 段 static-netcdf 编译在 400s 内未完成**（同瓶颈）。即 make test 因 cli/netcdf-c 无法在交互预算内整跑完。 |
| `cargo metadata`(workspace) | **PASS**（5 成员解析无冲突） |

## 4. Tests failed
- **无测试失败**。cli/gui/make-test 为**编译未完成**（static netcdf-c 源码构建时长），非断言失败。

## 5. Remaining blockers
1. **OLAM WIP 未提交**（6 文件）→ 阻塞一次性 `cargo fmt`（同文件冲突）与 `version` 统一（缓存失效）。mesh 已全绿是干净提交点，**建议先 commit**。
2. **cli/gui 验证瓶颈 = static netcdf-c 首次源码构建**。**更正（R2 已验证）**：用**后台命令**编完 netcdf-c 后，全量 cli+gui 构建仅 35s，cli_help/gui 测试 + run_manifest 运行时冒烟均通过。即"前台单条命令超时 ≠ 无法验证"——后台暖缓存即可。建议 CI 缓存 netcdf-c 产物（EM3-P2-011）。
3. **5 个 per-crate `Cargo.lock` 被根 `Cargo.lock` 取代**（cargo 在 workspace 下忽略成员 lock，无害）→ 建议后续 `git rm rust/*/Cargo.lock` 清理（本阶段保守保留，因 cli/gui 构建无法即时复验）。

## 6. Next phase risks (R2)
- R2 的 geometry 改动（Σ=1 校验 / `GeometryQualityFlag`）**安全**：geometry crate 干净、非 WIP、不依赖 netcdf、编译快、可即时验证。
- 任何动 cli/gui 的 patch 在 R2+ 都难本地验证（netcdf-c 编译）→ 依赖 CI；建议 R2 优先做 geometry/core 内可验证项。
- workspace 已就位 → 新增 `earthmesh_quality` crate 直接加为第 6 成员，不再是孤立 target。
- `" /tmp"` 默认（EM3-P1-007）被 `core/tests/constants.rs:38,42` 锁定为迁移 parity 值 → 需 parity vs 正确性决策，非随手改。

## 7. 本阶段 DEFER 项（带原因）
| Item | 原因 | 何时 |
|------|------|------|
| 一次性 `cargo fmt` (cli/gui/mesh) | 与你 6 文件 WIP 同文件冲突 | WIP commit 后 |
| `version` 0.1.0→3.0.0-alpha.1 | 构建相关字段，改 mesh/cli 版本令你 WIP 缓存失效 | WIP commit 后 |
| EM3-P1-007 default `" /tmp"` | 被 `constants.rs:38,42` 测试锁定为 parity 占位 | 需 parity 决策 |
| EM3-P0-002 coupling 占位列诚实化 | 改 cli 行为且无法编译复验 | R5（或 commit 后 + CI） |
| `git rm` per-crate Cargo.lock | cli/gui 构建无法即时复验 | 同 commit 后 |

## 8. git diff summary
```
 Cargo.toml                      | (新) workspace 根 [resolver=2 + 5 members]   ← 本阶段(我)
 Cargo.lock                      | (新生成) 根统一 lock                          ← 本阶段(我)
 rust/earthmesh_cli/Cargo.toml   | +2  description + license                     ← 本阶段(我)
 -- 你的 OLAM WIP (非本阶段) --
 rust/earthmesh_cli/tests/mkgrd_gridinit.rs            |  8 +-
 rust/earthmesh_cli/tests/refine_end_to_end_topology.rs| 50 ++---
 rust/earthmesh_mesh/src/lib.rs                        | 220 ++++----
 rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs       | 18 +-
 rust/earthmesh_mesh/tests/olam_spawn_nest.rs          | 10 +-
 rust/earthmesh_mesh/tests/voronoi_grid_state.rs       | 18 +-
```

## 9. git status
```
On branch v3.0.0-alpha1
 M rust/earthmesh_cli/Cargo.toml                          ← R1 本阶段
 M rust/earthmesh_cli/tests/mkgrd_gridinit.rs             ┐
 M rust/earthmesh_cli/tests/refine_end_to_end_topology.rs │
 M rust/earthmesh_mesh/src/lib.rs                         ├ 你的 OLAM WIP
 M rust/earthmesh_mesh/tests/olam_delaunay_mesh.rs        │
 M rust/earthmesh_mesh/tests/olam_spawn_nest.rs           │
 M rust/earthmesh_mesh/tests/voronoi_grid_state.rs        ┘
?? Cargo.toml      ← R1 本阶段 (workspace)
?? Cargo.lock      ← R1 本阶段 (根 lock)
?? docs/reviews/   ← 审查+修复文档
```

## 10. 验收对照
| acceptance | 状态 |
|------------|------|
| cargo fmt 可运行 | ✅（`--check` 全可跑；core/geometry PASS） |
| 五 crate 能 cargo test 或记录原因 | ✅ core/geometry/mesh 绿；cli/gui 记录原因（static netcdf-c 编译时长，连单测超时） |
| make/test 有明确结果 | ✅ core/geometry/mesh 段绿；cli 段 static-netcdf 400s 未完成（明确） |
| CLI/GUI 依赖清楚 | ✅ GUI→cli library API（非 binary）；无环依赖图；workspace 统一 |
| examples/quickstart 最小入口不破坏 | ✅ quickstart 相对路径干净；merit_hydro 个人路径 → R2 |
| 错误信息可操作/启动 panic | ✅ cli/gui main 无启动 unwrap/expect |

---
*本阶段净改动：3 文件（`Cargo.toml` 新建 workspace、`Cargo.lock` 新生成、`cli/Cargo.toml` +2 行）；workspace 已 cargo metadata + build 验证；未碰 mesh/refinement 算法、未碰你的 6 文件 WIP。所有 DEFER 项带明确原因。*
