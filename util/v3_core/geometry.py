from __future__ import annotations

from dataclasses import dataclass, field

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
