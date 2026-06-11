from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Sequence

from util.hydro_mesh.cell_mask_merge import merge_cell_masks
from util.hydro_mesh.earthmesh_intersection import write_earthmesh_intersection_geojson
from util.hydro_mesh.refinement_package import write_refinement_delivery_package
from util.v3_components.hydro_merit import read_merit_window, select_merit_tiles, write_merit_mask_outputs

_RIVER_CLASSES = ("R2", "R3")
_COAST_CLASSES = ("COAST_LAND", "COAST_OCEAN")


def write_merit_refinement_delivery_package(
    *,
    case_name: str,
    background_geojson: str | Path,
    merit_root: str | Path,
    bbox: tuple[float, float, float, float],
    log_path: str | Path,
    output_dir: str | Path,
    raw_merit_output_dir: str | Path | None = None,
    write_combined_raw_mask: bool = False,
    write_raw_surface_mask: bool = True,
    stride: int = 1,
    r2_width_m: float = 50.0,
    r3_width_m: float = 300.0,
    r2_upa_km2: float = 5000.0,
    r3_upa_km2: float = 50000.0,
    min_fraction: float = 0.0,
    title: str | None = None,
    max_background_cells: int | None = None,
    unit_sphere_area: bool = False,
) -> dict[str, Any]:
    """Build a hydro/coast delivery package from MERIT-Hydro masks and EarthMesh cells.

    The MERIT masks remain the raw 90m-derived source. This bridge writes the
    EarthMesh-cell intersection layers expected by `refinement_package`, then
    delegates package/HTML/CoLM-ready manifest generation to the existing package
    writer.
    """

    directory = Path(output_dir)
    directory.mkdir(parents=True, exist_ok=True)
    merit_dir = Path(raw_merit_output_dir) if raw_merit_output_dir is not None else directory / "merit_source"
    merit_outputs = write_merit_mask_outputs(
        merit_root,
        bbox=bbox,
        output_dir=merit_dir,
        stride=stride,
        r2_width_m=r2_width_m,
        r3_width_m=r3_width_m,
        r2_upa_km2=r2_upa_km2,
        r3_upa_km2=r3_upa_km2,
        write_combined_mask=write_combined_raw_mask,
        write_surface_mask=write_raw_surface_mask,
    )

    river_intersections = directory / f"{case_name}_merit_river_cell_intersections.geojson"
    coast_intersections = directory / f"{case_name}_merit_coast_cell_intersections.geojson"
    write_earthmesh_intersection_geojson(
        merit_outputs["river_masks"],
        river_intersections,
        cell_geojson=background_geojson,
        include_classes=_RIVER_CLASSES,
        min_fraction=min_fraction,
        unit_sphere_area=unit_sphere_area,
    )
    write_earthmesh_intersection_geojson(
        merit_outputs["coast_masks"],
        coast_intersections,
        cell_geojson=background_geojson,
        include_classes=_COAST_CLASSES,
        min_fraction=min_fraction,
        unit_sphere_area=unit_sphere_area,
    )

    compact_complete_cell_mask = None
    if not write_raw_surface_mask:
        compact_complete_cell_mask = directory / f"{case_name}_complete_cell_mask.geojson"
        _write_merit_sampled_complete_cell_mask(
            merit_root=merit_root,
            bbox=bbox,
            background_geojson=background_geojson,
            river_geojson=river_intersections,
            coast_geojson=coast_intersections,
            output_geojson=compact_complete_cell_mask,
            stride=stride,
        )

    manifest = write_refinement_delivery_package(
        case_name=case_name,
        background_geojson=background_geojson,
        river_geojson=river_intersections,
        coast_geojson=coast_intersections,
        surface_geojson=merit_outputs["surface_masks"],
        complete_cell_mask_geojson=compact_complete_cell_mask,
        log_path=log_path,
        output_dir=directory,
        title=title or case_name,
        max_background_cells=max_background_cells,
        unit_sphere_area=unit_sphere_area,
    )
    manifest_path = Path(str(manifest["files"]["manifest_json"]))

    bridge_summary_path = directory / "merit_package_bridge_summary.json"
    bridge_summary = _build_bridge_summary(
        case_name=case_name,
        merit_root=Path(merit_root),
        bbox=bbox,
        stride=stride,
        thresholds={
            "r2_width_m": r2_width_m,
            "r3_width_m": r3_width_m,
            "r2_upa_km2": r2_upa_km2,
            "r3_upa_km2": r3_upa_km2,
        },
        min_fraction=min_fraction,
        background_geojson=Path(background_geojson),
        merit_outputs=merit_outputs,
        river_intersections=river_intersections,
        coast_intersections=coast_intersections,
        manifest_path=manifest_path,
    )
    bridge_summary_path.write_text(json.dumps(bridge_summary, indent=2, sort_keys=True) + "\n")

    return {
        "manifest": manifest,
        "manifest_path": manifest_path,
        "bridge_summary": bridge_summary_path,
        "river_intersections": river_intersections,
        "coast_intersections": coast_intersections,
        "surface_masks": merit_outputs["surface_masks"],
        "merit_masks": merit_outputs["masks"],
        "merit_summary": merit_outputs["summary"],
        "html_map": Path(str(manifest["files"]["html_map"])),
        "complete_cell_mask_geojson": Path(str(manifest["files"]["complete_cell_mask_geojson"])),
    }


