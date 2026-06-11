from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable

VALID_CELL_TYPES = {"TRI", "HEX", "POLYGON", "MIXED"}
VALID_SURFACE_CLASSES = {"LAND", "OCEAN", "COAST", "LAKE", "ICE", "WETLAND", "UNKNOWN"}
VALID_HYDRO_CLASSES = {"NONE", "R0", "R1", "R2", "R3", "ESTUARY", "DELTA"}
VALID_COAST_CLASSES = {"NONE", "COAST_LAND", "COAST_OCEAN", "ESTUARY", "DELTA", "TIDAL_FLAT", "SHELF"}
VALID_INTERFACE_TYPES = {
    "land_ocean",
    "river_land",
    "river_ocean",
    "coast_ocean",
    "land_atmos",
    "ocean_atmos",
    "river_atmos",
}


@dataclass(frozen=True)
class CanonicalCell:
    cell_id: str
    cell_index: int
    cell_type: str
    center_lon: float
    center_lat: float
    area_m2: float
    vertices: list[tuple[float, float]]
    neighbors: list[str] = field(default_factory=list)
    surface_class: str = "UNKNOWN"
    hydro_class: str = "NONE"
    coast_class: str = "NONE"
    mesh_priority: int = 0
    component_roles: list[str] = field(default_factory=list)
    source_fractions: dict[str, float] = field(default_factory=dict)
    quality_flags: list[str] = field(default_factory=list)
    geometry_ref: str = ""
    orientation: str = "CCW"
    source_mesh_type: str = ""

    def __post_init__(self) -> None:
        if not self.cell_id:
            raise ValueError("cell_id must be non-empty")
        if self.cell_type not in VALID_CELL_TYPES:
            raise ValueError(f"cell_type must be one of {sorted(VALID_CELL_TYPES)}")
        if self.surface_class not in VALID_SURFACE_CLASSES:
            raise ValueError(f"surface_class must be one of {sorted(VALID_SURFACE_CLASSES)}")
        if self.hydro_class not in VALID_HYDRO_CLASSES:
            raise ValueError(f"hydro_class must be one of {sorted(VALID_HYDRO_CLASSES)}")
        if self.coast_class not in VALID_COAST_CLASSES:
            raise ValueError(f"coast_class must be one of {sorted(VALID_COAST_CLASSES)}")
        if self.area_m2 <= 0.0:
            raise ValueError("area_m2 must be positive")
        if len(self.vertices) < 3:
            raise ValueError("vertices must contain at least three lon/lat pairs")
        if self.mesh_priority < 0:
            raise ValueError("mesh_priority must be non-negative")
        fraction_sum = sum(self.source_fractions.values())
        if fraction_sum < -1.0e-12 or fraction_sum > 1.0 + 1.0e-9:
            raise ValueError("source_fractions must sum to at most 1.0")

    @property
    def is_exchange_cell(self) -> bool:
        return "exchange_cell" in self.component_roles

    @classmethod
    def minimal(cls, cell_id: str, *, cell_type: str = "POLYGON") -> "CanonicalCell":
        return cls(
            cell_id=cell_id,
            cell_index=0,
            cell_type=cell_type,
            center_lon=0.0,
            center_lat=0.0,
            area_m2=1.0,
            vertices=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
            neighbors=[],
            surface_class="UNKNOWN",
            hydro_class="NONE",
            coast_class="NONE",
            mesh_priority=0,
            component_roles=[],
            source_fractions={},
            quality_flags=[],
        )


@dataclass(frozen=True)
class ExchangeLink:
    source_cell_id: str
    target_cell_id: str
    source_role: str
    target_role: str
    interface_type: str
    exchange_area_m2: float
    exchange_fraction: float
    weight: float
    conservative: bool
    quality_flags: list[str] = field(default_factory=list)

    def __post_init__(self) -> None:
        if not self.source_cell_id or not self.target_cell_id:
            raise ValueError("source_cell_id and target_cell_id must be non-empty")
        if self.interface_type not in VALID_INTERFACE_TYPES:
            raise ValueError(f"interface_type must be one of {sorted(VALID_INTERFACE_TYPES)}")
        if self.exchange_area_m2 < 0.0:
            raise ValueError("exchange_area_m2 must be non-negative")
        if not 0.0 <= self.exchange_fraction <= 1.0:
            raise ValueError("exchange_fraction must be between 0 and 1")
        if self.weight < 0.0:
            raise ValueError("weight must be non-negative")


def validate_cell_collection(cells: Iterable[CanonicalCell]) -> list[CanonicalCell]:
    materialized = list(cells)
    seen: set[str] = set()
    for cell in materialized:
        if cell.cell_id in seen:
            raise ValueError(f"duplicate cell_id: {cell.cell_id}")
        seen.add(cell.cell_id)
    return materialized
