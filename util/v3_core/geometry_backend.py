from __future__ import annotations

from importlib import import_module
from typing import Protocol

from util.v3_core.geometry import MaskFeature, OverlayResult, overlay_cell_with_masks
from util.v3_core.schema import CanonicalCell


class GeometryBackend(Protocol):
    name: str

    def overlay_cells(
        self, cells: list[CanonicalCell], masks: list[MaskFeature]
    ) -> list[OverlayResult]:
        ...


class PythonGeometryBackend:
    name = "python_reference"

    def overlay_cells(
        self, cells: list[CanonicalCell], masks: list[MaskFeature]
    ) -> list[OverlayResult]:
        return [overlay_cell_with_masks(cell, masks) for cell in cells]


class RustGeometryBackend:
    name = "rust_pyo3"

    def __init__(self) -> None:
        try:
            self._rust_geometry = import_module("earthmesh_geometry")
        except ImportError as exc:
            raise RuntimeError(
                "Rust geometry backend is unavailable; run "
                "`python3 -m maturin develop --manifest-path rust/earthmesh_geometry/Cargo.toml` first"
            ) from exc

    def overlay_cells(
        self, cells: list[CanonicalCell], masks: list[MaskFeature]
    ) -> list[OverlayResult]:
        return [self._overlay_cell(cell, masks) for cell in cells]

    def _overlay_cell(self, cell: CanonicalCell, masks: list[MaskFeature]) -> OverlayResult:
        cell_area = float(self._rust_geometry.polygon_area(cell.vertices))
        if cell_area <= 0.0:
            return OverlayResult(
                cell_id=cell.cell_id,
                winning_class="",
                winning_priority=0,
                class_fractions={},
                source_feature_ids=[],
                quality_flags=["zero_area_cell"],
            )

        class_fractions: dict[str, float] = {}
        source_feature_ids: list[str] = []
        winning_class = ""
        winning_priority = 0

        for mask in masks:
            intersection_area = float(
                self._rust_geometry.intersection_area(cell.vertices, mask.polygon)
            )
            if intersection_area <= 1.0e-12:
                continue
            fraction = min(1.0, intersection_area / cell_area)
            class_fractions[mask.mask_class] = (
                class_fractions.get(mask.mask_class, 0.0) + fraction
            )
            source_feature_ids.append(mask.feature_id)
            if mask.priority >= winning_priority:
                winning_class = mask.mask_class
                winning_priority = mask.priority

        if not class_fractions:
            return OverlayResult(
                cell_id=cell.cell_id,
                winning_class="UNKNOWN",
                winning_priority=0,
                class_fractions={"UNKNOWN": 1.0},
                source_feature_ids=[],
                quality_flags=["missing_mask"],
            )

        return OverlayResult(
            cell_id=cell.cell_id,
            winning_class=winning_class,
            winning_priority=winning_priority,
            class_fractions={
                mask_class: min(1.0, fraction)
                for mask_class, fraction in class_fractions.items()
            },
            source_feature_ids=source_feature_ids,
            quality_flags=[],
        )


def get_geometry_backend(name: str = "python_reference") -> GeometryBackend:
    if name == "python_reference":
        return PythonGeometryBackend()
    if name in {"rust", "rust_pyo3"}:
        return RustGeometryBackend()
    raise ValueError(f"unsupported geometry backend: {name}")
