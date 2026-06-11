import json


def _feature_collection(features):
    return {"type": "FeatureCollection", "features": features}


def _cell(cell_id, area):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
        "properties": {
            "cell_id": cell_id,
            "source_areaCell": area,
            "source_areaCell_units": "unit_sphere",
        },
    }


def _intersection(cell_id, river_class, fraction, area_m2):
    feature = _cell(cell_id, 1.0)
    feature["properties"].update(
        {
            "river_class": river_class,
            "river_fraction": fraction,
            "estimated_river_area_m2": area_m2,
        }
    )
    return feature


def _coast_intersection(cell_id, coast_class, fraction, area_m2):
    feature = _cell(cell_id, 1.0)
    feature["properties"].update(
        {
            "mask_class": coast_class,
            "coastal_fraction": fraction,
            "estimated_coastal_area_m2": area_m2,
        }
    )
    return feature


def test_summarize_background_cells_reports_equivalent_size_stats():
    from util.hydro_mesh.refinement_eval import summarize_background_cells

    summary = summarize_background_cells(_feature_collection([_cell("a", 4.0), _cell("b", 9.0)]), unit_sphere_area=False)

    assert summary == {
        "cell_count": 2,
        "equivalent_cell_size_km_min": 0.002,
        "equivalent_cell_size_km_median": 0.0025,
        "equivalent_cell_size_km_max": 0.003,
    }


def test_summarize_intersections_reports_class_counts_and_fraction_range():
    from util.hydro_mesh.refinement_eval import summarize_intersections

    summary = summarize_intersections(
        _feature_collection(
            [
                _intersection("a", "R2", 0.25, 10.0),
                _intersection("b", "R3", 0.5, 20.0),
                _intersection("c", "R3", 0.75, 30.0),
            ]
        )
    )

    assert summary["feature_count"] == 3
    assert summary["class_counts"] == {"R2": 1, "R3": 2}
    assert summary["river_fraction_min"] == 0.25
    assert summary["river_fraction_max"] == 0.75
    assert summary["estimated_river_area_m2_sum"] == 60.0


def test_summarize_coast_intersections_reports_counts_fraction_and_area():
    from util.hydro_mesh.refinement_eval import summarize_coast_intersections

    summary = summarize_coast_intersections(
        _feature_collection(
            [
                _coast_intersection("a", "COAST", 0.25, 10.0),
                _coast_intersection("b", "COAST", 0.5, 20.0),
                _coast_intersection("c", "COAST_OCEAN", 0.75, 30.0),
            ]
        )
    )

    assert summary["feature_count"] == 3
    assert summary["class_counts"] == {"COAST": 2, "COAST_OCEAN": 1}
    assert summary["coastal_fraction_min"] == 0.25
    assert summary["coastal_fraction_median"] == 0.5
    assert summary["coastal_fraction_max"] == 0.75
    assert summary["estimated_coastal_area_m2_sum"] == 60.0


def test_parse_refinement_log_extracts_selected_and_retained_by_level():
    from util.hydro_mesh.refinement_eval import parse_refinement_log

    log_text = """
 refine_degree =            1
 需要细化的三角形个数(spc only):           96
 去除孤立细化三角形后，需要细化的三角形：          96
 refine_degree =            2
 需要细化的三角形个数(spc only):          185
 before num_ref =          185
 after num_ref =           86
 去除孤立细化三角形后，需要细化的三角形：          86
"""

    assert parse_refinement_log(log_text) == {
        "1": {"selected_triangles": 96, "retained_triangles": 96},
        "2": {
            "selected_triangles": 185,
            "before_nested_cleanup_triangles": 185,
            "after_nested_cleanup_triangles": 86,
            "retained_triangles": 86,
        },
    }


def test_write_refinement_eval_json_combines_inputs(tmp_path):
    from util.hydro_mesh.refinement_eval import write_refinement_eval_json

    background = tmp_path / "background.geojson"
    intersections = tmp_path / "intersections.geojson"
    log = tmp_path / "mkgrd.log"
    output = tmp_path / "eval.json"
    background.write_text(json.dumps(_feature_collection([_cell("a", 4.0)])))
    intersections.write_text(json.dumps(_feature_collection([_intersection("a", "R3", 0.5, 20.0)])))
    log.write_text(" refine_degree =            1\n 需要细化的三角形个数(spc only):           4\n 去除孤立细化三角形后，需要细化的三角形：          3\n")

    write_refinement_eval_json(background, intersections, output, log_path=log, unit_sphere_area=False)

    written = json.loads(output.read_text())
    assert written["background_cells"]["cell_count"] == 1
    assert written["river_intersections"]["class_counts"] == {"R3": 1}
    assert written["refinement_log"]["1"]["retained_triangles"] == 3


def test_write_refinement_eval_json_optionally_includes_coast_intersections(tmp_path):
    from util.hydro_mesh.refinement_eval import write_refinement_eval_json

    background = tmp_path / "background.geojson"
    intersections = tmp_path / "intersections.geojson"
    coast = tmp_path / "coast.geojson"
    output = tmp_path / "eval.json"
    background.write_text(json.dumps(_feature_collection([_cell("a", 4.0)])))
    intersections.write_text(json.dumps(_feature_collection([_intersection("a", "R3", 0.5, 20.0)])))
    coast.write_text(json.dumps(_feature_collection([_coast_intersection("a", "COAST", 0.25, 10.0)])))

    write_refinement_eval_json(
        background,
        intersections,
        output,
        coast_intersections_geojson=coast,
        unit_sphere_area=False,
    )

    written = json.loads(output.read_text())
    assert written["coast_intersections"]["feature_count"] == 1
    assert written["coast_intersections"]["class_counts"] == {"COAST": 1}
    assert written["coast_intersections"]["coastal_fraction_max"] == 0.25


def test_refinement_eval_cli_accepts_coast_intersections_geojson(tmp_path):
    from util.hydro_mesh.refinement_eval import main

    background = tmp_path / "background.geojson"
    intersections = tmp_path / "intersections.geojson"
    coast = tmp_path / "coast.geojson"
    output = tmp_path / "eval.json"
    background.write_text(json.dumps(_feature_collection([_cell("a", 4.0)])))
    intersections.write_text(json.dumps(_feature_collection([_intersection("a", "R3", 0.5, 20.0)])))
    coast.write_text(json.dumps(_feature_collection([_coast_intersection("a", "COAST", 0.25, 10.0)])))

    assert main(
        [
            str(background),
            str(intersections),
            str(output),
            "--coast-intersections-geojson",
            str(coast),
            "--file-area-m2",
        ]
    ) == 0

    assert json.loads(output.read_text())["coast_intersections"]["feature_count"] == 1
