import json
from pathlib import Path

import netCDF4
import numpy as np

from util.v3_components.hydro_merit_pipeline import run_merit_v3_pipeline


def test_run_merit_v3_pipeline_writes_cells_masks_v3_outputs_and_map(tmp_path):
    merit_root = tmp_path / "merit"
    output_dir = tmp_path / "out"
    merit_root.mkdir()
    _write_merit_fixture(merit_root / "n20e110.nc")

    outputs = run_merit_v3_pipeline(
        merit_root=merit_root,
        bbox=(110.0, 20.0, 110.005, 20.005),
        nx=3,
        ny=2,
        output_dir=output_dir,
        case_name="fixture_merit_v3",
        recipe_hash="fixture_recipe",
        adapters=["colm2024", "mpas"],
        stride=1,
        html_map=output_dir / "map.html",
        cell_id_prefix="fixture",
        r2_width_m=50.0,
        r3_width_m=300.0,
        r2_upa_km2=5000.0,
        r3_upa_km2=50000.0,
        refine_classes=["R2", "R3"],
        refine_factor=2,
    )

    assert outputs["cells_geojson"] == output_dir / "cells.geojson"
    assert outputs["masks_geojson"] == output_dir / "merit" / "merit_masks.geojson"
    assert outputs["river_masks"] == output_dir / "merit" / "merit_river_masks.geojson"
    assert outputs["coast_masks"] == output_dir / "merit" / "merit_coast_masks.geojson"
    assert outputs["surface_masks"] == output_dir / "merit" / "merit_surface_masks.geojson"
    assert outputs["manifest"] == output_dir / "v3" / "manifest.json"
    assert outputs["canonical_cells_geojson"] == output_dir / "v3" / "canonical_cells.geojson"
    assert outputs["html_map"] == output_dir / "map.html"
    assert outputs["pipeline_summary"] == output_dir / "pipeline_summary.json"
    assert all(path.exists() for path in outputs.values())

    manifest = json.loads(outputs["manifest"].read_text())
    assert manifest["case_name"] == "fixture_merit_v3"
    assert manifest["adapter_versions"] == {"colm2024": "0.1", "mpas": "0.1"}
    assert manifest["missing_mask_count"] == 0
    assert sum(manifest["mask_counts"].values()) > 6
    cells_payload = json.loads(outputs["cells_geojson"].read_text())
    assert len(cells_payload["features"]) > 6

    summary = json.loads(outputs["pipeline_summary"].read_text())
    assert summary["case_name"] == "fixture_merit_v3"
    assert summary["bbox"] == [110.0, 20.0, 110.005, 20.005]
    assert summary["grid"] == {"nx": 3, "ny": 2, "cell_id_prefix": "fixture"}
    assert summary["refinement"] == {"enabled": True, "classes": ["R2", "R3"], "factor": 2}
    assert summary["adapters"] == ["colm2024", "mpas"]
    assert summary["files"]["merit_summary"].endswith("merit/merit_mask_summary.json")
    assert summary["files"]["river_masks"].endswith("merit/merit_river_masks.geojson")
    assert summary["files"]["coast_masks"].endswith("merit/merit_coast_masks.geojson")
    assert summary["files"]["surface_masks"].endswith("merit/merit_surface_masks.geojson")


def _write_merit_fixture(path: Path) -> None:
    with netCDF4.Dataset(path, "w") as ds:
        ds.createDimension("longitude", 6)
        ds.createDimension("latitude", 6)
        lon = ds.createVariable("longitude", "f8", ("longitude",))
        lat = ds.createVariable("latitude", "f8", ("latitude",))
        lon[:] = np.array([110.0, 110.001, 110.002, 110.003, 110.004, 110.005])
        lat[:] = np.array([20.005, 20.004, 20.003, 20.002, 20.001, 20.0])
        for name, dtype in [("dir", "i1"), ("upa", "f4"), ("elv", "f4"), ("wth", "f4"), ("landtype_igbp", "i1")]:
            ds.createVariable(name, dtype, ("longitude", "latitude"))
        ds.variables["dir"][:, :] = 1
        ds.variables["upa"][:, :] = np.array(
            [
                [0, 0, 0, 0, 0, 0],
                [0, 1000, 6000, 1000, 0, 0],
                [0, 2000, 60000, 2000, 0, 0],
                [0, 1000, 6000, 1000, 0, 0],
                [0, 0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0, 0],
            ],
            dtype="f4",
        )
        ds.variables["wth"][:, :] = np.array(
            [
                [0, 0, 0, 0, 0, 0],
                [0, 10, 80, 10, 0, 0],
                [0, 20, 350, 20, 0, 0],
                [0, 10, 80, 10, 0, 0],
                [0, 0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0, 0],
            ],
            dtype="f4",
        )
        ds.variables["elv"][:, :] = 1.0
        ds.variables["landtype_igbp"][:, :] = 1


def test_hydro_merit_pipeline_cli_runs_one_command(tmp_path):
    merit_root = tmp_path / "merit_cli"
    output_dir = tmp_path / "out_cli"
    merit_root.mkdir()
    _write_merit_fixture(merit_root / "n20e110.nc")

    from util.v3_components.hydro_merit_pipeline import main

    exit_code = main(
        [
            "--merit-root",
            str(merit_root),
            "--bbox",
            "110.0",
            "20.0",
            "110.005",
            "20.005",
            "--nx",
            "3",
            "--ny",
            "2",
            "--output-dir",
            str(output_dir),
            "--case-name",
            "fixture_merit_v3_cli",
            "--recipe-hash",
            "fixture_recipe_cli",
            "--adapters",
            "colm2024,mpas",
            "--html-map",
            str(output_dir / "map.html"),
            "--cell-id-prefix",
            "fixture_cli",
            "--refine-classes",
            "R2,R3",
            "--refine-factor",
            "2",
        ]
    )

    assert exit_code == 0
    assert (output_dir / "pipeline_summary.json").exists()
    assert (output_dir / "v3" / "manifest.json").exists()
    assert (output_dir / "map.html").exists()
    summary = json.loads((output_dir / "pipeline_summary.json").read_text())
    assert summary["refinement"] == {"enabled": True, "classes": ["R2", "R3"], "factor": 2}


def test_run_merit_v3_pipeline_is_available_from_component_package():
    from util.v3_components import run_merit_v3_pipeline as exported
    from util.v3_components.hydro_merit_pipeline import run_merit_v3_pipeline

    assert exported is run_merit_v3_pipeline
