from __future__ import annotations

import math
import struct
from dataclasses import dataclass
from pathlib import Path

_DTYPE_FORMATS = {
    "float32": "f",
    "int32": "i",
}


@dataclass(frozen=True)
class CamaGridSpec:
    nx: int
    ny: int
    west: float
    south: float
    grid_size_deg: float
    little_endian: bool = True
    y_reversed_storage: bool = False

    def lon_center(self, x_index: int) -> float:
        return self.west + (x_index + 0.5) * self.grid_size_deg

    def lat_center(self, y_index: int) -> float:
        return self.south + (y_index + 0.5) * self.grid_size_deg

    def window_for_bbox(self, *, west: float, east: float, south: float, north: float) -> tuple[int, int, int, int]:
        x0 = max(0, math.floor((west - self.west) / self.grid_size_deg))
        x1 = min(self.nx, math.ceil((east - self.west) / self.grid_size_deg))
        y0 = max(0, math.floor((south - self.south) / self.grid_size_deg))
        y1 = min(self.ny, math.ceil((north - self.south) / self.grid_size_deg))
        if x1 <= x0 or y1 <= y0:
            raise ValueError("bbox does not overlap grid")
        return x0, y0, x1 - x0, y1 - y0

    def storage_y_index(self, y_index: int) -> int:
        if self.y_reversed_storage:
            return self.ny - 1 - y_index
        return y_index


def read_binary_window(
    binary_path: str | Path,
    grid: CamaGridSpec,
    *,
    x_start: int,
    y_start: int,
    width: int,
    height: int,
    dtype: str,
    components: int = 1,
    component_index: int = 0,
) -> list[list[float | int]]:
    if dtype not in _DTYPE_FORMATS:
        raise ValueError(f"Unsupported dtype: {dtype}")
    if components < 1:
        raise ValueError("components must be positive")
    if component_index < 0 or component_index >= components:
        raise ValueError("component_index must be inside components")
    if x_start < 0 or y_start < 0 or width < 1 or height < 1:
        raise ValueError("window indices and shape must be positive")
    if x_start + width > grid.nx or y_start + height > grid.ny:
        raise ValueError("requested window is outside grid")

    endian = "<" if grid.little_endian else ">"
    fmt = _DTYPE_FORMATS[dtype]
    item_size = struct.calcsize(endian + fmt)
    row_stride = grid.nx * components * item_size
    row_values = grid.nx * components
    unpack_fmt = endian + f"{row_values}{fmt}"
    result: list[list[float | int]] = []

    with Path(binary_path).open("rb") as handle:
        for y_index in range(y_start, y_start + height):
            handle.seek(grid.storage_y_index(y_index) * row_stride)
            raw = handle.read(row_stride)
            if len(raw) != row_stride:
                raise ValueError("binary file ended before requested window was read")
            full_row = struct.unpack(unpack_fmt, raw)
            values: list[float | int] = []
            for x_index in range(x_start, x_start + width):
                values.append(full_row[x_index * components + component_index])
            result.append(values)

    return result
