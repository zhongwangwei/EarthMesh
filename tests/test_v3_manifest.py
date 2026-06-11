import json

from util.v3_core.manifest import V3RunManifest


def test_manifest_writes_reproducible_summary_json(tmp_path):
    manifest = V3RunManifest(
        case_name="china_coast_hydro",
        recipe_hash="abc123",
        component_versions={"hydro_cama": "0.1"},
        adapter_versions={"colm2024": "0.1"},
        input_sources={"cama": "/data/glb_01min"},
        mask_counts={"LAND": 10, "OCEAN": 5, "COAST": 2},
        cell_counts_by_class={"HEX": 17},
        missing_mask_count=0,
        cell_size_distribution={"median_km": 5.6},
        coupling_row_counts={"river_land": 4},
        qa_artifacts=["case.html", "case.png"],
        warnings=[],
    )

    output = tmp_path / "manifest.json"
    manifest.write_json(output)
    payload = json.loads(output.read_text())

    assert payload["case_name"] == "china_coast_hydro"
    assert payload["missing_mask_count"] == 0
    assert payload["mask_counts"]["COAST"] == 2
    assert payload["qa_artifacts"] == ["case.html", "case.png"]
