# EarthMesh v3 Phase 23 Rust Geometry MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the first small Rust compute crate for v3 geometry parity: polygon area and convex polygon clipping/intersection area.

**Architecture:** Create `rust/earthmesh_geometry` as a dependency-free Rust library. It mirrors the Python reference geometry semantics for point arrays, polygon area, convex clipping, and intersection area on small fixtures. This phase intentionally avoids PyO3/maturin and does not replace Python execution; it establishes a tested Rust compute kernel that future Python bindings can call.

**Tech Stack:** Rust stable, Cargo, no external crates, existing Python tests remain unchanged.

---

### Task 1: Rust geometry crate

**Files:**
- Create: `rust/earthmesh_geometry/Cargo.toml`
- Create: `rust/earthmesh_geometry/src/lib.rs`
- Create: `rust/earthmesh_geometry/tests/geometry.rs`

- [x] Write RED integration tests for triangle/rectangle polygon area and rectangle intersection area.
- [x] Verify RED fails before crate implementation is present.
- [x] Implement `Point`, `polygon_area`, `clip_convex_polygon`, and `intersection_area`.
- [x] Verify `cargo test --manifest-path rust/earthmesh_geometry/Cargo.toml` passes.

### Task 2: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run Rust tests.
- [x] Run v3 Python tests and full Python suite.
- [x] Run compileall for Python v3/hydro modules.
- [x] Clean caches and commit with Lore protocol.
