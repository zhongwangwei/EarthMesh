# EarthMesh v3 Phase 12 Built-in GBA Demo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Provide a built-in Greater Bay Area smoke demo for v3 so users can run the complete canonical pipeline without preparing external GeoJSON inputs. The demo should exercise LAND, OCEAN, COAST, and R3 river semantics, adapter sidecars, canonical GeoJSON, and optional HTML map output.

**Architecture:** Add `util/v3_core/demo.py` with `build_demo_inputs(name)`. Extend the v3 CLI with `--demo gba` as an alternative to `--cells/--masks` or `--cells-geojson/--masks-geojson`. Keep demo geometry tiny and synthetic; it is a smoke-test product, not a scientific regional mesh.

**Tech Stack:** Python standard library, pytest, existing v3 pipeline. No new dependencies.

---

## Task 1: Built-in GBA demo inputs

- [ ] Add failing tests for demo cells/masks containing LAND, OCEAN, COAST, and R3 coverage.
- [ ] Implement `build_demo_inputs("gba")`.
- [ ] Verify focused tests and commit.

## Task 2: CLI `--demo gba`

- [ ] Add failing CLI test that runs the demo and writes manifest, canonical GeoJSON, and HTML map.
- [ ] Extend CLI argument handling to accept demo mode as an alternative input source.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
