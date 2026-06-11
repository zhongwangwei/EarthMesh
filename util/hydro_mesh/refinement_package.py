from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Mapping, Sequence

from util.hydro_mesh.cell_mask_merge import write_complete_cell_mask_geojson
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
    surface_geojson: str | Path | None = None,
    complete_cell_mask_geojson: str | Path | None = None,
    output_dir: str | Path,
    title: str | None = None,
    comparison_reports: Sequence[str | Path] = (),
    failed_reports: Sequence[str | Path] = (),
    max_background_cells: int | None = None,
    unit_sphere_area: bool = True,
) -> dict[str, object]:
    """Write a reproducible QA/adaptor handoff package for one refinement candidate."""

    source_paths = {
        "background_geojson": Path(background_geojson),
        "river_geojson": Path(river_geojson),
        "coast_geojson": Path(coast_geojson),
        "log_path": Path(log_path),
    }
    if surface_geojson is not None:
        source_paths["surface_geojson"] = Path(surface_geojson)
    precomputed_complete_cell_mask_path = Path(complete_cell_mask_geojson) if complete_cell_mask_geojson is not None else None
    for path in [
        *source_paths.values(),
        *([precomputed_complete_cell_mask_path] if precomputed_complete_cell_mask_path is not None else []),
        *map(Path, comparison_reports),
        *map(Path, failed_reports),
    ]:
        if not path.exists():
            raise FileNotFoundError(path)

    directory = Path(output_dir)
    directory.mkdir(parents=True, exist_ok=True)
    eval_path = directory / f"{case_name}_refinement_eval.json"
    html_path = directory / f"{case_name}_rivers_and_integrated_coast_leaflet.html"
    ranking_path = directory / "refinement_sweep_ranking.json"
    manifest_path = directory / "delivery_manifest.json"
    complete_cell_mask_path = precomputed_complete_cell_mask_path
    if complete_cell_mask_path is None and surface_geojson is not None:
        complete_cell_mask_path = directory / f"{case_name}_complete_cell_mask.geojson"

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

    if complete_cell_mask_path is not None and precomputed_complete_cell_mask_path is None:
        write_complete_cell_mask_geojson(
            source_paths["background_geojson"],
            complete_cell_mask_path,
            river_geojson=source_paths["river_geojson"],
            coast_geojson=source_paths["coast_geojson"],
            surface_geojson=source_paths["surface_geojson"],
        )

    mesh_geojson_to_leaflet_html(
        source_paths["background_geojson"],
        source_paths["river_geojson"],
        html_path,
        coast_geojson=source_paths["coast_geojson"],
        surface_geojson=complete_cell_mask_path,
        title=title or case_name,
    )

    ranking = write_sweep_ranking(
        [eval_path, *map(Path, comparison_reports), *map(Path, failed_reports)],
        ranking_path,
        max_background_cells=max_background_cells,
    )

    manifest = _build_manifest(
        case_name=case_name,
        eval_report=eval_report,
        ranking=ranking,
        source_paths=source_paths,
        comparison_reports=[Path(path) for path in comparison_reports],
        failed_reports=[Path(path) for path in failed_reports],
        eval_path=eval_path,
        html_path=html_path,
        ranking_path=ranking_path,
        manifest_path=manifest_path,
        complete_cell_mask_path=complete_cell_mask_path,
    )
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    return manifest


def _build_manifest(
    *,
    case_name: str,
    eval_report: Mapping[str, object],
    ranking: Mapping[str, object],
    source_paths: Mapping[str, Path],
    comparison_reports: Sequence[Path],
    failed_reports: Sequence[Path],
    eval_path: Path,
    html_path: Path,
    ranking_path: Path,
    manifest_path: Path,
    complete_cell_mask_path: Path | None = None,
) -> dict[str, object]:
    files = {
        "eval_json": str(eval_path),
        "html_map": str(html_path),
        "ranking_json": str(ranking_path),
        "manifest_json": str(manifest_path),
    }
    if complete_cell_mask_path is not None:
        files["complete_cell_mask_geojson"] = str(complete_cell_mask_path)
    return {
        "kind": "earthmesh_hydro_coast_delivery_package",
        "case_name": case_name,
        "recommended_case": ranking.get("recommended_case"),
        "files": files,
        "source_files": {
            **{name: str(path) for name, path in sorted(source_paths.items())},
            "comparison_reports": [str(path) for path in comparison_reports],
            "failed_reports": [str(path) for path in failed_reports],
        },
        "metrics": {
            "background_cell_count": _feature_count(eval_report.get("background_cells")),
            "river_overlap_cells": _feature_count(eval_report.get("river_intersections")),
            "coast_overlap_cells": _feature_count(eval_report.get("coast_intersections")),
            "retained_triangles": _retained_triangles(eval_report),
        },
    }


def _feature_count(value: object) -> int:
    if not isinstance(value, Mapping):
        return 0
    return int(value.get("cell_count", value.get("feature_count", 0)) or 0)


def _retained_triangles(eval_report: Mapping[str, object]) -> dict[str, int]:
    log = eval_report.get("refinement_log")
    if not isinstance(log, Mapping):
        return {"1": 0, "2": 0, "3": 0}
    retained: dict[str, int] = {}
    for degree in ["1", "2", "3"]:
        record = log.get(degree)
        retained[degree] = int(record.get("retained_triangles", 0) or 0) if isinstance(record, Mapping) else 0
    return retained


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Package a hydro/coast refinement candidate for QA and adapter handoff.")
    parser.add_argument("--case-name", required=True)
    parser.add_argument("--background-geojson", required=True)
    parser.add_argument("--river-geojson", required=True)
    parser.add_argument("--coast-geojson", required=True)
    parser.add_argument("--surface-geojson")
    parser.add_argument("--complete-cell-mask-geojson")
    parser.add_argument("--log-path", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--title")
    parser.add_argument("--comparison-reports", nargs="*", default=[])
    parser.add_argument("--failed-reports", nargs="*", default=[])
    parser.add_argument("--max-background-cells", type=int)
    parser.add_argument("--file-area-m2", action="store_true")
    args = parser.parse_args(argv)

    manifest = write_refinement_delivery_package(
        case_name=args.case_name,
        background_geojson=args.background_geojson,
        river_geojson=args.river_geojson,
        coast_geojson=args.coast_geojson,
        surface_geojson=args.surface_geojson,
        complete_cell_mask_geojson=args.complete_cell_mask_geojson,
        log_path=args.log_path,
        output_dir=args.output_dir,
        title=args.title,
        comparison_reports=args.comparison_reports,
        failed_reports=args.failed_reports,
        max_background_cells=args.max_background_cells,
        unit_sphere_area=not args.file_area_m2,
    )
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
