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

from util.v3_components.hydro_cama import hydro_record_semantics


def test_hydro_record_semantics_maps_r3_estuary_to_exchange_cell():
    semantics = hydro_record_semantics({
        "reach_id": "cama-10-20",
        "river_class": "R3",
        "is_estuary": True,
        "upstream_area_km2": 60000.0,
        "width_m": 1400.0,
    })

    assert semantics["hydro_class"] == "ESTUARY"
    assert semantics["mesh_priority"] == 3
    assert "cama_river" in semantics["component_roles"]
    assert "exchange_cell" in semantics["component_roles"]


def test_hydro_record_semantics_maps_r2_to_refinement_role():
    semantics = hydro_record_semantics({
        "reach_id": "cama-11-21",
        "river_class": "R2",
        "is_estuary": False,
    })

    assert semantics["hydro_class"] == "R2"
    assert semantics["mesh_priority"] == 2
    assert semantics["component_roles"] == ["cama_river"]
