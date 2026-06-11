from util.v3_core.geometry import MaskFeature, OverlayResult


def test_mask_feature_records_class_priority_and_polygon():
    feature = MaskFeature(
        feature_id="coast-1",
        mask_class="COAST",
        priority=10,
        polygon=[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)],
        properties={"source": "cama_elevtn"},
    )

    assert feature.feature_id == "coast-1"
    assert feature.mask_class == "COAST"
    assert feature.priority == 10
    assert feature.properties["source"] == "cama_elevtn"


def test_overlay_result_reports_winning_class_and_fractions():
    result = OverlayResult(
        cell_id="cell-1",
        winning_class="R3",
        winning_priority=30,
        class_fractions={"COAST": 0.25, "R3": 0.50},
        source_feature_ids=["coast-1", "river-1"],
        quality_flags=[],
    )

    assert result.covered_fraction == 0.75
    assert result.winning_class == "R3"

from util.v3_core.geometry import polygon_area, polygon_clip_convex


def test_polygon_area_handles_triangle_and_hexagon_like_polygon():
    triangle = [(0.0, 0.0), (2.0, 0.0), (0.0, 2.0)]
    rectangle = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)]

    assert polygon_area(triangle) == 2.0
    assert polygon_area(rectangle) == 2.0


def test_polygon_clip_convex_returns_intersection_polygon():
    subject = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]
    clip = [(1.0, 0.0), (3.0, 0.0), (3.0, 2.0), (1.0, 2.0)]

    intersection = polygon_clip_convex(subject, clip)

    assert round(polygon_area(intersection), 6) == 2.0

from util.v3_core.geometry import overlay_cell_with_masks
from util.v3_core.schema import CanonicalCell


def test_overlay_cell_with_masks_handles_triangle_cell():
    cell = CanonicalCell.minimal("tri-cell", cell_type="TRI")
    mask = MaskFeature(
        feature_id="land-mask",
        mask_class="LAND",
        priority=1,
        polygon=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
    )

    result = overlay_cell_with_masks(cell, [mask])

    assert result.cell_id == "tri-cell"
    assert result.winning_class == "LAND"
    assert result.class_fractions == {"LAND": 1.0}


def test_overlay_cell_with_masks_prefers_higher_priority_class():
    cell = CanonicalCell(
        cell_id="hex-cell",
        cell_index=1,
        cell_type="HEX",
        center_lon=1.0,
        center_lat=1.0,
        area_m2=4.0,
        vertices=[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
    )
    coast = MaskFeature("coast", "COAST", 10, [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)])
    river = MaskFeature("river", "R3", 30, [(1.0, 0.0), (2.0, 0.0), (2.0, 2.0), (1.0, 2.0)])

    result = overlay_cell_with_masks(cell, [coast, river])

    assert result.winning_class == "R3"
    assert result.winning_priority == 30
    assert round(result.class_fractions["COAST"], 6) == 1.0
    assert round(result.class_fractions["R3"], 6) == 0.5
    assert result.source_feature_ids == ["coast", "river"]

from util.v3_core.geometry import summarize_overlay_results


def test_summarize_overlay_results_counts_classes_and_missing_masks():
    results = [
        OverlayResult("a", "LAND", 1, {"LAND": 1.0}, ["land"], []),
        OverlayResult("b", "R3", 30, {"COAST": 1.0, "R3": 0.5}, ["coast", "river"], []),
        OverlayResult("c", "", 0, {}, [], ["missing_mask"]),
    ]

    summary = summarize_overlay_results(results)

    assert summary["cell_count"] == 3
    assert summary["winning_class_counts"] == {"LAND": 1, "R3": 1, "": 1}
    assert summary["missing_mask_count"] == 1
    assert summary["quality_flag_counts"] == {"missing_mask": 1}
