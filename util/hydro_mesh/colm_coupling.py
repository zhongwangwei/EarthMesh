from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from collections import Counter
from pathlib import Path
from typing import Any



_SURFACE_CLASS_CODES = {"UNKNOWN": 0, "LAND": 1, "OCEAN": 2, "COAST": 3}
_RIVER_CLASS_CODES = {"": 0, "R0": 0, "R1": 1, "R2": 2, "R3": 3}
_COAST_CLASS_CODES = {"": 0, "COAST": 1, "COAST_LAND": 2, "COAST_OCEAN": 3}

PACKAGE_COUPLING_FIELDS = [
    "case_name",
    "cell_id",
    "cell_index",
    "center_lon",
    "center_lat",
    "surface_class",
    "has_river",
    "river_class",
    "river_fraction",
    "estimated_river_area_m2",
    "has_coast",
    "coast_class",
    "coastal_fraction",
    "normalized_cell_area_m2",
    "source_areaCell",
    "source_areaCell_units",
]

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


def write_colm_package_coupling(
    delivery_manifest: str | Path,
    output_dir: str | Path,
) -> dict[str, Any]:
    """Write all-cell CoLM coupling CSV and summary from a hydro/coast delivery package."""

    manifest_path = Path(delivery_manifest)
    manifest = json.loads(manifest_path.read_text())
    source_files = manifest.get("source_files", {})
    if not isinstance(source_files, dict):
        raise ValueError("delivery manifest missing source_files")
    files = manifest.get("files", {})
    if not isinstance(files, dict):
        files = {}

    background_path = Path(str(source_files["background_geojson"]))
    river_path = Path(str(source_files["river_geojson"]))
    coast_path = Path(str(source_files["coast_geojson"]))
    surface_path, surface_source_kind = _resolve_package_surface_path(source_files, files)
    background = json.loads(background_path.read_text())
    river = json.loads(river_path.read_text())
    coast = json.loads(coast_path.read_text())
    surface = json.loads(surface_path.read_text()) if surface_path is not None else None

    case_name = str(manifest.get("case_name", ""))
    rows = _package_rows_from_collections(case_name, background, river, coast, surface=surface)

    directory = Path(output_dir)
    directory.mkdir(parents=True, exist_ok=True)
    csv_path = directory / "colm_coupling_cells.csv"
    netcdf_path = directory / "colm_coupling_cells.nc"
    summary_path = directory / "colm_coupling_summary.json"
    _write_package_csv(rows, csv_path)
    _write_package_netcdf(rows, netcdf_path, case_name=case_name, delivery_manifest=manifest_path)

    river_by_cell = _index_feature_properties(river)
    coast_by_cell = _index_feature_properties(coast)
    surface_by_cell = _index_feature_properties(surface) if surface is not None else {}
    summary = {
        "kind": "earthmesh_colm_coupling_summary",
        "case_name": case_name,
        "delivery_manifest": str(manifest_path),
        "background_geojson": str(background_path),
        "river_geojson": str(river_path),
        "coast_geojson": str(coast_path),
        **({"surface_geojson": str(surface_path)} if surface_path is not None else {}),
        **({"surface_source_kind": surface_source_kind} if surface_source_kind else {}),
        "background_cell_count": len(_features(background)),
        "river_overlap_record_count": len(_features(river)),
        "river_cell_count": len(river_by_cell),
        "coast_overlap_record_count": len(_features(coast)),
        "coast_cell_count": len(coast_by_cell),
        "surface_cell_count": len(surface_by_cell),
        "surface_class_counts": dict(Counter(str(row["surface_class"]) for row in rows)),
        "rows_written": len(rows),
        "csv_path": str(csv_path),
        "netcdf_path": str(netcdf_path),
    }
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    return {"csv_path": str(csv_path), "netcdf_path": str(netcdf_path), "summary_path": str(summary_path), "summary": summary}


def _resolve_package_surface_path(source_files: dict[str, Any], files: dict[str, Any]) -> tuple[Path | None, str]:
    if files.get("complete_cell_mask_geojson"):
        return Path(str(files["complete_cell_mask_geojson"])), "complete_cell_mask_geojson"
    if source_files.get("surface_geojson"):
        return Path(str(source_files["surface_geojson"])), "surface_geojson"
    return None, ""


