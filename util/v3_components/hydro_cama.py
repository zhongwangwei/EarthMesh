from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from util.v3_core.recipe import ComponentRecipe


@dataclass(frozen=True)
class HydroCamaConfig:
    map_dir: Path
    bbox: tuple[float, ...]
    target_dx_km: float
    classes: tuple[str, ...] = ("R2", "R3")
    coast_radius_cells: int = 3
    uparea_to_km2: float = 1.0e-6
    y_reversed_storage: bool = True

    def __post_init__(self) -> None:
        if len(self.bbox) != 4:
            raise ValueError("bbox must contain west, south, east, north")
        if self.target_dx_km <= 0.0:
            raise ValueError("target_dx_km must be positive")
        if self.coast_radius_cells < 1:
            raise ValueError("coast_radius_cells must be at least 1")
        if not self.classes:
            raise ValueError("classes must contain at least one hydro class")


def _as_bbox(value: Any) -> tuple[float, float, float, float]:
    if not isinstance(value, list) or len(value) != 4:
        raise ValueError("bbox must contain four numeric values")
    return tuple(float(item) for item in value)


def hydro_cama_config_from_recipe(recipe: ComponentRecipe) -> HydroCamaConfig:
    if recipe.type != "hydro_cama":
        raise ValueError("hydro_cama_config_from_recipe requires a hydro_cama component recipe")
    options = recipe.options
    return HydroCamaConfig(
        map_dir=Path(str(options["source"])),
        bbox=_as_bbox(options["bbox"]),
        target_dx_km=float(options["target_dx_km"]),
        classes=tuple(str(item) for item in options.get("classes", ["R2", "R3"])),
        coast_radius_cells=int(options.get("coast_radius_cells", 3)),
        uparea_to_km2=float(options.get("uparea_to_km2", 1.0e-6)),
        y_reversed_storage=bool(options.get("y_reversed_storage", True)),
    )

_HYDRO_PRIORITY = {"R0": 0, "R1": 1, "R2": 2, "R3": 3}


def hydro_record_semantics(record: dict[str, object]) -> dict[str, object]:
    river_class = str(record.get("river_class", "R0"))
    is_estuary = bool(record.get("is_estuary", False))
    hydro_class = "ESTUARY" if is_estuary and river_class == "R3" else river_class
    roles = ["cama_river"]
    if hydro_class in {"ESTUARY", "DELTA"}:
        roles.append("exchange_cell")
    return {
        "reach_id": str(record.get("reach_id", "")),
        "hydro_class": hydro_class,
        "mesh_priority": _HYDRO_PRIORITY.get(river_class, 0),
        "component_roles": roles,
        "upstream_area_km2": record.get("upstream_area_km2", ""),
        "river_width_m": record.get("width_m", ""),
    }
