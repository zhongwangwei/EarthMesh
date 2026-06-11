# EarthMesh v3 Phase 6 Pipeline Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Connect the Phase 3 geometry backend, Phase 5 all-cell mask semantics, and Phase 4 adapter export contracts into a small v3 pipeline contract. The pipeline should take canonical cells, mask features, and adapter names, then return updated cells, overlay QA, adapter export plans, and a reproducible run manifest.

**Architecture:** Add `util/v3_core/pipeline.py` with `V3PipelineResult` and `build_v3_pipeline_result()`. This is not yet a CLI and does not read CaMa binaries directly. It is the orchestration seam that future China/GBA examples, CoLM2024/CoLM20XX adapters, and Rust geometry backends will call.

**Tech Stack:** Python standard library, pytest, existing v3 core modules. No new dependencies.

---

## Tasks

### Task 1: Build pipeline result from cells, masks, adapters

- [ ] Add failing tests for overlaying all cells, applying semantics, producing adapter plans, and manifest QA counts.
- [ ] Implement `V3PipelineResult` and `build_v3_pipeline_result()`.
- [ ] Verify `python3 -m pytest tests/test_v3_pipeline.py -q`.
- [ ] Commit.

### Task 2: Expose pipeline API and verify repo

- [ ] Export pipeline API from `util.v3_core`.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
