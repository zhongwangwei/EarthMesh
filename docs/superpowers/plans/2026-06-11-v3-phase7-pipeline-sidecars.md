# EarthMesh v3 Phase 7 Pipeline Sidecar Output Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Let `V3PipelineResult` write a minimal reproducible sidecar directory containing `manifest.json` and one adapter export plan JSON per requested adapter. This bridges the in-memory v3 pipeline to future HTML demos, China/GBA examples, and model-specific writers without adding a CLI yet.

**Architecture:** Extend `util/v3_core/pipeline.py` with `V3PipelineResult.write_sidecars(output_dir)`. It delegates to existing `V3RunManifest.write_json()` and `AdapterExportPlan.write_json()`.

**Tech Stack:** Python standard library, pytest. No new dependencies.

---

## Task

- [ ] Add failing test for writing manifest and adapter JSON sidecars.
- [ ] Implement `write_sidecars()` returning a dict of written paths.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
