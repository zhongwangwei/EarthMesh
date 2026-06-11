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
    fig.text(
        left,
        0.925,
        f"Static QA polygons from {len(features):,} R2/R3 point buffers. This is not a final EarthMesh mask.",
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
    args = parser.parse_args(argv)
    write_corridor_geojson(args.input_geojson, args.output_geojson, include_classes=args.classes, segments=args.segments)
    if args.preview_png:
        write_corridor_preview_png(args.output_geojson, args.preview_png, title=args.title)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
