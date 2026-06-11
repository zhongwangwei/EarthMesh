# EarthMesh v3 Phase 13 Overlay QA Sidecar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Persist overlay QA as a first-class sidecar (`overlay_summary.json`) whenever a v3 pipeline result is written. This makes all-cell mask coverage, class counts, missing-mask counts, and quality flags auditable before CoLM/MPAS/FVCOM adapters consume the generated cells.

**Architecture:** Extend `V3PipelineResult.write_sidecars()` in `util/v3_core/pipeline.py` to write the existing `overlay_summary` dict as deterministic JSON. CLI inherits the behavior automatically.

**Tech Stack:** Python standard library, pytest. No new dependencies.

---

## Task

- [ ] Add failing tests for `overlay_summary.json` in pipeline and CLI outputs.
- [ ] Implement deterministic overlay summary writer.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
