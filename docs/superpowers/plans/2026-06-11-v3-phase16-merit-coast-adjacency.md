# EarthMesh v3 Phase 16 MERIT-Hydro Coast Adjacency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans and superpowers:test-driven-development. Implement with RED -> GREEN -> commit.

**Goal:** Extend the MERIT-Hydro bridge so LAND/OCEAN adjacency in the 90m landtype field produces explicit `COAST_LAND` and `COAST_OCEAN` masks. This gives v3 a high-resolution coastal refinement signal instead of relying only on coarse CaMa elevation or generic LAND/OCEAN masks.

**Architecture:** Keep the output as standard GeoJSON masks consumable by `util.v3_core.geojson_io`. Add coast classification inside `build_merit_masks()` using local 8-neighbor adjacency on the sampled window. River classes still override coast/surface classes by priority.

**Tech Stack:** Python standard library, netCDF4 test fixtures, numpy already used by the MERIT bridge. No new dependencies.

---

## Task

- [ ] Add failing tests for LAND/OCEAN adjacency producing `COAST_LAND` and `COAST_OCEAN`.
- [ ] Implement coast classification in `build_merit_masks()` and summary counts.
- [ ] Run a local GBA smoke with MERIT root and stride to verify coast counts.
- [ ] Run focused v3 tests, full suite, compileall.
- [ ] Commit.
