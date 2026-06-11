# EarthMesh v3 Phase 15 MERIT-Hydro Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Add a first MERIT-Hydro 90m bridge that can select NetCDF tiles by bbox, read only the requested window, classify river masks from `wth`/`upa`, classify LAND/OCEAN masks from `landtype_igbp`, and write v3-compatible mask GeoJSON plus a summary JSON. This replaces CaMa as the high-resolution source for fine river/coast refinement while leaving CaMa support available.

**Architecture:** Add `util/v3_components/hydro_merit.py` as a component boundary. Keep it independent of v3 core geometry/pipeline except for emitting standard GeoJSON properties that `util.v3_core.geojson_io` can consume as `MaskFeature`s. The first implementation uses bbox-window reads and optional stride to avoid loading global data.

**Tech Stack:** Python standard library, netCDF4 if available in runtime, pytest temporary NetCDF fixtures. No new dependencies added.

---

## Task 1: Tile selection and window reads

- [ ] Add failing tests for MERIT tile-name bbox parsing and selecting intersecting tiles.
- [ ] Add failing test using a tiny temporary NetCDF file to read a bbox window.
- [ ] Implement tile helpers and `read_merit_window()`.
- [ ] Verify focused tests and commit.

## Task 2: GeoJSON mask export

- [ ] Add failing tests for river R2/R3 and LAND/OCEAN mask GeoJSON generation.
- [ ] Implement `build_merit_masks()` and `write_merit_mask_outputs()`.
- [ ] Add CLI entry point for bbox/root/output-dir.
- [ ] Verify focused tests and commit.

## Task 3: Local smoke on `/Volumes/Data01/MERIT_Hydro`

- [ ] Run a small GBA bbox with stride to ensure the local dataset can be read.
- [ ] Verify outputs exist and summarize counts.
- [ ] Run v3/full tests and compileall.
- [ ] Commit any final export/API changes.
