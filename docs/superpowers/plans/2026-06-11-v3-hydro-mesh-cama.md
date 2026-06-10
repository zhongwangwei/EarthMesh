# V3 Hydro Mesh CaMa-Flood Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first EarthMesh v3 hydro-mesh layer that classifies CaMa-Flood river reaches into hybrid R0/R1/R2/R3 classes and exports downstream products that can drive CoLM2024-oriented river/coast refinement.

**Architecture:** Keep the existing Fortran mesh/refinement kernels intact at first. Add a small Python preprocessor package under `util/hydro_mesh/` that turns CaMa-like river reach attributes into explicit classification and mask/metadata products; later tasks connect those products to EarthMesh `specified`/external priority refinement. Tests define the classification contract before implementation.

**Tech Stack:** Python 3 standard library for initial classifier and CLI, `pytest` for tests, optional `netCDF4` for later CaMa-Flood map readers, existing Fortran EarthMesh executable for final mesh generation.

---

## File Structure

- Create: `util/hydro_mesh/__init__.py` — package exports for hydro-mesh utilities.
- Create: `util/hydro_mesh/classifier.py` — pure-Python R0/R1/R2/R3 river reach classifier; no file I/O.
- Create: `util/hydro_mesh/cli.py` — later command-line entry point for CSV/NetCDF inputs and JSON/CSV outputs.
- Create: `tests/test_hydro_classifier.py` — unit tests for the classification contract.
- Later create: `util/hydro_mesh/cama_reader.py` — CaMa-Flood NetCDF map reader once real map variable names/paths are confirmed.
- Later create: `util/hydro_mesh/export_masks.py` — converts classified reaches into EarthMesh `close` masks or external priority grids.
- Later create: `examples/hydro_mesh/cama_hybrid_example.yaml` — example config for CoLM2024-oriented river/coast refinement.
- Modify later: `README.md` — document v3 hydro mesh workflow after the first executable workflow exists.

## Classification Contract

River class meanings:

- `R0`: ignored or aggregated subgrid channel; no explicit river edge.
- `R1`: one-dimensional river edge only.
- `R2`: one-dimensional river edge plus EarthMesh refinement buffer.
- `R3`: explicit two-dimensional river corridor candidate plus one-dimensional topology.

Initial classifier inputs per reach:

- `reach_id: str`
- `upstream_area_km2: float`
- `width_m: float`
- `floodplain_width_m: float`
- `target_dx_km: float`
- `is_estuary: bool`
- `is_delta: bool`
- `is_coastal_wetland: bool`
- `is_major_confluence: bool`
- `user_force_2d: bool`

Default thresholds:

- R3 if estuary/delta/coastal wetland/major confluence/user force is true.
- R3 if `max(width_m, floodplain_width_m) >= 0.25 * target_dx_km * 1000`.
- R3 if `upstream_area_km2 >= 50000`.
- R2 if `upstream_area_km2 >= 10000`.
- R2 if `max(width_m, floodplain_width_m) >= 0.10 * target_dx_km * 1000`.
- R1 if `upstream_area_km2 >= 1000`.
- Otherwise R0.

These defaults are intentionally conservative and configurable later. They are not final scientific thresholds.

---

### Task 1: Add Pure Classification Core

**Files:**
- Create: `tests/test_hydro_classifier.py`
- Create: `util/hydro_mesh/__init__.py`
- Create: `util/hydro_mesh/classifier.py`

- [ ] **Step 1: Write the failing tests**

Create `tests/test_hydro_classifier.py`:

```python
from util.hydro_mesh.classifier import RiverReach, classify_reach


def test_estuary_is_explicit_2d_even_when_small():
    reach = RiverReach(
        reach_id="estuary-small",
        upstream_area_km2=800.0,
        width_m=50.0,
        floodplain_width_m=100.0,
        target_dx_km=10.0,
        is_estuary=True,
    )

    result = classify_reach(reach)

    assert result.river_class == "R3"
    assert "estuary" in result.reasons


def test_wide_reach_becomes_explicit_2d_relative_to_mesh_resolution():
    reach = RiverReach(
        reach_id="wide-mainstem",
        upstream_area_km2=2000.0,
        width_m=3000.0,
        floodplain_width_m=1500.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R3"
    assert "effective_width_fraction" in result.reasons


def test_medium_reach_gets_1d_with_refinement_buffer():
    reach = RiverReach(
        reach_id="medium-tributary",
        upstream_area_km2=12000.0,
        width_m=200.0,
        floodplain_width_m=300.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R2"
    assert "upstream_area_r2" in result.reasons


def test_small_reach_keeps_1d_topology_only():
    reach = RiverReach(
        reach_id="small-channel",
        upstream_area_km2=2000.0,
        width_m=50.0,
        floodplain_width_m=80.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R1"
    assert "upstream_area_r1" in result.reasons


def test_tiny_reach_is_aggregated():
    reach = RiverReach(
        reach_id="tiny-channel",
        upstream_area_km2=200.0,
        width_m=20.0,
        floodplain_width_m=20.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R0"
    assert result.reasons == ["below_explicit_thresholds"]
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
python3 -m pytest tests/test_hydro_classifier.py -q
```

