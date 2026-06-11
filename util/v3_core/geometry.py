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
