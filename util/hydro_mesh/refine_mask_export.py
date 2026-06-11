from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence

DEFAULT_CLASS_REFINE = {"R2": 1, "R3": 2}


@dataclass(frozen=True)
class CloseMaskSpec:
    river_class: str
    refine_degree: int
    coordinates: list[tuple[float, float]]
    source_feature_index: int
    ring_index: int


def parse_class_refine(values: Sequence[str] | None) -> dict[str, int]:
    """Parse CLI class-to-refinement mappings like ``R2=1 R3=2``."""

    if not values:
        return dict(DEFAULT_CLASS_REFINE)
    mapping: dict[str, int] = {}
    for value in values:
        if "=" not in value:
            raise ValueError(f"class refinement must use CLASS=DEGREE, got {value!r}")
        river_class, degree_text = value.split("=", 1)
        river_class = river_class.strip()
        if not river_class:
            raise ValueError(f"class refinement has empty class name: {value!r}")
        try:
            degree = int(degree_text)
        except ValueError as exc:
            raise ValueError(f"refinement degree must be an integer, got {degree_text!r}") from exc
        if degree < 1:
            raise ValueError(f"refinement degree must be >= 1, got {degree}")
        mapping[river_class] = degree
    return mapping


def _feature_class(feature: dict[str, object]) -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return ""
    return str(properties.get("river_class", ""))


def _exterior_rings(geometry: dict[str, object]) -> list[list[object]]:
    geometry_type = geometry.get("type")
    coordinates = geometry.get("coordinates", [])
    if geometry_type == "Polygon" and isinstance(coordinates, list):
        if coordinates and isinstance(coordinates[0], list):
            return [coordinates[0]]
        return []
    if geometry_type == "MultiPolygon" and isinstance(coordinates, list):
        rings: list[list[object]] = []
        for polygon in coordinates:
            if isinstance(polygon, list) and polygon and isinstance(polygon[0], list):
                rings.append(polygon[0])
        return rings
    return []


def _drop_duplicate_closure(coordinates: list[tuple[float, float]]) -> list[tuple[float, float]]:
    if len(coordinates) > 1 and coordinates[0] == coordinates[-1]:
        return coordinates[:-1]
    return coordinates


def _normalize_ring(ring: Iterable[object]) -> list[tuple[float, float]]:
    coordinates: list[tuple[float, float]] = []
    for point in ring:
        if not isinstance(point, list | tuple) or len(point) < 2:
            continue
        coordinates.append((float(point[0]), float(point[1])))
    return _drop_duplicate_closure(coordinates)


def _simplify_coordinates(
    coordinates: list[tuple[float, float]],
    *,
    tolerance_deg: float,
) -> list[tuple[float, float]]:
    if tolerance_deg <= 0.0 or len(coordinates) <= 3:
        return coordinates
    try:
        from shapely.geometry import Polygon
    except ImportError:
        return coordinates

    polygon = Polygon(coordinates)
    if polygon.is_empty or not polygon.is_valid:
        return coordinates
    simplified = polygon.simplify(tolerance_deg, preserve_topology=True)
    if simplified.is_empty or simplified.geom_type != "Polygon":
        return coordinates
    simplified_coords = [(float(lon), float(lat)) for lon, lat in simplified.exterior.coords]
    simplified_coords = _drop_duplicate_closure(simplified_coords)
    if len(simplified_coords) < 3:
        return coordinates
    return simplified_coords


def _ring_area(coordinates: Sequence[tuple[float, float]]) -> float:
    if len(coordinates) < 3:
        return 0.0
    twice_area = 0.0
    for idx, (lon1, lat1) in enumerate(coordinates):
        lon2, lat2 = coordinates[(idx + 1) % len(coordinates)]
        twice_area += lon1 * lat2 - lon2 * lat1
    return abs(twice_area) / 2.0


