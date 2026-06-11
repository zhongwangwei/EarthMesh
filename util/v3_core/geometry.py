from __future__ import annotations

from dataclasses import dataclass, field, replace

from util.v3_core.schema import CanonicalCell, VALID_COAST_CLASSES, VALID_HYDRO_CLASSES, VALID_SURFACE_CLASSES

Point = tuple[float, float]
Polygon = list[Point]


@dataclass(frozen=True)
class MaskFeature:
    feature_id: str
    mask_class: str
    priority: int
    polygon: Polygon
    properties: dict[str, object] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if not self.feature_id:
            raise ValueError("feature_id must be non-empty")
        if not self.mask_class:
            raise ValueError("mask_class must be non-empty")
        if self.priority < 0:
            raise ValueError("priority must be non-negative")
        if len(self.polygon) < 3:
            raise ValueError("polygon must contain at least three points")


@dataclass(frozen=True)
class OverlayResult:
    cell_id: str
    winning_class: str
    winning_priority: int
    class_fractions: dict[str, float] = field(default_factory=dict)
    source_feature_ids: list[str] = field(default_factory=list)
    quality_flags: list[str] = field(default_factory=list)

    @property
    def covered_fraction(self) -> float:
        return sum(self.class_fractions.values())


def polygon_area(polygon: Polygon) -> float:
    if len(polygon) < 3:
        return 0.0
    total = 0.0
    for index, (x0, y0) in enumerate(polygon):
        x1, y1 = polygon[(index + 1) % len(polygon)]
        total += x0 * y1 - x1 * y0
    return abs(total) * 0.5


def _signed_area(polygon: Polygon) -> float:
    total = 0.0
    for index, (x0, y0) in enumerate(polygon):
        x1, y1 = polygon[(index + 1) % len(polygon)]
        total += x0 * y1 - x1 * y0
    return total * 0.5


def _inside(point: Point, edge_start: Point, edge_end: Point, *, clip_ccw: bool) -> bool:
    x, y = point
    x0, y0 = edge_start
    x1, y1 = edge_end
    cross = (x1 - x0) * (y - y0) - (y1 - y0) * (x - x0)
    return cross >= -1.0e-12 if clip_ccw else cross <= 1.0e-12


def _line_intersection(a0: Point, a1: Point, b0: Point, b1: Point) -> Point:
    x1, y1 = a0
    x2, y2 = a1
    x3, y3 = b0
    x4, y4 = b1
    denominator = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
    if abs(denominator) < 1.0e-12:
        return a1
    px = ((x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4)) / denominator
    py = ((x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4)) / denominator
    return (px, py)


def polygon_clip_convex(subject: Polygon, clip: Polygon) -> Polygon:
    output = list(subject)
    clip_ccw = _signed_area(clip) >= 0.0
    for index, edge_start in enumerate(clip):
        edge_end = clip[(index + 1) % len(clip)]
        input_polygon = output
        output = []
        if not input_polygon:
            break
        previous = input_polygon[-1]
        for current in input_polygon:
            current_inside = _inside(current, edge_start, edge_end, clip_ccw=clip_ccw)
            previous_inside = _inside(previous, edge_start, edge_end, clip_ccw=clip_ccw)
            if current_inside:
                if not previous_inside:
                    output.append(_line_intersection(previous, current, edge_start, edge_end))
                output.append(current)
            elif previous_inside:
                output.append(_line_intersection(previous, current, edge_start, edge_end))
            previous = current
    return output


def overlay_cell_with_masks(cell: CanonicalCell, masks: list[MaskFeature]) -> OverlayResult:
    cell_area = polygon_area(cell.vertices)
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
        intersection = polygon_clip_convex(cell.vertices, mask.polygon)
        intersection_area = polygon_area(intersection)
        if intersection_area <= 1.0e-12:
            continue

        fraction = min(1.0, intersection_area / cell_area)
        class_fractions[mask.mask_class] = class_fractions.get(mask.mask_class, 0.0) + fraction
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
        class_fractions={mask_class: min(1.0, fraction) for mask_class, fraction in class_fractions.items()},
        source_feature_ids=source_feature_ids,
        quality_flags=[],
    )


def summarize_overlay_results(results: list[OverlayResult]) -> dict[str, object]:
    winning_class_counts: dict[str, int] = {}
    quality_flag_counts: dict[str, int] = {}
    missing_mask_count = 0

    for result in results:
        winning_class_counts[result.winning_class] = winning_class_counts.get(result.winning_class, 0) + 1
        if not result.class_fractions or "missing_mask" in result.quality_flags:
            missing_mask_count += 1
        for flag in result.quality_flags:
            quality_flag_counts[flag] = quality_flag_counts.get(flag, 0) + 1

    return {
        "cell_count": len(results),
        "winning_class_counts": winning_class_counts,
        "missing_mask_count": missing_mask_count,
        "quality_flag_counts": quality_flag_counts,
    }


def apply_overlay_to_cell(cell: CanonicalCell, result: OverlayResult) -> CanonicalCell:
    if cell.cell_id != result.cell_id:
        raise ValueError(f"overlay result cell_id={result.cell_id} does not match cell_id={cell.cell_id}")

    surface_class = cell.surface_class
    hydro_class = cell.hydro_class
    coast_class = cell.coast_class

    if result.winning_class in VALID_SURFACE_CLASSES:
        surface_class = result.winning_class
    if result.winning_class in VALID_HYDRO_CLASSES:
        hydro_class = result.winning_class
    if result.winning_class in VALID_COAST_CLASSES:
        coast_class = result.winning_class

    source_fractions = dict(result.class_fractions)
    quality_flags = list(dict.fromkeys([*cell.quality_flags, *result.quality_flags]))
    fraction_sum = sum(source_fractions.values())
    if fraction_sum > 1.0 + 1.0e-9:
        source_fractions = {key: value / fraction_sum for key, value in source_fractions.items()}
        quality_flags.append("normalized_source_fractions")

    return replace(
        cell,
        surface_class=surface_class,
        hydro_class=hydro_class,
        coast_class=coast_class,
        mesh_priority=max(cell.mesh_priority, result.winning_priority),
        source_fractions=source_fractions,
        quality_flags=quality_flags,
    )
