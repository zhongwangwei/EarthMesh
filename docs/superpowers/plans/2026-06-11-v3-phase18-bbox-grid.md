# EarthMesh v3 Phase 18 BBox Grid Cell Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Provide a lightweight bbox regular-grid generator that writes v3-compatible `cells.geojson`. This lets MERIT-Hydro masks run through the v3 pipeline without requiring an external EarthMesh cell file, enabling real regional smoke tests such as GBA MERIT masks + generated bbox cells + v3 map.

**Architecture:** Add `util/v3_core/grid.py` with pure helpers to generate `CanonicalCell` rectangles from `(min_lon, min_lat, max_lon, max_lat, nx, ny)` and a small CLI to write GeoJSON. This is not the final adaptive/refined mesh generator; it is a deterministic bootstrap grid for smoke tests and development.

**Tech Stack:** Python standard library, pytest, existing `CanonicalCell` and GeoJSON writer. No new dependencies.

---

## Task

- [ ] Add failing tests for bbox grid cell count, vertices, IDs, and GeoJSON output.
- [ ] Implement `generate_bbox_grid_cells()` and `write_bbox_grid_geojson()`.
- [ ] Add `python3 -m util.v3_core.grid` CLI.
- [ ] Run local MERIT + grid + v3 pipeline smoke.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
