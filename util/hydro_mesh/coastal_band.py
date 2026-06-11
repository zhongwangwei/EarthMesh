from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

from util.hydro_mesh.cama_binary import CamaGridSpec, read_binary_window
from util.hydro_mesh.cama_sample import grid_from_params_file


def land_mask_from_elevation(elevation: list[list[float | int]], *, undef: float = -9999.0) -> list[list[bool]]:
    """Return True where CaMa elevation has a valid land/domain value."""

    mask: list[list[bool]] = []
    for row in elevation:
        mask.append([math.isfinite(float(value)) and float(value) != undef for value in row])
    return mask


def coastal_band_cells(
    land_mask: list[list[bool]],
    *,
    radius_cells: int = 1,
    include_land_side: bool = True,
    include_ocean_side: bool = True,
) -> list[list[bool]]:
    """Select cells within ``radius_cells`` of a land/ocean transition.

    The returned mask can include both the land-side and ocean-side cells so the
    refinement envelope straddles the coastline instead of refining only inland.
    """

    if radius_cells < 1:
        raise ValueError("radius_cells must be at least 1")
    height = len(land_mask)
    widths = [len(row) for row in land_mask]
    if height == 0 or any(width != widths[0] for width in widths):
        raise ValueError("land_mask must be a non-empty rectangular grid")
    width = widths[0]
    band = [[False for _ in range(width)] for _ in range(height)]

    for y, row in enumerate(land_mask):
        for x, is_land in enumerate(row):
            if is_land and not include_land_side:
                continue
            if not is_land and not include_ocean_side:
                continue
            found_opposite = False
            for yy in range(max(0, y - radius_cells), min(height, y + radius_cells + 1)):
                for xx in range(max(0, x - radius_cells), min(width, x + radius_cells + 1)):
                    if xx == x and yy == y:
                        continue
                    if land_mask[yy][xx] != is_land:
                        found_opposite = True
                        break
                if found_opposite:
                    break
            band[y][x] = found_opposite
    return band


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


def coastal_band_geojson(
    grid: CamaGridSpec,
    *,
    x_start: int,
    y_start: int,
    band: list[list[bool]],
    land_mask: list[list[bool]],
    dissolve: bool = True,
) -> dict[str, object]:
    """Convert a coastal-band mask to GeoJSON polygons."""

    if len(band) != len(land_mask) or [len(row) for row in band] != [len(row) for row in land_mask]:
        raise ValueError("band and land_mask must share shape")

    features: list[dict[str, object]] = []
    cells: list[tuple[int, int, bool]] = []
    for row_offset, row in enumerate(band):
        for col_offset, selected in enumerate(row):
            if selected:
                cells.append((x_start + col_offset, y_start + row_offset, land_mask[row_offset][col_offset]))

    land_count = sum(1 for _, _, is_land in cells if is_land)
    ocean_count = len(cells) - land_count
    properties = {
        "mask_class": "COAST",
        "coastal_band_cell_count": len(cells),
        "land_side_cell_count": land_count,
        "ocean_side_cell_count": ocean_count,
        "corridor_source_geometry": "cama_elevtn_coastal_band",
    }

    if dissolve and cells:
        try:
            from shapely.geometry import box, mapping
            from shapely.ops import unary_union
        except ImportError as exc:  # pragma: no cover - exercised only without optional dependency
            raise RuntimeError("coastal band dissolve requires shapely") from exc

        polygons = []
        for x_index, y_index, _ in cells:
            west = grid.west + x_index * grid.grid_size_deg
            south = grid.south + y_index * grid.grid_size_deg
            polygons.append(box(west, south, west + grid.grid_size_deg, south + grid.grid_size_deg))
        dissolved = unary_union(polygons)
        if not dissolved.is_empty:
            features.append({"type": "Feature", "geometry": mapping(dissolved), "properties": properties})
        return _json_safe(_feature_collection(features))

    for x_index, y_index, is_land in cells:
        feature_properties = dict(properties)
        feature_properties.update(
            {
                "x_index": x_index,
                "y_index": y_index,
                "coastal_side": "land" if is_land else "ocean",
            }
        )
        features.append(
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [_cell_polygon(grid, x_index, y_index)]},
                "properties": feature_properties,
            }
        )
    return _feature_collection(features)


def write_coastal_band_geojson(
    map_dir: str | Path,
    output_geojson: str | Path,
    *,
    bbox: tuple[float, float, float, float],
    radius_cells: int = 3,
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
    land_mask = land_mask_from_elevation(elevation, undef=undef)
    band = coastal_band_cells(land_mask, radius_cells=radius_cells)
    collection = coastal_band_geojson(grid, x_start=x_start, y_start=y_start, band=band, land_mask=land_mask, dissolve=dissolve)
    output_path = Path(output_geojson)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(collection, indent=2, sort_keys=True) + "\n")
    return collection


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate a CaMa elevation-derived coastal-band GeoJSON mask.")
    parser.add_argument("map_dir", help="CaMa map directory containing params.txt and elevtn.bin")
    parser.add_argument("output_geojson", help="Output coastal-band GeoJSON")
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("WEST", "SOUTH", "EAST", "NORTH"), required=True)
    parser.add_argument("--radius-cells", type=int, default=3, help="Chebyshev cell radius around land/ocean transitions")
    parser.add_argument("--no-dissolve", action="store_true", help="Write individual 1min coastal cells instead of a dissolved mask")
    parser.add_argument("--no-yrev", action="store_true", help="Disable y-reversed binary row order")
    parser.add_argument("--undef", type=float, default=-9999.0, help="CaMa elevation undef value")
    args = parser.parse_args(argv)
    write_coastal_band_geojson(
        args.map_dir,
        args.output_geojson,
        bbox=tuple(args.bbox),
        radius_cells=args.radius_cells,
        y_reversed_storage=not args.no_yrev,
        dissolve=not args.no_dissolve,
        undef=args.undef,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
