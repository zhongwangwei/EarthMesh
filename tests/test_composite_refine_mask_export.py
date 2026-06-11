import json


def _polygon_feature(mask_class, x0):
    return {
        "type": "Feature",
        "geometry": {
            "type": "Polygon",
            "coordinates": [[[x0, 0], [x0 + 1, 0], [x0 + 1, 1], [x0, 1], [x0, 0]]],
        },
        "properties": {"river_class": mask_class} if mask_class.startswith("R") else {"mask_class": mask_class},
    }


def test_write_composite_close_mask_nmls_combines_river_and_coast_with_class_caps(tmp_path):
    from util.hydro_mesh.composite_refine_mask_export import write_composite_close_mask_nmls

    river_geojson = tmp_path / "river.geojson"
    coast_geojson = tmp_path / "coast.geojson"
    river_geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    _polygon_feature("R2", 0),
                    _polygon_feature("R2", 2),
                    _polygon_feature("R2", 4),
                    _polygon_feature("R3", 10),
                ],
            }
        )
    )
    coast_geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [_polygon_feature("COAST", 20), _polygon_feature("COAST", 22)],
            }
        )
    )
    output_prefix = tmp_path / "refine_spc_hydro_mix"

    summary = write_composite_close_mask_nmls(
        {
            "components": [
                {
                    "name": "river",
                    "input_geojson": str(river_geojson),
                    "class_refine": {"R2": 1, "R3": 3},
                    "max_rings_by_class": {"R2": 2, "R3": 1},
                },
                {
                    "name": "coast",
                    "input_geojson": str(coast_geojson),
                    "class_refine": {"COAST": 1},
                    "max_rings_by_class": {"COAST": 1},
                },
            ]
        },
        output_prefix,
    )

    assert [path.name for path in summary.paths] == [
        "refine_spc_hydro_mix_COAST_d1_001.nml",
        "refine_spc_hydro_mix_R2_d1_001.nml",
        "refine_spc_hydro_mix_R2_d1_002.nml",
        "refine_spc_hydro_mix_R3_d1_001.nml",
        "refine_spc_hydro_mix_R3_d2_001.nml",
        "refine_spc_hydro_mix_R3_d3_001.nml",
    ]
    assert summary.counts_by_component == {"coast": 1, "river": 5}
    assert summary.counts_by_class_degree == {"COAST_d1": 1, "R2_d1": 2, "R3_d1": 1, "R3_d2": 1, "R3_d3": 1}


def test_composite_cli_writes_summary_json(tmp_path):
    from util.hydro_mesh.composite_refine_mask_export import main

    river_geojson = tmp_path / "river.geojson"
    river_geojson.write_text(
        json.dumps({"type": "FeatureCollection", "features": [_polygon_feature("R2", 0)]})
    )
    recipe_json = tmp_path / "recipe.json"
    summary_json = tmp_path / "summary.json"
    recipe_json.write_text(
        json.dumps(
            {
                "components": [
                    {"name": "river", "input_geojson": str(river_geojson), "class_refine": {"R2": 1}}
                ]
            }
        )
    )

    assert main([str(recipe_json), str(tmp_path / "refine_spc"), "--summary-json", str(summary_json)]) == 0

    written = json.loads(summary_json.read_text())
    assert written["files_written"] == 1
    assert written["components"][0]["name"] == "river"
