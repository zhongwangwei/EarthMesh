from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from numbers import Integral
from pathlib import Path
from typing import Iterable, Iterator

_MASK_PRIORITY = {"LAND": 0, "OCEAN": 0, "BACKGROUND": 0, "COAST": 10, "R2": 20, "R3": 30}
_SURFACE_CLASSES = {"LAND", "OCEAN"}


@dataclass(frozen=True)
class _SurfaceLookup:
    geometries: list[object]
    classes: list[str]
    tree: object | None = None
    geometry_id_to_index: dict[int, int] | None = None

    def query(self, geometry: object) -> Iterator[tuple[object, str]]:
        if self.tree is None:
            candidates: Iterable[object] = self.geometries
        else:
            candidates = self.tree.query(geometry)  # type: ignore[attr-defined]
        for candidate in candidates:
            index = self._candidate_index(candidate)
            if index is None:
                continue
            surface_geometry = self.geometries[index]
            if surface_geometry.intersects(geometry):  # type: ignore[attr-defined]
                yield surface_geometry, self.classes[index]

    def _candidate_index(self, candidate: object) -> int | None:
        if isinstance(candidate, Integral):
            index = int(candidate)
            return index if 0 <= index < len(self.geometries) else None
        if self.geometry_id_to_index is not None:
            index = self.geometry_id_to_index.get(id(candidate))
            if index is not None:
                return index
        for index, geometry in enumerate(self.geometries):
            if geometry is candidate:
                return index
        return None


def _features(collection: dict[str, object]) -> list[dict[str, object]]:
    features = collection.get("features", [])
    if not isinstance(features, list):
        return []
    return [feature for feature in features if isinstance(feature, dict)]


def _cell_id(feature: dict[str, object]) -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict) or "cell_id" not in properties:
        raise ValueError("all cell-mask features require properties.cell_id")
    return str(properties["cell_id"])


def _feature_mask_class(feature: dict[str, object], *, default: str = "BACKGROUND") -> str:
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return default
    if properties.get("river_class") in {"R2", "R3"}:
        return str(properties["river_class"])
    if properties.get("mask_class") == "COAST":
        return "COAST"
    if properties.get("mask_class") in _SURFACE_CLASSES:
        return str(properties["mask_class"])
    if properties.get("surface_class") in _SURFACE_CLASSES:
        return str(properties["surface_class"])
    return default


def _surface_class_from_coast_feature(feature: dict[str, object] | None) -> str:
    if feature is None:
        return "BACKGROUND"
    properties = feature.get("properties", {})
    if not isinstance(properties, dict):
        return "BACKGROUND"
    value = properties.get("surface_class") or properties.get("mask_class") or properties.get("coast_class")
    if value == "COAST_LAND":
        return "LAND"
    if value == "COAST_OCEAN":
        return "OCEAN"
    return "BACKGROUND"


def _copy_feature(feature: dict[str, object]) -> dict[str, object]:
    return json.loads(json.dumps(feature, sort_keys=True))


def _index_best_by_cell(features: Iterable[dict[str, object]]) -> dict[str, dict[str, object]]:
    best: dict[str, dict[str, object]] = {}
    for feature in features:
        cell_id = _cell_id(feature)
        mask_class = _feature_mask_class(feature)
        current = best.get(cell_id)
        if current is None or _MASK_PRIORITY[mask_class] > _MASK_PRIORITY[_feature_mask_class(current)]:
            best[cell_id] = feature
    return best


def _build_surface_lookup(surface_cells: dict[str, object] | None) -> _SurfaceLookup | None:
    if not surface_cells:
        return None
    try:
        from shapely.geometry import shape
    except ImportError as exc:  # pragma: no cover - optional dependency missing path
        raise RuntimeError("surface land/ocean assignment requires shapely") from exc
    geometries: list[object] = []
    classes: list[str] = []
    for feature in _features(surface_cells):
        properties = feature.get("properties", {})
        geometry = feature.get("geometry", {})
        if not isinstance(properties, dict) or not isinstance(geometry, dict):
            continue
        surface_class = properties.get("surface_class") or properties.get("mask_class")
        if surface_class not in _SURFACE_CLASSES:
            continue
        surface_shape = shape(geometry)
        if surface_shape.is_empty:
            continue
        geometries.append(surface_shape)
        classes.append(str(surface_class))
    if not geometries:
        return None
    try:
        from shapely.strtree import STRtree
    except ImportError:  # pragma: no cover - old shapely fallback
        return _SurfaceLookup(geometries=geometries, classes=classes)
    return _SurfaceLookup(
        geometries=geometries,
        classes=classes,
        tree=STRtree(geometries),
        geometry_id_to_index={id(geometry): index for index, geometry in enumerate(geometries)},
    )


def _surface_class_for_cell(base: dict[str, object], surface_lookup: _SurfaceLookup | None) -> str:
    if surface_lookup is None:
        return "BACKGROUND"
    try:
        from shapely.geometry import shape
    except ImportError as exc:  # pragma: no cover - optional dependency missing path
        raise RuntimeError("surface land/ocean assignment requires shapely") from exc

    base_geometry = base.get("geometry", {})
    if not isinstance(base_geometry, dict):
        return "BACKGROUND"
    cell_shape = shape(base_geometry)
    if cell_shape.is_empty:
        return "BACKGROUND"

    best_class = "BACKGROUND"
    best_area = 0.0
    for surface_shape, surface_class in surface_lookup.query(cell_shape):
        area = cell_shape.intersection(surface_shape).area
        if area > best_area:
            best_area = area
            best_class = str(surface_class)
    return best_class


