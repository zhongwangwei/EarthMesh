from __future__ import annotations

import argparse
from pathlib import Path

from util.v3_core.geojson_io import write_cells_geojson
from util.v3_core.schema import CanonicalCell


def generate_bbox_grid_cells(
    bbox: tuple[float, float, float, float],
    *,
    nx: int,
    ny: int,
    cell_id_prefix: str = "cell",
) -> list[CanonicalCell]:
    if nx <= 0 or ny <= 0:
        raise ValueError("nx and ny must be positive")
    min_lon, min_lat, max_lon, max_lat = bbox
    if min_lon >= max_lon or min_lat >= max_lat:
        raise ValueError("bbox must be ordered as min_lon min_lat max_lon max_lat")

    dx = (max_lon - min_lon) / nx
    dy = (max_lat - min_lat) / ny
    cells: list[CanonicalCell] = []
    for j in range(ny):
        y0 = min_lat + j * dy
        y1 = y0 + dy
        for i in range(nx):
            x0 = min_lon + i * dx
            x1 = x0 + dx
            cell_index = j * nx + i
            cells.append(
                CanonicalCell(
                    cell_id=f"{cell_id_prefix}_{i:04d}_{j:04d}",
                    cell_index=cell_index,
                    cell_type="POLYGON",
                    center_lon=(x0 + x1) / 2.0,
                    center_lat=(y0 + y1) / 2.0,
                    area_m2=1.0,
                    vertices=[(x0, y0), (x1, y0), (x1, y1), (x0, y1)],
                    source_mesh_type="bbox_grid",
                )
            )
    return cells


def write_bbox_grid_geojson(
    bbox: tuple[float, float, float, float],
    *,
    nx: int,
    ny: int,
    output_path: str | Path,
    cell_id_prefix: str = "cell",
) -> Path:
    cells = generate_bbox_grid_cells(bbox, nx=nx, ny=ny, cell_id_prefix=cell_id_prefix)
    return write_cells_geojson(cells, output_path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate v3-compatible regular bbox grid cells as GeoJSON.")
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("MIN_LON", "MIN_LAT", "MAX_LON", "MAX_LAT"), required=True)
    parser.add_argument("--nx", type=int, required=True)
    parser.add_argument("--ny", type=int, required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--cell-id-prefix", default="cell")
    args = parser.parse_args(argv)
    write_bbox_grid_geojson(tuple(args.bbox), nx=args.nx, ny=args.ny, output_path=args.output, cell_id_prefix=args.cell_id_prefix)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
