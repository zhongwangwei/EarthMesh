# Package Complete Surface Mask Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package an EarthMesh-cell keyed LAND/OCEAN/COAST/R2/R3 complete mask and make CoLM coupling consume it first.

**Architecture:** Reuse `util.hydro_mesh.cell_mask_merge.write_complete_cell_mask_geojson()` inside the refinement package writer. Keep raw surface masks as source provenance, but expose the derived complete cell mask as a package file and make `colm_coupling` prefer that adapter-ready file.

**Tech Stack:** Python 3, GeoJSON, existing Shapely-backed cell mask merger, pytest.

---

### Task 1: Package complete cell mask artifact

**Files:**
- Modify: `tests/test_refinement_package.py`
- Modify: `util/hydro_mesh/refinement_package.py`

- [ ] **Step 1: Write failing test** asserting `write_refinement_delivery_package(..., surface_geojson=...)` writes `files.complete_cell_mask_geojson` and includes one output cell per background cell with LAND/OCEAN `surface_class`.
- [ ] **Step 2: Run test to verify RED** with `pytest tests/test_refinement_package.py::test_refinement_delivery_package_writes_complete_cell_mask_when_surface_geojson_is_supplied -q`; expected failure: missing `complete_cell_mask_geojson`.
- [ ] **Step 3: Implement minimal code** by importing `write_complete_cell_mask_geojson`, generating `<case>_complete_cell_mask.geojson` when `surface_geojson` is supplied, and adding the path to manifest `files`.
- [ ] **Step 4: Run package tests** with `pytest tests/test_refinement_package.py -q`.

### Task 2: CoLM package coupling prefers complete mask

**Files:**
- Modify: `tests/test_colm_coupling.py`
- Modify: `util/hydro_mesh/colm_coupling.py`

- [ ] **Step 1: Write failing test** where manifest contains `files.complete_cell_mask_geojson`; assert surface classes are loaded from that file and summary records `surface_source_kind=complete_cell_mask_geojson`.
- [ ] **Step 2: Run RED** with `pytest tests/test_colm_coupling.py::test_package_coupling_prefers_manifest_complete_cell_mask_geojson -q`; expected failure: surface remains UNKNOWN.
- [ ] **Step 3: Implement minimal code** by resolving surface path from `manifest.files.complete_cell_mask_geojson` first, falling back to `source_files.surface_geojson`.
- [ ] **Step 4: Run coupling tests** with `pytest tests/test_colm_coupling.py -q`.

### Task 3: Verify, document, commit

**Files:**
- Modify: `docs/hydro_mesh_data_requirements.md`

- [ ] **Step 1:** Update docs explaining that package output includes a derived complete mask when raw surface is supplied.
- [ ] **Step 2:** Run `pytest tests/test_refinement_package.py tests/test_colm_coupling.py tests/test_cell_mask_merge.py -q` and `python3 -m compileall util/hydro_mesh`.
- [ ] **Step 3:** If local CaMa/N112 inputs exist, regenerate package with `--surface-geojson`, run `colm_coupling package`, and inspect summary counts.
- [ ] **Step 4:** Commit with Lore protocol trailers.
