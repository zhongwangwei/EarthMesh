from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Iterable

_INCLUDED_CLASSES = {"R2", "R3"}
EARTH_RADIUS_M = 6_371_000.0


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
    return str(properties.get("river_class") or properties.get("mask_class") or "")


def _is_river_class(class_name: str) -> bool:
    return class_name.upper().startswith("R")


def _cell_id(feature: dict[str, object]) -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return ""
    return str(properties.get("cell_id", ""))


def _union_feature_geometries(collection: dict[str, object]) -> object | None:
    shape, unary_union = _require_shapely()
    geometries: list[object] = []
    for feature in collection.get("features", []):
        if not isinstance(feature, dict):
            continue
        geometry = feature.get("geometry", {})
        if not isinstance(geometry, dict) or geometry.get("type") not in {"Polygon", "MultiPolygon"}:
            continue
        geom = shape(geometry)
        if not geom.is_empty:
            geometries.append(geom)
    if not geometries:
        return None
    return unary_union(geometries)


def _normalized_area_properties(cell_properties: dict[str, object], river_fraction: float, *, unit_sphere_area: bool) -> dict[str, object]:
    source_area = cell_properties.get("source_areaCell")
    if source_area is None:
        return {}
    source_area_value = float(source_area)
    properties: dict[str, object] = {"source_estimated_river_area": source_area_value * river_fraction}
    if unit_sphere_area:
        normalized_cell_area_m2 = source_area_value * EARTH_RADIUS_M * EARTH_RADIUS_M
        properties.update(
            {
                "area_normalization": "unit_sphere_area_to_m2",
                "normalized_cell_area_m2": normalized_cell_area_m2,
                "estimated_river_area_m2": normalized_cell_area_m2 * river_fraction,
            }
        )
    return properties


def earthmesh_cells_to_corridor_intersections(
    cells: dict[str, object],
    corridors: dict[str, object],
    *,
    domain: dict[str, object] | None = None,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    min_fraction: float = 0.0,
    unit_sphere_area: bool = False,
) -> dict[str, object]:
    """Intersect EarthMesh cell polygons with river corridor polygons."""

    if min_fraction < 0 or min_fraction > 1:
        raise ValueError("min_fraction must be between 0 and 1")
    shape, unary_union = _require_shapely()
    included = set(include_classes)
    domain_geometry = _union_feature_geometries(domain) if domain is not None else None

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
        if domain_geometry is not None:
            geom = geom.intersection(domain_geometry)
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
            output_properties = dict(cell_properties)
            output_properties.update(
                {
                    "cell_id": _cell_id(cell),
                    "grid_kind": "earthmesh_cell_preview",
                    "corridor_source_geometry": "earthmesh_cell_intersection_preview",
                    "cell_area_deg2": cell_area_deg2,
                    "intersection_area_deg2": intersection_area_deg2,
                    "overlap_class": river_class,
                    "overlap_fraction": river_fraction,
                    "domain_clip_applied": domain_geometry is not None,
                }
            )
            if _is_river_class(river_class):
                output_properties["river_class"] = river_class
                output_properties["river_fraction"] = river_fraction
                output_properties.update(_normalized_area_properties(cell_properties, river_fraction, unit_sphere_area=unit_sphere_area))
            else:
                output_properties["mask_class"] = river_class
                if river_class == "COAST":
                    output_properties["coastal_fraction"] = river_fraction
            features.append({"type": "Feature", "geometry": cell_geometry, "properties": output_properties})
    return {"type": "FeatureCollection", "features": features}


