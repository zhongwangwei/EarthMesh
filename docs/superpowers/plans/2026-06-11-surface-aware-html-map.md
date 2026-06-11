# Surface-Aware HTML Map Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the package Leaflet HTML show complete LAND/OCEAN cell masks alongside river/coast layers.

**Architecture:** Extend the existing mesh Leaflet renderer with one optional `surface_cells` collection. Keep river/coast rendering unchanged, then pass the derived package complete mask to the renderer from `refinement_package.py`.

**Tech Stack:** Python 3, embedded GeoJSON, Leaflet HTML, pytest.

---

### Task 1: Renderer surface layer

**Files:**
- Modify: `tests/test_geojson_map.py`
- Modify: `util/hydro_mesh/geojson_map.py`

- [ ] Write failing tests for `render_mesh_leaflet_html(..., surface_cells=...)` and `mesh_geojson_to_leaflet_html(..., surface_geojson=...)`.
- [ ] Run the two tests and confirm they fail on missing parameters.
- [ ] Add the `surface_cells`/`surface_geojson` parameters, embedded `surfaceCells` JSON, LAND/OCEAN styling, legend entries, popup field, and layer control entry.
- [ ] Re-run `tests/test_geojson_map.py`.

### Task 2: Package HTML handoff

**Files:**
- Modify: `tests/test_refinement_package.py`
- Modify: `util/hydro_mesh/refinement_package.py`

- [ ] Write a failing test that a package with `surface_geojson` embeds derived `surface_class=LAND/OCEAN` values in its HTML.
- [ ] Run the test and confirm it fails when HTML receives no complete mask.
- [ ] Generate the complete mask before HTML rendering and pass it as `surface_geojson=complete_cell_mask_path`.
- [ ] Re-run package tests.

### Task 3: Verify and commit

**Files:**
- Modify: `docs/hydro_mesh_data_requirements.md`

- [ ] Document the surface-aware HTML path and smoke evidence.
- [ ] Run targeted tests, compileall, full pytest, and the real N112 smoke.
- [ ] Commit with Lore protocol trailers.
