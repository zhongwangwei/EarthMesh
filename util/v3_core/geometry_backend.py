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
        results = self._rust_geometry.overlay_cells(
            [(cell.cell_id, cell.vertices) for cell in cells],
            [
                (mask.feature_id, mask.mask_class, mask.priority, mask.polygon)
                for mask in masks
            ],
        )
        return [
            OverlayResult(
                cell_id=cell_id,
                winning_class=winning_class,
                winning_priority=winning_priority,
                class_fractions=dict(class_fractions),
                source_feature_ids=source_feature_ids,
                quality_flags=quality_flags,
            )
            for (
                cell_id,
                winning_class,
                winning_priority,
                class_fractions,
                source_feature_ids,
                quality_flags,
            ) in results
        ]

    def _overlay_cell(self, cell: CanonicalCell, masks: list[MaskFeature]) -> OverlayResult:
        winning_class, winning_priority, class_fractions, source_feature_ids, quality_flags = (
            self._rust_geometry.overlay_cell(
                cell.vertices,
                [
                    (mask.feature_id, mask.mask_class, mask.priority, mask.polygon)
                    for mask in masks
                ],
            )
        )
        return OverlayResult(
            cell_id=cell.cell_id,
            winning_class=winning_class,
            winning_priority=winning_priority,
            class_fractions=dict(class_fractions),
            source_feature_ids=source_feature_ids,
            quality_flags=quality_flags,
        )


def get_geometry_backend(name: str = "python_reference") -> GeometryBackend:
    if name == "python_reference":
        return PythonGeometryBackend()
    if name in {"rust", "rust_pyo3"}:
        return RustGeometryBackend()
    raise ValueError(f"unsupported geometry backend: {name}")