def _features(collection: dict[str, object]) -> list[dict[str, object]]:
    features = collection.get("features", [])
    return [feature for feature in features if isinstance(feature, dict)] if isinstance(features, list) else []


def _write_merit_sampled_complete_cell_mask(
    *,
    merit_root: str | Path,
    bbox: tuple[float, float, float, float],
    background_geojson: str | Path,
    river_geojson: str | Path,
    coast_geojson: str | Path,
    output_geojson: str | Path,
    stride: int,
) -> Path:
    background = json.loads(Path(background_geojson).read_text())
    river = json.loads(Path(river_geojson).read_text())
    coast = json.loads(Path(coast_geojson).read_text())
    windows = [read_merit_window(tile, bbox, stride=stride) for tile in select_merit_tiles(merit_root, bbox)]
    if not windows:
        raise ValueError(f"no MERIT-Hydro tiles intersect bbox={bbox}")

    surface_features = []
    for feature in _features(background):
        surface_feature = json.loads(json.dumps(feature, sort_keys=True))
        properties = dict(surface_feature.get("properties", {}) if isinstance(surface_feature.get("properties"), dict) else {})
        lon = _feature_lon(properties, surface_feature)
        lat = _feature_lat(properties, surface_feature)
        surface_class = _sample_surface_class(windows, lon, lat)
        properties["surface_class"] = surface_class
        properties["mask_class"] = surface_class
        properties["surface_source"] = "MERIT-Hydro landtype_igbp center_sample"
        surface_feature["properties"] = properties
        surface_features.append(surface_feature)

    complete = merge_cell_masks(
        background,
        river_cells=river,
        coast_cells=coast,
        surface_cells={"type": "FeatureCollection", "features": surface_features},
    )
    path = Path(output_geojson)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(complete, indent=2, sort_keys=True) + "\n")
    return path


def _feature_lon(properties: dict[str, object], feature: dict[str, object]) -> float:
    if properties.get("center_lon") is not None:
        return float(properties["center_lon"])
    return _geometry_center(feature)[0]


def _feature_lat(properties: dict[str, object], feature: dict[str, object]) -> float:
    if properties.get("center_lat") is not None:
        return float(properties["center_lat"])
    return _geometry_center(feature)[1]


def _geometry_center(feature: dict[str, object]) -> tuple[float, float]:
    geometry = feature.get("geometry", {})
    if not isinstance(geometry, dict):
        raise ValueError("background feature missing geometry")
    coordinates = geometry.get("coordinates", [])
    ring = coordinates[0] if isinstance(coordinates, list) and coordinates else []
    points = [point for point in ring if isinstance(point, list) and len(point) >= 2]
    if not points:
        raise ValueError("background feature has no polygon coordinates")
    return sum(float(point[0]) for point in points) / len(points), sum(float(point[1]) for point in points) / len(points)


def _sample_surface_class(windows: list[object], lon: float, lat: float) -> str:
    best_distance = float("inf")
    best_class = "OCEAN"
    for window in windows:
        lon_values = window.lon
        lat_values = window.lat
        if not (float(lon_values.min()) <= lon <= float(lon_values.max())):
            continue
        if not (float(lat_values.min()) <= lat <= float(lat_values.max())):
            continue
        i = int(abs(lon_values - lon).argmin())
        j = int(abs(lat_values - lat).argmin())
        distance = abs(float(lon_values[i]) - lon) + abs(float(lat_values[j]) - lat)
        if distance < best_distance:
            landtype = int(window.landtype_igbp[i, j])
            best_class = "OCEAN" if landtype in {0, 17} else "LAND"
            best_distance = distance
    return best_class