def _merge_properties(
    base: dict[str, object],
    *,
    surface_class: str,
    coast: dict[str, object] | None,
    river: dict[str, object] | None,
    mask_class: str,
) -> dict[str, object]:
    base_properties = dict(base.get("properties", {}) if isinstance(base.get("properties"), dict) else {})
    merged = dict(base_properties)
    sources: list[str] = []
    if surface_class not in _SURFACE_CLASSES:
        surface_class = _surface_class_from_coast_feature(coast)
    if surface_class in _SURFACE_CLASSES:
        merged["surface_class"] = surface_class
        sources.append("surface")
    for source_name, overlay in [("coast", coast), ("river", river)]:
        if overlay is None:
            continue
        overlay_properties = dict(overlay.get("properties", {}) if isinstance(overlay.get("properties"), dict) else {})
        for key, value in overlay_properties.items():
            if key not in {"mask_class", "surface_class"}:
                merged[key] = value
        sources.append(source_name)
    merged["mask_class"] = mask_class
    merged["hydro_mask_class"] = mask_class
    merged["mask_priority"] = _MASK_PRIORITY[mask_class]
    merged["mask_source"] = "+".join(sources) if sources else "background"
    merged["is_hydro_masked"] = mask_class in {"COAST", "R2", "R3"}
    return merged


def merge_cell_masks(
    background_cells: dict[str, object],
    *,
    river_cells: dict[str, object] | None = None,
    coast_cells: dict[str, object] | None = None,
    surface_cells: dict[str, object] | None = None,
    background_mask_class: str = "BACKGROUND",
) -> dict[str, object]:
    """Return one explicitly masked feature for every background EarthMesh cell.

    Priority is R3 > R2 > COAST > LAND/OCEAN/BACKGROUND.  Output geometry is
    always the background EarthMesh cell geometry, so the mask is a complete cell
    table rather than a sparse overlay.
    """

    if background_mask_class not in _MASK_PRIORITY:
        raise ValueError("background_mask_class must be a known mask class")

    river_by_cell = _index_best_by_cell(_features(river_cells or {"features": []}))
    coast_by_cell = _index_best_by_cell(_features(coast_cells or {"features": []}))
    surface_lookup = _build_surface_lookup(surface_cells)

    output_features: list[dict[str, object]] = []
    for base in _features(background_cells):
        cell_id = _cell_id(base)
        river = river_by_cell.get(cell_id)
        coast = coast_by_cell.get(cell_id)
        surface_class = _surface_class_for_cell(base, surface_lookup)
        base_class = surface_class if surface_class in _SURFACE_CLASSES else background_mask_class
        candidates: list[tuple[str, dict[str, object] | None]] = [(base_class, None)]
        if coast is not None:
            candidates.append(("COAST", coast))
        if river is not None:
            candidates.append((_feature_mask_class(river), river))
        mask_class, _overlay = max(candidates, key=lambda item: _MASK_PRIORITY[item[0]])
        feature = _copy_feature(base)
        feature["properties"] = _merge_properties(base, surface_class=surface_class, coast=coast, river=river, mask_class=mask_class)
        output_features.append(feature)

    return {"type": "FeatureCollection", "features": output_features}


def _read_geojson(path: str | Path) -> dict[str, object]:
    return json.loads(Path(path).read_text())


def write_complete_cell_mask_geojson(
    background_geojson: str | Path,
    output_geojson: str | Path,
    *,
    river_geojson: str | Path | None = None,
    coast_geojson: str | Path | None = None,
    surface_geojson: str | Path | None = None,
) -> dict[str, object]:
    collection = merge_cell_masks(
        _read_geojson(background_geojson),
        river_cells=_read_geojson(river_geojson) if river_geojson else None,
        coast_cells=_read_geojson(coast_geojson) if coast_geojson else None,
        surface_cells=_read_geojson(surface_geojson) if surface_geojson else None,
    )
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(collection, indent=2, sort_keys=True) + "\n")
    return collection


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Merge sparse river/coast overlays into a complete EarthMesh cell mask GeoJSON.")
    parser.add_argument("background_geojson", help="All EarthMesh cells in the QA/domain window")
    parser.add_argument("output_geojson", help="Output complete cell mask GeoJSON")
    parser.add_argument("--river-geojson", help="Sparse R2/R3 EarthMesh cell overlap GeoJSON")
    parser.add_argument("--coast-geojson", help="Sparse COAST EarthMesh cell overlap GeoJSON")
    parser.add_argument("--surface-geojson", help="LAND/OCEAN surface mask GeoJSON used to classify non-hydro cells")
    args = parser.parse_args(argv)
    write_complete_cell_mask_geojson(
        args.background_geojson,
        args.output_geojson,
        river_geojson=args.river_geojson,
        coast_geojson=args.coast_geojson,
        surface_geojson=args.surface_geojson,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
