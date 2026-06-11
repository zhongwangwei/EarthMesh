from __future__ import annotations

from dataclasses import dataclass

from util.v3_core.geometry import MaskFeature
from util.v3_core.schema import CanonicalCell


@dataclass(frozen=True)
class DemoInputs:
    name: str
    description: str
    cells: list[CanonicalCell]
    masks: list[MaskFeature]


def build_demo_inputs(name: str) -> DemoInputs:
    normalized = name.lower().replace("_", "-")
    if normalized not in {"gba", "greater-bay-area"}:
        raise ValueError(f"unknown v3 demo: {name}")
    return _build_gba_demo()


def _build_gba_demo() -> DemoInputs:
    cells = [
        _cell("gba_land", 0, [(113.70, 22.70), (114.10, 22.70), (113.70, 22.95)], "TRI"),
        _cell("gba_ocean", 1, [(113.70, 21.95), (114.10, 21.95), (113.70, 22.20)], "TRI"),
        _cell("gba_coast", 2, [(113.95, 22.20), (114.35, 22.20), (114.35, 22.45), (113.95, 22.45)], "HEX"),
        _cell("pearl_river", 3, [(113.35, 22.35), (113.75, 22.35), (113.75, 22.55), (113.35, 22.55)], "HEX"),
    ]
    masks = [
        MaskFeature("gba-land-mask", "LAND", 1, [(113.60, 22.60), (114.20, 22.60), (113.60, 23.05)]),
        MaskFeature("gba-ocean-mask", "OCEAN", 1, [(113.60, 21.85), (114.20, 21.85), (113.60, 22.25)]),
        MaskFeature("gba-coast-mask", "COAST_LAND", 15, [(113.90, 22.15), (114.40, 22.15), (114.40, 22.50), (113.90, 22.50)]),
        MaskFeature("pearl-river-mask", "R3", 30, [(113.30, 22.30), (113.80, 22.30), (113.80, 22.60), (113.30, 22.60)]),
    ]
    return DemoInputs(
        name="gba",
        description="Synthetic Greater Bay Area v3 smoke demo for land/ocean/coast/R3 pipeline QA; not a scientific mesh.",
        cells=cells,
        masks=masks,
    )


def _cell(cell_id: str, index: int, vertices: list[tuple[float, float]], cell_type: str) -> CanonicalCell:
    center_lon = sum(lon for lon, _lat in vertices) / len(vertices)
    center_lat = sum(lat for _lon, lat in vertices) / len(vertices)
    return CanonicalCell(
        cell_id=cell_id,
        cell_index=index,
        cell_type=cell_type,
        center_lon=center_lon,
        center_lat=center_lat,
        area_m2=1.0,
        vertices=vertices,
        source_mesh_type="v3_demo_gba",
    )
