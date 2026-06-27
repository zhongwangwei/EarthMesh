# Ponytail Refactor Audit - 2026-06-25

Scope: repository-wide refactor audit for the OLAM/Fortran migration code, with
special attention to GUI/config/runtime drift. This document is phase 1 output:
no algorithm changes are proposed here.

## Current Evidence

- Repository size: 377 tracked candidate files from `rg --files`.
- Largest files by current line count:
  - `rust/earthmesh_cli/src/lib.rs`: 37,678 lines.
  - `rust/earthmesh_mesh/src/lib.rs`: 22,448 lines.
  - `rust/earthmesh_cli/src/main.rs`: 2,690 lines.
  - `rust/earthmesh_core/src/lib.rs`: 2,437 lines.
  - `rust/earthmesh_project/src/lib.rs`: 1,882 lines.
  - `gui-tauri/dist/index.html`: 1,685 lines.
- Verification already run on the current tree:
  - `make fmt`
  - `make clippy`
  - `make clippy-gui`
  - `make test-fast`
  - `make test-gui`
  - `make check-method-c-neighbors`

## Findings

### Oversized Modules

`rust/earthmesh_cli/src/lib.rs` is the biggest maintenance risk. It mixes CLI
workflow orchestration, restart/refine state, NetCDF IO, project/namelist
bridges, quality/report side effects, and compatibility adapters. The next
split should move cohesive chunks without changing names first.

`rust/earthmesh_mesh/src/lib.rs` is the second largest risk. It combines
geometry kernels, Method-C refinement, icosahedron construction, spring
dynamics, mask postprocess helpers, and Fortran-indexed table adapters. The
test coverage is strong, so this is a good candidate for pure module extraction
after one low-risk GUI/project cleanup batch.

`gui-tauri/dist/index.html` is a single static app file. It is acceptable for a
minimal Tauri shell, but it is now large enough that command/API contract checks
in `Makefile` are carrying too much knowledge. Keep the frontend one-file for
now; do not introduce a framework unless the static file blocks a concrete
change.

### Legacy Naming

Legacy terms are still meaningful in some domains and should not be blindly
renamed:

- `OLAM`, `Fortran`, `legacy`: often document required compatibility behavior.
- One-based arrays and placeholder rows: required by engine parity tests.
- `mkgrd`, `mkrefine`, `NL%`, `RL%`: engine interface vocabulary.

Rename candidates are limited to names that describe current behavior poorly:

- Thin GUI/backend command names that still imply mock/static behavior.
- Deprecated GUI intent aliases already hidden from the user-facing surface.
- Internal wrappers whose only purpose is to adapt Fortran row layout and whose
  caller can be named by current EarthMesh behavior.

### GUI / Config / Runtime Drift

The current GUI split is moving in the right direction:

- Tauri command registration is centralized in `gui-tauri/src-tauri/src/lib.rs`.
- DTOs, project commands, file commands, runner, path discovery, and quality
  parsing are separated.
- `earthmesh_project` owns intent presets, validation, criteria, and lowering.
- `Makefile` contains GUI drift checks for command registration, template
  catalog alignment, hidden regional domains, refinement pass behavior, and
  stale wording.

Remaining drift risks:

- `Makefile` has many inline Node checks. They are useful gates, but the file is
  becoming a second test harness. Once stable, move only repeated checks into a
  tiny script; do not add a new test framework.
- GUI docs and HTML must keep using `earthmesh_project` as the source of truth.
- Any deletion of hidden/deprecated GUI aliases needs a migration test first.

### Dead-Code / Deletion Candidates

Do not delete any Fortran/OLAM compatibility code by name alone. Candidate
deletions need one global call-site scan and one targeted test.

Safer first candidates:

- Stale GUI wording already guarded by `check-gui-js`.
- Unused README/API rows that no longer match `generate_handler!`.
- Thin wrappers in the GUI backend that simply forward to one local function
  and add no validation, no IO boundary, and no clearer command name.

Risky candidates:

- `legacy_*` functions in CLI/mesh.
- Placeholder-row normalization.
- Any NetCDF reader/writer compatibility path.
- Any refinement branch guarded by old namelist fields.

## Recommended Structure

Short term, preserve crate boundaries and split within current crates:

- `earthmesh_cli`
  - `workflow`: top-level run modes and restart/refine orchestration.
  - `io`: NetCDF/gridfile/sidecar readers and writers.
  - `namelist`: CLI-facing project/core config bridge.
  - `refine`: area judge, contain, GetRef, and loop execution adapters.
  - `quality`: final quality gate execution and report writing.

- `earthmesh_mesh`
  - `icosahedron`: base grid construction and expansion.
  - `method_c`: selected-region refinement and perimeter handling.
  - `spring`: global/regional spring dynamics.
  - `grid_preprocess`: connectivity, areas, distances, ordering.
  - `mask_postprocess`: ocean/land boundary compaction and final mesh cleanup.

- `earthmesh_project`
  - Keep as schema owner for now. Split only after CLI/GUI contracts stop
    changing.

- `gui-tauri/src-tauri`
  - Keep current modules. Do not split further until a file exceeds a clear
    responsibility boundary.

## Phase 2 Safety Net

Use the current green commands as the safety baseline:

- Always run `make fmt` after Rust edits.
- Run `make test-gui && make clippy-gui` after GUI/backend contract edits.
- Run `make test-fast && make clippy` after core/project/mesh edits.
- Run `make check-method-c-neighbors` after touching Method-C or neighbor paths.
- Run slow/ignored tests only when touching their covered paths.

## First Implementation Batches

1. Freeze the GUI split: keep current `gui-tauri/src-tauri` modules, verify they
   compile, and only trim redundant checks or stale docs.
2. Pure extraction from `earthmesh_project/src/lib.rs` only if a cohesive
   schema/catalog section can move without changing public names.
3. Pure extraction from `earthmesh_mesh/src/lib.rs` for an isolated helper group
   with existing test files.
4. Pure extraction from `earthmesh_cli/src/lib.rs` only after selecting a small
   function cluster with no NetCDF side effects.
5. Delete stale GUI/doc strings only when already covered by `check-gui-js`.

## Risks

- Numeric parity risk: any reordering in mesh/refine kernels can change outputs.
- Compatibility risk: names that look obsolete may be externally required
  namelist or NetCDF schema fields.
- Test confidence risk: fast tests are broad, but slow ignored cases still guard
  expensive end-to-end parity.
- Review risk: a giant extraction diff is harder to review than a small rename
  or module move, even if behavior is unchanged.

## Stop Condition For Phase 1

Phase 1 is complete when this audit is present, the current safety baseline is
recorded, and the next batch is chosen from the low-risk list above.
