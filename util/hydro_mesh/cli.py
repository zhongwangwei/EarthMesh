from __future__ import annotations

import argparse
import csv
import json
from dataclasses import asdict
from pathlib import Path
from typing import Iterable

from util.hydro_mesh.classifier import RiverReach, classify_reach

_BOOLEAN_TRUE = {"1", "true", "t", "yes", "y"}
_BOOLEAN_FALSE = {"0", "false", "f", "no", "n", ""}


def parse_bool(value: str | bool | None) -> bool:
    if isinstance(value, bool):
        return value
    if value is None:
        return False
    normalized = value.strip().lower()
    if normalized in _BOOLEAN_TRUE:
        return True
    if normalized in _BOOLEAN_FALSE:
        return False
    raise ValueError(f"Cannot parse boolean value: {value!r}")


def _float_field(row: dict[str, str], name: str) -> float:
    try:
        return float(row[name])
    except KeyError as exc:
        raise ValueError(f"Missing required CSV column: {name}") from exc
    except ValueError as exc:
        raise ValueError(f"Column {name} must be numeric, got {row[name]!r}") from exc


def _reach_from_row(row: dict[str, str]) -> RiverReach:
    try:
        reach_id = row["reach_id"]
    except KeyError as exc:
        raise ValueError("Missing required CSV column: reach_id") from exc
    if not reach_id:
        raise ValueError("reach_id must not be empty")

    return RiverReach(
        reach_id=reach_id,
        upstream_area_km2=_float_field(row, "upstream_area_km2"),
        width_m=_float_field(row, "width_m"),
        floodplain_width_m=_float_field(row, "floodplain_width_m"),
        target_dx_km=_float_field(row, "target_dx_km"),
        is_estuary=parse_bool(row.get("is_estuary")),
        is_delta=parse_bool(row.get("is_delta")),
        is_coastal_wetland=parse_bool(row.get("is_coastal_wetland")),
        is_major_confluence=parse_bool(row.get("is_major_confluence")),
        user_force_2d=parse_bool(row.get("user_force_2d")),
    )


def classify_rows(rows: Iterable[dict[str, str]]) -> list[dict[str, object]]:
    records: list[dict[str, object]] = []
    for row in rows:
        reach = _reach_from_row(row)
        classification = classify_reach(reach)
        record = asdict(classification)
        records.append(record)
    return records


def classify_csv(input_csv: str | Path, output_jsonl: str | Path) -> list[dict[str, object]]:
    input_path = Path(input_csv)
    output_path = Path(output_jsonl)

    with input_path.open(newline="") as handle:
        records = classify_rows(csv.DictReader(handle))

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w") as handle:
        for record in records:
            handle.write(json.dumps(record, sort_keys=True) + "\n")

    return records


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Classify river reaches for EarthMesh v3 hydro mesh preprocessing.")
    parser.add_argument("input_csv", help="CSV file with river reach attributes")
    parser.add_argument("output_jsonl", help="Output JSON Lines classification file")
    args = parser.parse_args(argv)

    classify_csv(args.input_csv, args.output_jsonl)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
