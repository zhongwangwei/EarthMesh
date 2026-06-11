# EarthMesh v3 Phase 11 Canonical Cell Map Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Render v3 `canonical_cells.geojson` as a Leaflet HTML QA map with integrated land/ocean/coast/hydro/missing-mask styling. This gives immediate visual inspection of v3 outputs without waiting for the future regional HTML product.

**Architecture:** Add `util/v3_core/map.py` for v3-specific canonical cell map rendering. Keep the existing `util/hydro_mesh/geojson_map.py` unchanged for v2/sparse hydro layers. Extend the v3 CLI with optional `--html-map` output path.

**Tech Stack:** Python standard library (`json`, `html`, `pathlib`, `argparse`), pytest. No new dependencies.

---

## Task 1: v3 canonical Leaflet renderer

- [ ] Add failing tests proving the HTML embeds canonical cells and legends for LAND, OCEAN, COAST, R2/R3, UNKNOWN/missing masks.
- [ ] Implement `render_canonical_cells_leaflet_html()` and `canonical_cells_geojson_to_leaflet_html()`.
- [ ] Verify focused tests and commit.

## Task 2: CLI optional map output

- [ ] Add failing CLI test for `--html-map` writing a v3 map.
- [ ] Extend CLI to call the v3 map renderer after writing `canonical_cells.geojson`.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
