from util.v3_core.demo import build_demo_inputs
from util.v3_core.pipeline import build_v3_pipeline_result


def test_build_gba_demo_inputs_contains_core_semantic_layers():
    demo = build_demo_inputs("gba")

    assert demo.name == "gba"
    assert demo.description.startswith("Synthetic Greater Bay Area")
    assert len(demo.cells) >= 4
    assert len(demo.masks) >= 4
    assert {cell.cell_id for cell in demo.cells} >= {"gba_land", "gba_ocean", "gba_coast", "pearl_river"}
    assert {mask.mask_class for mask in demo.masks} >= {"LAND", "OCEAN", "COAST_LAND", "R3"}


def test_gba_demo_runs_through_v3_pipeline_semantics():
    demo = build_demo_inputs("gba")

    result = build_v3_pipeline_result(
        case_name="gba_demo",
        recipe_hash="demo",
        cells=demo.cells,
        masks=demo.masks,
        adapter_names=["colm2024", "mpas"],
    )

    by_id = {cell.cell_id: cell for cell in result.cells}
    assert by_id["gba_land"].surface_class == "LAND"
    assert by_id["gba_ocean"].surface_class == "OCEAN"
    assert by_id["gba_coast"].coast_class == "COAST_LAND"
    assert by_id["pearl_river"].hydro_class == "R3"
    assert result.manifest.missing_mask_count == 0
    assert result.manifest.mask_counts["R3"] == 1
