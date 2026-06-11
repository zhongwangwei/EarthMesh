import json

import pytest

from util.v3_core.adaptive_grid import refine_cells_by_mask_factors, refine_cells_by_masks, write_refined_cells_geojson
from util.v3_core.geometry import MaskFeature
from util.v3_core.grid import generate_bbox_grid_cells


def test_refine_cells_by_masks_splits_only_cells_intersecting_target_classes():
    cells = generate_bbox_grid_cells((0.0, 0.0, 2.0, 1.0), nx=2, ny=1, cell_id_prefix="base")
    river = MaskFeature("river-left", "R2", 20, [(0.25, 0.0), (0.75, 0.0), (0.75, 1.0), (0.25, 1.0)])
    land = MaskFeature("land-right", "LAND", 1, [(1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0)])

    refined = refine_cells_by_masks(cells, [river, land], refine_classes={"R2"}, factor=2)

    assert len(refined) == 5
    assert [cell.cell_id for cell in refined[:4]] == [
        "base_0000_0000_r00_00",
        "base_0000_0000_r01_00",
        "base_0000_0000_r00_01",
        "base_0000_0000_r01_01",
    ]
    assert refined[0].vertices == [(0.0, 0.0), (0.5, 0.0), (0.5, 0.5), (0.0, 0.5)]
    assert all(cell.geometry_ref == "base_0000_0000" for cell in refined[:4])
    assert all(cell.source_mesh_type == "bbox_grid_refined" for cell in refined[:4])
    assert all("refined_from_mask" in cell.quality_flags for cell in refined[:4])
    assert refined[-1].cell_id == "base_0001_0000"
    assert refined[-1].vertices == cells[1].vertices


def test_refine_cells_by_masks_uses_any_intersecting_selected_class_not_only_winner():
    cells = generate_bbox_grid_cells((0.0, 0.0, 1.0, 1.0), nx=1, ny=1, cell_id_prefix="cell")
    coast = MaskFeature("coast", "COAST_LAND", 10, [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
    narrow_river = MaskFeature("river", "R3", 30, [(0.4, 0.0), (0.6, 0.0), (0.6, 1.0), (0.4, 1.0)])

    refined = refine_cells_by_masks(cells, [coast, narrow_river], refine_classes={"COAST_LAND"}, factor=3)

    assert len(refined) == 9
    assert refined[0].cell_id == "cell_0000_0000_r00_00"
    assert refined[-1].cell_id == "cell_0000_0000_r02_02"


def test_refine_cells_by_masks_rejects_invalid_factor():
    cells = generate_bbox_grid_cells((0.0, 0.0, 1.0, 1.0), nx=1, ny=1)

    with pytest.raises(ValueError, match="factor must be at least 2"):
        refine_cells_by_masks(cells, [], refine_classes={"R2"}, factor=1)


def test_write_refined_cells_geojson_writes_refined_cell_file(tmp_path):
    cells = generate_bbox_grid_cells((0.0, 0.0, 1.0, 1.0), nx=1, ny=1, cell_id_prefix="base")
    river = MaskFeature("river", "R2", 20, [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])

    output = write_refined_cells_geojson(cells, [river], refine_classes={"R2"}, factor=2, output_path=tmp_path / "cells.geojson")

    payload = json.loads(output.read_text())
    assert output.name == "cells.geojson"
    assert len(payload["features"]) == 4
    assert payload["features"][0]["properties"]["cell_id"] == "base_0000_0000_r00_00"
    assert payload["features"][0]["properties"]["geometry_ref"] == "base_0000_0000"


def test_v3_core_lazy_exports_adaptive_grid_helpers():
    from util.v3_core import refine_cells_by_mask_factors as exported_factor_refine
    from util.v3_core import refine_cells_by_masks as exported_refine

    assert exported_refine is refine_cells_by_masks
    assert exported_factor_refine is refine_cells_by_mask_factors


def test_refine_cells_by_mask_factors_uses_max_factor_from_intersecting_classes():
    cells = generate_bbox_grid_cells((0.0, 0.0, 2.0, 1.0), nx=2, ny=1, cell_id_prefix="multi")
    coast = MaskFeature("coast-left", "COAST_LAND", 10, [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
    river = MaskFeature("river-left", "R3", 30, [(0.4, 0.0), (0.6, 0.0), (0.6, 1.0), (0.4, 1.0)])
    ocean = MaskFeature("ocean-right", "OCEAN", 1, [(1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0)])

    refined = refine_cells_by_mask_factors(
        cells,
        [coast, river, ocean],
        refine_class_factors={"COAST_LAND": 2, "R3": 3},
    )

    assert len(refined) == 10
    assert [cell.cell_id for cell in refined[:3]] == [
        "multi_0000_0000_r00_00",
        "multi_0000_0000_r01_00",
        "multi_0000_0000_r02_00",
    ]
    assert refined[8].cell_id == "multi_0000_0000_r02_02"
    assert refined[9].cell_id == "multi_0001_0000"


def test_refine_cells_by_mask_factors_rejects_invalid_class_factor():
    cells = generate_bbox_grid_cells((0.0, 0.0, 1.0, 1.0), nx=1, ny=1)

    with pytest.raises(ValueError, match="refine factor for R3 must be at least 2"):
        refine_cells_by_mask_factors(cells, [], refine_class_factors={"R3": 1})
