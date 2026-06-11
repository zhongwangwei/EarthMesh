import json

import pytest

from util.v3_core.recipe import V3Recipe, load_recipe


def test_load_flat_recipe_from_json(tmp_path):
    recipe_path = tmp_path / "case.json"
    recipe_path.write_text(json.dumps({
        "case": {"name": "china_coast_hydro", "output_dir": "/tmp/earthmesh-case"},
        "mesh": {"grid": "hex", "base_resolution": "N160", "kernel": "fortran_legacy"},
        "region": {"bbox": [73.0, 3.0, 136.0, 54.0]},
        "components": [
            {"type": "hydro_cama", "source": "/data/glb_01min", "classes": ["R2", "R3"]},
            {"type": "coastline", "source": "cama_elevtn", "coast_radius_cells": 3}
        ],
        "adapters": ["mpas", "colm2024", "fvcom", "colm20xx_schema"],
        "qa": {"html": True, "png": True, "summary_json": True}
    }))

    recipe = load_recipe(recipe_path)

    assert recipe.case_name == "china_coast_hydro"
    assert recipe.mesh.grid == "hex"
    assert recipe.region.bbox == (73.0, 3.0, 136.0, 54.0)
    assert [component.type for component in recipe.components] == ["hydro_cama", "coastline"]
    assert recipe.adapters == ["mpas", "colm2024", "fvcom", "colm20xx_schema"]


def test_recipe_rejects_missing_required_sections():
    with pytest.raises(ValueError, match="missing required recipe section"):
        V3Recipe.from_mapping({"case": {"name": "bad", "output_dir": "/tmp/bad"}})


def test_recipe_rejects_invalid_bbox_length():
    with pytest.raises(ValueError, match="bbox"):
        V3Recipe.from_mapping({
            "case": {"name": "bad", "output_dir": "/tmp/bad"},
            "mesh": {"grid": "tri", "base_resolution": "N64", "kernel": "fortran_legacy"},
            "region": {"bbox": [1.0, 2.0, 3.0]},
            "components": [],
            "adapters": ["fvcom"],
            "qa": {"html": False, "png": False, "summary_json": True},
        })
