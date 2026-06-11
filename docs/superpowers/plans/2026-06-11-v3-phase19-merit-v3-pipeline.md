# EarthMesh v3 Phase 19 MERIT-Hydro Regional Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Provide a one-command regional smoke pipeline that turns MERIT-Hydro 90m tiles into v3 masks, generates bootstrap bbox cells, runs the canonical v3 overlay/adapters pipeline, and optionally writes an HTML QA map. This removes the current manual three-command workflow and makes GBA/China-region experiments reproducible.

**Architecture:** Add `util/v3_components/hydro_merit_pipeline.py` as a thin orchestrator over existing stable modules:

1. `write_bbox_grid_geojson()` for bootstrap cells.
2. `write_merit_mask_outputs()` for MERIT-derived surface/coast/river masks.
3. `build_v3_pipeline_result()` + GeoJSON/map writers for canonical v3 outputs.

No adaptive refinement or new mask science is added in this phase; this is an operational wrapper with provenance sidecars.

**Tech Stack:** Python standard library plus existing project modules and test NetCDF fixture. No new dependencies.

---

## Task

- [x] Add failing tests for programmatic one-command pipeline outputs.
- [x] Implement `run_merit_v3_pipeline()` and CLI in `util/v3_components/hydro_merit_pipeline.py`.
- [x] Include a small `pipeline_summary.json` with generated file paths and parameters.
- [x] Run a real local GBA smoke against `/Volumes/Data01/MERIT_Hydro` with coarse stride.
- [x] Run focused v3 tests, full suite, compileall.
- [x] Commit.
