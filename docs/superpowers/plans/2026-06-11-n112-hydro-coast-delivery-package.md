# N112 Hydro/Coast Delivery Package Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reproducible package command that turns a selected hydro/coast refinement candidate into eval JSON, integrated HTML QA, ranking JSON, and a delivery manifest.

**Architecture:** Implement a focused orchestration module under `util/hydro_mesh/refinement_package.py`. Reuse existing `refinement_eval`, `refinement_sweep`, and `geojson_map` code instead of duplicating metric, ranking, or HTML rendering logic. Keep the command file-based and dry-run-free: it packages existing artifacts and does not rerun EarthMesh Fortran.

**Tech Stack:** Python standard library, existing hydro-mesh Python modules, pytest.

---

## File Structure

- Create: `util/hydro_mesh/refinement_package.py` — package writer, manifest builder, CLI.
- Create: `tests/test_refinement_package.py` — synthetic GeoJSON/log tests for writer and CLI.
- Modify: `docs/hydro_mesh_data_requirements.md` — record the concrete command for packaging the N112 candidate.

### Task 1: Package writer API

**Files:**
- Create: `tests/test_refinement_package.py`
- Create: `util/hydro_mesh/refinement_package.py`

- [ ] **Step 1: Write the failing test**

Create `tests/test_refinement_package.py` with a test that calls the desired API:

```python
import json
from pathlib import Path


def _feature_collection(features):
    return {"type": "FeatureCollection", "features": features}


def _cell(cell_id, area=4.0):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
        "properties": {"cell_id": cell_id, "source_areaCell": area},
    }


def _river(cell_id, river_class="R3"):
    feature = _cell(cell_id)
    feature["properties"].update({"river_class": river_class, "river_fraction": 0.5})
    return feature


def _coast(cell_id):
    feature = _cell(cell_id)
    feature["properties"].update({"mask_class": "COAST", "coastal_fraction": 0.25})
    return feature


def test_write_refinement_delivery_package_creates_manifest_eval_html_and_ranking(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    log = tmp_path / "mkgrd.log"
    output_dir = tmp_path / "package"
    background.write_text(json.dumps(_feature_collection([_cell("a"), _cell("b", 9.0)])))
    river.write_text(json.dumps(_feature_collection([_river("a", "R2"), _river("b", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("a")])))
    log.write_text(
        " refine_degree =            3\n"
        " 需要细化的三角形个数(spc only):           4\n"
        " 去除孤立细化三角形后，需要细化的三角形：          3\n"
    )

    manifest = write_refinement_delivery_package(
        case_name="N112_r3d3_cst20",
        background_geojson=background,
        river_geojson=river,
        coast_geojson=coast,
        log_path=log,
        output_dir=output_dir,
        title="Package smoke",
        max_background_cells=10,
        unit_sphere_area=False,
    )

    assert manifest["kind"] == "earthmesh_hydro_coast_delivery_package"
    assert manifest["case_name"] == "N112_r3d3_cst20"
    assert manifest["recommended_case"] == "N112_r3d3_cst20"
    assert manifest["metrics"]["background_cell_count"] == 2
    assert manifest["metrics"]["river_overlap_cells"] == 2
    assert manifest["metrics"]["coast_overlap_cells"] == 1
    assert manifest["metrics"]["retained_triangles"] == {"1": 0, "2": 0, "3": 3}
    for key in ["eval_json", "html_map", "ranking_json", "manifest_json"]:
        assert Path(manifest["files"][key]).exists()
    html_text = Path(manifest["files"]["html_map"]).read_text()
    assert "Package smoke" in html_text
    written_manifest = json.loads(Path(manifest["files"]["manifest_json"]).read_text())
    assert written_manifest == manifest
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
python3 -m pytest tests/test_refinement_package.py::test_write_refinement_delivery_package_creates_manifest_eval_html_and_ranking -q
```

Expected: FAIL because `util.hydro_mesh.refinement_package` does not exist.

- [ ] **Step 3: Write minimal implementation**

Create `util/hydro_mesh/refinement_package.py` with:

