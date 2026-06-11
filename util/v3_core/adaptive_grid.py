from __future__ import annotations

from dataclasses import replace
from pathlib import Path
from typing import Iterable

from util.v3_core.geojson_io import write_cells_geojson
from util.v3_core.geometry import MaskFeature, overlay_cell_with_masks, polygon_area
from util.v3_core.schema import CanonicalCell


def refine_cells_by_masks(
    cells: Iterable[CanonicalCell],
    masks: list[MaskFeature],
    *,
    refine_classes: set[str],
    factor: int = 2,
) -> list[CanonicalCell]:
    if factor < 2:
        raise ValueError("factor must be at least 2")
    if not refine_classes:
        return list(cells)

    refined: list[CanonicalCell] = []
    next_index = 0
    for cell in cells:
        overlay = overlay_cell_with_masks(cell, masks)
        should_refine = any(mask_class in refine_classes for mask_class in overlay.class_fractions)
        if should_refine:
            children = _split_rectangular_cell(cell, factor=factor, first_index=next_index)
            refined.extend(children)
            next_index += len(children)
        else:
            refined.append(replace(cell, cell_index=next_index))
            next_index += 1
    return refined


def write_refined_cells_geojson(
    cells: Iterable[CanonicalCell],
    masks: list[MaskFeature],
    *,
    refine_classes: set[str],
    factor: int,
    output_path: str | Path,
) -> Path:
    refined = refine_cells_by_masks(cells, masks, refine_classes=refine_classes, factor=factor)
    return write_cells_geojson(refined, output_path)


def _split_rectangular_cell(cell: CanonicalCell, *, factor: int, first_index: int) -> list[CanonicalCell]:
    min_lon = min(lon for lon, _lat in cell.vertices)
    max_lon = max(lon for lon, _lat in cell.vertices)
    min_lat = min(lat for _lon, lat in cell.vertices)
    max_lat = max(lat for _lon, lat in cell.vertices)
    dx = (max_lon - min_lon) / factor
    dy = (max_lat - min_lat) / factor
    area = cell.area_m2 / float(factor * factor)
    if area <= 0.0:
        area = polygon_area(cell.vertices) / float(factor * factor)

    children: list[CanonicalCell] = []
    for j in range(factor):
        y0 = min_lat + j * dy
        y1 = y0 + dy
        for i in range(factor):
            x0 = min_lon + i * dx
            x1 = x0 + dx
            child_vertices = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
            children.append(
                replace(
                    cell,
                    cell_id=f"{cell.cell_id}_r{i:02d}_{j:02d}",
                    cell_index=first_index + len(children),
                    cell_type="POLYGON",
                    center_lon=(x0 + x1) / 2.0,
                    center_lat=(y0 + y1) / 2.0,
                    area_m2=area,
                    vertices=child_vertices,
                    neighbors=[],
                    source_fractions={},
                    quality_flags=list(dict.fromkeys([*cell.quality_flags, "refined_from_mask"])),
                    geometry_ref=cell.cell_id,
                    source_mesh_type=f"{cell.source_mesh_type or 'unknown'}_refined",
                )
            )
    return children
