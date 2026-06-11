from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Iterable

_INCLUDED_CLASSES = {"R2", "R3"}


def _require_shapely():
    try:
        from shapely.geometry import shape
        from shapely.ops import unary_union
    except ImportError as exc:
        raise RuntimeError("EarthMesh corridor intersection requires shapely") from exc
    return shape, unary_union


def _normalize_lon(lon_deg: float) -> float:
    normalized = ((lon_deg + 180.0) % 360.0) - 180.0
    if normalized == -180.0 and lon_deg > 0:
        return 180.0
    return normalized


def _stable_degree(value: float) -> float:
    return round(float(value), 12)


def _degrees_from_radians(values) -> list[float]:
    return [_stable_degree(_normalize_lon(math.degrees(float(value)))) for value in values]


def _lat_degrees_from_radians(values) -> list[float]:
    return [_stable_degree(math.degrees(float(value))) for value in values]


def _center_in_bbox(lon: float, lat: float, bbox: tuple[float, float, float, float] | None) -> bool:
    if bbox is None:
        return True
    west, south, east, north = bbox
    return west <= lon <= east and south <= lat <= north


def read_mpas_cell_polygons(
    mesh_netcdf: str | Path,
    *,
    bbox: tuple[float, float, float, float] | None = None,
    max_cells: int | None = None,
) -> dict[str, object]:
    """Read MPAS/EarthMesh cell polygons from a NetCDF mesh file as GeoJSON."""

    try:
        from netCDF4 import Dataset
    except ImportError as exc:
        raise RuntimeError("read_mpas_cell_polygons requires netCDF4") from exc

    features: list[dict[str, object]] = []
    with Dataset(mesh_netcdf) as ds:
        lon_cell = _degrees_from_radians(ds.variables["lonCell"][:])
        lat_cell = _lat_degrees_from_radians(ds.variables["latCell"][:])
        lon_vertex = _degrees_from_radians(ds.variables["lonVertex"][:])
        lat_vertex = _lat_degrees_from_radians(ds.variables["latVertex"][:])
        n_edges_on_cell = ds.variables["nEdgesOnCell"][:]
        vertices_on_cell = ds.variables["verticesOnCell"][:]
        cell_ids = ds.variables["indexToCellID"][:] if "indexToCellID" in ds.variables else None
        area_cell_var = ds.variables["areaCell"] if "areaCell" in ds.variables else None
        area_cell = area_cell_var[:] if area_cell_var is not None else None
        area_cell_units = getattr(area_cell_var, "units", "file_units") if area_cell_var is not None else None

        for cell_index, (center_lon, center_lat) in enumerate(zip(lon_cell, lat_cell, strict=True)):
            if not _center_in_bbox(center_lon, center_lat, bbox):
                continue
            n_edges = int(n_edges_on_cell[cell_index])
            vertex_ids = [int(value) for value in vertices_on_cell[cell_index, :n_edges]]
            ring = [[lon_vertex[vertex_id - 1], lat_vertex[vertex_id - 1]] for vertex_id in vertex_ids if vertex_id > 0]
            if len(ring) < 3:
                continue
            ring.append(ring[0])
            cell_id = str(int(cell_ids[cell_index])) if cell_ids is not None else str(cell_index + 1)
            properties: dict[str, object] = {
                "cell_id": cell_id,
                "cell_index": cell_index,
                "grid_kind": "earthmesh_cell",
                "center_lon": center_lon,
                "center_lat": center_lat,
            }
            if area_cell is not None:
                properties["source_areaCell"] = float(area_cell[cell_index])
                properties["source_areaCell_units"] = str(area_cell_units or "file_units")
            features.append(
                {
                    "type": "Feature",
                    "geometry": {"type": "Polygon", "coordinates": [ring]},
                    "properties": properties,
                }
            )
            if max_cells is not None and len(features) >= max_cells:
                break
    return {"type": "FeatureCollection", "features": features}


def _feature_class(feature: dict[str, object]) -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return ""
    return str(properties.get("river_class", ""))


def _cell_id(feature: dict[str, object]) -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return ""
    return str(properties.get("cell_id", ""))


