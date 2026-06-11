from __future__ import annotations

from dataclasses import dataclass

from util.hydro_mesh.cama_binary import CamaGridSpec
from util.hydro_mesh.classifier import RiverReach


@dataclass(frozen=True)
class CamaReachRecord:
    reach: RiverReach
    x_index: int
    y_index: int
    lon: float
    lat: float
    river_length_m: float
    downstream_x: int
    downstream_y: int


def _same_shape(reference: list[list[float | int]], *others: list[list[float | int]]) -> bool:
    height = len(reference)
    widths = [len(row) for row in reference]
    for other in others:
        if len(other) != height:
            return False
        if [len(row) for row in other] != widths:
            return False
    return True


def _is_valid_channel(uparea_km2: float, width_m: float, rivlen_m: float) -> bool:
    return uparea_km2 > 0.0 and width_m > 0.0 and rivlen_m > 0.0


def build_reach_inventory(
    grid: CamaGridSpec,
    *,
    x_start: int,
    y_start: int,
    target_dx_km: float,
    uparea_km2: list[list[float]],
    width_m: list[list[float]],
    rivlen_m: list[list[float]],
    next_x: list[list[int]],
    next_y: list[list[int]],
    uparea_to_km2: float = 1.0,
) -> list[CamaReachRecord]:
    if not _same_shape(uparea_km2, width_m, rivlen_m, next_x, next_y):
        raise ValueError("all CaMa window arrays must share the same shape")

    records: list[CamaReachRecord] = []
    for row_offset, row in enumerate(uparea_km2):
        for col_offset, uparea in enumerate(row):
            width = width_m[row_offset][col_offset]
            length = rivlen_m[row_offset][col_offset]
            if not _is_valid_channel(uparea, width, length):
                continue

            x_index = x_start + col_offset
            y_index = y_start + row_offset
            downstream_x = int(next_x[row_offset][col_offset])
            downstream_y = int(next_y[row_offset][col_offset])
            is_estuary = downstream_x == 0 and downstream_y == 0
            reach = RiverReach(
                reach_id=f"cama-{y_index}-{x_index}",
                upstream_area_km2=float(uparea) * uparea_to_km2,
                width_m=float(width),
                floodplain_width_m=0.0,
                target_dx_km=target_dx_km,
                is_estuary=is_estuary,
            )
            records.append(
                CamaReachRecord(
                    reach=reach,
                    x_index=x_index,
                    y_index=y_index,
                    lon=grid.lon_center(x_index),
                    lat=grid.lat_center(y_index),
                    river_length_m=float(length),
                    downstream_x=downstream_x,
                    downstream_y=downstream_y,
                )
            )

    return records


def read_reach_inventory_window(
    map_dir: str,
    grid: CamaGridSpec,
    *,
    x_start: int,
    y_start: int,
    width: int,
    height: int,
    target_dx_km: float,
    uparea_to_km2: float = 1.0,
) -> list[CamaReachRecord]:
    from pathlib import Path

    from util.hydro_mesh.cama_binary import read_binary_window, read_cama_nextxy_window

    root = Path(map_dir)
    uparea = read_binary_window(
        root / "uparea.bin",
        grid,
        x_start=x_start,
        y_start=y_start,
        width=width,
        height=height,
        dtype="float32",
    )
    river_width = read_binary_window(
        root / "width.bin",
        grid,
        x_start=x_start,
        y_start=y_start,
        width=width,
        height=height,
        dtype="float32",
    )
    rivlen = read_binary_window(
        root / "rivlen.bin",
        grid,
        x_start=x_start,
        y_start=y_start,
        width=width,
        height=height,
        dtype="float32",
    )
    next_x, next_y = read_cama_nextxy_window(
        root / "nextxy.bin",
        grid,
        x_start=x_start,
        y_start=y_start,
        width=width,
        height=height,
    )
    return build_reach_inventory(
        grid,
        x_start=x_start,
        y_start=y_start,
        target_dx_km=target_dx_km,
        uparea_km2=uparea,
        width_m=river_width,
        rivlen_m=rivlen,
        next_x=next_x,
        next_y=next_y,
        uparea_to_km2=uparea_to_km2,
    )
