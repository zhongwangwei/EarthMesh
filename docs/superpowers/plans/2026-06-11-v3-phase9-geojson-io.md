# EarthMesh v3 Phase 9 GeoJSON I/O Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Let v3 consume the GeoJSON FeatureCollection outputs already used by the hydro/coast prototype. Cell polygons become `CanonicalCell` objects and mask polygons become `MaskFeature` objects. The minimal v3 CLI should accept either internal JSON or GeoJSON input paths.

**Architecture:** Add `util/v3_core/geojson_io.py` with pure conversion helpers. Keep geometry semantics in `geometry.py` and pipeline orchestration in `pipeline.py`; this module is only file-format boundary glue.

**Tech Stack:** Python standard library, pytest. No new dependencies.

---

## Tasks

### Task 1: GeoJSON to v3 objects

- [ ] Add failing tests for loading Polygon cells and Polygon masks from FeatureCollections.
- [ ] Implement `geojson_cells_to_canonical()` and `geojson_masks_to_features()` plus file loaders.
- [ ] Verify focused tests.
- [ ] Commit.

### Task 2: CLI GeoJSON input options

- [ ] Add failing CLI smoke test using `--cells-geojson` and `--masks-geojson`.
- [ ] Extend CLI input selection while preserving existing JSON options.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
