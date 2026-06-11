from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable, Sequence

DEFAULT_R2_CAPS = [40, 60, 80]
DEFAULT_COAST_CAPS = [10, 20, 40]
DEFAULT_R3_CAP = 19
DEFAULT_BUFFER_DEG_BY_REFINE_DEGREE = {"1": 1.5, "2": 1.0, "3": 0.5}
DEFAULT_SIMPLIFY_TOLERANCE_DEG = 0.005


def build_river_coast_sweep(
    *,
    river_geojson: str | Path,
    coast_geojson: str | Path,
    r2_caps: Sequence[int] = DEFAULT_R2_CAPS,
    coast_caps: Sequence[int] = DEFAULT_COAST_CAPS,
    r3_cap: int = DEFAULT_R3_CAP,
    max_masks_per_refine_degree: int = 999,
    buffer_deg_by_refine_degree: dict[str, float] | None = None,
    simplify_tolerance_deg: float = DEFAULT_SIMPLIFY_TOLERANCE_DEG,
) -> list[dict[str, object]]:
    """Build composite close-mask recipe dictionaries for an R2 x COAST sweep."""

    buffers = dict(buffer_deg_by_refine_degree or DEFAULT_BUFFER_DEG_BY_REFINE_DEGREE)
    cases: list[dict[str, object]] = []
    for r2_cap in sorted(int(value) for value in r2_caps):
        for coast_cap in sorted(int(value) for value in coast_caps):
            case_name = f"r2cap{r2_cap}_coast{coast_cap}"
            cases.append(
                {
                    "case_name": case_name,
                    "r2_cap": r2_cap,
                    "coast_cap": coast_cap,
                    "recipe": {
                        "max_masks_per_refine_degree": int(max_masks_per_refine_degree),
                        "components": [
                            {
                                "name": "coastline_support",
                                "input_geojson": str(coast_geojson),
                                "class_refine": {"COAST": 1},
                                "max_rings_by_class": {"COAST": coast_cap},
                                "simplify_tolerance_deg": float(simplify_tolerance_deg),
                            },
                            {
                                "name": "ranked_river_corridors",
                                "input_geojson": str(river_geojson),
                                "class_refine": {"R2": 1, "R3": 3},
                                "max_rings_by_class": {"R2": r2_cap, "R3": int(r3_cap)},
                                "buffer_deg_by_refine_degree": buffers,
                                "simplify_tolerance_deg": float(simplify_tolerance_deg),
                            },
                        ],
                    },
                }
            )
    return cases


