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


def _collection(features):
    return {"type": "FeatureCollection", "features": features}


def _write_package_fixture(tmp_path):
    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    manifest = tmp_path / "delivery_manifest.json"
    background.write_text(json.dumps(_collection([
        _feature({"cell_id": "a", "cell_index": 1, "center_lon": 120.0, "center_lat": 30.0, "source_areaCell": 10.0, "source_areaCell_units": "m^2"}),
        _feature({"cell_id": "b", "cell_index": 2, "center_lon": 121.0, "center_lat": 31.0, "source_areaCell": 20.0, "source_areaCell_units": "m^2"}),
        _feature({"cell_id": "c", "cell_index": 3, "center_lon": 122.0, "center_lat": 32.0, "source_areaCell": 30.0, "source_areaCell_units": "m^2"}),
    ])))
    river.write_text(json.dumps(_collection([
        _feature({"cell_id": "a", "river_class": "R3", "river_fraction": 0.4, "estimated_river_area_m2": 40.0, "normalized_cell_area_m2": 100.0}),
        _feature({"cell_id": "b", "river_class": "R2", "river_fraction": 0.2, "estimated_river_area_m2": 20.0, "normalized_cell_area_m2": 100.0}),
    ])))
    coast.write_text(json.dumps(_collection([
        _feature({"cell_id": "a", "mask_class": "COAST", "coastal_fraction": 0.3}),
        _feature({"cell_id": "c", "mask_class": "COAST", "coastal_fraction": 0.8}),
    ])))
    manifest.write_text(json.dumps({
        "kind": "earthmesh_hydro_coast_delivery_package",
        "case_name": "fixture_case",
        "recommended_case": "fixture_case",
        "source_files": {
            "background_geojson": str(background),
            "river_geojson": str(river),
            "coast_geojson": str(coast),
            "log_path": str(tmp_path / "mkgrd.log"),
        },
    }))
    return manifest


def test_write_colm_package_coupling_writes_all_background_cells(tmp_path):
    from util.hydro_mesh.colm_coupling import write_colm_package_coupling

    manifest = _write_package_fixture(tmp_path)
    output_dir = tmp_path / "colm"

    result = write_colm_package_coupling(manifest, output_dir)

    assert result["summary"]["background_cell_count"] == 3
    assert result["summary"]["river_cell_count"] == 2
    assert result["summary"]["coast_cell_count"] == 2
    assert result["summary"]["rows_written"] == 3
    rows = list(csv.DictReader(open(result["csv_path"], newline="")))
    assert [row["cell_id"] for row in rows] == ["a", "b", "c"]
    assert rows[0]["has_river"] == "true"
    assert rows[0]["river_class"] == "R3"
    assert rows[0]["has_coast"] == "true"
    assert rows[0]["coast_class"] == "COAST"
    assert rows[1]["has_river"] == "true"
    assert rows[1]["has_coast"] == "false"
    assert rows[2]["has_river"] == "false"
    assert rows[2]["has_coast"] == "true"
    assert rows[2]["surface_class"] == "COAST"


def test_colm_coupling_package_cli_writes_csv_and_summary(tmp_path):
    from util.hydro_mesh.colm_coupling import main

    manifest = _write_package_fixture(tmp_path)
    output_dir = tmp_path / "cli_colm"

    assert main(["package", "--delivery-manifest", str(manifest), "--output-dir", str(output_dir)]) == 0

    assert (output_dir / "colm_coupling_cells.csv").exists()
    summary = json.loads((output_dir / "colm_coupling_summary.json").read_text())
    assert summary["case_name"] == "fixture_case"
    assert summary["rows_written"] == 3


def test_colm_coupling_package_cli_works_from_process_argv(tmp_path, monkeypatch):
    from util.hydro_mesh.colm_coupling import main

    manifest = _write_package_fixture(tmp_path)
    output_dir = tmp_path / "argv_colm"
    monkeypatch.setattr(
        "sys.argv",
        [
            "colm_coupling.py",
            "package",
            "--delivery-manifest",
            str(manifest),
            "--output-dir",
            str(output_dir),
        ],
    )

    assert main() == 0
    assert (output_dir / "colm_coupling_cells.csv").exists()


def test_package_coupling_aggregates_multiple_overlap_records_per_cell(tmp_path):
    from util.hydro_mesh.colm_coupling import write_colm_package_coupling

    background = tmp_path / "background.geojson"
    river = tmp_path / "river.geojson"
    coast = tmp_path / "coast.geojson"
    manifest = tmp_path / "delivery_manifest.json"
    background.write_text(json.dumps(_collection([
        _feature({"cell_id": "a", "cell_index": 1, "source_areaCell": 10.0}),
    ])))
    river.write_text(json.dumps(_collection([
        _feature({"cell_id": "a", "river_class": "R2", "river_fraction": 0.2, "estimated_river_area_m2": 20.0, "normalized_cell_area_m2": 100.0}),
        _feature({"cell_id": "a", "river_class": "R3", "river_fraction": 0.4, "estimated_river_area_m2": 40.0, "normalized_cell_area_m2": 100.0}),
    ])))
    coast.write_text(json.dumps(_collection([
        _feature({"cell_id": "a", "mask_class": "COAST", "coastal_fraction": 0.3}),
        _feature({"cell_id": "a", "mask_class": "COAST", "coastal_fraction": 0.4}),
    ])))
    manifest.write_text(json.dumps({
        "case_name": "aggregate_case",
        "source_files": {
            "background_geojson": str(background),
            "river_geojson": str(river),
            "coast_geojson": str(coast),
        },
    }))

    result = write_colm_package_coupling(manifest, tmp_path / "colm")

    row = next(csv.DictReader(open(result["csv_path"], newline="")))
    assert row["river_class"] == "R3"
    assert row["river_fraction"] == "0.6"
    assert row["estimated_river_area_m2"] == "60.0"
    assert row["coastal_fraction"] == "0.7"
    assert result["summary"]["river_overlap_record_count"] == 2
    assert result["summary"]["river_cell_count"] == 1
    assert result["summary"]["coast_overlap_record_count"] == 2
    assert result["summary"]["coast_cell_count"] == 1
