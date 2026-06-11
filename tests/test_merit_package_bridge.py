import json
from pathlib import Path

import netCDF4
import numpy as np


def _collection(features):
    return {"type": "FeatureCollection", "features": features}


def _cell(cell_id, x0, y0, x1, y1, index):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [[[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]]},
        "properties": {
            "cell_id": cell_id,
            "cell_index": index,
            "center_lon": (x0 + x1) / 2,
            "center_lat": (y0 + y1) / 2,
            "source_areaCell": 1.0,
            "source_areaCell_units": "m^2",
        },
    }


def test_write_merit_refinement_delivery_package_builds_package_inputs(tmp_path):
    from util.hydro_mesh.merit_package_bridge import write_merit_refinement_delivery_package
    from util.hydro_mesh.colm_coupling import write_colm_package_coupling

    merit_root = tmp_path / "merit"
    merit_root.mkdir()
    _write_merit_fixture(merit_root / "n20e110.nc")
    background = tmp_path / "background.geojson"
    background.write_text(json.dumps(_collection([
        _cell("west", 110.0, 20.0, 110.003, 20.006, 1),
        _cell("east", 110.003, 20.0, 110.006, 20.006, 2),
    ])))
    log = tmp_path / "mkgrd.log"
    log.write_text(" refine_degree =            3\n 去除孤立细化三角形后，需要细化的三角形：          2\n")

    result = write_merit_refinement_delivery_package(
        case_name="fixture_merit_package",
        background_geojson=background,
        merit_root=merit_root,
        bbox=(110.0, 20.0, 110.005, 20.005),
        log_path=log,
        output_dir=tmp_path / "package",
        stride=1,
        r2_width_m=50.0,
        r3_width_m=300.0,
        r2_upa_km2=5000.0,
        r3_upa_km2=50000.0,
        unit_sphere_area=False,
    )

    assert result["manifest_path"].name == "delivery_manifest.json"
    assert result["river_intersections"].exists()
    assert result["coast_intersections"].exists()
    assert result["surface_masks"].exists()
    assert result["merit_summary"].exists()
    assert result["bridge_summary"].exists()

    river_payload = json.loads(result["river_intersections"].read_text())
    coast_payload = json.loads(result["coast_intersections"].read_text())
    assert {feature["properties"]["river_class"] for feature in river_payload["features"]} >= {"R2", "R3"}
    assert {feature["properties"]["mask_class"] for feature in coast_payload["features"]} >= {"COAST_LAND", "COAST_OCEAN"}
    assert all("coastal_fraction" in feature["properties"] for feature in coast_payload["features"])

    manifest = json.loads(result["manifest_path"].read_text())
    assert manifest["files"]["complete_cell_mask_geojson"].endswith("fixture_merit_package_complete_cell_mask.geojson")
    assert manifest["source_files"]["surface_geojson"] == str(result["surface_masks"])
    assert manifest["source_files"]["river_geojson"] == str(result["river_intersections"])
    assert manifest["source_files"]["coast_geojson"] == str(result["coast_intersections"])

    coupling = write_colm_package_coupling(result["manifest_path"], tmp_path / "colm")
    summary = coupling["summary"]
    assert summary["rows_written"] == 2
    assert summary["surface_source_kind"] == "complete_cell_mask_geojson"
    assert summary["surface_class_counts"]


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
        landtype = np.ones((6, 6), dtype="i1")
        landtype[3:, :] = 17
        ds.variables["landtype_igbp"][:, :] = landtype
