# EarthMesh v3 Phase 3 Geometry MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a shape-agnostic geometry MVP contract for v3 mask-cell overlap, class priority merge, source fractions, and QA counters, with a Python reference backend first and a Rust-compatible backend interface reserved for later acceleration.

**Architecture:** Add `util/v3_core/geometry.py` for pure-Python reference geometry contracts and `util/v3_core/geometry_backend.py` for backend selection. Phase 3 does not require a Rust toolchain or new dependencies. Instead, it fixes the exact input/output semantics that a future Rust/PyO3 implementation must match. Tests cover TRI, HEX, and POLYGON cells, multiple masks, class priority, and backend parity.

**Tech Stack:** Python 3 standard library (`dataclasses`, `typing`, `math`), `pytest`, existing `util.v3_core.schema.CanonicalCell`; no new runtime dependencies in this phase.

---

## File Structure

Create or modify:

- Create: `util/v3_core/geometry.py` — polygon area, bbox filtering, convex polygon clipping, overlap fractions, class priority merge, QA counters.
- Create: `util/v3_core/geometry_backend.py` — backend protocol and Python reference backend registry.
- Modify: `util/v3_core/__init__.py` — export selected geometry contract classes/functions.
- Create: `tests/test_v3_geometry.py` — reference geometry behavior tests.
- Create: `tests/test_v3_geometry_backend.py` — backend selection and parity contract tests.

No Rust crate is created in this phase. The Rust MVP will be a later implementation of the same backend protocol once the contract is stable.

---

### Task 1: Add Shape-Agnostic Geometry Dataclasses

**Files:**
- Create: `util/v3_core/geometry.py`
- Create: `tests/test_v3_geometry.py`

- [ ] **Step 1: Write failing tests for mask layers and overlap results**

Create `tests/test_v3_geometry.py`:

```python
from util.v3_core.geometry import MaskFeature, OverlayResult


def test_mask_feature_records_class_priority_and_polygon():
    feature = MaskFeature(
        feature_id="coast-1",
        mask_class="COAST",
        priority=10,
        polygon=[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)],
        properties={"source": "cama_elevtn"},
    )

    assert feature.feature_id == "coast-1"
    assert feature.mask_class == "COAST"
    assert feature.priority == 10
    assert feature.properties["source"] == "cama_elevtn"


def test_overlay_result_reports_winning_class_and_fractions():
    result = OverlayResult(
        cell_id="cell-1",
        winning_class="R3",
        winning_priority=30,
        class_fractions={"COAST": 0.25, "R3": 0.50},
        source_feature_ids=["coast-1", "river-1"],
        quality_flags=[],
    )

    assert result.covered_fraction == 0.75
    assert result.winning_class == "R3"
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: FAIL with `ModuleNotFoundError: No module named 'util.v3_core.geometry'`.

- [ ] **Step 3: Implement geometry dataclasses**

Create `util/v3_core/geometry.py`:

```python
from __future__ import annotations

from dataclasses import dataclass, field

Point = tuple[float, float]
Polygon = list[Point]


@dataclass(frozen=True)
class MaskFeature:
    feature_id: str
    mask_class: str
    priority: int
    polygon: Polygon
    properties: dict[str, object] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if not self.feature_id:
            raise ValueError("feature_id must be non-empty")
        if not self.mask_class:
            raise ValueError("mask_class must be non-empty")
        if self.priority < 0:
            raise ValueError("priority must be non-negative")
        if len(self.polygon) < 3:
            raise ValueError("polygon must contain at least three points")


@dataclass(frozen=True)
class OverlayResult:
    cell_id: str
    winning_class: str
    winning_priority: int
    class_fractions: dict[str, float] = field(default_factory=dict)
    source_feature_ids: list[str] = field(default_factory=list)
    quality_flags: list[str] = field(default_factory=list)

    @property
    def covered_fraction(self) -> float:
        return sum(self.class_fractions.values())
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add util/v3_core/geometry.py tests/test_v3_geometry.py
git commit -m "Add v3 geometry overlay dataclasses

Constraint: Rust acceleration must match a stable Python geometry contract.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_geometry.py -q"
```

---

### Task 2: Add Polygon Area and Convex Clipping Reference Helpers

**Files:**
- Modify: `util/v3_core/geometry.py`
- Modify: `tests/test_v3_geometry.py`

- [ ] **Step 1: Add failing geometry helper tests**

Append to `tests/test_v3_geometry.py`:

```python
from util.v3_core.geometry import polygon_area, polygon_clip_convex


