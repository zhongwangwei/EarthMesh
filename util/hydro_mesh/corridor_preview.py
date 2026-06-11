from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Iterable

_INCLUDED_CLASSES = {"R2", "R3"}
_METERS_PER_DEGREE_LAT = 111_320.0


def _radius_for_feature(feature: dict[str, object]) -> float:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        properties = {}
    river_class = str(properties.get("river_class", ""))
    width_m = float(properties.get("width_m", 0.0) or 0.0)
    if river_class == "R3":
        return max(1_800.0, width_m)
    if river_class == "R2":
        return max(700.0, min(1_500.0, width_m * 2.0 if width_m > 0 else 700.0))
    return 0.0


def _circle_ring(lon: float, lat: float, radius_m: float, *, segments: int) -> list[list[float]]:
    if segments < 8:
        raise ValueError("segments must be at least 8")
    lat_radius = radius_m / _METERS_PER_DEGREE_LAT
    lon_scale = max(math.cos(math.radians(lat)), 1e-6)
    lon_radius = radius_m / (_METERS_PER_DEGREE_LAT * lon_scale)
    ring: list[list[float]] = []
    for idx in range(segments):
        angle = 2.0 * math.pi * idx / segments
        ring.append([lon + lon_radius * math.cos(angle), lat + lat_radius * math.sin(angle)])
    ring.append(ring[0])
    return ring


def geojson_points_to_corridors(
    collection: dict[str, object],
    *,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    segments: int = 24,
) -> dict[str, object]:
    """Convert classified point features into approximate corridor-buffer polygons for QA."""

    included = set(include_classes)
    corridors: list[dict[str, object]] = []
    for feature in collection.get("features", []):
        if not isinstance(feature, dict):
            continue
        geometry = feature.get("geometry", {})
        properties = feature.get("properties", {})
        if not isinstance(geometry, dict) or not isinstance(properties, dict):
            continue
        if geometry.get("type") != "Point":
            continue
        river_class = str(properties.get("river_class", ""))
        if river_class not in included:
            continue
        coordinates = geometry.get("coordinates", [])
        if not isinstance(coordinates, list | tuple) or len(coordinates) < 2:
            continue
        lon = float(coordinates[0])
        lat = float(coordinates[1])
        radius_m = _radius_for_feature(feature)
        output_properties = dict(properties)
        output_properties.update(
            {
                "corridor_kind": "preview_buffer",
                "corridor_radius_m": radius_m,
                "corridor_source_geometry": "point_circle",
            }
        )
        corridors.append(
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [_circle_ring(lon, lat, radius_m, segments=segments)]},
                "properties": output_properties,
            }
        )
    return {"type": "FeatureCollection", "features": corridors}


def write_corridor_geojson(
    input_geojson: str | Path,
    output_geojson: str | Path,
    *,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    segments: int = 24,
) -> dict[str, object]:
    collection = json.loads(Path(input_geojson).read_text())
    corridors = geojson_points_to_corridors(collection, include_classes=include_classes, segments=segments)
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(corridors, indent=2, sort_keys=True) + "\n")
    return corridors


def _feature_point(feature: dict[str, object]) -> tuple[float, float] | None:
    geometry = feature.get("geometry", {})
    if not isinstance(geometry, dict) or geometry.get("type") != "Point":
        return None
    coordinates = geometry.get("coordinates", [])
    if not isinstance(coordinates, list | tuple) or len(coordinates) < 2:
        return None
    return float(coordinates[0]), float(coordinates[1])


def _feature_class(feature: dict[str, object]) -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return ""
    return str(properties.get("river_class", ""))


def _feature_reach_id(feature: dict[str, object]) -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return ""
    return str(properties.get("reach_id", ""))


def _distance_km(a: tuple[float, float], b: tuple[float, float]) -> float:
    mid_lat = math.radians((a[1] + b[1]) / 2.0)
    dx = (b[0] - a[0]) * _METERS_PER_DEGREE_LAT * math.cos(mid_lat)
    dy = (b[1] - a[1]) * _METERS_PER_DEGREE_LAT
    return math.hypot(dx, dy) / 1000.0


