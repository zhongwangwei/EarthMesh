# EarthMesh v3 Phase 4 Adapter Export Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement each task with RED -> GREEN -> commit.

**Goal:** Move v3 model adapters from shape-only validation to stable export planning contracts for MPAS, FVCOM, CoLM2024, CoLM20XX, and generic ESMF. This phase still avoids model-specific binary/NetCDF writers; it defines the adapter-side manifest that later concrete writers must satisfy.

**Architecture:** Extend `util/v3_core/adapters.py` with an `AdapterExportPlan` dataclass and `plan_export()` method. Default adapters declare output format and required canonical fields. The plan summarizes cell type counts, warnings, required fields, and sidecar files. This keeps the v3 core flat while reserving concrete writer implementations for later phases.

**Tech Stack:** Python 3 standard library, pytest, existing `CanonicalCell` and adapter registry. No new dependencies.

---

## File Structure

Modify:

- `util/v3_core/adapters.py` — adapter export plan dataclass and default adapter metadata.
- `util/v3_core/__init__.py` — export `AdapterExportPlan`.
- `tests/test_v3_adapters.py` — adapter planning and JSON sidecar tests.

---

## Tasks

### Task 1: Add adapter export plan contract

- [ ] Add failing tests proving `plan_export()` returns adapter name/version, cell type counts, required fields, warnings, and output format.
- [ ] Implement `AdapterExportPlan` and `SchemaOnlyAdapter.plan_export()`.
- [ ] Verify `python3 -m pytest tests/test_v3_adapters.py -q`.
- [ ] Commit.

### Task 2: Add JSON sidecar writer

- [ ] Add failing test proving export plans write deterministic JSON.
- [ ] Implement `AdapterExportPlan.to_dict()` and `write_json()`.
- [ ] Verify `python3 -m pytest tests/test_v3_adapters.py -q`.
- [ ] Commit.

### Task 3: Expose and validate Phase 4 API

- [ ] Export `AdapterExportPlan` from `util.v3_core`.
- [ ] Run focused v3 tests, full suite, and compileall.
- [ ] Commit export change if needed.
