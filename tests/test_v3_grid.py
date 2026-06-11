import json
import subprocess
import sys

from util.v3_core.grid import generate_bbox_grid_cells, main, write_bbox_grid_geojson


def test_generate_bbox_grid_cells_creates_ordered_polygon_cells():
    cells = generate_bbox_grid_cells((113.0, 22.0, 115.0, 23.0), nx=2, ny=1, cell_id_prefix="gba")

    assert len(cells) == 2
    assert [cell.cell_id for cell in cells] == ["gba_0000_0000", "gba_0001_0000"]
    assert [cell.cell_index for cell in cells] == [0, 1]
    assert all(cell.cell_type == "POLYGON" for cell in cells)
    assert cells[0].center_lon == 113.5
    assert cells[0].center_lat == 22.5
    assert cells[0].vertices == [(113.0, 22.0), (114.0, 22.0), (114.0, 23.0), (113.0, 23.0)]
    assert cells[1].vertices == [(114.0, 22.0), (115.0, 22.0), (115.0, 23.0), (114.0, 23.0)]


def test_write_bbox_grid_geojson_writes_v3_cells(tmp_path):
    output = tmp_path / "cells.geojson"

    path = write_bbox_grid_geojson((113.0, 22.0, 114.0, 23.0), nx=1, ny=1, output_path=output, cell_id_prefix="cell")

    payload = json.loads(path.read_text())
    assert payload["type"] == "FeatureCollection"
    assert payload["features"][0]["properties"]["cell_id"] == "cell_0000_0000"
    assert payload["features"][0]["properties"]["source_mesh_type"] == "bbox_grid"


def test_bbox_grid_cli_writes_geojson(tmp_path):
    output = tmp_path / "cli_cells.geojson"

    exit_code = main([
        "--bbox", "113.0", "22.0", "114.0", "23.0",
        "--nx", "1",
        "--ny", "1",
        "--output", str(output),
        "--cell-id-prefix", "cli",
    ])

    assert exit_code == 0
    payload = json.loads(output.read_text())
    assert payload["features"][0]["properties"]["cell_id"] == "cli_0000_0000"


def test_v3_core_lazy_exports_grid_helpers():
    from util.v3_core import generate_bbox_grid_cells as exported_generate

    assert exported_generate is generate_bbox_grid_cells


def test_v3_grid_module_help_runs_without_runtime_warning():
    result = subprocess.run(
        [sys.executable, "-m", "util.v3_core.grid", "--help"],
        check=True,
        capture_output=True,
        text=True,
    )

    assert "RuntimeWarning" not in result.stderr
    assert "Generate v3-compatible regular bbox grid cells" in result.stdout