```python
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Sequence

from util.hydro_mesh.geojson_map import mesh_geojson_to_leaflet_html
from util.hydro_mesh.refinement_eval import write_refinement_eval_json
from util.hydro_mesh.refinement_sweep import write_sweep_ranking


def write_refinement_delivery_package(
    *,
    case_name: str,
    background_geojson: str | Path,
    river_geojson: str | Path,
    coast_geojson: str | Path,
    log_path: str | Path,
    output_dir: str | Path,
    title: str | None = None,
    comparison_reports: Sequence[str | Path] = (),
    failed_reports: Sequence[str | Path] = (),
    max_background_cells: int | None = None,
    unit_sphere_area: bool = True,
) -> dict[str, object]:
    source_paths = {
        "background_geojson": Path(background_geojson),
        "river_geojson": Path(river_geojson),
        "coast_geojson": Path(coast_geojson),
        "log_path": Path(log_path),
    }
    for path in [*source_paths.values(), *map(Path, comparison_reports), *map(Path, failed_reports)]:
        if not path.exists():
            raise FileNotFoundError(path)

    directory = Path(output_dir)
    directory.mkdir(parents=True, exist_ok=True)
    eval_path = directory / f"{case_name}_refinement_eval.json"
    html_path = directory / f"{case_name}_rivers_and_integrated_coast_leaflet.html"
    ranking_path = directory / "refinement_sweep_ranking.json"
    manifest_path = directory / "delivery_manifest.json"

    eval_report = write_refinement_eval_json(
        source_paths["background_geojson"],
        source_paths["river_geojson"],
        eval_path,
        coast_intersections_geojson=source_paths["coast_geojson"],
        log_path=source_paths["log_path"],
        unit_sphere_area=unit_sphere_area,
    )
    eval_report["case_name"] = case_name
    eval_report["status"] = "pass"
    eval_path.write_text(json.dumps(eval_report, indent=2, sort_keys=True) + "\n")

    mesh_geojson_to_leaflet_html(
        source_paths["river_geojson"],
        html_path,
        background_geojson=source_paths["background_geojson"],
        coast_geojson=source_paths["coast_geojson"],
        title=title or case_name,
    )

    ranking_report_paths = [eval_path, *map(Path, comparison_reports), *map(Path, failed_reports)]
    ranking = write_sweep_ranking(
        ranking_report_paths,
        ranking_path,
        max_background_cells=max_background_cells,
    )

    manifest = _build_manifest(
        case_name=case_name,
        eval_report=eval_report,
        ranking=ranking,
        source_paths=source_paths,
        eval_path=eval_path,
        html_path=html_path,
        ranking_path=ranking_path,
        manifest_path=manifest_path,
    )
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest
```

Include helper functions `_build_manifest()`, `_retained_triangles()`, `_feature_count()`, and `main()` in Task 2.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
python3 -m pytest tests/test_refinement_package.py::test_write_refinement_delivery_package_creates_manifest_eval_html_and_ranking -q
```

Expected: PASS.

### Task 2: CLI, missing-input behavior, docs

**Files:**
- Modify: `tests/test_refinement_package.py`
- Modify: `util/hydro_mesh/refinement_package.py`
- Modify: `docs/hydro_mesh_data_requirements.md`

- [ ] **Step 1: Add failing CLI and error tests**

Append tests for `main()` and missing inputs:

```python

