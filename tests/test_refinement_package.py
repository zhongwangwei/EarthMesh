import json
from pathlib import Path


def _feature_collection(features):
    return {"type": "FeatureCollection", "features": features}


def _cell(cell_id, area=4.0):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
        "properties": {"cell_id": cell_id, "source_areaCell": area},
    }


def _cell_rect(cell_id, x0, y0, x1, y1, area=1.0):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]]},
        "properties": {"cell_id": cell_id, "source_areaCell": area},
    }


def _river(cell_id, river_class="R3"):
    feature = _cell(cell_id)
    feature["properties"].update({"river_class": river_class, "river_fraction": 0.5})
    return feature


def _coast(cell_id):
    feature = _cell(cell_id)
    feature["properties"].update({"mask_class": "COAST", "coastal_fraction": 0.25})
    return feature


def test_write_refinement_delivery_package_creates_manifest_eval_html_and_ranking(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    log = tmp_path / "mkgrd.log"
    output_dir = tmp_path / "package"
    background.write_text(json.dumps(_feature_collection([_cell("a"), _cell("b", 9.0)])))
    river.write_text(json.dumps(_feature_collection([_river("a", "R2"), _river("b", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("a")])))
    log.write_text(
        " refine_degree =            3\n"
        " 需要细化的三角形个数(spc only):           4\n"
        " 去除孤立细化三角形后，需要细化的三角形：          3\n"
    )

    manifest = write_refinement_delivery_package(
        case_name="N112_r3d3_cst20",
        background_geojson=background,
        river_geojson=river,
        coast_geojson=coast,
        log_path=log,
        output_dir=output_dir,
        title="Package smoke",
        max_background_cells=10,
        unit_sphere_area=False,
    )

    assert manifest["kind"] == "earthmesh_hydro_coast_delivery_package"
    assert manifest["case_name"] == "N112_r3d3_cst20"
    assert manifest["recommended_case"] == "N112_r3d3_cst20"
    assert manifest["metrics"]["background_cell_count"] == 2
    assert manifest["metrics"]["river_overlap_cells"] == 2
    assert manifest["metrics"]["coast_overlap_cells"] == 1
    assert manifest["metrics"]["retained_triangles"] == {"1": 0, "2": 0, "3": 3}
    for key in ["eval_json", "html_map", "ranking_json", "manifest_json"]:
        assert Path(manifest["files"][key]).exists()
    html_text = Path(manifest["files"]["html_map"]).read_text()
    assert "Package smoke" in html_text
    written_manifest = json.loads(Path(manifest["files"]["manifest_json"]).read_text())
    assert written_manifest == manifest


def test_refinement_package_cli_writes_package(tmp_path):
    from util.hydro_mesh.refinement_package import main

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    log = tmp_path / "mkgrd.log"
    output_dir = tmp_path / "package"
    background.write_text(json.dumps(_feature_collection([_cell("a")])))
    river.write_text(json.dumps(_feature_collection([_river("a", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("a")])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          7\n")

    assert main([
        "--case-name", "cli_case",
        "--background-geojson", str(background),
        "--river-geojson", str(river),
        "--coast-geojson", str(coast),
        "--log-path", str(log),
        "--output-dir", str(output_dir),
        "--title", "CLI package",
        "--max-background-cells", "5",
        "--file-area-m2",
    ]) == 0

    manifest = json.loads((output_dir / "delivery_manifest.json").read_text())
    assert manifest["case_name"] == "cli_case"
    assert manifest["recommended_case"] == "cli_case"
    assert (output_dir / "cli_case_rivers_and_integrated_coast_leaflet.html").exists()


def test_write_refinement_delivery_package_rejects_missing_required_input(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    missing = tmp_path / "missing.geojson"
    try:
        write_refinement_delivery_package(
            case_name="bad",
            background_geojson=missing,
            river_geojson=missing,
            coast_geojson=missing,
            log_path=missing,
            output_dir=tmp_path / "package",
        )
    except FileNotFoundError as exc:
        assert str(missing) in str(exc)
    else:
        raise AssertionError("expected missing input to raise FileNotFoundError")


def test_delivery_manifest_records_optional_ranking_report_inputs(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    log = tmp_path / "mkgrd.log"
    comparison = tmp_path / "n96_eval.json"
    failed = tmp_path / "n128_failed_eval.json"
    background.write_text(json.dumps(_feature_collection([_cell("a")])))
    river.write_text(json.dumps(_feature_collection([_river("a", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("a")])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          7\n")
    comparison.write_text(json.dumps({
        "kind": "earthmesh_hydro_refinement_eval",
        "case_name": "N96_r3d3_cst20",
        "status": "pass",
        "background_cells": {"cell_count": 5, "equivalent_cell_size_km_median": 10.0},
        "river_intersections": {"feature_count": 2},
        "coast_intersections": {"feature_count": 1},
        "refinement_log": {"3": {"retained_triangles": 2}},
    }))
    failed.write_text(json.dumps({
        "kind": "earthmesh_hydro_refinement_eval",
        "case_name": "N128_r3d3_cst20",
        "status": "failed",
        "background_cells": {"cell_count": 0},
        "river_intersections": {"feature_count": 0},
        "coast_intersections": {"feature_count": 0},
        "refinement_log": {"3": {"retained_triangles": 99}},
    }))

    manifest = write_refinement_delivery_package(
        case_name="N112_r3d3_cst20",
        background_geojson=background,
        river_geojson=river,
        coast_geojson=coast,
        log_path=log,
        output_dir=tmp_path / "package",
        comparison_reports=[comparison],
        failed_reports=[failed],
        unit_sphere_area=False,
    )

    assert manifest["source_files"]["comparison_reports"] == [str(comparison)]
    assert manifest["source_files"]["failed_reports"] == [str(failed)]


def test_refinement_delivery_package_records_optional_surface_geojson(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    surface = tmp_path / "surface.geojson"
    log = tmp_path / "mkgrd.log"
    background.write_text(json.dumps(_feature_collection([_cell("a")])))
    river.write_text(json.dumps(_feature_collection([_river("a", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("a")])))
    surface.write_text(json.dumps(_feature_collection([_cell("a")])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          7\n")

    manifest = write_refinement_delivery_package(
        case_name="surface_case",
        background_geojson=background,
        river_geojson=river,
        coast_geojson=coast,
        surface_geojson=surface,
        log_path=log,
        output_dir=tmp_path / "package",
        unit_sphere_area=False,
    )

    assert manifest["source_files"]["surface_geojson"] == str(surface)
    written = json.loads((tmp_path / "package" / "delivery_manifest.json").read_text())
    assert written["source_files"]["surface_geojson"] == str(surface)


def test_refinement_package_cli_accepts_surface_geojson(tmp_path):
    from util.hydro_mesh.refinement_package import main

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    surface = tmp_path / "surface.geojson"
    log = tmp_path / "mkgrd.log"
    output_dir = tmp_path / "package"
    background.write_text(json.dumps(_feature_collection([_cell("a")])))
    river.write_text(json.dumps(_feature_collection([_river("a", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("a")])))
    surface.write_text(json.dumps(_feature_collection([_cell("a")])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          7\n")

    assert main([
        "--case-name", "surface_cli",
        "--background-geojson", str(background),
        "--river-geojson", str(river),
        "--coast-geojson", str(coast),
        "--surface-geojson", str(surface),
        "--log-path", str(log),
        "--output-dir", str(output_dir),
        "--file-area-m2",
    ]) == 0

    manifest = json.loads((output_dir / "delivery_manifest.json").read_text())
    assert manifest["source_files"]["surface_geojson"] == str(surface)


def test_refinement_delivery_package_writes_complete_cell_mask_when_surface_geojson_is_supplied(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    surface = tmp_path / "surface.geojson"
    log = tmp_path / "mkgrd.log"
    output_dir = tmp_path / "package"
    background.write_text(json.dumps(_feature_collection([
        _cell_rect("land_cell", 0, 0, 1, 1),
        _cell_rect("ocean_cell", 1, 0, 2, 1),
    ])))
    river.write_text(json.dumps(_feature_collection([_river("land_cell", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("ocean_cell")])))
    surface.write_text(json.dumps(_feature_collection([
        {
            "type": "Feature",
            "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
            "properties": {"mask_class": "LAND"},
        },
        {
            "type": "Feature",
            "geometry": {"type": "Polygon", "coordinates": [[[1, 0], [2, 0], [2, 1], [1, 1], [1, 0]]]},
            "properties": {"mask_class": "OCEAN"},
        },
    ])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          7\n")

    manifest = write_refinement_delivery_package(
        case_name="surface_cells",
        background_geojson=background,
        river_geojson=river,
        coast_geojson=coast,
        surface_geojson=surface,
        log_path=log,
        output_dir=output_dir,
        unit_sphere_area=False,
    )

    complete_path = Path(manifest["files"]["complete_cell_mask_geojson"])
    assert complete_path.exists()
    complete = json.loads(complete_path.read_text())
    assert len(complete["features"]) == 2
    by_id = {feature["properties"]["cell_id"]: feature["properties"] for feature in complete["features"]}
    assert by_id["land_cell"]["surface_class"] == "LAND"
    assert by_id["land_cell"]["mask_class"] == "R3"
    assert by_id["ocean_cell"]["surface_class"] == "OCEAN"
    assert by_id["ocean_cell"]["mask_class"] == "COAST"
    written = json.loads((output_dir / "delivery_manifest.json").read_text())
    assert written["files"]["complete_cell_mask_geojson"] == str(complete_path)


def test_refinement_delivery_package_embeds_complete_surface_mask_in_html_when_supplied(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    surface = tmp_path / "surface.geojson"
    log = tmp_path / "mkgrd.log"
    background.write_text(json.dumps(_feature_collection([
        _cell_rect("land_cell", 0, 0, 1, 1),
        _cell_rect("ocean_cell", 1, 0, 2, 1),
    ])))
    river.write_text(json.dumps(_feature_collection([_river("land_cell", "R3")])))
    coast.write_text(json.dumps(_feature_collection([_coast("ocean_cell")])))
    surface.write_text(json.dumps(_feature_collection([
        {
            "type": "Feature",
            "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
            "properties": {"mask_class": "LAND"},
        },
        {
            "type": "Feature",
            "geometry": {"type": "Polygon", "coordinates": [[[1, 0], [2, 0], [2, 1], [1, 1], [1, 0]]]},
            "properties": {"mask_class": "OCEAN"},
        },
    ])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          7\n")

    manifest = write_refinement_delivery_package(
        case_name="surface_html",
        background_geojson=background,
        river_geojson=river,
        coast_geojson=coast,
        surface_geojson=surface,
        log_path=log,
        output_dir=tmp_path / "package",
        unit_sphere_area=False,
    )

    html_text = Path(manifest["files"]["html_map"]).read_text()
    assert "const surfaceCells =" in html_text
    assert "complete LAND/OCEAN cell mask" in html_text
    assert '"surface_class": "LAND"' in html_text
    assert '"surface_class": "OCEAN"' in html_text
    assert "land_cell" in html_text
    assert "ocean_cell" in html_text


def test_refinement_package_accepts_precomputed_complete_cell_mask(tmp_path):
    from util.hydro_mesh.refinement_package import write_refinement_delivery_package

    background = tmp_path / "background_precomputed.geojson"
    river = tmp_path / "river_precomputed.geojson"
    coast = tmp_path / "coast_precomputed.geojson"
    complete = tmp_path / "complete_precomputed.geojson"
    log = tmp_path / "mkgrd_precomputed.log"
    package = tmp_path / "package_precomputed"

    background.write_text(json.dumps(_feature_collection([_cell_rect("a", 0, 0, 1, 1, 1)])))
    river.write_text(json.dumps(_feature_collection([])))
    coast.write_text(json.dumps(_feature_collection([])))
    complete.write_text(json.dumps(_feature_collection([{**_cell_rect("a", 0, 0, 1, 1, 1), "properties": {**_cell_rect("a", 0, 0, 1, 1, 1)["properties"], "surface_class": "LAND", "mask_class": "LAND"}}])))
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          1\n")

    manifest = write_refinement_delivery_package(
        case_name="precomputed_complete",
        background_geojson=background,
        river_geojson=river,
        coast_geojson=coast,
        complete_cell_mask_geojson=complete,
        log_path=log,
        output_dir=package,
        unit_sphere_area=False,
    )

    assert manifest["files"]["complete_cell_mask_geojson"] == str(complete)
    assert "surface_geojson" not in manifest["source_files"]
    assert Path(manifest["files"]["html_map"]).exists()
