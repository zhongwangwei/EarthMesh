# EarthMesh v3 Phase 17 MERIT-Hydro Layered Mask Outputs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Split MERIT-Hydro mask outputs into reusable semantic layers: combined masks, river masks (`R2/R3`), coast masks (`COAST_LAND/COAST_OCEAN`), and surface masks (`LAND/OCEAN`). This makes the MERIT bridge directly useful for separate river refinement, coastal refinement, and land/ocean QA.

**Architecture:** Keep the existing combined `merit_masks.geojson` for v3 pipeline compatibility. Add deterministic filtered GeoJSON files in `write_merit_mask_outputs()` and expose a pure helper `split_merit_mask_layers()` for testing.

**Tech Stack:** Python standard library, pytest, existing MERIT bridge. No new dependencies.

---

## Task

- [ ] Add failing tests for `split_merit_mask_layers()` and file outputs.
- [ ] Implement semantic layer filtering and deterministic output files.
- [ ] Run a local GBA MERIT smoke to verify layer counts.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
