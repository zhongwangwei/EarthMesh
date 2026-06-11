# EarthMesh v3 Phase 2 Hydro-CaMa Component Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wrap the existing `util/hydro_mesh/` CaMa river/coast utilities behind a v3 component boundary without changing existing hydro-CaMa behavior.

**Architecture:** Add a small component contract in `util/v3_core/components.py` and a new `util/v3_components/hydro_cama.py` bridge. The bridge validates flat v3 recipe component options, reports planned canonical product layers, maps hydro classes into v3 semantic fields, and provides dry-run plans for existing hydro-CaMa utilities. It must not move, rewrite, or break existing `util/hydro_mesh/` modules.

**Tech Stack:** Python 3 standard library (`dataclasses`, `pathlib`, `typing`), `pytest`, existing `util.v3_core.recipe.ComponentRecipe`, existing `util.v3_core.schema` semantic vocabulary, existing `util.hydro_mesh` modules as downstream implementation candidates.

---

## File Structure

Create or modify:

- Create: `util/v3_core/components.py` — generic component context/result/product dataclasses.
- Modify: `util/v3_core/__init__.py` — export component contract classes.
- Create: `util/v3_components/__init__.py` — package marker for v3 components.
- Create: `util/v3_components/hydro_cama.py` — hydro-CaMa component bridge and config parser.
- Create: `tests/test_v3_components.py` — component contract tests.
- Create: `tests/test_v3_hydro_cama_component.py` — hydro-CaMa bridge tests.

No files under `util/hydro_mesh/` should be modified in this phase unless a test exposes a compatibility bug in existing behavior.

---

### Task 1: Add Generic v3 Component Contract

**Files:**
- Create: `util/v3_core/components.py`
- Modify: `util/v3_core/__init__.py`
- Create: `tests/test_v3_components.py`

- [ ] **Step 1: Write failing component contract tests**

Create `tests/test_v3_components.py`:

```python
from pathlib import Path

from util.v3_core.components import ComponentProduct, ComponentResult, ComponentRunContext


def test_component_run_context_carries_case_paths_and_dry_run_flag(tmp_path):
    context = ComponentRunContext(
        case_name="gba_hydro",
        output_dir=tmp_path / "out",
        work_dir=tmp_path / "work",
        dry_run=True,
    )

    assert context.case_name == "gba_hydro"
    assert context.output_dir == Path(tmp_path / "out")
    assert context.work_dir == Path(tmp_path / "work")
    assert context.dry_run is True


def test_component_result_lists_products_and_warnings():
    product = ComponentProduct(
        layer_name="hydro_reaches",
        semantic_type="hydro",
        path=Path("hydro/reaches.jsonl"),
        description="Classified CaMa reaches",
    )
    result = ComponentResult(
        component_name="hydro_cama",
        products=[product],
        warnings=["width_source=fallback_width_bin"],
    )

    assert result.product_names == ["hydro_reaches"]
    assert result.has_warnings is True
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_components.py -q
```

Expected: FAIL with `ModuleNotFoundError: No module named 'util.v3_core.components'`.

- [ ] **Step 3: Implement generic component contract**

Create `util/v3_core/components.py`:

```python
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class ComponentRunContext:
    case_name: str
    output_dir: Path
    work_dir: Path
    dry_run: bool = True


@dataclass(frozen=True)
class ComponentProduct:
    layer_name: str
    semantic_type: str
    path: Path
    description: str


@dataclass(frozen=True)
class ComponentResult:
    component_name: str
    products: list[ComponentProduct] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    @property
    def product_names(self) -> list[str]:
        return [product.layer_name for product in self.products]

    @property
    def has_warnings(self) -> bool:
        return bool(self.warnings)
```

Modify `util/v3_core/__init__.py` to include:

```python
from util.v3_core.components import ComponentProduct, ComponentResult, ComponentRunContext
```

and add these names to `__all__`:

```python
"ComponentProduct",
"ComponentResult",
"ComponentRunContext",
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_components.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add util/v3_core/components.py util/v3_core/__init__.py tests/test_v3_components.py
git commit -m "Add v3 component result contract

Constraint: Components should report canonical products without owning model adapter semantics.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_components.py -q"
```

---

### Task 2: Add Hydro-CaMa Component Config Parser

