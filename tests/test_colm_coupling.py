import csv
import json


def _feature(properties):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
        "properties": properties,
    }


def test_intersections_to_coupling_rows_keeps_stable_colm_fields():
    from util.hydro_mesh.colm_coupling import intersections_to_coupling_rows

    collection = {
        "type": "FeatureCollection",
        "features": [
            _feature(
                {
                    "cell_id": "10",
                    "cell_index": 9,
                    "river_class": "R3",
                    "river_fraction": 0.25,
                    "estimated_river_area_m2": 125.0,
                    "normalized_cell_area_m2": 500.0,
                    "center_lon": 120.5,
                    "center_lat": 31.5,
                    "domain_clip_applied": True,
                    "area_normalization": "unit_sphere_area_to_m2",
                }
            ),
            _feature({"cell_id": "11", "river_class": "R2", "river_fraction": 0.01}),
        ],
    }

    rows = intersections_to_coupling_rows(collection, min_fraction=0.05)

    assert rows == [
        {
            "cell_id": "10",
            "cell_index": 9,
            "river_class": "R3",
            "river_fraction": 0.25,
            "estimated_river_area_m2": 125.0,
            "normalized_cell_area_m2": 500.0,
            "center_lon": 120.5,
            "center_lat": 31.5,
            "domain_clip_applied": True,
            "area_normalization": "unit_sphere_area_to_m2",
        }
    ]


def test_write_colm_coupling_csv_writes_ordered_header_and_rows(tmp_path):
    from util.hydro_mesh.colm_coupling import COUPLING_FIELDS, write_colm_coupling_csv

    input_geojson = tmp_path / "intersections.geojson"
    output_csv = tmp_path / "coupling.csv"
    input_geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    _feature({"cell_id": "b", "river_class": "R2", "river_fraction": 0.2, "cell_index": 2}),
                    _feature({"cell_id": "a", "river_class": "R3", "river_fraction": 0.4, "cell_index": 1}),
                ],
            }
        )
    )

    rows = write_colm_coupling_csv(input_geojson, output_csv)

    assert [row["cell_id"] for row in rows] == ["a", "b"]
    with output_csv.open(newline="") as handle:
        reader = csv.DictReader(handle)
        assert reader.fieldnames == COUPLING_FIELDS
        written = list(reader)
    assert written[0]["cell_id"] == "a"
    assert written[0]["river_class"] == "R3"
    assert written[0]["river_fraction"] == "0.4"


def test_write_colm_coupling_jsonl_writes_one_record_per_line(tmp_path):
    from util.hydro_mesh.colm_coupling import write_colm_coupling_jsonl

    input_geojson = tmp_path / "intersections.geojson"
    output_jsonl = tmp_path / "coupling.jsonl"
    input_geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    _feature({"cell_id": "a", "river_class": "R3", "river_fraction": 0.4}),
                ],
            }
        )
    )

    write_colm_coupling_jsonl(input_geojson, output_jsonl)

    lines = output_jsonl.read_text().splitlines()
    assert len(lines) == 1
    assert json.loads(lines[0])["cell_id"] == "a"
