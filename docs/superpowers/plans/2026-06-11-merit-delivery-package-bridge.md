# MERIT Delivery Package Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a minimal bridge from MERIT-Hydro masks into the existing hydro/coast package and CoLM coupling handoff.

**Architecture:** Add `util/hydro_mesh/merit_package_bridge.py` as orchestration glue. It calls the existing MERIT mask writer, reuses EarthMesh intersection for river/coast cell overlays, then delegates package generation to `refinement_package.py`.

**Tech Stack:** Python 3, netCDF4 fixtures, existing MERIT/v3/hydro_mesh modules, pytest.

---

### Task 1: Bridge package writer

**Files:**
- Create: `util/hydro_mesh/merit_package_bridge.py`
- Create: `tests/test_merit_package_bridge.py`

- [x] Write RED fixture test for `write_merit_refinement_delivery_package()`.
- [x] Implement MERIT mask generation, river/coast EarthMesh intersections, package delegation, bridge summary, and CLI.
- [x] Verify package manifest, complete cell mask, HTML, and CoLM coupling consume the derived files.

### Task 2: Coast fraction compatibility

**Files:**
- Modify: `util/hydro_mesh/earthmesh_intersection.py`

- [x] Ensure `COAST_LAND` and `COAST_OCEAN` intersections carry `coastal_fraction`, not only generic `overlap_fraction`.
- [x] Verify bridge tests assert all coast features include `coastal_fraction`.

### Task 3: Local smoke and docs

**Files:**
- Modify: `docs/hydro_mesh_data_requirements.md`

- [x] Run focused tests and compileall.
- [x] Run local `/Volumes/Data01/MERIT_Hydro` GBA 0.2 degree smoke.
- [x] Document command, outputs, counts, and full-domain caution.