**Files:**
- Create: `util/v3_components/__init__.py`
- Create: `util/v3_components/hydro_cama.py`
- Create: `tests/test_v3_hydro_cama_component.py`

- [ ] **Step 1: Write failing config parser tests**

Create `tests/test_v3_hydro_cama_component.py`:

```python
from pathlib import Path

import pytest

from util.v3_core.recipe import ComponentRecipe
from util.v3_components.hydro_cama import HydroCamaConfig, hydro_cama_config_from_recipe


def test_hydro_cama_config_from_recipe_parses_required_options():
    recipe = ComponentRecipe(
        type="hydro_cama",
        options={
            "source": "/data/glb_01min",
            "bbox": [112.0, 20.0, 115.0, 24.0],
            "target_dx_km": 5.6,
            "classes": ["R2", "R3"],
            "coast_radius_cells": 3,
        },
    )

    config = hydro_cama_config_from_recipe(recipe)

    assert config.map_dir == Path("/data/glb_01min")
    assert config.bbox == (112.0, 20.0, 115.0, 24.0)
    assert config.target_dx_km == 5.6
    assert config.classes == ("R2", "R3")
    assert config.coast_radius_cells == 3


def test_hydro_cama_config_rejects_wrong_component_type():
    recipe = ComponentRecipe(type="coastline", options={"source": "/data/glb_01min"})

    with pytest.raises(ValueError, match="hydro_cama"):
        hydro_cama_config_from_recipe(recipe)


def test_hydro_cama_config_requires_four_value_bbox():
    with pytest.raises(ValueError, match="bbox"):
        HydroCamaConfig(
            map_dir=Path("/data/glb_01min"),
            bbox=(1.0, 2.0, 3.0),
            target_dx_km=5.0,
            classes=("R2",),
            coast_radius_cells=3,
        )
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_hydro_cama_component.py -q
```

Expected: FAIL with `ModuleNotFoundError: No module named 'util.v3_components'`.

- [ ] **Step 3: Implement config parser**

Create `util/v3_components/__init__.py`:

```python
"""EarthMesh v3 component bridges."""
```

Create `util/v3_components/hydro_cama.py`:

```python
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from util.v3_core.recipe import ComponentRecipe


@dataclass(frozen=True)
class HydroCamaConfig:
    map_dir: Path
    bbox: tuple[float, float, float, float]
    target_dx_km: float
    classes: tuple[str, ...] = ("R2", "R3")
    coast_radius_cells: int = 3
    uparea_to_km2: float = 1.0e-6
    y_reversed_storage: bool = True

    def __post_init__(self) -> None:
        if len(self.bbox) != 4:
            raise ValueError("bbox must contain west, south, east, north")
        if self.target_dx_km <= 0.0:
            raise ValueError("target_dx_km must be positive")
        if self.coast_radius_cells < 1:
            raise ValueError("coast_radius_cells must be at least 1")
        if not self.classes:
            raise ValueError("classes must contain at least one hydro class")


def _as_bbox(value: Any) -> tuple[float, float, float, float]:
    if not isinstance(value, list) or len(value) != 4:
        raise ValueError("bbox must contain four numeric values")
    return tuple(float(item) for item in value)


def hydro_cama_config_from_recipe(recipe: ComponentRecipe) -> HydroCamaConfig:
    if recipe.type != "hydro_cama":
        raise ValueError("hydro_cama_config_from_recipe requires a hydro_cama component recipe")
    options = recipe.options
    return HydroCamaConfig(
        map_dir=Path(str(options["source"])),
        bbox=_as_bbox(options["bbox"]),
        target_dx_km=float(options["target_dx_km"]),
        classes=tuple(str(item) for item in options.get("classes", ["R2", "R3"])),
        coast_radius_cells=int(options.get("coast_radius_cells", 3)),
        uparea_to_km2=float(options.get("uparea_to_km2", 1.0e-6)),
        y_reversed_storage=bool(options.get("y_reversed_storage", True)),
    )
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_hydro_cama_component.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add util/v3_components/__init__.py util/v3_components/hydro_cama.py tests/test_v3_hydro_cama_component.py
git commit -m "Add hydro CaMa v3 component config parser

Constraint: Phase 2 should bridge existing hydro_mesh behavior without moving or rewriting it.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_hydro_cama_component.py -q"
```

---

