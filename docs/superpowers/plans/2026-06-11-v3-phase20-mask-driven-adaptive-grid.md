# EarthMesh v3 Phase 20 Mask-Driven Adaptive Grid Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a deterministic first-pass adaptive grid that locally subdivides bbox cells intersecting selected MERIT/v3 mask classes such as rivers and coastlines.

**Architecture:** Add `util/v3_core/adaptive_grid.py` as a pure refinement helper over existing `CanonicalCell` and `MaskFeature` contracts. The MERIT regional pipeline keeps the existing regular bbox grid path by default, and enables refinement only when `--refine-classes` is provided. This phase does not attempt conformal mesh topology, hanging-node repair, or MPAS/FVCOM final mesh generation; it creates v3-compatible polygon subcells for overlay QA and adapter-contract smoke tests.

**Tech Stack:** Python standard library, existing v3 geometry/grid/GeoJSON modules, pytest. No new dependencies.

---

### Task 1: Adaptive grid helper

**Files:**
- Create: `util/v3_core/adaptive_grid.py`
- Test: `tests/test_v3_adaptive_grid.py`

- [x] Write RED tests that split only cells intersecting selected mask classes.
- [x] Verify RED fails because `util.v3_core.adaptive_grid` does not exist.
- [x] Implement `refine_cells_by_masks()` and `write_refined_cells_geojson()`.
- [x] Verify focused adaptive-grid tests pass.

### Task 2: Pipeline integration

**Files:**
- Modify: `util/v3_components/hydro_merit_pipeline.py`
- Modify: `util/v3_core/__init__.py`
- Modify: `tests/test_v3_hydro_merit_pipeline.py`

- [x] Write RED tests for `refine_classes` in `run_merit_v3_pipeline()` and CLI `--refine-classes/--refine-factor`.
- [x] Implement optional refinement after MERIT masks are written and before v3 overlay.
- [x] Add lazy core exports for adaptive-grid helpers.
- [x] Verify focused pipeline tests pass.

### Task 3: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run real GBA MERIT smoke with `--refine-classes R2,COAST_LAND,COAST_OCEAN --refine-factor 2`.
- [x] Run focused v3 tests, all v3 tests, full test suite, and compileall.
- [x] Clean caches and commit with Lore protocol.
