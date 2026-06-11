from __future__ import annotations

import argparse
import json
import math
import re
import statistics
from collections import Counter
from pathlib import Path

from util.hydro_mesh.earthmesh_intersection import EARTH_RADIUS_M


def _features(collection: dict[str, object]) -> list[dict[str, object]]:
    features = collection.get("features", [])
    if not isinstance(features, list):
        return []
    return [feature for feature in features if isinstance(feature, dict)]


def _properties(feature: dict[str, object]) -> dict[str, object]:
    properties = feature.get("properties", {})
    return properties if isinstance(properties, dict) else {}


def _round(value: float) -> float:
    return round(float(value), 12)


def _source_area_m2(properties: dict[str, object], *, unit_sphere_area: bool) -> float | None:
    if "normalized_cell_area_m2" in properties:
        return float(properties["normalized_cell_area_m2"])
    if "source_areaCell" not in properties:
        return None
    source_area = float(properties["source_areaCell"])
    return source_area * EARTH_RADIUS_M * EARTH_RADIUS_M if unit_sphere_area else source_area


def summarize_background_cells(collection: dict[str, object], *, unit_sphere_area: bool = True) -> dict[str, object]:
    """Summarize background/domain EarthMesh cells from GeoJSON."""

    features = _features(collection)
    areas = [
        area
        for area in (
            _source_area_m2(_properties(feature), unit_sphere_area=unit_sphere_area)
            for feature in features
        )
        if area is not None and area > 0.0
    ]
    summary: dict[str, object] = {"cell_count": len(features)}
    if not areas:
        return summary
    sizes_km = [math.sqrt(area) / 1000.0 for area in areas]
    summary.update(
        {
            "equivalent_cell_size_km_min": _round(min(sizes_km)),
            "equivalent_cell_size_km_median": _round(statistics.median(sizes_km)),
            "equivalent_cell_size_km_max": _round(max(sizes_km)),
        }
    )
    return summary


def summarize_intersections(collection: dict[str, object]) -> dict[str, object]:
    """Summarize EarthMesh cell x river-class intersection GeoJSON."""

    features = _features(collection)
    class_counts: Counter[str] = Counter()
    fractions: list[float] = []
    river_area_sum = 0.0
    for feature in features:
        properties = _properties(feature)
        river_class = str(properties.get("river_class", ""))
        if river_class:
            class_counts[river_class] += 1
        if "river_fraction" in properties:
            fractions.append(float(properties["river_fraction"]))
        if "estimated_river_area_m2" in properties:
            river_area_sum += float(properties["estimated_river_area_m2"])

    summary: dict[str, object] = {
        "feature_count": len(features),
        "class_counts": dict(sorted(class_counts.items())),
    }
    if fractions:
        summary.update(
            {
                "river_fraction_min": _round(min(fractions)),
                "river_fraction_median": _round(statistics.median(fractions)),
                "river_fraction_max": _round(max(fractions)),
            }
        )
    if river_area_sum > 0.0:
        summary["estimated_river_area_m2_sum"] = _round(river_area_sum)
    return summary


def summarize_coast_intersections(collection: dict[str, object]) -> dict[str, object]:
    """Summarize EarthMesh cell x coast-class intersection GeoJSON."""

    features = _features(collection)
    class_counts: Counter[str] = Counter()
    fractions: list[float] = []
    coastal_area_sum = 0.0
    for feature in features:
        properties = _properties(feature)
        coast_class = str(
            properties.get("mask_class")
            or properties.get("overlap_class")
            or properties.get("coast_class")
            or ""
        )
        if coast_class:
            class_counts[coast_class] += 1
        if "coastal_fraction" in properties:
            fractions.append(float(properties["coastal_fraction"]))
        elif "coast_fraction" in properties:
            fractions.append(float(properties["coast_fraction"]))
        if "estimated_coastal_area_m2" in properties:
            coastal_area_sum += float(properties["estimated_coastal_area_m2"])
        elif "estimated_coast_area_m2" in properties:
            coastal_area_sum += float(properties["estimated_coast_area_m2"])

    summary: dict[str, object] = {
        "feature_count": len(features),
        "class_counts": dict(sorted(class_counts.items())),
    }
    if fractions:
        summary.update(
            {
                "coastal_fraction_min": _round(min(fractions)),
                "coastal_fraction_median": _round(statistics.median(fractions)),
                "coastal_fraction_max": _round(max(fractions)),
            }
        )
    if coastal_area_sum > 0.0:
        summary["estimated_coastal_area_m2_sum"] = _round(coastal_area_sum)
    return summary