def write_sweep_recipes(
    *,
    output_dir: str | Path,
    river_geojson: str | Path,
    coast_geojson: str | Path,
    r2_caps: Sequence[int] = DEFAULT_R2_CAPS,
    coast_caps: Sequence[int] = DEFAULT_COAST_CAPS,
    r3_cap: int = DEFAULT_R3_CAP,
) -> dict[str, Path]:
    """Write sweep recipe JSON files and a manifest; return written paths by case name."""

    directory = Path(output_dir)
    directory.mkdir(parents=True, exist_ok=True)
    cases = build_river_coast_sweep(
        river_geojson=river_geojson,
        coast_geojson=coast_geojson,
        r2_caps=r2_caps,
        coast_caps=coast_caps,
        r3_cap=r3_cap,
    )
    paths: dict[str, Path] = {}
    manifest_cases: list[dict[str, object]] = []
    for case in cases:
        case_name = str(case["case_name"])
        recipe_path = directory / f"{case_name}_recipe.json"
        recipe_path.write_text(json.dumps(case["recipe"], indent=2, sort_keys=True) + "\n")
        paths[case_name] = recipe_path
        manifest_cases.append(
            {
                "case_name": case_name,
                "r2_cap": case["r2_cap"],
                "coast_cap": case["coast_cap"],
                "recipe_json": str(recipe_path),
            }
        )

    manifest_path = directory / "sweep_manifest.json"
    manifest_path.write_text(
        json.dumps(
            {
                "kind": "earthmesh_refinement_sweep_manifest",
                "case_count": len(cases),
                "river_geojson": str(river_geojson),
                "coast_geojson": str(coast_geojson),
                "r2_caps": [int(value) for value in r2_caps],
                "coast_caps": [int(value) for value in coast_caps],
                "r3_cap": int(r3_cap),
                "cases": manifest_cases,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    paths["manifest"] = manifest_path
    return paths


def rank_sweep_reports(
    reports: Iterable[dict[str, object]],
    *,
    max_background_cells: int | None = None,
) -> list[dict[str, object]]:
    """Rank existing refinement evaluation reports into promotion candidates."""

    rows = [_rankable_row(report, max_background_cells=max_background_cells) for report in reports]
    rows.sort(key=_ranking_key)
    for index, row in enumerate(rows, start=1):
        row["rank"] = index
    return rows


def write_sweep_ranking(
    report_paths: Sequence[str | Path],
    output_json: str | Path,
    *,
    max_background_cells: int | None = None,
) -> dict[str, object]:
    reports: list[dict[str, object]] = []
    for report_path in report_paths:
        path = Path(report_path)
        report = json.loads(path.read_text())
        if "case_name" not in report:
            report["case_name"] = path.stem
        reports.append(report)

    ranked = rank_sweep_reports(reports, max_background_cells=max_background_cells)
    recommended = next((row["case_name"] for row in ranked if row["promotion_status"] == "candidate"), None)
    payload = {
        "kind": "earthmesh_refinement_sweep_ranking",
        "recommended_case": recommended,
        "ranked_cases": ranked,
    }
    output_path = Path(output_json)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return payload


def _rankable_row(report: dict[str, object], *, max_background_cells: int | None) -> dict[str, object]:
    background = _mapping(report.get("background_cells"))
    river = _mapping(report.get("river_intersections"))
    coast = _mapping(report.get("coast_intersections"))
    status = str(report.get("status", "pass"))
    background_cells = int(background.get("cell_count", 0) or 0)
    promotion_status = _promotion_status(status, background_cells, max_background_cells)
    retained = {
        "1": _retained(report, "1"),
        "2": _retained(report, "2"),
        "3": _retained(report, "3"),
    }
    return {
        "case_name": str(report.get("case_name", "")),
        "status": status,
        "promotion_status": promotion_status,
        "background_cell_count": background_cells,
        "background_median_dx_km": float(background.get("equivalent_cell_size_km_median", 0.0) or 0.0),
        "river_overlap_cells": int(river.get("feature_count", 0) or 0),
        "coast_overlap_cells": int(coast.get("feature_count", 0) or 0),
        "retained_triangles": retained,
    }


def _ranking_key(row: dict[str, object]) -> tuple[object, ...]:
    retained = _mapping(row["retained_triangles"])
    return (
        _promotion_bucket(str(row["promotion_status"])),
        -int(retained.get("3", 0) or 0),
        -int(retained.get("2", 0) or 0),
        -int(retained.get("1", 0) or 0),
        -int(row["river_overlap_cells"]),
        -int(row["coast_overlap_cells"]),
        float(row["background_median_dx_km"]),
        int(row["background_cell_count"]),
        str(row["case_name"]),
    )


def _promotion_status(status: str, background_cells: int, max_background_cells: int | None) -> str:
    if status != "pass":
        return "failed"
    if max_background_cells is not None and background_cells > max_background_cells:
        return "blocked_background_cell_cap"
    return "candidate"


def _promotion_bucket(promotion_status: str) -> int:
    if promotion_status == "candidate":
        return 0
    if promotion_status == "blocked_background_cell_cap":
        return 1
    return 2


def _retained(report: dict[str, object], degree: str) -> int:
    log = _mapping(report.get("refinement_log"))
    degree_record = _mapping(log.get(degree))
    return int(degree_record.get("retained_triangles", 0) or 0)


def _mapping(value: object) -> dict[str, object]:
    return value if isinstance(value, dict) else {}


def _parse_int_csv(value: str) -> list[int]:
    parsed = [int(item.strip()) for item in value.split(",") if item.strip()]
    if not parsed:
        raise ValueError("expected at least one integer value")
    return parsed


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate and rank EarthMesh hydro/coast refinement sweep cases.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    write_parser = subparsers.add_parser("write-recipes", help="Write composite close-mask recipes for an R2 x COAST sweep.")
    write_parser.add_argument("--river-geojson", required=True)
    write_parser.add_argument("--coast-geojson", required=True)
    write_parser.add_argument("--output-dir", required=True)
    write_parser.add_argument("--r2-caps", default="40,60,80")
    write_parser.add_argument("--coast-caps", default="10,20,40")
    write_parser.add_argument("--r3-cap", type=int, default=DEFAULT_R3_CAP)

    rank_parser = subparsers.add_parser("rank", help="Rank existing refinement evaluation JSON reports.")
    rank_parser.add_argument("--reports", nargs="+", required=True)
    rank_parser.add_argument("--output-json", required=True)
    rank_parser.add_argument("--max-background-cells", type=int)

    args = parser.parse_args(argv)
    if args.command == "write-recipes":
        paths = write_sweep_recipes(
            output_dir=args.output_dir,
            river_geojson=args.river_geojson,
            coast_geojson=args.coast_geojson,
            r2_caps=_parse_int_csv(args.r2_caps),
            coast_caps=_parse_int_csv(args.coast_caps),
            r3_cap=args.r3_cap,
        )
        print(json.dumps({name: str(path) for name, path in sorted(paths.items())}, indent=2, sort_keys=True))
        return 0
    if args.command == "rank":
        payload = write_sweep_ranking(
            args.reports,
            args.output_json,
            max_background_cells=args.max_background_cells,
        )
        print(json.dumps(payload, indent=2, sort_keys=True))
        return 0
    raise AssertionError(f"unhandled command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