def _segment_ring(
    start: tuple[float, float],
    end: tuple[float, float],
    radius_m: float,
) -> list[list[float]]:
    mid_lat = math.radians((start[1] + end[1]) / 2.0)
    lon_scale = max(math.cos(mid_lat), 1e-6)
    sx = start[0] * _METERS_PER_DEGREE_LAT * lon_scale
    sy = start[1] * _METERS_PER_DEGREE_LAT
    ex = end[0] * _METERS_PER_DEGREE_LAT * lon_scale
    ey = end[1] * _METERS_PER_DEGREE_LAT
    dx = ex - sx
    dy = ey - sy
    length = math.hypot(dx, dy)
    if length == 0:
        return _circle_ring(start[0], start[1], radius_m, segments=12)
    px = -dy / length * radius_m
    py = dx / length * radius_m

    def to_lonlat(x: float, y: float) -> list[float]:
        return [x / (_METERS_PER_DEGREE_LAT * lon_scale), y / _METERS_PER_DEGREE_LAT]

    ring = [
        to_lonlat(sx + px, sy + py),
        to_lonlat(ex + px, ey + py),
        to_lonlat(ex - px, ey - py),
        to_lonlat(sx - px, sy - py),
    ]
    ring.append(ring[0])
    return ring


def geojson_points_to_neighbor_corridors(
    collection: dict[str, object],
    *,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    max_link_distance_km: float = 3.0,
    max_radius_m: float = 2_500.0,
) -> dict[str, object]:
    """Create fallback corridor polygons by linking nearest same-class point candidates."""

    included = set(include_classes)
    candidates: list[dict[str, object]] = []
    for feature in collection.get("features", []):
        if not isinstance(feature, dict):
            continue
        if _feature_class(feature) not in included:
            continue
        if _feature_point(feature) is None:
            continue
        candidates.append(feature)

    links: set[tuple[str, str]] = set()
    corridor_features: list[dict[str, object]] = []
    for feature in candidates:
        start = _feature_point(feature)
        if start is None:
            continue
        feature_class = _feature_class(feature)
        reach_id = _feature_reach_id(feature)
        nearest: tuple[float, dict[str, object], tuple[float, float]] | None = None
        for other in candidates:
            other_reach_id = _feature_reach_id(other)
            if other is feature or other_reach_id == reach_id or _feature_class(other) != feature_class:
                continue
            end = _feature_point(other)
            if end is None:
                continue
            distance = _distance_km(start, end)
            if distance > max_link_distance_km:
                continue
            if nearest is None or distance < nearest[0] or (distance == nearest[0] and other_reach_id < _feature_reach_id(nearest[1])):
                nearest = (distance, other, end)
        if nearest is None:
            continue
        other = nearest[1]
        end = nearest[2]
        other_reach_id = _feature_reach_id(other)
        link_key = tuple(sorted((reach_id, other_reach_id)))
        if link_key in links:
            continue
        links.add(link_key)
        radius_m = min(max_radius_m, max(_radius_for_feature(feature), _radius_for_feature(other)))
        corridor_features.append(
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [_segment_ring(start, end, radius_m)]},
                "properties": {
                    "corridor_kind": "preview_buffer",
                    "corridor_source_geometry": "nearest_neighbor_segment",
                    "river_class": feature_class,
                    "from_reach_id": reach_id,
                    "to_reach_id": other_reach_id,
                    "link_distance_km": nearest[0],
                    "corridor_radius_m": radius_m,
                },
            }
        )
    return {"type": "FeatureCollection", "features": corridor_features}


def write_neighbor_corridor_geojson(
    input_geojson: str | Path,
    output_geojson: str | Path,
    *,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    max_link_distance_km: float = 3.0,
    max_radius_m: float = 2_500.0,
) -> dict[str, object]:
    collection = json.loads(Path(input_geojson).read_text())
    corridors = geojson_points_to_neighbor_corridors(
        collection,
        include_classes=include_classes,
        max_link_distance_km=max_link_distance_km,
        max_radius_m=max_radius_m,
    )
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(corridors, indent=2, sort_keys=True) + "\n")
    return corridors