Expected: FAIL because `util.hydro_mesh.classifier` does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `util/hydro_mesh/__init__.py`:

```python
"""Hydro-mesh preprocessing utilities for EarthMesh v3."""
```

Create `util/hydro_mesh/classifier.py`:

```python
from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class RiverReach:
    reach_id: str
    upstream_area_km2: float
    width_m: float
    floodplain_width_m: float
    target_dx_km: float
    is_estuary: bool = False
    is_delta: bool = False
    is_coastal_wetland: bool = False
    is_major_confluence: bool = False
    user_force_2d: bool = False


@dataclass(frozen=True)
class ClassificationThresholds:
    explicit_2d_width_fraction: float = 0.25
    refine_width_fraction: float = 0.10
    explicit_2d_upstream_area_km2: float = 50000.0
    refine_upstream_area_km2: float = 10000.0
    keep_1d_upstream_area_km2: float = 1000.0


@dataclass(frozen=True)
class RiverClassification:
    reach_id: str
    river_class: str
    effective_width_m: float
    reasons: list[str] = field(default_factory=list)


def classify_reach(
    reach: RiverReach,
    thresholds: ClassificationThresholds | None = None,
) -> RiverClassification:
    thresholds = thresholds or ClassificationThresholds()
    effective_width_m = max(reach.width_m, reach.floodplain_width_m)
    target_dx_m = reach.target_dx_km * 1000.0

    reasons: list[str] = []
    if reach.is_estuary:
        reasons.append("estuary")
    if reach.is_delta:
        reasons.append("delta")
    if reach.is_coastal_wetland:
        reasons.append("coastal_wetland")
    if reach.is_major_confluence:
        reasons.append("major_confluence")
    if reach.user_force_2d:
        reasons.append("user_force_2d")

    if reasons:
        return RiverClassification(reach.reach_id, "R3", effective_width_m, reasons)

    if target_dx_m <= 0.0:
        raise ValueError("target_dx_km must be positive")

    if effective_width_m >= thresholds.explicit_2d_width_fraction * target_dx_m:
        return RiverClassification(
            reach.reach_id,
            "R3",
            effective_width_m,
            ["effective_width_fraction"],
        )

    if reach.upstream_area_km2 >= thresholds.explicit_2d_upstream_area_km2:
        return RiverClassification(
            reach.reach_id,
            "R3",
            effective_width_m,
            ["upstream_area_r3"],
        )

    if reach.upstream_area_km2 >= thresholds.refine_upstream_area_km2:
        return RiverClassification(
            reach.reach_id,
            "R2",
            effective_width_m,
            ["upstream_area_r2"],
        )

    if effective_width_m >= thresholds.refine_width_fraction * target_dx_m:
        return RiverClassification(
            reach.reach_id,
            "R2",
            effective_width_m,
            ["refine_width_fraction"],
        )

    if reach.upstream_area_km2 >= thresholds.keep_1d_upstream_area_km2:
        return RiverClassification(
            reach.reach_id,
            "R1",
            effective_width_m,
            ["upstream_area_r1"],
        )

    return RiverClassification(
        reach.reach_id,
        "R0",
        effective_width_m,
        ["below_explicit_thresholds"],
    )
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
python3 -m pytest tests/test_hydro_classifier.py -q
```

Expected: `5 passed`.

- [ ] **Step 5: Run syntax check**

Run:

```bash
python3 -m compileall util/hydro_mesh tests/test_hydro_classifier.py
```

Expected: no syntax errors.

### Task 2: Add Batch Classifier CLI

**Files:**
- Modify: `util/hydro_mesh/classifier.py`
- Create: `util/hydro_mesh/cli.py`
- Create: `tests/test_hydro_cli.py`

- [ ] **Step 1: Write failing CLI test**

Create a test that writes a temporary CSV with columns `reach_id,upstream_area_km2,width_m,floodplain_width_m,target_dx_km,is_estuary`, invokes `classify_csv`, and asserts JSON-like records include `R3` for estuary and `R1` for a small but retained channel.

