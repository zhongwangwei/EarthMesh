import json


def _report(case_name, *, status="pass", l1=0, l2=0, l3=0, cells=0, median=0.0, river=0, coast=0):
    return {
        "case_name": case_name,
        "status": status,
        "background_cells": {
            "cell_count": cells,
            "equivalent_cell_size_km_median": median,
        },
        "river_intersections": {"feature_count": river},
        "coast_intersections": {"feature_count": coast},
        "refinement_log": {
            "1": {"retained_triangles": l1},
            "2": {"retained_triangles": l2},
            "3": {"retained_triangles": l3},
        },
    }


def test_build_river_coast_sweep_creates_stable_composite_recipes():
    from util.hydro_mesh.refinement_sweep import build_river_coast_sweep

    cases = build_river_coast_sweep(
        river_geojson="river.geojson",
        coast_geojson="coast.geojson",
        r2_caps=[40, 60],
        coast_caps=[10, 20],
    )

    assert [case["case_name"] for case in cases] == [
        "r2cap40_coast10",
        "r2cap40_coast20",
        "r2cap60_coast10",
        "r2cap60_coast20",
    ]
    recipe = cases[0]["recipe"]
    assert recipe["max_masks_per_refine_degree"] == 999
    assert recipe["components"][0]["name"] == "coastline_support"
    assert recipe["components"][0]["input_geojson"] == "coast.geojson"
    assert recipe["components"][0]["class_refine"] == {"COAST": 1}
    assert recipe["components"][0]["max_rings_by_class"] == {"COAST": 10}
    assert recipe["components"][1]["name"] == "ranked_river_corridors"
    assert recipe["components"][1]["input_geojson"] == "river.geojson"
    assert recipe["components"][1]["class_refine"] == {"R2": 1, "R3": 3}
    assert recipe["components"][1]["max_rings_by_class"] == {"R2": 40, "R3": 19}
    assert recipe["components"][1]["buffer_deg_by_refine_degree"] == {"1": 1.5, "2": 1.0, "3": 0.5}


def test_write_sweep_recipes_writes_manifest_and_case_json(tmp_path):
    from util.hydro_mesh.refinement_sweep import write_sweep_recipes

    paths = write_sweep_recipes(
        output_dir=tmp_path,
        river_geojson="river.geojson",
        coast_geojson="coast.geojson",
        r2_caps=[40],
        coast_caps=[10, 20],
    )

    assert sorted(paths) == ["manifest", "r2cap40_coast10", "r2cap40_coast20"]
    manifest = json.loads(paths["manifest"].read_text())
    assert manifest["kind"] == "earthmesh_refinement_sweep_manifest"
    assert manifest["case_count"] == 2
    assert manifest["cases"][0]["recipe_json"].endswith("r2cap40_coast10_recipe.json")
    recipe = json.loads(paths["r2cap40_coast20"].read_text())
    assert recipe["components"][0]["max_rings_by_class"] == {"COAST": 20}


def test_rank_sweep_reports_prefers_retained_high_level_refinement_then_overlap_and_size():
    from util.hydro_mesh.refinement_sweep import rank_sweep_reports

    ranked = rank_sweep_reports(
        [
            _report("failed", status="failed", l1=999, l2=999, l3=999, cells=10, median=1, river=999, coast=999),
            _report("too_big", l1=120, l2=120, l3=120, cells=5000, median=5, river=600, coast=500),
            _report("coarser", l1=100, l2=90, l3=80, cells=500, median=18, river=200, coast=80),
            _report("better_l3", l1=90, l2=91, l3=90, cells=600, median=20, river=190, coast=75),
            _report("tie_smaller", l1=90, l2=91, l3=90, cells=550, median=12, river=190, coast=75),
        ],
        max_background_cells=1000,
    )

    assert [item["case_name"] for item in ranked] == ["tie_smaller", "better_l3", "coarser", "too_big", "failed"]
    assert ranked[0]["rank"] == 1
    assert ranked[0]["promotion_status"] == "candidate"
    assert ranked[3]["promotion_status"] == "blocked_background_cell_cap"
    assert ranked[4]["promotion_status"] == "failed"


def test_write_sweep_ranking_reads_reports_and_writes_recommendation(tmp_path):
    from util.hydro_mesh.refinement_sweep import write_sweep_ranking

    report_a = tmp_path / "a.json"
    report_b = tmp_path / "b.json"
    output = tmp_path / "ranking.json"
    report_a.write_text(json.dumps(_report("a", l1=1, l2=1, l3=1, cells=100, median=20, river=10, coast=5)))
    report_b.write_text(json.dumps(_report("b", l1=1, l2=1, l3=2, cells=120, median=25, river=8, coast=3)))

    payload = write_sweep_ranking([report_a, report_b], output)

    assert payload["recommended_case"] == "b"
    written = json.loads(output.read_text())
    assert written["ranked_cases"][0]["case_name"] == "b"


def test_refinement_sweep_cli_write_recipes_and_rank(tmp_path):
    from util.hydro_mesh.refinement_sweep import main

    recipes_dir = tmp_path / "recipes"
    assert main([
        "write-recipes",
        "--river-geojson",
        "river.geojson",
        "--coast-geojson",
        "coast.geojson",
        "--output-dir",
        str(recipes_dir),
        "--r2-caps",
        "40,60",
        "--coast-caps",
        "10",
    ]) == 0
    assert (recipes_dir / "sweep_manifest.json").exists()
    assert (recipes_dir / "r2cap60_coast10_recipe.json").exists()

    report = tmp_path / "report.json"
    ranking = tmp_path / "ranking.json"
    report.write_text(json.dumps(_report("cli_case", l1=2, l2=3, l3=4, cells=100, median=10, river=8, coast=2)))
    assert main(["rank", "--reports", str(report), "--output-json", str(ranking)]) == 0
    assert json.loads(ranking.read_text())["recommended_case"] == "cli_case"
