from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Iterable

_DEFAULT_CLASSES = {"R2", "R3"}


def records_to_feature_collection(
    records: Iterable[dict[str, object]],
    *,
    include_classes: set[str] | None = None,
) -> dict[str, object]:
    include_classes = include_classes or _DEFAULT_CLASSES
    features: list[dict[str, object]] = []
    for record in records:
        river_class = str(record.get("river_class", ""))
        if river_class not in include_classes:
            continue
        lon = float(record["lon"])
        lat = float(record["lat"])
        features.append(
            {
                "type": "Feature",
                "geometry": {"type": "Point", "coordinates": [lon, lat]},
                "properties": dict(record),
            }
        )
    return {"type": "FeatureCollection", "features": features}


def _read_jsonl(path: str | Path) -> list[dict[str, object]]:
    records: list[dict[str, object]] = []
    for line in Path(path).read_text().splitlines():
        if line.strip():
            records.append(json.loads(line))
    return records


def classified_jsonl_to_geojson(
    input_jsonl: str | Path,
    output_geojson: str | Path,
    *,
    include_classes: set[str] | None = None,
) -> dict[str, object]:
    collection = records_to_feature_collection(_read_jsonl(input_jsonl), include_classes=include_classes)
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(collection, indent=2, sort_keys=True) + "\n")
    return collection


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Export classified hydro-mesh JSONL records to GeoJSON points.")
    parser.add_argument("input_jsonl", help="Input classified JSON Lines file")
    parser.add_argument("output_geojson", help="Output GeoJSON FeatureCollection")
    parser.add_argument(
        "--classes",
        nargs="+",
        default=sorted(_DEFAULT_CLASSES),
        help="River classes to include; default: R2 R3",
    )
    args = parser.parse_args(argv)
    classified_jsonl_to_geojson(args.input_jsonl, args.output_geojson, include_classes=set(args.classes))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
