from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any, Sequence

from util.hydro_mesh.geojson_io import read_json

_KNOWN_SURFACE_CLASSES = {"LAND", "OCEAN"}


def evaluate_hydro_mesh_qa(
    delivery_manifest_json: str | Path,
    *,
    colm_summary_json: str | Path | None = None,
    min_river_cells: int = 1,
    min_coast_cells: int = 1,
) -> dict[str, Any]:
    """Evaluate delivery-package QA gates needed before promoting a hydro mesh.

    The gates are intentionally conservative: every background cell must have a
    complete mask row, land/ocean surface classes must be explicit, river/coast
    overlays must be non-empty, and optional CoLM coupling output must preserve
    the same all-cell row count without UNKNOWN surface classes.
    """

    manifest_path = Path(delivery_manifest_json)
    manifest = json.loads(manifest_path.read_text())
    metrics = dict(manifest.get("metrics", {}) if isinstance(manifest.get("metrics"), dict) else {})
    background_count = int(metrics.get("background_cell_count", 0) or 0)
    river_cells = int(metrics.get("river_overlap_cells", 0) or 0)
    coast_cells = int(metrics.get("coast_overlap_cells", 0) or 0)

    files = manifest.get("files", {})
    complete_mask_path = Path(str(files["complete_cell_mask_geojson"])) if isinstance(files, dict) and files.get("complete_cell_mask_geojson") else None
    complete_features: list[dict[str, Any]] = []
    surface_counts: Counter[str] = Counter()
    unknown_surface_count = 0
    if complete_mask_path is not None:
        complete_mask = read_json(complete_mask_path)
        complete_features = _features(complete_mask)
        for feature in complete_features:
            surface_class = str(_properties(feature).get("surface_class", "UNKNOWN") or "UNKNOWN")
            surface_counts[surface_class] += 1
            if surface_class not in _KNOWN_SURFACE_CLASSES:
                unknown_surface_count += 1

    checks = [
        _check("complete_mask_present", complete_mask_path is not None, observed=str(complete_mask_path) if complete_mask_path else ""),
        _check(
            "complete_mask_cell_count_matches_background",
            len(complete_features) == background_count,
            observed=len(complete_features),
            expected=background_count,
        ),
        _check("surface_classes_known", unknown_surface_count == 0, observed=unknown_surface_count, expected=0),
        _check(
            "land_ocean_both_present",
            surface_counts.get("LAND", 0) > 0 and surface_counts.get("OCEAN", 0) > 0,
            observed=dict(sorted(surface_counts.items())),
            expected={"LAND": ">0", "OCEAN": ">0"},
        ),
        _check("river_cells_present", river_cells >= min_river_cells, observed=river_cells, expected=f">={min_river_cells}"),
        _check("coast_cells_present", coast_cells >= min_coast_cells, observed=coast_cells, expected=f">={min_coast_cells}"),
    ]

    colm_summary = None
    if colm_summary_json is not None:
        colm_summary = json.loads(Path(colm_summary_json).read_text())
        rows_written = int(colm_summary.get("rows_written", 0) or 0)
        colm_surface_counts = colm_summary.get("surface_class_counts", {})
        unknown_colm = int(colm_surface_counts.get("UNKNOWN", 0) or 0) if isinstance(colm_surface_counts, dict) else 0
        checks.extend(
            [
                _check("colm_rows_match_background", rows_written == background_count, observed=rows_written, expected=background_count),
                _check("colm_surface_unknown_zero", unknown_colm == 0, observed=unknown_colm, expected=0),
            ]
        )

    status = "pass" if all(check["status"] == "pass" for check in checks) else "fail"
    report = {
        "kind": "earthmesh_hydro_mesh_qa_report",
        "status": status,
        "delivery_manifest": str(manifest_path),
        "colm_summary_json": str(colm_summary_json) if colm_summary_json is not None else None,
        "thresholds": {
            "min_river_cells": min_river_cells,
            "min_coast_cells": min_coast_cells,
            "max_unknown_surface_cells": 0,
            "require_land_ocean_both_present": True,
        },
        "metrics": {
            "background_cell_count": background_count,
            "complete_mask_cell_count": len(complete_features),
            "surface_class_counts": dict(sorted(surface_counts.items())),
            "river_overlap_cells": river_cells,
            "coast_overlap_cells": coast_cells,
            **({"colm_rows_written": colm_summary.get("rows_written", 0)} if isinstance(colm_summary, dict) else {}),
        },
        "checks": checks,
    }
    return report


def write_hydro_mesh_qa_report(
    delivery_manifest_json: str | Path,
    output_json: str | Path,
    *,
    colm_summary_json: str | Path | None = None,
    min_river_cells: int = 1,
    min_coast_cells: int = 1,
) -> dict[str, Any]:
    report = evaluate_hydro_mesh_qa(
        delivery_manifest_json,
        colm_summary_json=colm_summary_json,
        min_river_cells=min_river_cells,
        min_coast_cells=min_coast_cells,
    )
    path = Path(output_json)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def _check(check_id: str, passed: bool, *, observed: Any, expected: Any | None = None) -> dict[str, Any]:
    payload = {
        "id": check_id,
        "status": "pass" if passed else "fail",
        "observed": observed,
    }
    if expected is not None:
        payload["expected"] = expected
    return payload


def _features(collection: dict[str, object]) -> list[dict[str, Any]]:
    features = collection.get("features", [])
    if not isinstance(features, list):
        return []
    return [feature for feature in features if isinstance(feature, dict)]


def _properties(feature: dict[str, Any]) -> dict[str, Any]:
    properties = feature.get("properties", {})
    return properties if isinstance(properties, dict) else {}


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Evaluate hydro/coast mesh QA gates for a delivery package.")
    parser.add_argument("--delivery-manifest", required=True)
    parser.add_argument("--output-json", required=True)
    parser.add_argument("--colm-summary-json")
    parser.add_argument("--min-river-cells", type=int, default=1)
    parser.add_argument("--min-coast-cells", type=int, default=1)
    args = parser.parse_args(argv)
    report = write_hydro_mesh_qa_report(
        args.delivery_manifest,
        args.output_json,
        colm_summary_json=args.colm_summary_json,
        min_river_cells=args.min_river_cells,
        min_coast_cells=args.min_coast_cells,
    )
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
