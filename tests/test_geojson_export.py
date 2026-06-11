import json

from util.hydro_mesh.geojson_export import classified_jsonl_to_geojson, records_to_feature_collection


def test_records_to_feature_collection_filters_classes_and_preserves_properties():
    records = [
        {
            "reach_id": "r0",
            "river_class": "R0",
            "lon": 120.0,
            "lat": 30.0,
            "upstream_area_km2": 10.0,
            "width_m": 20.0,
        },
        {
            "reach_id": "r2",
            "river_class": "R2",
            "lon": 121.0,
            "lat": 31.0,
            "upstream_area_km2": 20000.0,
            "width_m": 200.0,
        },
        {
            "reach_id": "r3",
            "river_class": "R3",
            "lon": 122.0,
            "lat": 32.0,
            "upstream_area_km2": 100000.0,
            "width_m": 3000.0,
        },
    ]

    collection = records_to_feature_collection(records, include_classes={"R2", "R3"})

    assert collection["type"] == "FeatureCollection"
    assert [feature["properties"]["reach_id"] for feature in collection["features"]] == ["r2", "r3"]
    assert collection["features"][0]["geometry"] == {"type": "Point", "coordinates": [121.0, 31.0]}
    assert collection["features"][1]["properties"]["width_m"] == 3000.0


def test_classified_jsonl_to_geojson_writes_feature_collection(tmp_path):
    input_jsonl = tmp_path / "classified.jsonl"
    output_geojson = tmp_path / "r23.geojson"
    input_jsonl.write_text(
        json.dumps({"reach_id": "r1", "river_class": "R1", "lon": 120.0, "lat": 30.0}) + "\n"
        + json.dumps({"reach_id": "r3", "river_class": "R3", "lon": 121.0, "lat": 31.0}) + "\n"
    )

    collection = classified_jsonl_to_geojson(input_jsonl, output_geojson, include_classes={"R3"})

    assert output_geojson.exists()
    written = json.loads(output_geojson.read_text())
    assert written == collection
    assert len(written["features"]) == 1
    assert written["features"][0]["properties"]["reach_id"] == "r3"
