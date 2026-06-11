# EarthMesh v3 Phase 22 Adapter Cell Artifacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make MPAS, FVCOM, CoLM2024, CoLM20XX, and generic ESMF adapters emit a concrete, stable cell table artifact in addition to the existing JSON export-plan sidecar.

**Architecture:** Keep adapter-specific binary/NetCDF writers out of scope. Add a deterministic CSV artifact writer in `util/v3_core/adapters.py` that serializes canonical cell metadata each adapter can consume as an intermediate handoff. Extend `V3PipelineResult.write_sidecars()` so each adapter JSON sidecar references its own `adapter_<name>_cells.csv` plus manifest and overlay summary.

**Tech Stack:** Python standard library `csv`/`json`, existing adapter/pipeline modules, pytest. No new dependencies.

---

### Task 1: Adapter cell table writer

**Files:**
- Modify: `util/v3_core/adapters.py`
- Modify: `tests/test_v3_adapters.py`

- [x] Write RED tests for deterministic adapter cell CSV output and path return.
- [x] Implement `write_adapter_cell_table()` with stable columns and JSON-encoded list/dict fields.
- [x] Verify adapter tests pass.

### Task 2: Pipeline sidecar integration

**Files:**
- Modify: `util/v3_core/pipeline.py`
- Modify: `tests/test_v3_pipeline.py`

- [x] Write RED tests proving `write_sidecars()` creates adapter cell CSV files and includes `files.cells` in adapter JSON.
- [x] Implement artifact writing before adapter plan JSON serialization.
- [x] Verify pipeline tests pass.

### Task 3: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run v3 CLI demo or MERIT smoke and confirm adapter cell CSV files exist for requested adapters.
- [x] Run focused v3 tests, all v3 tests, full test suite, and compileall.
- [x] Clean caches and commit with Lore protocol.
