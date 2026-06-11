# EarthMesh v3 Phase 24 PyO3 Runtime Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and import the Rust geometry crate as a Python runtime extension, then expose a `RustGeometryBackend` that can replace the Python reference backend for v3 overlay geometry calculations.

**Architecture:** Extend `rust/earthmesh_geometry` with PyO3 bindings for `polygon_area()` and `intersection_area()`, configured for maturin. Add `RustGeometryBackend` in `util/v3_core/geometry_backend.py`; it imports the compiled `earthmesh_geometry` module and uses Rust area/intersection functions while preserving Python overlay semantics, priorities, QA flags, and `OverlayResult` shape. Keep `get_geometry_backend()` defaulting to Python and add explicit names `rust`/`rust_pyo3`.

**Tech Stack:** Rust stable, PyO3, maturin, Python standard library, pytest. No runtime dependency is required unless the Rust backend is explicitly requested.

---

### Task 1: PyO3 extension module

**Files:**
- Modify: `rust/earthmesh_geometry/Cargo.toml`
- Modify: `rust/earthmesh_geometry/src/lib.rs`
- Create: `rust/earthmesh_geometry/pyproject.toml`
- Create: `tests/test_v3_rust_runtime.py`

- [x] Write RED test expecting imported `earthmesh_geometry.polygon_area()` and `intersection_area()` after maturin develop.
- [x] Configure PyO3 crate type and module functions.
- [x] Verify `python3 -m maturin develop --manifest-path rust/earthmesh_geometry/Cargo.toml` then Python import test passes.

### Task 2: RustGeometryBackend

**Files:**
- Modify: `util/v3_core/geometry_backend.py`
- Modify: `util/v3_core/__init__.py`
- Modify: `tests/test_v3_geometry_backend.py`

- [x] Write RED tests for `get_geometry_backend("rust")` and parity with `PythonGeometryBackend`.
- [x] Implement `RustGeometryBackend` using imported Rust functions and Python-side overlay semantics.
- [x] Verify backend tests pass after maturin develop.

### Task 3: Validation and commit

**Files:**
- Modify: this plan, mark completed after verification.

- [x] Run maturin develop.
- [x] Run cargo tests.
- [x] Run Rust runtime/backend Python tests, all v3 tests, full Python suite, and compileall.
- [x] Clean build caches and commit with Lore protocol.
