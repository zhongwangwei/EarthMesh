from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Mapping

from util.v3_core.geometry import MaskFeature, Polygon, polygon_area
from util.v3_core.schema import CanonicalCell

_MASK_PRIORITY = {
    "UNKNOWN": 0,
    "LAND": 1,
    "OCEAN": 1,
    "COAST": 10,
    "COAST_LAND": 10,
    "COAST_OCEAN": 10,
    "SHELF": 10,
    "ESTUARY": 25,
    "DELTA": 25,
    "R0": 5,
    "R1": 10,
    "R2": 20,
    "R3": 30,
}


def load_cells_geojson(path: str | Path) -> list[CanonicalCell]:
    return geojson_cells_to_canonical(json.loads(Path(path).read_text()))


def load_masks_geojson(path: str | Path) -> list[MaskFeature]:
    return geojson_masks_to_features(json.loads(Path(path).read_text()))


def write_cells_geojson(cells: list[CanonicalCell], path: str | Path) -> Path:
    output_path = Path(path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(canonical_cells_to_geojson(cells), indent=2, sort_keys=True) + "\n")
    return output_path


def canonical_cells_to_geojson(cells: list[CanonicalCell]) -> dict[str, object]:
    return {
        "type": "FeatureCollection",
        "features": [_canonical_cell_to_feature(cell) for cell in cells],
    }


def geojson_cells_to_canonical(collection: Mapping[str, Any]) -> list[CanonicalCell]:
    cells: list[CanonicalCell] = []
    for index, feature in enumerate(_features(collection)):
        properties = _properties(feature)
        vertices = _polygon_vertices(feature)
        cell_id = str(properties.get("cell_id") or properties.get("id") or f"cell-{index}")
        area_m2 = float(properties.get("area_m2") or properties.get("source_areaCell") or polygon_area(vertices))
        center_lon = float(properties.get("center_lon") or properties.get("lon") or _mean_coordinate(vertices, 0))
        center_lat = float(properties.get("center_lat") or properties.get("lat") or _mean_coordinate(vertices, 1))
        cells.append(
            CanonicalCell(
                cell_id=cell_id,
                cell_index=int(properties.get("cell_index", index)),
                cell_type=str(properties.get("cell_type", "POLYGON")),
                center_lon=center_lon,
                center_lat=center_lat,
                area_m2=area_m2,
                vertices=vertices,
                surface_class=str(properties.get("surface_class", "UNKNOWN")),
                hydro_class=str(properties.get("hydro_class", "NONE")),
                coast_class=str(properties.get("coast_class", "NONE")),
                mesh_priority=int(properties.get("mesh_priority", 0)),
                geometry_ref=str(properties.get("geometry_ref", "")),
                source_mesh_type=str(properties.get("source_mesh_type", "geojson")),
            )
        )
    return cells


def _canonical_cell_to_feature(cell: CanonicalCell) -> dict[str, object]:
    ring = [[lon, lat] for lon, lat in cell.vertices]
    if ring and ring[0] != ring[-1]:
        ring.append(list(ring[0]))
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [ring]},
        "properties": {
            "cell_id": cell.cell_id,
            "cell_index": cell.cell_index,
            "cell_type": cell.cell_type,
            "center_lon": cell.center_lon,
            "center_lat": cell.center_lat,
            "area_m2": cell.area_m2,
            "surface_class": cell.surface_class,
            "hydro_class": cell.hydro_class,
            "coast_class": cell.coast_class,
            "mesh_priority": cell.mesh_priority,
            "source_fractions": cell.source_fractions,
            "quality_flags": cell.quality_flags,
            "geometry_ref": cell.geometry_ref,
            "source_mesh_type": cell.source_mesh_type,
        },
    }


def geojson_masks_to_features(collection: Mapping[str, Any]) -> list[MaskFeature]:
    masks: list[MaskFeature] = []
    for index, feature in enumerate(_features(collection)):
        properties = _properties(feature)
        mask_class = _mask_class(properties)
        feature_id = str(properties.get("feature_id") or properties.get("reach_id") or properties.get("cell_id") or f"mask-{index}")
        priority = int(properties.get("priority", _MASK_PRIORITY.get(mask_class, 0)))
        masks.append(
            MaskFeature(
                feature_id=feature_id,
                mask_class=mask_class,
                priority=priority,
                polygon=_polygon_vertices(feature),
                properties=dict(properties),
            )
        )
    return masks


def _features(collection: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    if collection.get("type") != "FeatureCollection":
        raise ValueError("GeoJSON input must be a FeatureCollection")
    features = collection.get("features")
    if not isinstance(features, list):
        raise ValueError("GeoJSON FeatureCollection requires a features list")
    return [feature for feature in features if isinstance(feature, Mapping)]


def _properties(feature: Mapping[str, Any]) -> Mapping[str, Any]:
    properties = feature.get("properties", {})
    if not isinstance(properties, Mapping):
        return {}
    return properties


def _polygon_vertices(feature: Mapping[str, Any]) -> Polygon:
    geometry = feature.get("geometry")
    if not isinstance(geometry, Mapping) or geometry.get("type") != "Polygon":
        raise ValueError("v3 GeoJSON bridge currently requires Polygon geometries")
    coordinates = geometry.get("coordinates")
    if not isinstance(coordinates, list) or not coordinates or not isinstance(coordinates[0], list):
        raise ValueError("Polygon geometry requires an exterior ring")
    vertices = [(float(point[0]), float(point[1])) for point in coordinates[0]]
    if len(vertices) >= 2 and vertices[0] == vertices[-1]:
        vertices = vertices[:-1]
    return vertices


def _mask_class(properties: Mapping[str, Any]) -> str:
    for key in ["river_class", "hydro_class", "mask_class", "surface_class", "coast_class"]:
        value = properties.get(key)
        if value:
            mask_class = str(value)
            if mask_class == "BACKGROUND":
                return "UNKNOWN"
            return mask_class
    return "UNKNOWN"


def _mean_coordinate(vertices: Polygon, axis: int) -> float:
    return sum(point[axis] for point in vertices) / len(vertices)