def _package_rows_from_collections(
    case_name: str,
    background: dict[str, object],
    river: dict[str, object],
    coast: dict[str, object],
    *,
    surface: dict[str, object] | None = None,
) -> list[dict[str, Any]]:
    river_by_cell = _index_feature_properties(river)
    coast_by_cell = _index_feature_properties(coast)
    surface_by_cell = _index_feature_properties(surface) if surface is not None else {}
    rows: list[dict[str, Any]] = []
    for feature in _features(background):
        properties = feature.get("properties", {})
        if not isinstance(properties, dict):
            continue
        cell_id = str(properties.get("cell_id", ""))
        if not cell_id:
            continue
        river_properties = river_by_cell.get(cell_id, {})
        coast_properties = coast_by_cell.get(cell_id, {})
        surface_properties = surface_by_cell.get(cell_id, {})
        has_river = bool(river_properties)
        has_coast = bool(coast_properties)
        surface_class = _surface_class_from_properties(surface_properties) if surface_properties else ("COAST" if has_coast else "UNKNOWN")
        rows.append(
            {
                "case_name": case_name,
                "cell_id": cell_id,
                "cell_index": properties.get("cell_index", ""),
                "center_lon": properties.get("center_lon", ""),
                "center_lat": properties.get("center_lat", ""),
                "surface_class": surface_class,
                "has_river": "true" if has_river else "false",
                "river_class": river_properties.get("river_class", ""),
                "river_fraction": river_properties.get("river_fraction", ""),
                "estimated_river_area_m2": river_properties.get("estimated_river_area_m2", ""),
                "has_coast": "true" if has_coast else "false",
                "coast_class": coast_properties.get("mask_class", coast_properties.get("coast_class", "")),
                "coastal_fraction": coast_properties.get("coastal_fraction", coast_properties.get("coast_fraction", "")),
                "normalized_cell_area_m2": river_properties.get("normalized_cell_area_m2", properties.get("normalized_cell_area_m2", "")),
                "source_areaCell": properties.get("source_areaCell", ""),
                "source_areaCell_units": properties.get("source_areaCell_units", ""),
            }
        )
    return sorted(rows, key=lambda row: int(row["cell_index"]) if str(row["cell_index"]).isdigit() else str(row["cell_id"]))


def _features(collection: dict[str, object]) -> list[dict[str, object]]:
    features = collection.get("features", [])
    if not isinstance(features, list):
        return []
    return [feature for feature in features if isinstance(feature, dict)]


def _index_feature_properties(collection: dict[str, object]) -> dict[str, dict[str, Any]]:
    indexed: dict[str, dict[str, Any]] = {}
    for feature in _features(collection):
        properties = feature.get("properties", {})
        if not isinstance(properties, dict):
            continue
        cell_id = str(properties.get("cell_id", ""))
        if not cell_id:
            continue
        if cell_id not in indexed:
            indexed[cell_id] = dict(properties)
            continue
        indexed[cell_id] = _merge_overlap_properties(indexed[cell_id], properties)
    return indexed


def _merge_overlap_properties(existing: dict[str, Any], new: dict[str, Any]) -> dict[str, Any]:
    merged = dict(existing)
    if "river_class" in existing or "river_class" in new:
        merged["river_class"] = _dominant_river_class(str(existing.get("river_class", "")), str(new.get("river_class", "")))
        merged["river_fraction"] = _bounded_sum(existing.get("river_fraction"), new.get("river_fraction"))
        merged["estimated_river_area_m2"] = _numeric_sum(
            existing.get("estimated_river_area_m2"), new.get("estimated_river_area_m2")
        )
        if not merged.get("normalized_cell_area_m2") and new.get("normalized_cell_area_m2"):
            merged["normalized_cell_area_m2"] = new.get("normalized_cell_area_m2")
    if (
        "mask_class" in existing
        or "mask_class" in new
        or "coastal_fraction" in existing
        or "coastal_fraction" in new
        or "coast_fraction" in existing
        or "coast_fraction" in new
    ):
        merged["mask_class"] = existing.get("mask_class") or new.get("mask_class") or existing.get("coast_class") or new.get("coast_class", "")
        merged["coastal_fraction"] = _bounded_sum(
            existing.get("coastal_fraction", existing.get("coast_fraction")),
            new.get("coastal_fraction", new.get("coast_fraction")),
        )
    return merged


def _dominant_river_class(left: str, right: str) -> str:
    order = {"": 0, "R0": 0, "R1": 1, "R2": 2, "R3": 3}
    return left if order.get(left, 0) >= order.get(right, 0) else right


def _numeric_sum(left: Any, right: Any) -> float | str:
    total = _as_float(left) + _as_float(right)
    return round(total, 12) if total else ""


def _bounded_sum(left: Any, right: Any) -> float | str:
    total = min(1.0, _as_float(left) + _as_float(right))
    return round(total, 12) if total else ""


def _surface_class_from_properties(properties: dict[str, Any]) -> str:
    value = str(properties.get("surface_class") or properties.get("mask_class") or "UNKNOWN")
    if value in {"LAND", "OCEAN", "COAST", "UNKNOWN"}:
        return value
    if value == "COAST_LAND":
        return "LAND"
    if value == "COAST_OCEAN":
        return "OCEAN"
    return "UNKNOWN"


