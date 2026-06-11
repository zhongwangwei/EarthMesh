# Surface-Aware CoLM Coupling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional surface-layer support so delivery packages and CoLM coupling tables can carry LAND/OCEAN surface classes while preserving current N112 behavior when no surface layer exists.

**Architecture:** Extend the existing package manifest writer and CoLM package coupling writer. Keep the field file-based and optional: `surface_geojson` appears only when supplied. Normalize surface classes in `colm_coupling.py` to a small coupling-safe vocabulary.

**Tech Stack:** Python standard library, pytest, existing hydro-mesh GeoJSON package tools.

---

## File Structure

- Modify: `util/hydro_mesh/refinement_package.py` — optional `surface_geojson` API and CLI flag.
- Modify: `tests/test_refinement_package.py` — manifest and CLI coverage for surface path.
- Modify: `util/hydro_mesh/colm_coupling.py` — optional surface join and summary count.
- Modify: `tests/test_colm_coupling.py` — surface join tests.
- Modify: `docs/hydro_mesh_data_requirements.md` — document that surface layer is optional and expected to be EarthMesh-cell keyed.

### Task 1: Package manifest surface path

- [ ] Write failing test: `write_refinement_delivery_package(..., surface_geojson=surface)` records `source_files.surface_geojson`.
- [ ] Run focused test and confirm failure.
- [ ] Implement optional argument and CLI `--surface-geojson`.
- [ ] Verify package tests pass.

### Task 2: CoLM coupling surface join

- [ ] Write failing test: package manifest with `surface_geojson` outputs LAND/OCEAN surface classes and keeps coast flags separate.
- [ ] Run focused test and confirm failure.
- [ ] Implement optional surface collection load, `_surface_class_from_properties()`, and summary `surface_cell_count`.
- [ ] Verify CoLM tests pass.

### Task 3: Real compatibility smoke and commit

- [ ] Run current N112 package/coupling commands without surface layer; expected counts remain `2574` rows, `372` river cells, `374` coast cells.
- [ ] Run focused tests and compileall.
- [ ] Commit with Lore trailers.

## Self-Review

- Spec coverage: optional package field, CLI flag, coupling join, summary count, compatibility smoke are covered.
- Deferred-work scan: no deferred markers are present.
- Compatibility: existing package and CoLM commands continue to work without surface input.