def geojson_to_close_mask_specs(
    collection: dict[str, object],
    *,
    class_refine: dict[str, int] | None = None,
    simplify_tolerance_deg: float = 0.0,
    max_rings_per_class: int | None = None,
    max_masks_per_refine_degree: int | None = 99,
) -> list[CloseMaskSpec]:
    """Convert corridor polygon GeoJSON into EarthMesh close-mask specs.

    EarthMesh closes the curve internally by connecting point ``i`` to
    ``mod(i, close_num) + 1``.  Specs therefore store exterior rings without the
    duplicate final GeoJSON closure coordinate.
    """

    refine_by_class = class_refine or DEFAULT_CLASS_REFINE
    candidates: list[tuple[float, CloseMaskSpec]] = []
    features = collection.get("features", [])
    if not isinstance(features, list):
        return []

    for feature_index, feature in enumerate(features):
        if not isinstance(feature, dict):
            continue
        river_class = _feature_class(feature)
        if river_class not in refine_by_class:
            continue
        geometry = feature.get("geometry", {})
        if not isinstance(geometry, dict):
            continue
        for ring_index, ring in enumerate(_exterior_rings(geometry)):
            coordinates = _normalize_ring(ring)
            coordinates = _simplify_coordinates(coordinates, tolerance_deg=simplify_tolerance_deg)
            if len(coordinates) < 3:
                continue
            spec = CloseMaskSpec(
                river_class=river_class,
                refine_degree=refine_by_class[river_class],
                coordinates=coordinates,
                source_feature_index=feature_index,
                ring_index=ring_index,
            )
            candidates.append((_ring_area(coordinates), spec))

    candidates.sort(key=lambda item: (-item[0], item[1].river_class, item[1].source_feature_index, item[1].ring_index))
    emitted_by_class: dict[str, int] = {}
    emitted_by_refine_degree: dict[int, int] = {}
    specs: list[CloseMaskSpec] = []
    for _, spec in candidates:
        if max_rings_per_class is not None and emitted_by_class.get(spec.river_class, 0) >= max_rings_per_class:
            continue
        if (
            max_masks_per_refine_degree is not None
            and emitted_by_refine_degree.get(spec.refine_degree, 0) >= max_masks_per_refine_degree
        ):
            continue
        specs.append(spec)
        emitted_by_class[spec.river_class] = emitted_by_class.get(spec.river_class, 0) + 1
        emitted_by_refine_degree[spec.refine_degree] = emitted_by_refine_degree.get(spec.refine_degree, 0) + 1
    specs.sort(key=lambda spec: (spec.river_class, spec.source_feature_index, spec.ring_index))
    return specs


def _close_mask_text(spec: CloseMaskSpec) -> str:
    lines = [f"close_num = {len(spec.coordinates)}", f"close_refine = {spec.refine_degree}"]
    lines.extend(f"{lon:.8f} {lat:.8f}" for lon, lat in spec.coordinates)
    return "\n".join(lines) + "\n"


def write_close_mask_specs(specs: Sequence[CloseMaskSpec], output_prefix: str | Path) -> list[Path]:
    prefix = Path(output_prefix)
    prefix.parent.mkdir(parents=True, exist_ok=True)
    for stale_path in prefix.parent.glob(f"{prefix.name}_*.nml"):
        stale_path.unlink()
    counts_by_class: dict[str, int] = {}
    paths: list[Path] = []
    for spec in specs:
        counts_by_class[spec.river_class] = counts_by_class.get(spec.river_class, 0) + 1
        path = prefix.with_name(f"{prefix.name}_{spec.river_class}_{counts_by_class[spec.river_class]:03d}.nml")
        path.write_text(_close_mask_text(spec))
        paths.append(path)
    return paths


def write_close_mask_nmls(
    input_geojson: str | Path,
    output_prefix: str | Path,
    *,
    class_refine: dict[str, int] | None = None,
    simplify_tolerance_deg: float = 0.0,
    max_rings_per_class: int | None = None,
    max_masks_per_refine_degree: int | None = 99,
) -> list[Path]:
    collection = json.loads(Path(input_geojson).read_text())
    specs = geojson_to_close_mask_specs(
        collection,
        class_refine=class_refine,
        simplify_tolerance_deg=simplify_tolerance_deg,
        max_rings_per_class=max_rings_per_class,
        max_masks_per_refine_degree=max_masks_per_refine_degree,
    )
    return write_close_mask_specs(specs, output_prefix)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Export CaMa corridor polygons as EarthMesh close refinement mask .nml files."
    )
    parser.add_argument("input_geojson", help="Corridor Polygon/MultiPolygon GeoJSON")
    parser.add_argument("output_prefix", help="Output file prefix used by RL%mask_refine_spc_fprefix")
    parser.add_argument(
        "--class-refine",
        nargs="+",
        default=None,
        metavar="CLASS=DEGREE",
        help="River-class refinement degrees. Default: R2=1 R3=2",
    )
    parser.add_argument(
        "--simplify-tolerance-deg",
        type=float,
        default=0.0,
        help="Optional Shapely polygon simplification tolerance in degrees before writing rings.",
    )
    parser.add_argument(
        "--max-rings-per-class",
        type=int,
        default=None,
        help="Optional smoke-test cap on number of close masks emitted per river class.",
    )
    parser.add_argument(
        "--max-masks-per-refine-degree",
        type=int,
        default=99,
        help="Compatibility cap for EarthMesh I2.2 close-mask temp numbering. Default: 99.",
    )
    args = parser.parse_args(argv)

    paths = write_close_mask_nmls(
        args.input_geojson,
        args.output_prefix,
        class_refine=parse_class_refine(args.class_refine),
        simplify_tolerance_deg=args.simplify_tolerance_deg,
        max_rings_per_class=args.max_rings_per_class,
        max_masks_per_refine_degree=args.max_masks_per_refine_degree,
    )
    summary = {
        "output_prefix": str(args.output_prefix),
        "files_written": len(paths),
        "files": [str(path) for path in paths],
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
