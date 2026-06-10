from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class RiverReach:
    reach_id: str
    upstream_area_km2: float
    width_m: float
    floodplain_width_m: float
    target_dx_km: float
    is_estuary: bool = False
    is_delta: bool = False
    is_coastal_wetland: bool = False
    is_major_confluence: bool = False
    user_force_2d: bool = False


@dataclass(frozen=True)
class ClassificationThresholds:
    explicit_2d_width_fraction: float = 0.25
    refine_width_fraction: float = 0.10
    explicit_2d_upstream_area_km2: float = 50000.0
    refine_upstream_area_km2: float = 10000.0
    keep_1d_upstream_area_km2: float = 1000.0


@dataclass(frozen=True)
class RiverClassification:
    reach_id: str
    river_class: str
    effective_width_m: float
    reasons: list[str] = field(default_factory=list)


def classify_reach(
    reach: RiverReach,
    thresholds: ClassificationThresholds | None = None,
) -> RiverClassification:
    thresholds = thresholds or ClassificationThresholds()
    effective_width_m = max(reach.width_m, reach.floodplain_width_m)
    target_dx_m = reach.target_dx_km * 1000.0

    if target_dx_m <= 0.0:
        raise ValueError("target_dx_km must be positive")

    reasons: list[str] = []
    if reach.is_estuary:
        reasons.append("estuary")
    if reach.is_delta:
        reasons.append("delta")
    if reach.is_coastal_wetland:
        reasons.append("coastal_wetland")
    if reach.is_major_confluence:
        reasons.append("major_confluence")
    if reach.user_force_2d:
        reasons.append("user_force_2d")

    if reasons:
        return RiverClassification(reach.reach_id, "R3", effective_width_m, reasons)

    if effective_width_m >= thresholds.explicit_2d_width_fraction * target_dx_m:
        return RiverClassification(
            reach.reach_id,
            "R3",
            effective_width_m,
            ["effective_width_fraction"],
        )

    if reach.upstream_area_km2 >= thresholds.explicit_2d_upstream_area_km2:
        return RiverClassification(
            reach.reach_id,
            "R3",
            effective_width_m,
            ["upstream_area_r3"],
        )

    if reach.upstream_area_km2 >= thresholds.refine_upstream_area_km2:
        return RiverClassification(
            reach.reach_id,
            "R2",
            effective_width_m,
            ["upstream_area_r2"],
        )

    if effective_width_m >= thresholds.refine_width_fraction * target_dx_m:
        return RiverClassification(
            reach.reach_id,
            "R2",
            effective_width_m,
            ["refine_width_fraction"],
        )

    if reach.upstream_area_km2 >= thresholds.keep_1d_upstream_area_km2:
        return RiverClassification(
            reach.reach_id,
            "R1",
            effective_width_m,
            ["upstream_area_r1"],
        )

    return RiverClassification(
        reach.reach_id,
        "R0",
        effective_width_m,
        ["below_explicit_thresholds"],
    )
