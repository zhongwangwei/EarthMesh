import json


def _feature_collection(features):
    return {"type": "FeatureCollection", "features": features}


def _cell(cell_id):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
        "properties": {"cell_id": cell_id, "source_areaCell": 1.0},
    }


def test_merge_cell_masks_outputs_one_masked_feature_per_background_cell_with_priority():
    from util.hydro_mesh.cell_mask_merge import merge_cell_masks

    background = _feature_collection([_cell("land"), _cell("coast"), _cell("r2"), _cell("r3"), _cell("both")])
    coast = _feature_collection(
        [
            {**_cell("coast"), "properties": {"cell_id": "coast", "mask_class": "COAST", "coastal_fraction": 0.25}},
            {**_cell("both"), "properties": {"cell_id": "both", "mask_class": "COAST", "coastal_fraction": 0.5}},
        ]
    )
    rivers = _feature_collection(
        [
            {**_cell("r2"), "properties": {"cell_id": "r2", "river_class": "R2", "river_fraction": 0.3}},
            {**_cell("r3"), "properties": {"cell_id": "r3", "river_class": "R3", "river_fraction": 0.4}},
            {**_cell("both"), "properties": {"cell_id": "both", "river_class": "R3", "river_fraction": 0.6}},
        ]
    )

    merged = merge_cell_masks(background, river_cells=rivers, coast_cells=coast)

    by_id = {feature["properties"]["cell_id"]: feature for feature in merged["features"]}
    assert list(by_id) == ["land", "coast", "r2", "r3", "both"]
    assert by_id["land"]["properties"]["mask_class"] == "BACKGROUND"
    assert by_id["land"]["properties"]["mask_source"] == "background"
    assert by_id["coast"]["properties"]["mask_class"] == "COAST"
    assert by_id["coast"]["properties"]["coastal_fraction"] == 0.25
    assert by_id["r2"]["properties"]["mask_class"] == "R2"
    assert by_id["r3"]["properties"]["mask_class"] == "R3"
    assert by_id["both"]["properties"]["mask_class"] == "R3"
    assert by_id["both"]["properties"]["coastal_fraction"] == 0.5
    assert len(merged["features"]) == len(background["features"])


def test_write_complete_cell_mask_geojson_round_trips_files(tmp_path):
    from util.hydro_mesh.cell_mask_merge import write_complete_cell_mask_geojson

    background_path = tmp_path / "background.geojson"
    river_path = tmp_path / "river.geojson"
    coast_path = tmp_path / "coast.geojson"
    output_path = tmp_path / "complete.geojson"
    background_path.write_text(json.dumps(_feature_collection([_cell("land"), _cell("coast")])) + "\n")
    river_path.write_text(json.dumps(_feature_collection([])) + "\n")
    coast_path.write_text(json.dumps(_feature_collection([{**_cell("coast"), "properties": {"cell_id": "coast", "mask_class": "COAST"}}])) + "\n")

    write_complete_cell_mask_geojson(background_path, output_path, river_geojson=river_path, coast_geojson=coast_path)

    written = json.loads(output_path.read_text())
    assert [feature["properties"]["mask_class"] for feature in written["features"]] == ["BACKGROUND", "COAST"]