def test_refinement_package_cli_writes_package(tmp_path):
    from util.hydro_mesh.refinement_package import main

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    log = tmp_path / "mkgrd.log"
    output_dir = tmp_path / "package"
    background.write_text(json.dumps(_feature_collection([_cell("a")])))
    river.write_text(json.dumps(_feature_collection([_river("a", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("a")])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          7\n")

    assert main([
        "--case-name", "cli_case",
        "--background-geojson", str(background),
        "--river-geojson", str(river),
        "--coast-geojson", str(coast),
        "--log-path", str(log),
        "--output-dir", str(output_dir),
        "--title", "CLI package",
        "--max-background-cells", "5",
        "--file-area-m2",
    ]) == 0

    manifest = json.loads((output_dir / "delivery_manifest.json").read_text())
    assert manifest["case_name"] == "cli_case"
    assert manifest["recommended_case"] == "cli_case"
    assert (output_dir / "cli_case_rivers_and_integrated_coast_leaflet.html").exists()


def test_write_refinement_delivery_package_rejects_missing_required_input(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    missing = tmp_path / "missing.geojson"
    try:
        write_refinement_delivery_package(
            case_name="bad",
            background_geojson=missing,
            river_geojson=missing,
            coast_geojson=missing,
            log_path=missing,
            output_dir=tmp_path / "package",
        )
    except FileNotFoundError as exc:
        assert str(missing) in str(exc)
    else:
        raise AssertionError("expected missing input to raise FileNotFoundError")
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
python3 -m pytest tests/test_refinement_package.py -q
```

Expected: FAIL because `main()` and helper manifest functions are incomplete.

- [ ] **Step 3: Implement CLI and manifest helpers**

Complete `util/hydro_mesh/refinement_package.py` with:

```python

def _build_manifest(...):
    return {
        "kind": "earthmesh_hydro_coast_delivery_package",
        "case_name": case_name,
        "recommended_case": ranking.get("recommended_case"),
        "files": {...},
        "source_files": {...},
        "metrics": {...},
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Package a hydro/coast refinement candidate for QA and adapter handoff.")
    parser.add_argument("--case-name", required=True)
    parser.add_argument("--background-geojson", required=True)
    parser.add_argument("--river-geojson", required=True)
    parser.add_argument("--coast-geojson", required=True)
    parser.add_argument("--log-path", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--title")
    parser.add_argument("--comparison-reports", nargs="*", default=[])
    parser.add_argument("--failed-reports", nargs="*", default=[])
    parser.add_argument("--max-background-cells", type=int)
    parser.add_argument("--file-area-m2", action="store_true")
    args = parser.parse_args(argv)
    manifest = write_refinement_delivery_package(...)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0
```

Update `docs/hydro_mesh_data_requirements.md` with a concrete N112 packaging command using the current scratch paths.

- [ ] **Step 4: Verify focused tests and compile**

Run:

```bash
python3 -m pytest tests/test_refinement_package.py tests/test_refinement_eval.py tests/test_refinement_sweep.py tests/test_geojson_map.py -q
python3 -m compileall util/hydro_mesh/refinement_package.py
```

Expected: all tests pass and compileall prints no errors.

- [ ] **Step 5: Real scratch smoke**

Run the N112 package command against `/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch` and write to:

```text
/Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20
```

Expected files:

```text
N112_r3d3_cst20_refinement_eval.json
N112_r3d3_cst20_rivers_and_integrated_coast_leaflet.html
refinement_sweep_ranking.json
delivery_manifest.json
```

- [ ] **Step 6: Full verification and commit**

Run:

```bash
python3 -m pytest tests/test_refinement_package.py tests/test_refinement_eval.py tests/test_refinement_sweep.py tests/test_geojson_map.py -q
python3 -m compileall util/hydro_mesh/refinement_package.py
```

Commit:

```bash
git add util/hydro_mesh/refinement_package.py tests/test_refinement_package.py docs/hydro_mesh_data_requirements.md docs/superpowers/plans/2026-06-11-n112-hydro-coast-delivery-package.md
git commit -m "Package promoted hydro coast refinement candidates

Constraint: N112 promotion must be reproducible from explicit GeoJSON/log inputs without rerunning mkgrd.x.
Rejected: Keep using hand-opened scratch HTML paths | they do not provide a stable adapter handoff or ranking manifest.
Confidence: high
Scope-risk: narrow
Directive: Treat delivery_manifest.json as the next coupling-table input boundary.
Tested: python3 -m pytest tests/test_refinement_package.py tests/test_refinement_eval.py tests/test_refinement_sweep.py tests/test_geojson_map.py -q; python3 -m compileall util/hydro_mesh/refinement_package.py"
```

## Self-Review

- Spec coverage: The plan implements package writer, CLI, manifest, docs, tests, and real N112 smoke from the design spec.
- Deferred-work scan: No deferred deferred markers are used; all commands and file paths are explicit.
- Type consistency: API names are consistently `write_refinement_delivery_package()` and `main()`; manifest keys are consistently `files`, `source_files`, `metrics`, and `recommended_case`.
