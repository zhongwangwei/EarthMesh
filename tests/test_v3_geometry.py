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
