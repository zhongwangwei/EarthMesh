from __future__ import annotations

import argparse
import json
from pathlib import Path

from util.hydro_mesh.cama_binary import CamaGridSpec, read_binary_window
from util.hydro_mesh.cama_sample import grid_from_params_file
from util.hydro_mesh.coastal_band import land_mask_from_elevation


def _cell_polygon(grid: CamaGridSpec, x_index: int, y_index: int) -> list[list[float]]:
    west = grid.west + x_index * grid.grid_size_deg
    east = west + grid.grid_size_deg
    south = grid.south + y_index * grid.grid_size_deg
    north = south + grid.grid_size_deg
    return [[west, south], [east, south], [east, north], [west, north], [west, south]]


def _feature_collection(features: list[dict[str, object]]) -> dict[str, object]:
    return {"type": "FeatureCollection", "features": features}


def _json_safe(collection: dict[str, object]) -> dict[str, object]:
    return json.loads(json.dumps(collection, sort_keys=True))


def surface_mask_geojson(
    grid: CamaGridSpec,
    *,
    x_start: int,
    y_start: int,
    land_mask: list[list[bool]],
    dissolve: bool = True,
) -> dict[str, object]:
    """Convert a CaMa land mask window into LAND/OCEAN GeoJSON polygons."""

    if not land_mask or any(len(row) != len(land_mask[0]) for row in land_mask):
        raise ValueError("land_mask must be a non-empty rectangular grid")

    cells: list[tuple[int, int, str]] = []
    for row_offset, row in enumerate(land_mask):
        for col_offset, is_land in enumerate(row):
            cells.append((x_start + col_offset, y_start + row_offset, "LAND" if is_land else "OCEAN"))

    if dissolve:
        try:
            from shapely.geometry import box, mapping
            from shapely.ops import unary_union
        except ImportError as exc:  # pragma: no cover - optional dependency missing path
            raise RuntimeError("surface mask dissolve requires shapely") from exc
        features: list[dict[str, object]] = []
        for surface_class in ["LAND", "OCEAN"]:
            polygons = []
            for x_index, y_index, cell_class in cells:
                if cell_class != surface_class:
                    continue
                west = grid.west + x_index * grid.grid_size_deg
                south = grid.south + y_index * grid.grid_size_deg
                polygons.append(box(west, south, west + grid.grid_size_deg, south + grid.grid_size_deg))
            if not polygons:
                continue
            dissolved = unary_union(polygons)
            if not dissolved.is_empty:
                features.append(
                    {
                        "type": "Feature",
                        "geometry": mapping(dissolved),
                        "properties": {
                            "surface_class": surface_class,
                            "mask_class": surface_class,
                            "surface_source_geometry": "cama_elevtn_surface_mask",
                            "source_cell_count": len(polygons),
                        },
                    }
                )
        return _json_safe(_feature_collection(features))

    features = []
    for x_index, y_index, surface_class in cells:
        features.append(
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [_cell_polygon(grid, x_index, y_index)]},
                "properties": {
                    "surface_class": surface_class,
                    "mask_class": surface_class,
                    "surface_source_geometry": "cama_elevtn_surface_mask_cell",
                    "x_index": x_index,
                    "y_index": y_index,
                },
            }
        )
    return _feature_collection(features)


def write_surface_mask_geojson(
    map_dir: str | Path,
    output_geojson: str | Path,
    *,
    bbox: tuple[float, float, float, float],
    y_reversed_storage: bool = True,
    dissolve: bool = True,
    undef: float = -9999.0,
) -> dict[str, object]:
    root = Path(map_dir)
    grid = grid_from_params_file(root / "params.txt", y_reversed_storage=y_reversed_storage)
    west, south, east, north = bbox
    x_start, y_start, width, height = grid.window_for_bbox(west=west, east=east, south=south, north=north)
    elevation = read_binary_window(
        root / "elevtn.bin",
        grid,
        x_start=x_start,
        y_start=y_start,
        width=width,
        height=height,
        dtype="float32",
    )
    collection = surface_mask_geojson(
        grid,
        x_start=x_start,
        y_start=y_start,
        land_mask=land_mask_from_elevation(elevation, undef=undef),
        dissolve=dissolve,
    )
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(collection, indent=2, sort_keys=True) + "\n")
    return collection


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate a CaMa elevation-derived LAND/OCEAN surface mask GeoJSON.")
    parser.add_argument("map_dir", help="CaMa map directory containing params.txt and elevtn.bin")
    parser.add_argument("output_geojson", help="Output LAND/OCEAN surface-mask GeoJSON")
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("WEST", "SOUTH", "EAST", "NORTH"), required=True)
    parser.add_argument("--no-dissolve", action="store_true", help="Write individual 1min surface cells instead of dissolved LAND/OCEAN masks")
    parser.add_argument("--no-yrev", action="store_true", help="Disable y-reversed binary row order")
    parser.add_argument("--undef", type=float, default=-9999.0, help="CaMa elevation undef value")
    args = parser.parse_args(argv)
    write_surface_mask_geojson(
        args.map_dir,
        args.output_geojson,
        bbox=tuple(args.bbox),
        y_reversed_storage=not args.no_yrev,
        dissolve=not args.no_dissolve,
        undef=args.undef,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