def write_earthmesh_intersection_geojson(
    corridor_geojson: str | Path,
    output_geojson: str | Path,
    *,
    cell_geojson: str | Path | None = None,
    mpas_mesh: str | Path | None = None,
    domain_geojson: str | Path | None = None,
    bbox: tuple[float, float, float, float] | None = None,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    min_fraction: float = 0.0,
    max_cells: int | None = None,
    unit_sphere_area: bool = False,
) -> dict[str, object]:
    if (cell_geojson is None) == (mpas_mesh is None):
        raise ValueError("provide exactly one of cell_geojson or mpas_mesh")
    corridors = json.loads(Path(corridor_geojson).read_text())
    if cell_geojson is not None:
        cells = json.loads(Path(cell_geojson).read_text())
    else:
        cells = read_mpas_cell_polygons(mpas_mesh, bbox=bbox, max_cells=max_cells)
    domain = json.loads(Path(domain_geojson).read_text()) if domain_geojson is not None else None
    intersections = earthmesh_cells_to_corridor_intersections(
        cells,
        corridors,
        domain=domain,
        include_classes=include_classes,
        min_fraction=min_fraction,
        unit_sphere_area=unit_sphere_area,
    )
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(intersections, indent=2, sort_keys=True) + "\n")
    return intersections


def _polygon_rings_from_geometry(geometry: dict[str, object]) -> list[list[list[float]]]:
    geometry_type = geometry.get("type")
    coordinates = geometry.get("coordinates", [])
    if geometry_type == "Polygon":
        return [coordinates[0]] if coordinates else []
    if geometry_type == "MultiPolygon":
        rings: list[list[list[float]]] = []
        for polygon in coordinates:
            if polygon:
                rings.append(polygon[0])
        return rings
    return []


def _load_feature_collection(path: str | Path | None) -> dict[str, object]:
    if path is None:
        return {"type": "FeatureCollection", "features": []}
    return json.loads(Path(path).read_text())