def _write_package_netcdf(
    rows: list[dict[str, Any]],
    output_netcdf: str | Path,
    *,
    case_name: str,
    delivery_manifest: str | Path,
) -> Path:
    try:
        import netCDF4
    except ImportError as exc:  # pragma: no cover - dependency is available in test/runtime env
        raise RuntimeError("CoLM package NetCDF export requires netCDF4") from exc

    output_path = Path(output_netcdf)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with netCDF4.Dataset(output_path, "w", format="NETCDF4") as ds:
        ds.createDimension("cell", len(rows))
        ds.setncattr("kind", "earthmesh_colm_coupling_netcdf")
        ds.setncattr("case_name", case_name)
        ds.setncattr("delivery_manifest", str(delivery_manifest))
        ds.setncattr("surface_class_code_meanings", "0=UNKNOWN 1=LAND 2=OCEAN 3=COAST")
        ds.setncattr("river_class_code_meanings", "0=none/R0 1=R1 2=R2 3=R3")
        ds.setncattr("coast_class_code_meanings", "0=none 1=COAST 2=COAST_LAND 3=COAST_OCEAN")

        cell_id = ds.createVariable("cell_id", str, ("cell",))
        cell_index = ds.createVariable("cell_index", "i4", ("cell",))
        center_lon = ds.createVariable("center_lon", "f8", ("cell",))
        center_lat = ds.createVariable("center_lat", "f8", ("cell",))
        surface_class_code = ds.createVariable("surface_class_code", "i1", ("cell",))
        has_river = ds.createVariable("has_river", "i1", ("cell",))
        river_class_code = ds.createVariable("river_class_code", "i1", ("cell",))
        river_fraction = ds.createVariable("river_fraction", "f8", ("cell",))
        estimated_river_area_m2 = ds.createVariable("estimated_river_area_m2", "f8", ("cell",))
        has_coast = ds.createVariable("has_coast", "i1", ("cell",))
        coast_class_code = ds.createVariable("coast_class_code", "i1", ("cell",))
        coastal_fraction = ds.createVariable("coastal_fraction", "f8", ("cell",))
        normalized_cell_area_m2 = ds.createVariable("normalized_cell_area_m2", "f8", ("cell",))
        source_area_cell = ds.createVariable("source_areaCell", "f8", ("cell",))

        center_lon.units = "degrees_east"
        center_lat.units = "degrees_north"
        river_fraction.units = "1"
        coastal_fraction.units = "1"
        estimated_river_area_m2.units = "m2"
        normalized_cell_area_m2.units = "m2"
        source_area_cell.units = "source_areaCell_units from CSV when available"

        for index, row in enumerate(rows):
            cell_id[index] = str(row.get("cell_id", ""))
            cell_index[index] = _int_or_default(row.get("cell_index"), 0)
            center_lon[index] = _float_or_nan(row.get("center_lon"))
            center_lat[index] = _float_or_nan(row.get("center_lat"))
            surface_class_code[index] = _SURFACE_CLASS_CODES.get(str(row.get("surface_class", "UNKNOWN")), 0)
            has_river[index] = 1 if str(row.get("has_river", "")).lower() == "true" else 0
            river_class_code[index] = _RIVER_CLASS_CODES.get(str(row.get("river_class", "")), 0)
            river_fraction[index] = _float_or_nan(row.get("river_fraction"))
            estimated_river_area_m2[index] = _float_or_nan(row.get("estimated_river_area_m2"))
            has_coast[index] = 1 if str(row.get("has_coast", "")).lower() == "true" else 0
            coast_class_code[index] = _COAST_CLASS_CODES.get(str(row.get("coast_class", "")), 0)
            coastal_fraction[index] = _float_or_nan(row.get("coastal_fraction"))
            normalized_cell_area_m2[index] = _float_or_nan(row.get("normalized_cell_area_m2"))
            source_area_cell[index] = _float_or_nan(row.get("source_areaCell"))
    return output_path


def _float_or_nan(value: Any) -> float:
    if value is None or value == "":
        return math.nan
    return float(value)


def _int_or_default(value: Any, default: int) -> int:
    if value is None or value == "":
        return default
    return int(value)

def _write_package_csv(rows: list[dict[str, Any]], output_csv: str | Path) -> Path:
    output_path = Path(output_csv)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=PACKAGE_COUPLING_FIELDS)
        writer.writeheader()
        writer.writerows(rows)
    return output_path


def main(argv: list[str] | None = None) -> int:
    effective_argv = list(sys.argv[1:] if argv is None else argv)
    if effective_argv and effective_argv[0] == "package":
        parser = argparse.ArgumentParser(description="Export a delivery package as all-cell CoLM coupling metadata.")
        parser.add_argument("package")
        parser.add_argument("--delivery-manifest", required=True)
        parser.add_argument("--output-dir", required=True)
        args = parser.parse_args(effective_argv)
        result = write_colm_package_coupling(args.delivery_manifest, args.output_dir)
        print(json.dumps(result["summary"], indent=2, sort_keys=True))
        return 0

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