def parse_refinement_log(log_text: str) -> dict[str, dict[str, int]]:
    """Extract selected/retained triangle counts per specified-refinement level."""

    current_degree: str | None = None
    result: dict[str, dict[str, int]] = {}
    for line in log_text.splitlines():
        numbers = [int(value) for value in re.findall(r"-?\d+", line)]
        if "refine_degree" in line and numbers:
            current_degree = str(numbers[-1])
            result.setdefault(current_degree, {})
        elif "需要细化的三角形个数" in line and current_degree and numbers:
            result.setdefault(current_degree, {})["selected_triangles"] = numbers[-1]
        elif re.search(r"\bbefore\s+num_ref\b", line) and current_degree and numbers:
            result.setdefault(current_degree, {})["before_nested_cleanup_triangles"] = numbers[-1]
        elif re.search(r"\bafter\s+num_ref\b", line) and current_degree and numbers:
            result.setdefault(current_degree, {})["after_nested_cleanup_triangles"] = numbers[-1]
        elif "去除孤立细化三角形后" in line and current_degree and numbers:
            result.setdefault(current_degree, {})["retained_triangles"] = numbers[-1]
    return result


def build_refinement_eval(
    background_cells: dict[str, object],
    intersections: dict[str, object],
    *,
    coast_intersections: dict[str, object] | None = None,
    log_text: str | None = None,
    unit_sphere_area: bool = True,
) -> dict[str, object]:
    report: dict[str, object] = {
        "kind": "earthmesh_hydro_refinement_eval",
        "background_cells": summarize_background_cells(background_cells, unit_sphere_area=unit_sphere_area),
        "river_intersections": summarize_intersections(intersections),
    }
    if coast_intersections is not None:
        report["coast_intersections"] = summarize_coast_intersections(coast_intersections)
    if log_text is not None:
        report["refinement_log"] = parse_refinement_log(log_text)
    return report


def write_refinement_eval_json(
    background_geojson: str | Path,
    intersections_geojson: str | Path,
    output_json: str | Path,
    *,
    coast_intersections_geojson: str | Path | None = None,
    log_path: str | Path | None = None,
    unit_sphere_area: bool = True,
) -> dict[str, object]:
    background = json.loads(Path(background_geojson).read_text())
    intersections = json.loads(Path(intersections_geojson).read_text())
    coast_intersections = (
        json.loads(Path(coast_intersections_geojson).read_text())
        if coast_intersections_geojson is not None
        else None
    )
    log_text = Path(log_path).read_text() if log_path is not None else None
    report = build_refinement_eval(
        background,
        intersections,
        coast_intersections=coast_intersections,
        log_text=log_text,
        unit_sphere_area=unit_sphere_area,
    )
    output_path = Path(output_json)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Summarize EarthMesh hydro-refinement cells and river overlaps.")
    parser.add_argument("background_geojson", help="GeoJSON containing all background/domain EarthMesh cells")
    parser.add_argument("intersections_geojson", help="GeoJSON containing river-overlap EarthMesh cells")
    parser.add_argument("output_json", help="Output evaluation JSON")
    parser.add_argument(
        "--coast-intersections-geojson",
        default=None,
        help="Optional GeoJSON containing coast-overlap EarthMesh cells",
    )
    parser.add_argument("--log-path", default=None, help="Optional mkgrd.x log path")
    parser.add_argument(
        "--file-area-m2",
        action="store_true",
        help="Treat source_areaCell values as square meters instead of unit-sphere areas.",
    )
    args = parser.parse_args(argv)

    report = write_refinement_eval_json(
        args.background_geojson,
        args.intersections_geojson,
        args.output_json,
        coast_intersections_geojson=args.coast_intersections_geojson,
        log_path=args.log_path,
        unit_sphere_area=not args.file_area_m2,
    )
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