def write_earthmesh_cell_preview_png(
    overlap_geojson: str | Path,
    output_png: str | Path,
    *,
    background_cell_geojson: str | Path | None = None,
    title: str = "EarthMesh river-cell preview",
) -> None:
    """Render all EarthMesh cells in gray and river-overlap cells in class colors."""

    import matplotlib.pyplot as plt
    from matplotlib.patches import Patch
    from matplotlib.patches import Polygon as MatplotlibPolygon

    background = _load_feature_collection(background_cell_geojson)
    overlap = _load_feature_collection(overlap_geojson)
    background_features = [feature for feature in background.get("features", []) if isinstance(feature, dict)]
    overlap_features = [feature for feature in overlap.get("features", []) if isinstance(feature, dict)]
    colors = {"R2": (0.96, 0.62, 0.04, 0.42), "R3": (0.86, 0.15, 0.15, 0.50)}
    edges = {"R2": "#92400e", "R3": "#7f1d1d"}
    points: list[tuple[float, float]] = []

    fig, ax = plt.subplots(figsize=(9.5, 8), dpi=180)
    for feature in background_features:
        geometry = feature.get("geometry", {})
        if not isinstance(geometry, dict):
            continue
        for ring in _polygon_rings_from_geometry(geometry):
            points.extend((float(lon), float(lat)) for lon, lat in ring)
            ax.add_patch(
                MatplotlibPolygon(
                    ring,
                    closed=True,
                    facecolor=(0.88, 0.90, 0.94, 0.55),
                    edgecolor="#AEB7C8",
                    linewidth=0.22,
                    zorder=1,
                )
            )

    for feature in overlap_features:
        geometry = feature.get("geometry", {})
        properties = feature.get("properties", {})
        if not isinstance(geometry, dict) or not isinstance(properties, dict):
            continue
        river_class = str(properties.get("river_class", ""))
        for ring in _polygon_rings_from_geometry(geometry):
            points.extend((float(lon), float(lat)) for lon, lat in ring)
            ax.add_patch(
                MatplotlibPolygon(
                    ring,
                    closed=True,
                    facecolor=colors.get(river_class, (0.15, 0.39, 0.92, 0.35)),
                    edgecolor=edges.get(river_class, "#2563eb"),
                    linewidth=0.35,
                    zorder=3 if river_class == "R3" else 2,
                )
            )

    if points:
        lons = [point[0] for point in points]
        lats = [point[1] for point in points]
        lon_pad = max((max(lons) - min(lons)) * 0.04, 0.02)
        lat_pad = max((max(lats) - min(lats)) * 0.04, 0.02)
        ax.set_xlim(min(lons) - lon_pad, max(lons) + lon_pad)
        ax.set_ylim(min(lats) - lat_pad, max(lats) + lat_pad)
    ax.set_aspect("equal", adjustable="box")
    ax.set_xlabel("Longitude (degrees east)")
    ax.set_ylabel("Latitude (degrees north)")
    ax.grid(True, color="#E6E8F0", linewidth=0.8)
    ax.set_facecolor("#FFFFFF")
    fig.patch.set_facecolor("#FCFCFD")
    for spine in ["top", "right"]:
        ax.spines[spine].set_visible(False)
    ax.legend(
        handles=[
            Patch(facecolor=(0.88, 0.90, 0.94, 0.55), edgecolor="#AEB7C8", label="EarthMesh land/domain cell"),
            Patch(facecolor=colors["R2"], edgecolor=edges["R2"], label="R2 river-overlap cell"),
            Patch(facecolor=colors["R3"], edgecolor=edges["R3"], label="R3 river-overlap cell"),
        ],
        loc="lower left",
        frameon=True,
        facecolor="white",
        edgecolor="#D7DBE7",
    )
    fig.subplots_adjust(top=0.84, left=0.1, right=0.96, bottom=0.09)
    left = ax.get_position().x0
    fig.text(left, 0.965, title, ha="left", va="top", fontsize=14, fontweight="semibold", color="#1F2430")
    fig.text(
        left,
        0.925,
        f"Gray background: {len(background_features):,} EarthMesh cells. Overlay: {len(overlap_features):,} R2/R3 river cells.",
        ha="left",
        va="top",
        fontsize=9,
        color="#6F768A",
    )
    output_path = Path(output_png)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output_path, bbox_inches="tight", facecolor=fig.get_facecolor())
    plt.close(fig)


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
    parser.add_argument("--domain-geojson", help="Optional domain/coastline mask polygon GeoJSON used to clip corridors before cell intersection")
    parser.add_argument("--unit-sphere-area", action="store_true", help="Treat source areaCell values as unit-sphere areas and add normalized m2 estimates")
    parser.add_argument("--preview-png", help="Optional PNG preview path")
    parser.add_argument("--background-cell-geojson", help="Optional all-cell GeoJSON background for preview PNG")
    parser.add_argument("--title", default="EarthMesh river-cell intersection preview", help="Preview PNG title")
    args = parser.parse_args(argv)

    intersections = write_earthmesh_intersection_geojson(
        args.corridor_geojson,
        args.output_geojson,
        cell_geojson=args.cell_geojson,
        mpas_mesh=args.mpas_mesh,
        domain_geojson=args.domain_geojson,
        bbox=tuple(args.bbox) if args.bbox is not None else None,
        include_classes=args.classes,
        min_fraction=args.min_fraction,
        max_cells=args.max_cells,
        unit_sphere_area=args.unit_sphere_area,
    )
    if args.preview_png:
        background_geojson = args.background_cell_geojson
        if background_geojson is None and args.mpas_mesh is not None:
            background_path = Path(args.output_geojson).with_suffix(".background_cells.geojson")
            background = read_mpas_cell_polygons(args.mpas_mesh, bbox=tuple(args.bbox) if args.bbox is not None else None, max_cells=args.max_cells)
            background_path.write_text(json.dumps(background, indent=2, sort_keys=True) + "\n")
            background_geojson = background_path
        write_earthmesh_cell_preview_png(args.output_geojson, args.preview_png, background_cell_geojson=background_geojson, title=args.title)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