def _feature_index(feature: dict[str, object]) -> tuple[int, int] | None:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return None
    if "x_index" not in properties or "y_index" not in properties:
        return None
    return int(properties["x_index"]), int(properties["y_index"])


def _feature_downstream_index(feature: dict[str, object]) -> tuple[int, int] | None:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return None
    downstream_x = int(properties.get("downstream_x", -9999))
    downstream_y = int(properties.get("downstream_y", -9999))
    if downstream_x < 0 or downstream_y < 0:
        return None
    return downstream_x, downstream_y


def geojson_points_to_downstream_corridors(
    collection: dict[str, object],
    *,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    max_radius_m: float = 2_500.0,
) -> dict[str, object]:
    """Create corridor polygons from explicit CaMa downstream index links."""

    included = set(include_classes)
    candidates: dict[tuple[int, int], dict[str, object]] = {}
    for feature in collection.get("features", []):
        if not isinstance(feature, dict):
            continue
        if _feature_class(feature) not in included or _feature_point(feature) is None:
            continue
        index = _feature_index(feature)
        if index is not None:
            candidates[index] = feature

    corridor_features: list[dict[str, object]] = []
    for index in sorted(candidates):
        feature = candidates[index]
        downstream_index = _feature_downstream_index(feature)
        if downstream_index is None or downstream_index not in candidates or downstream_index == index:
            continue
        start = _feature_point(feature)
        end = _feature_point(candidates[downstream_index])
        if start is None or end is None:
            continue
        downstream = candidates[downstream_index]
        radius_m = min(max_radius_m, max(_radius_for_feature(feature), _radius_for_feature(downstream)))
        feature_class = _feature_class(feature)
        corridor_features.append(
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [_segment_ring(start, end, radius_m)]},
                "properties": {
                    "corridor_kind": "preview_buffer",
                    "corridor_source_geometry": "cama_downstream_segment",
                    "river_class": feature_class,
                    "from_reach_id": _feature_reach_id(feature),
                    "to_reach_id": _feature_reach_id(downstream),
                    "from_x_index": index[0],
                    "from_y_index": index[1],
                    "to_x_index": downstream_index[0],
                    "to_y_index": downstream_index[1],
                    "link_distance_km": _distance_km(start, end),
                    "corridor_radius_m": radius_m,
                },
            }
        )
    return {"type": "FeatureCollection", "features": corridor_features}


def write_downstream_corridor_geojson(
    input_geojson: str | Path,
    output_geojson: str | Path,
    *,
    include_classes: Iterable[str] = _INCLUDED_CLASSES,
    max_radius_m: float = 2_500.0,
) -> dict[str, object]:
    collection = json.loads(Path(input_geojson).read_text())
    corridors = geojson_points_to_downstream_corridors(collection, include_classes=include_classes, max_radius_m=max_radius_m)
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(corridors, indent=2, sort_keys=True) + "\n")
    return corridors


def preview_geometry_label(features: list[dict[str, object]]) -> str:
    source_geometries = {
        str(feature.get("properties", {}).get("corridor_source_geometry", "corridor"))
        for feature in features
        if isinstance(feature.get("properties", {}), dict)
    }
    if "cama_downstream_segment" in source_geometries:
        return "CaMa downstream segment buffers"
    if "nearest_neighbor_segment" in source_geometries:
        return "nearest-neighbor segment buffers"
    return "point buffers"


