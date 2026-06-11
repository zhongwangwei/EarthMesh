import json


def test_build_close_refinement_recipe_records_masks_and_namelist_overrides():
    from util.hydro_mesh.refinement_recipe import build_close_refinement_recipe

    recipe = build_close_refinement_recipe(
        input_geojson="/scratch/corridors.geojson",
        output_prefix="/scratch/refine_spc_hydro",
        class_refine={"R2": 1, "R3": 3},
        buffer_deg_by_refine_degree={1: 1.0, 2: 0.3, 3: 0.1},
        simplify_tolerance_deg=0.005,
        example_namelist="/scratch/hydro.nml",
    )

    assert recipe["earthmesh_namelist_overrides"] == {
        "RL%refine_spc": ".TRUE.",
        "RL%max_iter_spc": "3",
        "RL%mask_refine_spc_type": "'close'",
        "RL%mask_refine_spc_fprefix": "'/scratch/refine_spc_hydro'",
    }
    assert recipe["close_mask_command"] == [
        "python3",
        "-m",
        "util.hydro_mesh.refine_mask_export",
        "/scratch/corridors.geojson",
        "/scratch/refine_spc_hydro",
        "--class-refine",
        "R2=1",
        "R3=3",
        "--buffer-deg-by-refine-degree",
        "1=1.0",
        "2=0.3",
        "3=0.1",
        "--simplify-tolerance-deg",
        "0.005",
    ]
    assert recipe["smoke_run_command"] == ["./mkgrd.x", "/scratch/hydro.nml"]


def test_write_close_refinement_recipe_json_writes_readable_recipe(tmp_path):
    from util.hydro_mesh.refinement_recipe import write_close_refinement_recipe_json

    output_json = tmp_path / "recipe.json"

    write_close_refinement_recipe_json(
        output_json,
        input_geojson="/scratch/corridors.geojson",
        output_prefix="/scratch/refine_spc_hydro",
        class_refine={"R3": 2},
        buffer_deg_by_refine_degree={1: 1.0, 2: 0.2},
    )

    written = json.loads(output_json.read_text())
    assert written["class_refine"] == {"R3": 2}
    assert written["earthmesh_namelist_overrides"]["RL%max_iter_spc"] == "2"