### Task 3: Map Hydro Classes to v3 Semantic Layers

**Files:**
- Modify: `util/v3_components/hydro_cama.py`
- Modify: `tests/test_v3_hydro_cama_component.py`

- [ ] **Step 1: Add failing semantic mapping tests**

Append to `tests/test_v3_hydro_cama_component.py`:

```python
from util.v3_components.hydro_cama import hydro_record_semantics


def test_hydro_record_semantics_maps_r3_estuary_to_exchange_cell():
    semantics = hydro_record_semantics({
        "reach_id": "cama-10-20",
        "river_class": "R3",
        "is_estuary": True,
        "upstream_area_km2": 60000.0,
        "width_m": 1400.0,
    })

    assert semantics["hydro_class"] == "ESTUARY"
    assert semantics["mesh_priority"] == 3
    assert "cama_river" in semantics["component_roles"]
    assert "exchange_cell" in semantics["component_roles"]


def test_hydro_record_semantics_maps_r2_to_refinement_role():
    semantics = hydro_record_semantics({
        "reach_id": "cama-11-21",
        "river_class": "R2",
        "is_estuary": False,
    })

    assert semantics["hydro_class"] == "R2"
    assert semantics["mesh_priority"] == 2
    assert semantics["component_roles"] == ["cama_river"]
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_hydro_cama_component.py -q
```

Expected: FAIL with `ImportError` or `AttributeError` for missing `hydro_record_semantics`.

- [ ] **Step 3: Implement hydro semantic mapping**

Append to `util/v3_components/hydro_cama.py`:

```python
_HYDRO_PRIORITY = {"R0": 0, "R1": 1, "R2": 2, "R3": 3}


def hydro_record_semantics(record: dict[str, object]) -> dict[str, object]:
    river_class = str(record.get("river_class", "R0"))
    is_estuary = bool(record.get("is_estuary", False))
    hydro_class = "ESTUARY" if is_estuary and river_class == "R3" else river_class
    roles = ["cama_river"]
    if hydro_class in {"ESTUARY", "DELTA"}:
        roles.append("exchange_cell")
    return {
        "reach_id": str(record.get("reach_id", "")),
        "hydro_class": hydro_class,
        "mesh_priority": _HYDRO_PRIORITY.get(river_class, 0),
        "component_roles": roles,
        "upstream_area_km2": record.get("upstream_area_km2", ""),
        "river_width_m": record.get("width_m", ""),
    }
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_hydro_cama_component.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add util/v3_components/hydro_cama.py tests/test_v3_hydro_cama_component.py
git commit -m "Map hydro CaMa classes to v3 semantics

Constraint: Hydro-CaMa should expose v3 semantic fields without changing existing classifier thresholds.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_hydro_cama_component.py -q"
```

---

### Task 4: Add Hydro-CaMa Dry-Run Component Planning

**Files:**
- Modify: `util/v3_components/hydro_cama.py`
- Modify: `tests/test_v3_hydro_cama_component.py`

- [ ] **Step 1: Add failing component plan test**

Append to `tests/test_v3_hydro_cama_component.py`:

```python
from util.v3_core.components import ComponentRunContext
from util.v3_components.hydro_cama import HydroCamaComponent


def test_hydro_cama_component_dry_run_reports_expected_products(tmp_path):
    config = HydroCamaConfig(
        map_dir=Path("/data/glb_01min"),
        bbox=(112.0, 20.0, 115.0, 24.0),
        target_dx_km=5.6,
        classes=("R2", "R3"),
        coast_radius_cells=3,
    )
    component = HydroCamaComponent(config)
    context = ComponentRunContext(
        case_name="gba_hydro",
        output_dir=tmp_path / "out",
        work_dir=tmp_path / "work",
        dry_run=True,
    )

    result = component.run(context)

    assert result.component_name == "hydro_cama"
    assert result.product_names == [
        "hydro_reaches",
        "hydro_corridors",
        "surface_mask",
        "coastal_band",
        "colm_coupling",
    ]
    assert result.warnings == ["dry_run_only"]
```

- [ ] **Step 2: Run tests to verify RED**

Run:

```bash
python3 -m pytest tests/test_v3_hydro_cama_component.py -q
```

Expected: FAIL because `HydroCamaComponent` does not exist.

- [ ] **Step 3: Implement dry-run component planner**

