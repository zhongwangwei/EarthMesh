# EarthMesh v3 Phase 21 Class-Factor Refinement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow different MERIT/v3 mask classes to request different local grid refinement factors, e.g. `R3=4`, `R2=2`, and coast classes `=2`.

**Architecture:** Extend the Phase20 adaptive-grid helper with a class-to-factor API. For each source cell, compute overlay fractions once and refine by the maximum configured factor among all intersecting configured mask classes. Keep the existing uniform `refine_classes + refine_factor` API as a wrapper for backward compatibility. Extend the MERIT pipeline with `refine_class_factors` and CLI `--refine-class-factors` without removing existing options.

**Tech Stack:** Python standard library, existing v3 adaptive-grid/geometry/pipeline modules, pytest. No new dependencies.

---

### Task 1: Core class-factor refinement

**Files:**
- Modify: `util/v3_core/adaptive_grid.py`
- Modify: `tests/test_v3_adaptive_grid.py`

- [x] Write RED tests for `refine_cells_by_mask_factors()` choosing the maximum factor among intersecting configured classes.
- [x] Implement `refine_cells_by_mask_factors()` and route `refine_cells_by_masks()` through it.
- [x] Verify adaptive-grid tests pass.

### Task 2: MERIT pipeline class-factor integration

**Files:**
- Modify: `util/v3_components/hydro_merit_pipeline.py`
- Modify: `tests/test_v3_hydro_merit_pipeline.py`
- Modify: `util/v3_core/__init__.py`

- [x] Write RED tests for programmatic `refine_class_factors` and CLI `--refine-class-factors`.
- [x] Implement parsing for `CLASS=FACTOR` comma-separated CLI values.
- [x] Make `refine_class_factors` override the uniform shortcut when provided.
- [x] Include `class_factors` in `pipeline_summary.json`.
- [x] Verify focused pipeline tests pass.

### Task 3: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run real GBA MERIT smoke with `--refine-class-factors R3=4,R2=3,COAST_LAND=2,COAST_OCEAN=2`.
- [x] Run focused v3 tests, all v3 tests, full test suite, and compileall.
- [x] Clean caches and commit with Lore protocol.
