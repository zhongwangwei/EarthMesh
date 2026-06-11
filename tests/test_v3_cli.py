import json

from util.v3_core.cli import main


def test_v3_cli_runs_pipeline_from_json_inputs(tmp_path):
    cells_path = tmp_path / "cells.json"
    masks_path = tmp_path / "masks.json"
    output_dir = tmp_path / "out"
    cells_path.write_text(
        json.dumps(
            [
                {
                    "cell_id": "land",
                    "cell_index": 0,
                    "cell_type": "TRI",
                    "center_lon": 0.0,
                    "center_lat": 0.0,
                    "area_m2": 0.5,
                    "vertices": [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                },
                {
                    "cell_id": "water",
                    "cell_index": 1,
                    "cell_type": "HEX",
                    "center_lon": 2.0,
                    "center_lat": 2.0,
                    "area_m2": 0.5,
                    "vertices": [[2.0, 2.0], [3.0, 2.0], [2.0, 3.0]],
                },
            ]
        )
    )
    masks_path.write_text(
        json.dumps(
            [
                {
                    "feature_id": "land-mask",
                    "mask_class": "LAND",
                    "priority": 1,
                    "polygon": [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                }
            ]
        )
    )

    exit_code = main(
        [
            "--case-name",
            "cli_case",
            "--recipe-hash",
            "abc123",
            "--cells",
            str(cells_path),
            "--masks",
            str(masks_path),
            "--adapters",
            "colm2024,mpas",
            "--output-dir",
            str(output_dir),
        ]
    )

    assert exit_code == 0
    manifest = json.loads((output_dir / "manifest.json").read_text())
    overlay_summary = json.loads((output_dir / "overlay_summary.json").read_text())
    adapter = json.loads((output_dir / "adapter_colm2024.json").read_text())
    projected_cells = json.loads((output_dir / "canonical_cells.json").read_text())
    projected_geojson = json.loads((output_dir / "canonical_cells.geojson").read_text())
    assert manifest["case_name"] == "cli_case"
    assert manifest["missing_mask_count"] == 1
    assert overlay_summary["missing_mask_count"] == 1
    assert overlay_summary["winning_class_counts"] == {"LAND": 1, "UNKNOWN": 1}
    assert adapter["adapter_name"] == "colm2024"
    assert [cell["surface_class"] for cell in projected_cells] == ["LAND", "UNKNOWN"]
    assert projected_geojson["features"][0]["properties"]["surface_class"] == "LAND"


def test_v3_cli_runs_pipeline_from_geojson_inputs(tmp_path):
    cells_path = tmp_path / "cells.geojson"
    masks_path = tmp_path / "masks.geojson"
    output_dir = tmp_path / "geojson_out"
    cells_path.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {
                            "type": "Polygon",
                            "coordinates": [[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]],
                        },
                        "properties": {"cell_id": "land", "cell_type": "TRI", "area_m2": 0.5},
                    }
                ],
            }
        )
    )
    masks_path.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {
                            "type": "Polygon",
                            "coordinates": [[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]],
                        },
                        "properties": {"feature_id": "land-mask", "surface_class": "LAND"},
                    }
                ],
            }
        )
    )

    exit_code = main(
        [
            "--case-name",
            "geojson_case",
            "--recipe-hash",
            "abc123",
            "--cells-geojson",
            str(cells_path),
            "--masks-geojson",
            str(masks_path),
            "--adapters",
            "colm2024",
            "--output-dir",
            str(output_dir),
        ]
    )

    assert exit_code == 0
    manifest = json.loads((output_dir / "manifest.json").read_text())
    projected_cells = json.loads((output_dir / "canonical_cells.json").read_text())
    assert manifest["mask_counts"] == {"LAND": 1}
    assert projected_cells[0]["surface_class"] == "LAND"


def test_v3_cli_writes_optional_html_map(tmp_path):
    cells_path = tmp_path / "cells.geojson"
    masks_path = tmp_path / "masks.geojson"
    output_dir = tmp_path / "html_out"
    html_map = tmp_path / "html_out" / "v3_map.html"
    cells_path.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {
                            "type": "Polygon",
                            "coordinates": [[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]],
                        },
                        "properties": {"cell_id": "land", "cell_type": "TRI", "area_m2": 0.5},
                    }
                ],
            }
        )
    )
    masks_path.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {
                            "type": "Polygon",
                            "coordinates": [[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]],
                        },
                        "properties": {"feature_id": "land-mask", "surface_class": "LAND"},
                    }
                ],
            }
        )
    )

    exit_code = main(
        [
            "--case-name",
            "html_case",
            "--recipe-hash",
            "abc123",
            "--cells-geojson",
            str(cells_path),
            "--masks-geojson",
            str(masks_path),
            "--adapters",
            "colm2024",
            "--output-dir",
            str(output_dir),
            "--html-map",
            str(html_map),
        ]
    )

    assert exit_code == 0
    text = html_map.read_text()
    assert "html_case" in text
    assert "const canonicalCells =" in text
    assert "LAND" in text


def test_v3_cli_runs_builtin_gba_demo(tmp_path):
    output_dir = tmp_path / "gba_demo"
    html_map = output_dir / "map.html"

    exit_code = main(
        [
            "--case-name",
            "gba_demo",
            "--recipe-hash",
            "demo",
            "--demo",
            "gba",
            "--adapters",
            "colm2024,mpas",
            "--output-dir",
            str(output_dir),
            "--html-map",
            str(html_map),
        ]
    )

    assert exit_code == 0
    manifest = json.loads((output_dir / "manifest.json").read_text())
    cells = json.loads((output_dir / "canonical_cells.json").read_text())
    html = html_map.read_text()
    assert manifest["case_name"] == "gba_demo"
    assert manifest["missing_mask_count"] == 0
    assert manifest["mask_counts"]["R3"] == 1
    assert {cell["cell_id"] for cell in cells} >= {"gba_land", "gba_ocean", "gba_coast", "pearl_river"}
    assert "pearl_river" in html
