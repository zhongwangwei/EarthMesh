# EarthMesh v3 Phase 10 Canonical GeoJSON Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Write semantically projected `CanonicalCell` results back to GeoJSON so v3 outputs can be embedded in Leaflet/HTML maps and inspected alongside existing hydro/coast QA layers.

**Architecture:** Extend `util/v3_core/geojson_io.py` with `canonical_cells_to_geojson()` and `write_cells_geojson()`. Extend the minimal CLI to emit `canonical_cells.geojson` next to `canonical_cells.json` and sidecars.

**Tech Stack:** Python standard library, pytest. No new dependencies.

---

## Task

- [ ] Add failing tests for CanonicalCell to GeoJSON conversion and CLI output file.
- [ ] Implement GeoJSON writer helpers and CLI write.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