- [ ] **Step 2: Verify failure**

Run:

```bash
python3 -m pytest tests/test_hydro_cli.py -q
```

Expected: FAIL because `util.hydro_mesh.cli` does not exist.

- [ ] **Step 3: Implement CSV reader/writer without external dependencies**

Use Python `csv` and `json` only. Convert boolean strings `true/false/1/0/yes/no` deterministically.

- [ ] **Step 4: Verify CLI tests pass**

Run:

```bash
python3 -m pytest tests/test_hydro_cli.py tests/test_hydro_classifier.py -q
```

Expected: all tests pass.

### Task 3: Add CaMa-Flood Data Contract Probe

**Files:**
- Create: `util/hydro_mesh/cama_contract.py`
- Create: `tests/test_cama_contract.py`

- [ ] **Step 1: Write failing tests for required variable discovery**

Test that a variable inventory containing plausible CaMa fields can be mapped to canonical names: upstream area, channel width, floodplain width if available, downstream index/topology if available, lon, lat.

- [ ] **Step 2: Implement inventory mapper**

Do not assume one fixed CaMa variable name yet. Implement a small alias table that reports missing required fields clearly.

- [ ] **Step 3: Verify tests pass**

Run:

```bash
python3 -m pytest tests/test_cama_contract.py tests/test_hydro_classifier.py -q
```

Expected: all tests pass.

### Task 4: Add Example Config and Data Requirements Document

**Files:**
- Create: `examples/hydro_mesh/cama_hybrid_example.yaml`
- Create: `docs/hydro_mesh_data_requirements.md`

- [ ] **Step 1: Write a config example with conservative defaults**

Include fields for `cama_map_dir`, `target_dx_km`, R3/R2 thresholds, estuary forcing, and output paths.

- [ ] **Step 2: Document required CaMa-Flood inputs**

List required fields: reach center lon/lat or grid cell centers, downstream topology, upstream area, channel width, river length, optional floodplain width/elevation, optional discharge or inundation, and coastline/land-sea mask source.

- [ ] **Step 3: Verify docs contain no placeholders**

Run:

```bash
rg -n "TBD|TODO|fill in|placeholder" examples/hydro_mesh docs/hydro_mesh_data_requirements.md
```

Expected: no matches.

### Task 5: Connect to EarthMesh Refinement Products

**Files:**
- Create: `util/hydro_mesh/export_masks.py`
- Create: `tests/test_export_masks.py`

- [ ] **Step 1: Write failing test for priority records**

Given classified reaches, assert R3 receives priority 3, R2 receives priority 2, R1 receives priority 1, and R0 receives priority 0.

- [ ] **Step 2: Implement priority conversion**

Keep it pure Python first. Do not write NetCDF until the priority semantics are tested.

- [ ] **Step 3: Verify test pass**

Run:

```bash
python3 -m pytest tests/test_export_masks.py tests/test_hydro_classifier.py -q
```

Expected: all tests pass.

### Task 6: Baseline Build/Static Validation

**Files:**
- No source changes expected unless tests reveal integration issues.

- [ ] **Step 1: Run Python tests**

```bash
python3 -m pytest tests -q
```

Expected: all tests pass.

- [ ] **Step 2: Run Python syntax check**

```bash
python3 -m compileall util tests
```

Expected: no syntax errors.

- [ ] **Step 3: Run Fortran dry-run build check**

```bash
make -n
```

Expected: command expansion succeeds without invoking destructive outputs.

---

## Required External Data Before Real CaMa Reader

Implementation can start without real CaMa files by using tests and CSV fixtures. To build the actual CaMa-Flood reader and validate scientific behavior, request these from the user:

1. One small CaMa-Flood map directory or representative NetCDF files for the target resolution.
2. Exact variable names in that map for upstream area, channel width, river length, downstream topology, lon, lat, and optional floodplain width/elevation.
3. Target domain: global, China, or a named basin/coastal region.
4. Target EarthMesh minimum grid spacing for the first CoLM2024 case.
5. Whether R3 explicit 2D river corridors should start with only estuaries/mainstems or include broader floodplains.

## Self-Review

- Spec coverage: The plan covers the approved C hybrid path: R0/R1/R2/R3 classification, 1D/2D decision rules, CaMa-Flood data contract, and EarthMesh refinement product handoff.
- Placeholder scan: No `TBD`, `TODO`, or deferred undefined requirements are present.
- Type consistency: `RiverReach`, `ClassificationThresholds`, and `RiverClassification` are defined before use and reused consistently.