Append to `util/v3_components/hydro_cama.py`:

```python
from util.v3_core.components import ComponentProduct, ComponentResult, ComponentRunContext


class HydroCamaComponent:
    name = "hydro_cama"
    version = "0.1"

    def __init__(self, config: HydroCamaConfig) -> None:
        self.config = config

    def run(self, context: ComponentRunContext) -> ComponentResult:
        base = context.output_dir / "hydro_cama"
        products = [
            ComponentProduct("hydro_reaches", "hydro", base / "classified_reaches.jsonl", "Classified CaMa reach records"),
            ComponentProduct("hydro_corridors", "hydro", base / "river_corridors.geojson", "R2/R3 river corridor polygons"),
            ComponentProduct("surface_mask", "coast", base / "surface_mask.geojson", "LAND/OCEAN cell mask from CaMa elevation"),
            ComponentProduct("coastal_band", "coast", base / "coastal_band.geojson", "CaMa elevation-derived coastal band"),
            ComponentProduct("colm_coupling", "coupling", base / "colm_coupling.csv", "CoLM-oriented river-cell coupling table"),
        ]
        warnings = ["dry_run_only"] if context.dry_run else []
        return ComponentResult(component_name=self.name, products=products, warnings=warnings)
```

- [ ] **Step 4: Run tests to verify GREEN**

Run:

```bash
python3 -m pytest tests/test_v3_hydro_cama_component.py -q
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add util/v3_components/hydro_cama.py tests/test_v3_hydro_cama_component.py
git commit -m "Add hydro CaMa dry-run component planner

Constraint: Phase 2 should report canonical product layers before invoking existing hydro_mesh utilities.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests/test_v3_hydro_cama_component.py -q"
```

---

### Task 5: Full Phase 2 Validation

**Files:**
- No new files expected unless earlier tests require small import fixes.

- [ ] **Step 1: Run focused v3 component tests**

Run:

```bash
python3 -m pytest tests/test_v3_components.py tests/test_v3_hydro_cama_component.py -q
```

Expected: all focused component tests pass.

- [ ] **Step 2: Run existing hydro tests to prove no behavior break**

Run:

```bash
python3 -m pytest \
  tests/test_cama_binary.py \
  tests/test_cama_contract.py \
  tests/test_cama_inventory.py \
  tests/test_cama_sample.py \
  tests/test_cama_surface_mask.py \
  tests/test_coastal_band.py \
  tests/test_colm_coupling.py \
  tests/test_earthmesh_intersection.py \
  tests/test_hydro_classifier.py \
  tests/test_hydro_cli.py \
  -q
```

Expected: all existing hydro tests pass.

- [ ] **Step 3: Run full Python suite**

Run:

```bash
python3 -m pytest tests -q
```

Expected: all tests pass.

- [ ] **Step 4: Run syntax check**

Run:

```bash
python3 -m compileall util/v3_core util/v3_components tests/test_v3_components.py tests/test_v3_hydro_cama_component.py
```

Expected: no syntax errors.

- [ ] **Step 5: Commit validation-only import fixes if needed**

If Step 1-4 required code changes, commit them with:

```bash
git add <changed-files>
git commit -m "Finish hydro CaMa v3 component validation

Constraint: Phase 2 must preserve existing hydro_mesh behavior while adding v3 component boundaries.
Confidence: high
Scope-risk: narrow
Tested: python3 -m pytest tests -q; python3 -m compileall util/v3_core util/v3_components tests/test_v3_components.py tests/test_v3_hydro_cama_component.py"
```

If no files changed after validation, do not create an empty commit.

---

## Self-Review

- Spec coverage: This plan implements the Phase 2 architecture requirement to formalize `hydro_cama` as a v3 component while keeping existing `util/hydro_mesh/` utilities intact.
- Scope control: The plan does not migrate CaMa binary readers, corridor generation, coastal-band generation, EarthMesh intersections, or CoLM coupling writers; it wraps and names their v3 product layers first.
- Shape compatibility: The component products are semantic layers and do not assume tri or hex cells.
- CoLM future reserve: The planned products include hydro, coast, and coupling layers that can feed CoLM2024 and future CoLM20XX adapters.
- Verification: The plan includes focused v3 component tests, existing hydro regression tests, the full test suite, and syntax checks.