def earthmesh_cells_to_corridor_intersections(
    cells: dict[str, object],
    corridors: dict[str, object],
    *,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    min_fraction: float = 0.0,
) -> dict[str, object]:
    """Intersect EarthMesh cell polygons with river corridor polygons."""

    if min_fraction < 0 or min_fraction > 1:
        raise ValueError("min_fraction must be between 0 and 1")
    shape, unary_union = _require_shapely()
    included = set(include_classes)

    grouped_corridors: dict[str, list[object]] = {}
    for feature in corridors.get("features", []):
        if not isinstance(feature, dict):
            continue
        geometry = feature.get("geometry", {})
        if not isinstance(geometry, dict) or geometry.get("type") not in {"Polygon", "MultiPolygon"}:
            continue
        river_class = _feature_class(feature)
        if river_class not in included:
            continue
        geom = shape(geometry)
        if not geom.is_empty:
            grouped_corridors.setdefault(river_class, []).append(geom)

    class_geometries = {river_class: unary_union(geoms) for river_class, geoms in grouped_corridors.items()}
    features: list[dict[str, object]] = []
    for cell in cells.get("features", []):
        if not isinstance(cell, dict):
            continue
        cell_geometry = cell.get("geometry", {})
        cell_properties = cell.get("properties", {})
        if not isinstance(cell_geometry, dict) or not isinstance(cell_properties, dict):
            continue
        if cell_geometry.get("type") not in {"Polygon", "MultiPolygon"}:
            continue
        cell_shape = shape(cell_geometry)
        cell_area_deg2 = cell_shape.area
        if cell_area_deg2 <= 0:
            continue
        for river_class in sorted(class_geometries):
            intersection_area_deg2 = cell_shape.intersection(class_geometries[river_class]).area
            if intersection_area_deg2 <= 0:
                continue
            river_fraction = intersection_area_deg2 / cell_area_deg2
            if river_fraction < min_fraction:
                continue
            source_area = cell_properties.get("source_areaCell")
            estimated_area = float(source_area) * river_fraction if source_area is not None else None
            output_properties = dict(cell_properties)
            output_properties.update(
                {
                    "cell_id": _cell_id(cell),
                    "river_class": river_class,
                    "grid_kind": "earthmesh_cell_preview",
                    "corridor_source_geometry": "earthmesh_cell_intersection_preview",
                    "cell_area_deg2": cell_area_deg2,
                    "intersection_area_deg2": intersection_area_deg2,
                    "river_fraction": river_fraction,
                }
            )
            if estimated_area is not None:
                output_properties["source_estimated_river_area"] = estimated_area
            features.append({"type": "Feature", "geometry": cell_geometry, "properties": output_properties})
    return {"type": "FeatureCollection", "features": features}


def write_earthmesh_intersection_geojson(
    corridor_geojson: str | Path,
    output_geojson: str | Path,
    *,
    cell_geojson: str | Path | None = None,
    mpas_mesh: str | Path | None = None,
    bbox: tuple[float, float, float, float] | None = None,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    min_fraction: float = 0.0,
    max_cells: int | None = None,
) -> dict[str, object]:
    if (cell_geojson is None) == (mpas_mesh is None):
        raise ValueError("provide exactly one of cell_geojson or mpas_mesh")
    corridors = json.loads(Path(corridor_geojson).read_text())
    if cell_geojson is not None:
        cells = json.loads(Path(cell_geojson).read_text())
    else:
        cells = read_mpas_cell_polygons(mpas_mesh, bbox=bbox, max_cells=max_cells)
    intersections = earthmesh_cells_to_corridor_intersections(
        cells,
        corridors,
        include_classes=include_classes,
        min_fraction=min_fraction,
    )
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(intersections, indent=2, sort_keys=True) + "\n")
    return intersections


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Intersect EarthMesh cell polygons with hydro corridor GeoJSON.")
    parser.add_argument("corridor_geojson", help="Input R2/R3 corridor polygon GeoJSON")
    parser.add_argument("output_geojson", help="Output EarthMesh cell overlap GeoJSON")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--cell-geojson", help="Input EarthMesh cell polygon GeoJSON")
    source.add_argument("--mpas-mesh", help="Input MPAS/EarthMesh NetCDF mesh with lon/lat vertices")
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("WEST", "SOUTH", "EAST", "NORTH"), help="Optional bbox for MPAS cell-center filtering")
    parser.add_argument("--classes", nargs="+", default=sorted(_INCLUDED_CLASSES), help="River classes to include; default: R2 R3")
    parser.add_argument("--min-fraction", type=float, default=0.0, help="Minimum corridor overlap fraction")
    parser.add_argument("--max-cells", type=int, help="Optional maximum cells to read from MPAS mesh")
    parser.add_argument("--preview-png", help="Optional PNG preview path")
    parser.add_argument("--title", default="EarthMesh river-cell intersection preview", help="Preview PNG title")
    args = parser.parse_args(argv)

    write_earthmesh_intersection_geojson(
        args.corridor_geojson,
        args.output_geojson,
        cell_geojson=args.cell_geojson,
        mpas_mesh=args.mpas_mesh,
        bbox=tuple(args.bbox) if args.bbox is not None else None,
        include_classes=args.classes,
        min_fraction=args.min_fraction,
        max_cells=args.max_cells,
    )
    if args.preview_png:
        from util.hydro_mesh.corridor_preview import write_corridor_preview_png

        write_corridor_preview_png(args.output_geojson, args.preview_png, title=args.title)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