def test_polygon_area_handles_triangle_and_hexagon_like_polygon():
    triangle = [(0.0, 0.0), (2.0, 0.0), (0.0, 2.0)]
    rectangle = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)]

    assert polygon_area(triangle) == 2.0
    assert polygon_area(rectangle) == 2.0


def test_polygon_clip_convex_returns_intersection_polygon():
    subject = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]
    clip = [(1.0, 0.0), (3.0, 0.0), (3.0, 2.0), (1.0, 2.0)]

    intersection = polygon_clip_convex(subject, clip)

    assert round(polygon_area(intersection), 6) == 2.0
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: FAIL because `polygon_area` and `polygon_clip_convex` do not exist.

- [ ] **Step 3: Implement area and convex clipping**

Append to `util/v3_core/geometry.py`:

```python

def polygon_area(polygon: Polygon) -> float:
    if len(polygon) < 3:
        return 0.0
    total = 0.0
    for index, (x0, y0) in enumerate(polygon):
        x1, y1 = polygon[(index + 1) % len(polygon)]
        total += x0 * y1 - x1 * y0
    return abs(total) * 0.5


def _signed_area(polygon: Polygon) -> float:
    total = 0.0
    for index, (x0, y0) in enumerate(polygon):
        x1, y1 = polygon[(index + 1) % len(polygon)]
        total += x0 * y1 - x1 * y0
    return total * 0.5


def _inside(point: Point, edge_start: Point, edge_end: Point, *, clip_ccw: bool) -> bool:
    x, y = point
    x0, y0 = edge_start
    x1, y1 = edge_end
    cross = (x1 - x0) * (y - y0) - (y1 - y0) * (x - x0)
    return cross >= -1.0e-12 if clip_ccw else cross <= 1.0e-12


def _line_intersection(a0: Point, a1: Point, b0: Point, b1: Point) -> Point:
    x1, y1 = a0
    x2, y2 = a1
    x3, y3 = b0
    x4, y4 = b1
    denominator = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
    if abs(denominator) < 1.0e-12:
        return a1
    px = ((x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4)) / denominator
    py = ((x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4)) / denominator
    return (px, py)


def polygon_clip_convex(subject: Polygon, clip: Polygon) -> Polygon:
    output = list(subject)
    clip_ccw = _signed_area(clip) >= 0.0
    for index, edge_start in enumerate(clip):
        edge_end = clip[(index + 1) % len(clip)]
        input_polygon = output
        output = []
        if not input_polygon:
            break
        previous = input_polygon[-1]
        for current in input_polygon:
            current_inside = _inside(current, edge_start, edge_end, clip_ccw=clip_ccw)
            previous_inside = _inside(previous, edge_start, edge_end, clip_ccw=clip_ccw)
            if current_inside:
                if not previous_inside:
                    output.append(_line_intersection(previous, current, edge_start, edge_end))
                output.append(current)
            elif previous_inside:
                output.append(_line_intersection(previous, current, edge_start, edge_end))
            previous = current
    return output
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add util/v3_core/geometry.py tests/test_v3_geometry.py
git commit -m "Add v3 reference polygon clipping helpers

Constraint: The Python backend provides correctness fixtures for future Rust parity.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_geometry.py -q"
```

---

### Task 3: Add Cell-Mask Overlay and Priority Merge

**Files:**
- Modify: `util/v3_core/geometry.py`
- Modify: `tests/test_v3_geometry.py`

- [ ] **Step 1: Add failing overlay tests for TRI and HEX cells**

Append to `tests/test_v3_geometry.py`:

```python
from util.v3_core.geometry import overlay_cell_with_masks
from util.v3_core.schema import CanonicalCell


def test_overlay_cell_with_masks_handles_triangle_cell():
    cell = CanonicalCell.minimal("tri-cell", cell_type="TRI")
    mask = MaskFeature(
        feature_id="land-mask",
        mask_class="LAND",
        priority=1,
        polygon=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
    )

    result = overlay_cell_with_masks(cell, [mask])

    assert result.cell_id == "tri-cell"
    assert result.winning_class == "LAND"
    assert result.class_fractions == {"LAND": 1.0}


def test_overlay_cell_with_masks_prefers_higher_priority_class():
    cell = CanonicalCell(
        cell_id="hex-cell",
        cell_index=1,
        cell_type="HEX",
        center_lon=1.0,
        center_lat=1.0,
        area_m2=4.0,
        vertices=[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
    )
    coast = MaskFeature("coast", "COAST", 10, [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)])
    river = MaskFeature("river", "R3", 30, [(1.0, 0.0), (2.0, 0.0), (2.0, 2.0), (1.0, 2.0)])

    result = overlay_cell_with_masks(cell, [coast, river])

    assert result.winning_class == "R3"
    assert result.winning_priority == 30
    assert round(result.class_fractions["COAST"], 6) == 1.0
    assert round(result.class_fractions["R3"], 6) == 0.5
    assert result.source_feature_ids == ["coast", "river"]
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: FAIL because `overlay_cell_with_masks` does not exist.

- [ ] **Step 3: Implement overlay function**

Append to `util/v3_core/geometry.py`:

```python
from util.v3_core.schema import CanonicalCell