def write_corridor_preview_png(input_geojson: str | Path, output_png: str | Path, *, title: str = "Hydro corridor preview") -> None:
    """Render preview corridor polygons as a static PNG for quick QA."""

    import matplotlib.pyplot as plt
    from matplotlib.patches import Polygon as MatplotlibPolygon
    from matplotlib.patches import Patch

    collection = json.loads(Path(input_geojson).read_text())
    features = [feature for feature in collection.get("features", []) if isinstance(feature, dict)]
    colors = {"R2": (0.96, 0.62, 0.04, 0.30), "R3": (0.86, 0.15, 0.15, 0.38)}
    edges = {"R2": "#92400e", "R3": "#7f1d1d"}
    order = {"R2": 0, "R3": 1}
    points: list[tuple[float, float]] = []

    fig, ax = plt.subplots(figsize=(9.5, 8), dpi=180)
    for feature in sorted(features, key=lambda item: order.get(str(item.get("properties", {}).get("river_class", "")), 99)):
        geometry = feature.get("geometry", {})
        properties = feature.get("properties", {})
        if not isinstance(geometry, dict) or not isinstance(properties, dict):
            continue
        if geometry.get("type") != "Polygon":
            continue
        rings = geometry.get("coordinates", [])
        if not rings:
            continue
        ring = rings[0]
        river_class = str(properties.get("river_class", ""))
        points.extend((float(lon), float(lat)) for lon, lat in ring)
        patch = MatplotlibPolygon(
            ring,
            closed=True,
            facecolor=colors.get(river_class, (0.15, 0.39, 0.92, 0.25)),
            edgecolor=edges.get(river_class, "#2563eb"),
            linewidth=0.25,
            zorder=2 if river_class == "R2" else 3,
        )
        ax.add_patch(patch)

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
            Patch(facecolor=colors["R2"], edgecolor=edges["R2"], label="R2 preview corridor"),
            Patch(facecolor=colors["R3"], edgecolor=edges["R3"], label="R3 preview corridor"),
        ],
        loc="lower left",
        frameon=True,
        facecolor="white",
        edgecolor="#D7DBE7",
    )
    fig.subplots_adjust(top=0.84, left=0.1, right=0.96, bottom=0.09)
    left = ax.get_position().x0
    fig.text(left, 0.965, title, ha="left", va="top", fontsize=14, fontweight="semibold", color="#1F2430")
    geometry_label = preview_geometry_label(features)
    fig.text(
        left,
        0.925,
        f"Static QA polygons from {len(features):,} R2/R3 {geometry_label}. This is not a final EarthMesh mask.",
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
    parser = argparse.ArgumentParser(description="Create preview river-corridor polygons from classified R2/R3 point GeoJSON.")
    parser.add_argument("input_geojson", help="Input classified point GeoJSON")
    parser.add_argument("output_geojson", help="Output preview corridor polygon GeoJSON")
    parser.add_argument("--classes", nargs="+", default=sorted(_INCLUDED_CLASSES), help="Classes to buffer; default: R2 R3")
    parser.add_argument("--segments", type=int, default=24, help="Circle segments per point; default: 24")
    parser.add_argument("--preview-png", help="Optional PNG preview path for the corridor polygons")
    parser.add_argument("--title", default="Hydro corridor preview", help="Preview PNG title")
    parser.add_argument("--neighbor-links", action="store_true", help="Create nearest-neighbor segment corridors instead of point circles")
    parser.add_argument("--downstream-links", action="store_true", help="Create corridors from explicit CaMa downstream indices")
    parser.add_argument("--max-link-distance-km", type=float, default=3.0, help="Maximum distance for nearest-neighbor links")
    parser.add_argument("--max-radius-m", type=float, default=2_500.0, help="Maximum preview corridor radius")
    args = parser.parse_args(argv)
    if args.downstream_links and args.neighbor_links:
        raise ValueError("choose only one of --downstream-links or --neighbor-links")
    if args.downstream_links:
        write_downstream_corridor_geojson(
            args.input_geojson,
            args.output_geojson,
            include_classes=args.classes,
            max_radius_m=args.max_radius_m,
        )
    elif args.neighbor_links:
        write_neighbor_corridor_geojson(
            args.input_geojson,
            args.output_geojson,
            include_classes=args.classes,
            max_link_distance_km=args.max_link_distance_km,
            max_radius_m=args.max_radius_m,
        )
    else:
        write_corridor_geojson(args.input_geojson, args.output_geojson, include_classes=args.classes, segments=args.segments)
    if args.preview_png:
        write_corridor_preview_png(args.output_geojson, args.preview_png, title=args.title)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
