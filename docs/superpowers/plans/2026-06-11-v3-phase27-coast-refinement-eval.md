# EarthMesh v3 Phase 27 Coast Refinement Evaluation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend refinement evaluation reports with optional integrated-coast EarthMesh cell metrics so Phase26 sweep rankings can use both river and coast evidence from one JSON report.

**Architecture:** Keep existing river/background evaluation behavior unchanged. Add a small coast-intersection summarizer that reads the same GeoJSON feature shape produced by integrated coast QA layers and reports feature count, class counts, coastal fraction range, and optional estimated coastal area. Add an optional `--coast-intersections-geojson` CLI argument that injects `coast_intersections` into the output report.

**Tech Stack:** Python standard library, existing GeoJSON evaluation utilities, pytest. No new dependencies.

---

### Task 1: Coast intersection summary helper

**Files:**
- Modify: `tests/test_refinement_eval.py`
- Modify: `util/hydro_mesh/refinement_eval.py`

- [x] Write RED test for `summarize_coast_intersections()` using `mask_class=COAST`, `coastal_fraction`, and `estimated_coastal_area_m2` properties.
- [x] Implement `summarize_coast_intersections()`.
- [x] Verify focused refinement eval tests pass.

### Task 2: Optional coast report field and CLI

**Files:**
- Modify: `tests/test_refinement_eval.py`
- Modify: `util/hydro_mesh/refinement_eval.py`

- [x] Write RED test for `write_refinement_eval_json(..., coast_intersections_geojson=...)` and CLI `--coast-intersections-geojson`.
- [x] Extend `build_refinement_eval()`, `write_refinement_eval_json()`, and CLI argument parsing.
- [x] Verify focused tests pass.

### Task 3: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run focused refinement eval/sweep tests.
- [x] Run v3 tests, full tests, compileall, and diff checks.
- [x] Clean caches and commit with Lore protocol.
