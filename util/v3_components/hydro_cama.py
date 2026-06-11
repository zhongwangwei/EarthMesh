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
