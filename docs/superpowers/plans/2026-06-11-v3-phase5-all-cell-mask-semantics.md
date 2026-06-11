# EarthMesh v3 Phase 5 All-Cell Mask Semantics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement each task with RED -> GREEN -> commit.

**Goal:** Enforce the v3 invariant that every mesh cell receives an explicit mask semantic. Cells with no source-mask overlap must not remain blank; they become `UNKNOWN` with a `missing_mask` QA flag. Overlay results can then be projected back into `CanonicalCell` surface/hydro/coast fields for CoLM2024, CoLM20XX, MPAS, FVCOM, and generic coupling adapters.

**Architecture:** Extend `util/v3_core/geometry.py`. Keep overlay computation shape-agnostic. Add a small pure function that turns an `OverlayResult` into an updated `CanonicalCell` without mutating the original cell.

**Tech Stack:** Python standard library, pytest, existing `CanonicalCell`/`OverlayResult` contracts. No new dependencies.

---

## Tasks

### Task 1: Make missing mask explicit

- [ ] Add failing test for cells with no mask overlap returning `UNKNOWN` and `missing_mask`.
- [ ] Implement explicit missing-mask fallback in `overlay_cell_with_masks()`.
- [ ] Update QA summary expectations.
- [ ] Verify `python3 -m pytest tests/test_v3_geometry.py -q`.
- [ ] Commit.

### Task 2: Project overlay semantics into CanonicalCell

- [ ] Add failing tests for LAND/OCEAN surface class, R2/R3 hydro class, and coast class projection.
- [ ] Implement `apply_overlay_to_cell()` using `dataclasses.replace()`.
- [ ] Export the helper from `util.v3_core`.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
