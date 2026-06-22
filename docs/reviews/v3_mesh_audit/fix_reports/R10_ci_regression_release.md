# R10 — CI / Regression Examples / Release Hardening (MVP) 报告

> 阶段：R10（最小有效回归 + 发布检查）· 配套 [RELEASE_TRACKER.md](../RELEASE_TRACKER.md) / [FIX_PLAN.md](../FIX_PLAN.md)
> 日期：2026-06-22 · cargo/rustc 1.95.0 (Homebrew)
> 原则：不依赖大型外部数据;MERIT-Hydro 大数据测试 `#[ignore]`/feature-gated;smoke 用 tiny synthetic;CI 失败信息 actionable;examples 区分 runnable template vs external-data。

## 1. 新增/完善内容
- **CI**(`.github/workflows/ci.yml`,新增):`fast` job(fmt + 5 个无 netcdf crate,秒级、无系统依赖)+ `heavy` job(apt 系统 netcdf + eframe 依赖,cli/gui 动态链接)。两 job 均有 `if: failure()` 步骤打印**本地复现命令**。
- **Makefile**:`fmt`/`test` 补齐 quality+refine_planner;新增 `test-fast`(无 netcdf 回归门)+ `release-check`(fmt + test-fast 发布快门)。
- **examples 分类**(`examples/README.md` 新增 + `examples_paths.rs` 新测试):runnable template(quickstart + default/)无 `${EARTHMESH_DATA}`;external-data case(merit_hydro/*)须带 README。
- **Snapshot 测试**(`earthmesh_quality/tests/snapshot.rs` 新增):tiny 合成网格 → coupling CSV / manifest JSON 字节级金值。

## 2. Test matrix
| # 要求 | 落地 | 位置 | 外部数据 |
|--------|------|------|----------|
| 1 fmt check | `make fmt`(7 crate)+ CI fast | Makefile / ci.yml | 无 |
| 2 test core/geometry/mesh/cli/gui | `make test`(+quality+refine)`make test-gui`;CI fast+heavy | Makefile / ci.yml | cli/gui 需 netcdf |
| 3 quickstart smoke | `00_quickstart_n16.nml`(tiny)+ GUI 默认载入 | examples / gui | 无 |
| 4 default atmos/land/ocean 验证 | `default/*.nml` + 分类测试 | examples / examples_paths.rs | 无 |
| 5 MERIT no-data / missing-root | `quality::hydro_coast`(merit_root_exists=false→Fail;no-tiles→coverage 0) | hydro_coast.rs 测试 (R6) | 无(合成) |
| 6 geometry validation | `geometry::safety`(validate_polygon/fraction/buffer/dateline) | safety.rs 测试 (R3) | 无 |
| 7 quality report 生成 | `quality` io 测试 + snapshot | quality tests (R4) + snapshot.rs | 无 |
| 8 coupling report 生成 | `quality::coupling` 测试 + snapshot | coupling.rs 测试 (R7) + snapshot.rs | 无 |
| 9 snapshot small outputs | coupling CSV / manifest JSON 金值 | snapshot.rs | 无 |
| 10 ignored slow ext-data | `make test-slow`(`-- --ignored`)+ static-netcdf feature | Makefile | 需真实数据/netcdf |
| 11 release checklist | 本报告 §6 + `make release-check` | 本文件 / Makefile | 无 |
| 12 local dev commands | 本报告 §5 | 本文件 | — |

## 3. Commands
```sh
# 快回归（无 NetCDF，秒级）—— CI fast job 同款
make test-fast
make fmt
make release-check         # = fmt + test-fast 发布快门

# 全量（需 NetCDF；本地用打包 static-netcdf，无需系统库）
make test                  # core/geometry/mesh/quality/refine_planner + cli(static-netcdf)
make test-gui
make test-slow             # ignored 的大数据/慢测试

# 单 crate
cargo test -p earthmesh_quality --all-targets
cargo test -p earthmesh_refine_planner --all-targets
```

## 4. Required dependencies
- **fast 门(无外部数据)**:仅 Rust toolchain + rustfmt。无系统库。
- **heavy 门(cli/gui)**:系统 NetCDF（`libnetcdf-dev libhdf5-dev pkg-config`）+ eframe Linux 依赖（`libgtk-3-dev libxkbcommon-dev libwayland-dev libxcb-*-dev libgl1-mesa-dev`）。
- **本地替代**:`make`(`--features static-netcdf`)从源码构建 netcdf-c+HDF5+zlib,**无需系统 NetCDF**(首次慢,之后缓存)。

## 5. Local developer commands
```sh
make build                 # 构建 cli → ./mkgrd.x（static-netcdf）
./mkgrd.x examples/00_quickstart_n16.nml      # 跑 runnable 模板
make test-fast             # 改纯 Rust crate 后的快循环
make fmt                   # 提交前格式检查
make release-check         # 打 tag 前快门
make check-method-c-neighbors                 # OLAM Method-C 邻接校验
```

## 6. Release checklist（打 tag 前）
- [ ] `make release-check` 绿（fmt + 5 个无 netcdf crate）。
- [ ] `make test` 绿（含 cli static-netcdf）。
- [ ] `make test-gui` 绿。
- [ ] `make test-slow` 绿（或确认 ignored 的大数据测试在有数据环境跑过）。
- [ ] CI 两 job(fast/heavy)在 PR 上绿。
- [ ] `examples/` 无个人绝对路径（`examples_paths` 测试守护）；runnable/external 分类正确。
- [ ] `RELEASE_TRACKER.md` 的 P0/P1 项已勾。
- [ ] `CHANGELOG`/版本号更新（`Cargo.toml` workspace 各 crate version）。
- [ ] `run_manifest.json` 记录 software_version/git_sha（R2）。
- [ ] 烟雾跑一遍 quickstart + 一个 default 模板,确认 mkgrd.x 退出 0 且产出网格。

## 7. Ignored / slow / external-data tests
`make test-slow`（全部 `-- --ignored`，需真实数据/netcdf，CI 不跑）：
- `mkgrd_mask_restart`（mask restart）
- `colm_coupling_csv_from_mesh::mesh_plus_landtype_classifies_cells_and_writes_colm_netcdf`（landtype→CoLM netcdf）
- `refine_end_to_end_topology::specified_bbox_refine_produces_consistent_closed_mpas`
- `mkgrd_gridinit::run_mkgrd_gridinit_global_matches_fortran_nxp64_gridfile_fixture`
- MERIT-Hydro 真实数据:经 `${EARTHMESH_DATA}` + `merit_hydro/*` case 手动跑（无自动测试，纯诊断在 `quality::hydro_coast` 用合成数据覆盖）。

## 8. Verification（本阶段实测）
- `make test-fast` → **5 crate 全绿,0 failed**（mesh 93 + quality 含新 snapshot 2 + refine_planner 6 + core 含新 examples 分类 + geometry;mesh 1 ignored）。
- **全 7 crate `cargo fmt --check` PASS**（本期应用 `cargo fmt` 清掉 mesh/cli/gui 既有格式欠债,使 CI `make fmt` 步骤可绿）。
- fmt 后 `cargo build -p earthmesh_cli --features static-netcdf` + `-p earthmesh_gui` → exit 0（纯格式化不改逻辑）。
- 新 snapshot 测试（coupling CSV / manifest JSON 金值）+ examples 分类测试 通过。
- ci.yml 为静态 YAML（CI runner 实跑需 push 到 GitHub,见 §9 blocker 1）。

## 9. Remaining blockers
1. **CI 未实跑**:yaml 已就位,需 push 到 GitHub 才能验证 runner 上 apt netcdf + eframe 依赖确实够（heavy job 首次可能需补依赖）。fast job 无系统依赖,风险低。
2. **cli/gui 无 headless 自动 smoke**:cli 真实跑 quickstart 需 netcdf;CI heavy job 跑单元/集成测试但未跑 `./mkgrd.x examples/...` 端到端（可加一步 `cargo run -- examples/00_quickstart_n16.nml` smoke,留后续）。
3. **MERIT 真实数据无 CI 覆盖**:按要求 ignored;诊断逻辑由合成数据单测覆盖。
4. **版本/CHANGELOG**:workspace 各 crate 仍 0.1.0;正式发布前需统一版本策略。
5. **未 push**:本地领先 origin（见 git status）。

## 10. git status
见报告提交后 `git log`/`git status`（R10 代码 + docs 两 commit，工作树干净）。本地 R1–R10 + OLAM 共 ~19 commit 未 push。
