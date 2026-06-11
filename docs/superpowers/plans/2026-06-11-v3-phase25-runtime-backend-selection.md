# EarthMesh v3 Phase 25 Runtime Geometry Backend Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let v3 pipeline and regional MERIT workflows select the Python or Rust geometry backend at runtime and record the selected backend in reproducibility sidecars.

**Architecture:** Keep `PythonGeometryBackend` as the default. Add a string `geometry_backend_name` control surface to `build_v3_pipeline_result()`, v3 CLI, and MERIT regional pipeline; resolve it through `get_geometry_backend()` only at the pipeline boundary. Store the effective backend name in `overlay_summary.json` and `pipeline_summary.json` so QA artifacts reveal whether Python or Rust geometry was used.

**Tech Stack:** Python standard library, existing PyO3/maturin Rust extension, pytest. No new dependencies.

---

### Task 1: Pipeline provenance for selected geometry backend

**Files:**
- Modify: `tests/test_v3_pipeline.py`
- Modify: `util/v3_core/pipeline.py`

- [x] Write RED test that passes a custom backend name and expects `overlay_summary["geometry_backend"]` to equal that name.
- [x] Add `geometry_backend_name` to `build_v3_pipeline_result()` while preserving direct `geometry_backend` injection.
- [x] Verify `python3 -m pytest tests/test_v3_pipeline.py -q` passes.

### Task 2: CLI selects geometry backend

**Files:**
- Modify: `tests/test_v3_cli.py`
- Modify: `util/v3_core/cli.py`

- [x] Write RED CLI test for `--geometry-backend python_reference` and `overlay_summary.json` provenance.
- [x] Add `--geometry-backend` argument and pass it to `build_v3_pipeline_result()`.
- [x] Verify `python3 -m pytest tests/test_v3_cli.py -q` passes.

### Task 3: MERIT regional pipeline selects geometry backend

**Files:**
- Modify: `tests/test_v3_hydro_merit_pipeline.py`
- Modify: `util/v3_components/hydro_merit_pipeline.py`

- [x] Write RED tests that `run_merit_v3_pipeline(..., geometry_backend="python_reference")` records backend in `pipeline_summary.json`, and CLI accepts `--geometry-backend`.
- [x] Add `geometry_backend` parameter and CLI argument; pass it through to v3 pipeline and summary.
- [x] Verify `python3 -m pytest tests/test_v3_hydro_merit_pipeline.py -q` passes.

### Task 4: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run focused tests for pipeline, CLI, MERIT pipeline, and geometry backend.
- [x] Run all v3 tests and full Python suite.
- [x] Run compileall and `git diff --check`.
- [x] Clean caches and commit with Lore protocol.
