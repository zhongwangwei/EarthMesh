from __future__ import annotations

from typing import Protocol

from util.v3_core.geometry import MaskFeature, OverlayResult, overlay_cell_with_masks
from util.v3_core.schema import CanonicalCell


class GeometryBackend(Protocol):
    name: str

    def overlay_cells(self, cells: list[CanonicalCell], masks: list[MaskFeature]) -> list[OverlayResult]:
        ...


class PythonGeometryBackend:
    name = "python_reference"

    def overlay_cells(self, cells: list[CanonicalCell], masks: list[MaskFeature]) -> list[OverlayResult]:
        return [overlay_cell_with_masks(cell, masks) for cell in cells]


def get_geometry_backend(name: str = "python_reference") -> GeometryBackend:
    if name != "python_reference":
        raise ValueError(f"unsupported geometry backend: {name}")
    return PythonGeometryBackend()
