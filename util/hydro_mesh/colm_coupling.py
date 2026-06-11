from __future__ import annotations

import argparse
import csv
import json
from pathlib import Path
from typing import Any

COUPLING_FIELDS = [
    "cell_id",
    "cell_index",
    "river_class",
    "river_fraction",
    "estimated_river_area_m2",
    "normalized_cell_area_m2",
    "center_lon",
    "center_lat",
    "domain_clip_applied",
    "area_normalization",
]


def _as_float(value: Any, default: float = 0.0) -> float:
    if value is None or value == "":
        return default
    return float(value)


def _as_bool_or_empty(value: Any) -> bool | str:
    if isinstance(value, bool):
        return value
    if value is None:
        return ""
    return bool(value)


def _row_from_properties(properties: dict[str, Any]) -> dict[str, Any]:
    return {
        "cell_id": str(properties.get("cell_id", "")),
        "cell_index": properties.get("cell_index", ""),
        "river_class": str(properties.get("river_class", "")),
        "river_fraction": _as_float(properties.get("river_fraction")),
        "estimated_river_area_m2": properties.get("estimated_river_area_m2", ""),
        "normalized_cell_area_m2": properties.get("normalized_cell_area_m2", ""),
        "center_lon": properties.get("center_lon", ""),
        "center_lat": properties.get("center_lat", ""),
        "domain_clip_applied": _as_bool_or_empty(properties.get("domain_clip_applied")),
        "area_normalization": properties.get("area_normalization", ""),
    }


def intersections_to_coupling_rows(
    collection: dict[str, object],
    *,
    min_fraction: float = 0.0,
) -> list[dict[str, Any]]:
    """Convert EarthMesh corridor-intersection GeoJSON features into CoLM coupling rows."""

    if min_fraction < 0 or min_fraction > 1:
        raise ValueError("min_fraction must be between 0 and 1")
    rows: list[dict[str, Any]] = []
    for feature in collection.get("features", []):
        if not isinstance(feature, dict):
            continue
        properties = feature.get("properties", {})
        if not isinstance(properties, dict):
            continue
        row = _row_from_properties(properties)
        if row["river_fraction"] < min_fraction:
            continue
        if not row["cell_id"] or not row["river_class"]:
            continue
        rows.append(row)
    return sorted(rows, key=lambda row: (str(row["cell_id"]), str(row["river_class"])))


def write_colm_coupling_csv(
    input_geojson: str | Path,
    output_csv: str | Path,
    *,
    min_fraction: float = 0.0,
) -> list[dict[str, Any]]:
    collection = json.loads(Path(input_geojson).read_text())
    rows = intersections_to_coupling_rows(collection, min_fraction=min_fraction)
    output_path = Path(output_csv)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=COUPLING_FIELDS)
        writer.writeheader()
        writer.writerows(rows)
    return rows


def write_colm_coupling_jsonl(
    input_geojson: str | Path,
    output_jsonl: str | Path,
    *,
    min_fraction: float = 0.0,
) -> list[dict[str, Any]]:
    collection = json.loads(Path(input_geojson).read_text())
    rows = intersections_to_coupling_rows(collection, min_fraction=min_fraction)
    output_path = Path(output_jsonl)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Export EarthMesh river-cell intersections as CoLM-style coupling tables.")
    parser.add_argument("input_geojson", help="Input EarthMesh cell-intersection GeoJSON")
    parser.add_argument("output_table", help="Output coupling table path")
    parser.add_argument("--format", choices=("csv", "jsonl"), default="csv", help="Output format; default: csv")
    parser.add_argument("--min-fraction", type=float, default=0.0, help="Minimum river overlap fraction to export")
    args = parser.parse_args(argv)
    if args.format == "jsonl":
        write_colm_coupling_jsonl(args.input_geojson, args.output_table, min_fraction=args.min_fraction)
    else:
        write_colm_coupling_csv(args.input_geojson, args.output_table, min_fraction=args.min_fraction)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
