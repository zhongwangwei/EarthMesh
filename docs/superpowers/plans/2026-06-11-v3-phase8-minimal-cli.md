# EarthMesh v3 Phase 8 Minimal Pipeline CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Provide a minimal file-based v3 pipeline CLI that can run the object-level pipeline from JSON inputs and write reproducible outputs. This is a bridge for regional examples before real CaMa/GeoJSON/NetCDF readers and model writers are implemented.

**Architecture:** Add `util/v3_core/cli.py` with `main(argv=None)`. It reads canonical cell JSON and mask feature JSON, runs `build_v3_pipeline_result()`, writes pipeline sidecars, and writes `canonical_cells.json` containing semantically projected cells.

**Tech Stack:** Python standard library (`argparse`, `json`, `dataclasses`, `pathlib`), pytest. No new dependencies.

---

## Task

- [ ] Add failing CLI smoke test with small TRI/HEX/POLYGON inputs.
- [ ] Implement `util/v3_core/cli.py`.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