def overlay_cell_with_masks(cell: CanonicalCell, masks: list[MaskFeature]) -> OverlayResult:
    cell_area = polygon_area(cell.vertices)
    if cell_area <= 0.0:
        return OverlayResult(
            cell_id=cell.cell_id,
            winning_class="",
            winning_priority=0,
            class_fractions={},
            source_feature_ids=[],
            quality_flags=["zero_area_cell"],
        )

    class_fractions: dict[str, float] = {}
    source_feature_ids: list[str] = []
    winning_class = ""
    winning_priority = 0

    for mask in masks:
        intersection = polygon_clip_convex(cell.vertices, mask.polygon)
        area = polygon_area(intersection)
        if area <= 1.0e-12:
            continue
        fraction = min(1.0, area / cell_area)
        class_fractions[mask.mask_class] = class_fractions.get(mask.mask_class, 0.0) + fraction
        source_feature_ids.append(mask.feature_id)
        if mask.priority >= winning_priority:
            winning_priority = mask.priority
            winning_class = mask.mask_class

    return OverlayResult(
        cell_id=cell.cell_id,
        winning_class=winning_class,
        winning_priority=winning_priority,
        class_fractions={key: min(1.0, value) for key, value in class_fractions.items()},
        source_feature_ids=source_feature_ids,
        quality_flags=[],
    )
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add util/v3_core/geometry.py tests/test_v3_geometry.py
git commit -m "Add v3 reference mask cell overlay

Constraint: Overlay logic must support triangle, hex, and general polygon cells.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_geometry.py -q"
```

---

### Task 4: Add Overlay QA Counters

**Files:**
- Modify: `util/v3_core/geometry.py`
- Modify: `tests/test_v3_geometry.py`

- [ ] **Step 1: Add failing QA summary tests**

Append to `tests/test_v3_geometry.py`:

```python
from util.v3_core.geometry import summarize_overlay_results


def test_summarize_overlay_results_counts_classes_and_missing_masks():
    results = [
        OverlayResult("a", "LAND", 1, {"LAND": 1.0}, ["land"], []),
        OverlayResult("b", "R3", 30, {"COAST": 1.0, "R3": 0.5}, ["coast", "river"], []),
        OverlayResult("c", "", 0, {}, [], ["missing_mask"]),
    ]

    summary = summarize_overlay_results(results)

    assert summary["cell_count"] == 3
    assert summary["winning_class_counts"] == {"LAND": 1, "R3": 1, "": 1}
    assert summary["missing_mask_count"] == 1
    assert summary["quality_flag_counts"] == {"missing_mask": 1}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: FAIL because `summarize_overlay_results` does not exist.

- [ ] **Step 3: Implement summary helper**

Append to `util/v3_core/geometry.py`:

```python

def summarize_overlay_results(results: list[OverlayResult]) -> dict[str, object]:
    winning_class_counts: dict[str, int] = {}
    quality_flag_counts: dict[str, int] = {}
    missing_mask_count = 0
    for result in results:
        winning_class_counts[result.winning_class] = winning_class_counts.get(result.winning_class, 0) + 1
        if not result.class_fractions:
            missing_mask_count += 1
        for flag in result.quality_flags:
            quality_flag_counts[flag] = quality_flag_counts.get(flag, 0) + 1
    return {
        "cell_count": len(results),
        "winning_class_counts": winning_class_counts,
        "missing_mask_count": missing_mask_count,
        "quality_flag_counts": quality_flag_counts,
    }
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add util/v3_core/geometry.py tests/test_v3_geometry.py
git commit -m "Add v3 overlay QA counters

Constraint: Geometry overlays must report manifest-friendly QA counters.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_geometry.py -q"
```

---

### Task 5: Add Geometry Backend Protocol and Python Reference Backend

**Files:**
- Create: `util/v3_core/geometry_backend.py`
- Create: `tests/test_v3_geometry_backend.py`

- [ ] **Step 1: Write failing backend tests**

Create `tests/test_v3_geometry_backend.py`:

