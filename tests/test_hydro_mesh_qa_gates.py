import json
from pathlib import Path


def _feature_collection(features):
    return {"type": "FeatureCollection", "features": features}


def _cell(cell_id, surface_class="LAND", mask_class=None):
    properties = {"cell_id": cell_id, "surface_class": surface_class}
    if mask_class is not None:
        properties["mask_class"] = mask_class
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
        "properties": properties,
    }


def _write_manifest(tmp_path: Path, *, background_count: int, complete_mask_features: list[dict], river_cells=1, coast_cells=1):
    package = tmp_path / "package"
    package.mkdir()
    complete = package / "complete_cell_mask.geojson"
    complete.write_text(json.dumps(_feature_collection(complete_mask_features)))
    manifest = {
        "kind": "earthmesh_hydro_coast_delivery_package",
        "case_name": "qa_case",
        "files": {"complete_cell_mask_geojson": str(complete)},
        "metrics": {
            "background_cell_count": background_count,
            "river_overlap_cells": river_cells,
            "coast_overlap_cells": coast_cells,
        },
    }
    manifest_path = package / "delivery_manifest.json"
    manifest_path.write_text(json.dumps(manifest))
    return manifest_path


def test_hydro_mesh_qa_gates_pass_for_complete_land_ocean_river_coast_package(tmp_path):
    from util.hydro_mesh.qa_gates import evaluate_hydro_mesh_qa

    manifest_path = _write_manifest(
        tmp_path,
        background_count=3,
        complete_mask_features=[
            _cell("land", "LAND", "LAND"),
            _cell("ocean", "OCEAN", "OCEAN"),
            _cell("river", "LAND", "R3"),
        ],
        river_cells=1,
        coast_cells=1,
    )
    colm_summary = tmp_path / "colm_summary.json"
    colm_summary.write_text(json.dumps({
        "rows_written": 3,
        "surface_class_counts": {"LAND": 2, "OCEAN": 1},
        "river_cell_count": 1,
        "coast_cell_count": 1,
    }))

    report = evaluate_hydro_mesh_qa(manifest_path, colm_summary_json=colm_summary)

    assert report["kind"] == "earthmesh_hydro_mesh_qa_report"
    assert report["status"] == "pass"
    assert report["metrics"]["background_cell_count"] == 3
    assert report["metrics"]["complete_mask_cell_count"] == 3
    assert report["metrics"]["surface_class_counts"] == {"LAND": 2, "OCEAN": 1}
    assert {check["id"]: check["status"] for check in report["checks"]} == {
        "complete_mask_present": "pass",
        "complete_mask_cell_count_matches_background": "pass",
        "surface_classes_known": "pass",
        "land_ocean_both_present": "pass",
        "river_cells_present": "pass",
        "coast_cells_present": "pass",
        "colm_rows_match_background": "pass",
        "colm_surface_unknown_zero": "pass",
    }


def test_hydro_mesh_qa_gates_fail_for_missing_unknown_and_colm_mismatch(tmp_path):
    from util.hydro_mesh.qa_gates import evaluate_hydro_mesh_qa

    manifest_path = _write_manifest(
        tmp_path,
        background_count=3,
        complete_mask_features=[
            _cell("land", "LAND", "LAND"),
            _cell("unknown", "UNKNOWN", "BACKGROUND"),
        ],
        river_cells=0,
        coast_cells=0,
    )
    colm_summary = tmp_path / "bad_colm_summary.json"
    colm_summary.write_text(json.dumps({
        "rows_written": 2,
        "surface_class_counts": {"LAND": 1, "UNKNOWN": 1},
        "river_cell_count": 0,
        "coast_cell_count": 0,
    }))

    report = evaluate_hydro_mesh_qa(manifest_path, colm_summary_json=colm_summary)

    assert report["status"] == "fail"
    failed = {check["id"]: check for check in report["checks"] if check["status"] == "fail"}
    assert sorted(failed) == [
        "coast_cells_present",
        "colm_rows_match_background",
        "colm_surface_unknown_zero",
        "complete_mask_cell_count_matches_background",
        "land_ocean_both_present",
        "river_cells_present",
        "surface_classes_known",
    ]
    assert failed["complete_mask_cell_count_matches_background"]["observed"] == 2
    assert failed["complete_mask_cell_count_matches_background"]["expected"] == 3
    assert failed["surface_classes_known"]["observed"] == 1
    assert failed["colm_rows_match_background"]["observed"] == 2
    assert failed["colm_rows_match_background"]["expected"] == 3


def test_hydro_mesh_qa_cli_writes_report_and_returns_failure_code(tmp_path):
    from util.hydro_mesh.qa_gates import main

    manifest_path = _write_manifest(
        tmp_path,
        background_count=1,
        complete_mask_features=[_cell("unknown", "UNKNOWN", "BACKGROUND")],
        river_cells=0,
        coast_cells=0,
    )
    report_path = tmp_path / "qa_report.json"

    assert main(["--delivery-manifest", str(manifest_path), "--output-json", str(report_path)]) == 1
    payload = json.loads(report_path.read_text())
    assert payload["status"] == "fail"
