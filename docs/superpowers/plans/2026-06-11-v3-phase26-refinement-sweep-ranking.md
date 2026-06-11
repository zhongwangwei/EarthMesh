# EarthMesh v3 Phase 26 Refinement Sweep Ranking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the documented river/coast recipe sweep reproducible by generating composite close-mask recipes and ranking existing evaluation JSON reports into a recommended candidate.

**Architecture:** Add a lightweight `util/hydro_mesh/refinement_sweep.py` module. It does not run `mkgrd.x`; it writes recipe JSON files for `R2` and `COAST` cap grids and ranks already-produced evaluation summaries using retained high-level refinement, river/coast overlap counts, and median cell size. This converts the current manual `R2={40,60,80}` by `COAST={10,20,40}` comparison into a deterministic QA control surface.

**Tech Stack:** Python standard library, existing composite close-mask recipe schema, pytest. No new dependencies.

---

### Task 1: Generate river/coast sweep recipes

**Files:**
- Create: `tests/test_refinement_sweep.py`
- Create: `util/hydro_mesh/refinement_sweep.py`

- [x] Write RED tests proving `build_river_coast_sweep()` creates all R2/COAST combinations with stable case names and composite recipe dictionaries.
- [x] Implement `build_river_coast_sweep()` and `write_sweep_recipes()`.
- [x] Verify `python3 -m pytest tests/test_refinement_sweep.py -q` passes for recipe generation.

### Task 2: Rank sweep evaluation reports

**Files:**
- Modify: `tests/test_refinement_sweep.py`
- Modify: `util/hydro_mesh/refinement_sweep.py`

- [x] Write RED tests proving `rank_sweep_reports()` prefers passing candidates with higher retained L3/L2 refinement, then more river/coast cells, then smaller median cell size, while demoting failed candidates and candidates above an optional background-cell cap.
- [x] Implement `rank_sweep_reports()` and `write_sweep_ranking()`.
- [x] Verify focused tests pass.

### Task 3: Add CLI control surface

**Files:**
- Modify: `tests/test_refinement_sweep.py`
- Modify: `util/hydro_mesh/refinement_sweep.py`

- [x] Write RED CLI tests for `write-recipes` and `rank` subcommands.
- [x] Implement argparse subcommands.
- [x] Verify focused tests pass.

### Task 4: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run focused refinement sweep tests.
- [x] Run hydro/v3 focused tests, all tests, and compileall.
- [x] Run `git diff --check` and clean caches.
- [x] Commit with Lore protocol.