```python
from util.v3_core.geometry import MaskFeature
from util.v3_core.geometry_backend import PythonGeometryBackend, get_geometry_backend
from util.v3_core.schema import CanonicalCell


def test_get_geometry_backend_returns_python_backend_by_default():
    backend = get_geometry_backend()

    assert isinstance(backend, PythonGeometryBackend)


def test_python_backend_overlays_cells_with_masks():
    backend = PythonGeometryBackend()
    cell = CanonicalCell.minimal("cell", cell_type="POLYGON")
    mask = MaskFeature("land", "LAND", 1, [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)])

    results = backend.overlay_cells([cell], [mask])

    assert len(results) == 1
    assert results[0].winning_class == "LAND"
    assert results[0].class_fractions == {"LAND": 1.0}
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_geometry_backend.py -q
```

Expected: FAIL with `ModuleNotFoundError: No module named 'util.v3_core.geometry_backend'`.

- [ ] **Step 3: Implement backend protocol and Python backend**

Create `util/v3_core/geometry_backend.py`:

```python
from __future__ import annotations

from typing import Protocol

from util.v3_core.geometry import MaskFeature, OverlayResult, overlay_cell_with_masks
from util.v3_core.schema import CanonicalCell


class GeometryBackend(Protocol):
    name: str

    def overlay_cells(self, cells: list[CanonicalCell], masks: list[MaskFeature]) -> list[OverlayResult]:
        ...


class PythonGeometryBackend:
    name = "python_reference"

    def overlay_cells(self, cells: list[CanonicalCell], masks: list[MaskFeature]) -> list[OverlayResult]:
        return [overlay_cell_with_masks(cell, masks) for cell in cells]


def get_geometry_backend(name: str = "python_reference") -> GeometryBackend:
    if name != "python_reference":
        raise ValueError(f"unsupported geometry backend: {name}")
    return PythonGeometryBackend()
```

- [ ] **Step 4: Run backend tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_geometry_backend.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 5**

```bash
git add util/v3_core/geometry_backend.py tests/test_v3_geometry_backend.py
git commit -m "Add v3 geometry backend protocol

Constraint: Future Rust acceleration must enter through the same backend contract as the Python reference implementation.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_geometry_backend.py -q"
```

---

### Task 6: Export Geometry APIs and Run Full Validation

**Files:**
- Modify: `util/v3_core/__init__.py`
- Test: full v3 and full suite validation.

- [ ] **Step 1: Export public geometry APIs**

Modify `util/v3_core/__init__.py` to export:

```python
from util.v3_core.geometry import (
    MaskFeature,
    OverlayResult,
    overlay_cell_with_masks,
    polygon_area,
    polygon_clip_convex,
    summarize_overlay_results,
)
from util.v3_core.geometry_backend import PythonGeometryBackend, get_geometry_backend
```

and add these names to `__all__`.

- [ ] **Step 2: Run focused geometry tests**

Run:

```bash
python3 -m pytest tests/test_v3_geometry.py tests/test_v3_geometry_backend.py -q
```

Expected: all focused geometry tests pass.

- [ ] **Step 3: Run all v3 tests**

Run:

```bash
python3 -m pytest tests/test_v3_*.py -q
```

Expected: all v3 tests pass.

- [ ] **Step 4: Run full Python suite**

Run:

```bash
python3 -m pytest tests -q
```

Expected: all tests pass.

- [ ] **Step 5: Run syntax check**

Run:

```bash
python3 -m compileall util/v3_core tests/test_v3_geometry.py tests/test_v3_geometry_backend.py
```

Expected: no syntax errors.

- [ ] **Step 6: Commit exports**

```bash
git add util/v3_core/__init__.py
git commit -m "Expose v3 geometry MVP APIs

Constraint: Geometry MVP public APIs should remain backend-neutral and Rust-compatible.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_geometry.py tests/test_v3_geometry_backend.py -q; python3 -m pytest tests/test_v3_*.py -q; python3 -m pytest tests -q; python3 -m compileall util/v3_core tests/test_v3_geometry.py tests/test_v3_geometry_backend.py"
```

---

## Self-Review

- Spec coverage: This plan implements Phase 3: mask-cell intersection, priority/fraction merge, and Rust/Python parity contract groundwork.
- Scope control: It does not introduce PyO3, maturin, Cargo, or a Rust crate yet; it first locks the interface a future Rust backend must match.
- Shape compatibility: Tests cover TRI, HEX, and POLYGON cells through `CanonicalCell`.
- CoLM future reserve: Overlay results expose class fractions and QA counters that can feed CoLM2024/CoLM20XX coupling products.
- Verification: Focused geometry tests, all v3 tests, full suite, and syntax checks are required before completion.
