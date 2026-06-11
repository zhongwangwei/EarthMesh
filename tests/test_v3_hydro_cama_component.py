from pathlib import Path

import pytest

from util.v3_core.recipe import ComponentRecipe
from util.v3_components.hydro_cama import HydroCamaConfig, hydro_cama_config_from_recipe


def test_hydro_cama_config_from_recipe_parses_required_options():
    recipe = ComponentRecipe(
        type="hydro_cama",
        options={
            "source": "/data/glb_01min",
            "bbox": [112.0, 20.0, 115.0, 24.0],
            "target_dx_km": 5.6,
            "classes": ["R2", "R3"],
            "coast_radius_cells": 3,
        },
    )

    config = hydro_cama_config_from_recipe(recipe)

    assert config.map_dir == Path("/data/glb_01min")
    assert config.bbox == (112.0, 20.0, 115.0, 24.0)
    assert config.target_dx_km == 5.6
    assert config.classes == ("R2", "R3")
    assert config.coast_radius_cells == 3


def test_hydro_cama_config_rejects_wrong_component_type():
    recipe = ComponentRecipe(type="coastline", options={"source": "/data/glb_01min"})

    with pytest.raises(ValueError, match="hydro_cama"):
        hydro_cama_config_from_recipe(recipe)


def test_hydro_cama_config_requires_four_value_bbox():
    with pytest.raises(ValueError, match="bbox"):
        HydroCamaConfig(
            map_dir=Path("/data/glb_01min"),
            bbox=(1.0, 2.0, 3.0),
            target_dx_km=5.0,
            classes=("R2",),
            coast_radius_cells=3,
        )