def _feature_count(path: Path | None) -> int:
    if path is None:
        return 0
    payload = json.loads(path.read_text())
    features = payload.get("features", [])
    return len(features) if isinstance(features, list) else 0


def _build_bridge_summary(
    *,
    case_name: str,
    merit_root: Path,
    bbox: tuple[float, float, float, float],
    stride: int,
    thresholds: dict[str, float],
    min_fraction: float,
    background_geojson: Path,
    merit_outputs: dict[str, Path | None],
    river_intersections: Path,
    coast_intersections: Path,
    manifest_path: Path,
) -> dict[str, Any]:
    return {
        "kind": "earthmesh_merit_refinement_delivery_bridge_summary",
        "case_name": case_name,
        "merit_root": str(merit_root),
        "bbox": list(bbox),
        "stride": stride,
        "thresholds": thresholds,
        "min_fraction": min_fraction,
        "files": {
            "background_geojson": str(background_geojson),
            "merit_masks": str(merit_outputs["masks"]) if merit_outputs["masks"] is not None else None,
            "merit_river_masks": str(merit_outputs["river_masks"]) if merit_outputs["river_masks"] is not None else None,
            "merit_coast_masks": str(merit_outputs["coast_masks"]) if merit_outputs["coast_masks"] is not None else None,
            "merit_surface_masks": str(merit_outputs["surface_masks"]) if merit_outputs["surface_masks"] is not None else None,
            "merit_summary": str(merit_outputs["summary"]) if merit_outputs["summary"] is not None else None,
            "river_intersections": str(river_intersections),
            "coast_intersections": str(coast_intersections),
            "delivery_manifest": str(manifest_path),
        },
        "counts": {
            "merit_river_mask_features": _feature_count(merit_outputs["river_masks"]),
            "merit_coast_mask_features": _feature_count(merit_outputs["coast_masks"]),
            "merit_surface_mask_features": _feature_count(merit_outputs["surface_masks"]),
            "river_intersection_features": _feature_count(river_intersections),
            "coast_intersection_features": _feature_count(coast_intersections),
        },
    }


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build a hydro/coast delivery package from MERIT-Hydro masks.")
    parser.add_argument("--case-name", required=True)
    parser.add_argument("--background-geojson", required=True)
    parser.add_argument("--merit-root", required=True)
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("WEST", "SOUTH", "EAST", "NORTH"), required=True)
    parser.add_argument("--log-path", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument(
        "--raw-merit-output-dir",
        help="Optional external directory for large raw MERIT mask GeoJSONs; keeps the delivery package slim.",
    )
    parser.add_argument(
        "--write-combined-raw-mask",
        action="store_true",
        help="Also write the duplicate combined merit_masks.geojson for forensic/debug use.",
    )
    parser.add_argument(
        "--skip-raw-surface-mask",
        action="store_true",
        help="Skip the large raw MERIT surface GeoJSON and write a compact cell-keyed complete mask sampled from MERIT landtype instead.",
    )
    parser.add_argument("--stride", type=int, default=1)
    parser.add_argument("--r2-width-m", type=float, default=50.0)
    parser.add_argument("--r3-width-m", type=float, default=300.0)
    parser.add_argument("--r2-upa-km2", type=float, default=5000.0)
    parser.add_argument("--r3-upa-km2", type=float, default=50000.0)
    parser.add_argument("--min-fraction", type=float, default=0.0)
    parser.add_argument("--title")
    parser.add_argument("--max-background-cells", type=int)
    parser.add_argument("--unit-sphere-area", action="store_true")
    args = parser.parse_args(argv)

    result = write_merit_refinement_delivery_package(
        case_name=args.case_name,
        background_geojson=args.background_geojson,
        merit_root=args.merit_root,
        bbox=tuple(args.bbox),
        log_path=args.log_path,
        output_dir=args.output_dir,
        raw_merit_output_dir=args.raw_merit_output_dir,
        write_combined_raw_mask=args.write_combined_raw_mask,
        write_raw_surface_mask=not args.skip_raw_surface_mask,
        stride=args.stride,
        r2_width_m=args.r2_width_m,
        r3_width_m=args.r3_width_m,
        r2_upa_km2=args.r2_upa_km2,
        r3_upa_km2=args.r3_upa_km2,
        min_fraction=args.min_fraction,
        title=args.title,
        max_background_cells=args.max_background_cells,
        unit_sphere_area=args.unit_sphere_area,
    )
    printable = {key: str(value) if isinstance(value, Path) else value for key, value in result.items() if key != "manifest"}
    print(json.dumps(printable, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
