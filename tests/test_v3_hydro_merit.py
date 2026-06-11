import json
from pathlib import Path

import netCDF4
import numpy as np

from util.v3_components.hydro_merit import (
    build_merit_masks,
    MeritWindow,
    read_merit_window,
    select_merit_tiles,
    split_merit_mask_layers,
    tile_bounds_from_name,
    write_merit_mask_outputs,
)


def test_tile_bounds_from_merit_name():
    assert tile_bounds_from_name("n20e110.nc") == (110.0, 20.0, 115.0, 25.0)
    assert tile_bounds_from_name("s05w010.nc") == (-10.0, -5.0, -5.0, 0.0)


def test_select_merit_tiles_for_bbox(tmp_path):
    for name in ["n20e110.nc", "n20e115.nc", "n25e110.nc", "s05w010.nc"]:
        (tmp_path / name).write_text("placeholder")

    selected = select_merit_tiles(tmp_path, bbox=(112.8, 21.5, 114.8, 23.5))

    assert [path.name for path in selected] == ["n20e110.nc"]


def test_read_merit_window_reads_bbox_subset(tmp_path):
    tile = tmp_path / "n20e110.nc"
    _write_merit_fixture(tile)

    window = read_merit_window(tile, bbox=(110.0, 20.0, 110.005, 20.005), stride=2)

    assert isinstance(window, MeritWindow)
    assert window.tile_path == tile
    assert window.lon.shape == (3,)
    assert window.lat.shape == (3,)
    assert window.wth.shape == (3, 3)
    assert window.upa.shape == (3, 3)
    assert float(window.wth.max()) > 300.0
    assert float(window.upa.max()) > 50000.0


def test_build_merit_masks_classifies_rivers_and_surface(tmp_path):
    tile = tmp_path / "n20e110.nc"
    _write_merit_fixture(tile)
    window = read_merit_window(tile, bbox=(110.0, 20.0, 110.005, 20.005), stride=1)

    masks, summary = build_merit_masks([window], r2_width_m=50.0, r3_width_m=300.0, r2_upa_km2=5000.0, r3_upa_km2=50000.0)

    classes = [feature["properties"]["mask_class"] for feature in masks["features"]]
    assert "R2" in classes
    assert "R3" in classes
    assert "LAND" in classes
    assert summary["mask_counts"]["R3"] == 1
    assert summary["mask_counts"]["R2"] >= 1
    assert summary["tile_count"] == 1


def test_build_merit_masks_marks_land_ocean_adjacency_as_coast(tmp_path):
    tile = tmp_path / "n20e110.nc"
    _write_merit_coast_fixture(tile)
    window = read_merit_window(tile, bbox=(110.0, 20.0, 110.005, 20.005), stride=1)

    masks, summary = build_merit_masks([window])

    classes = [feature["properties"]["mask_class"] for feature in masks["features"]]
    assert "COAST_LAND" in classes
    assert "COAST_OCEAN" in classes
    assert summary["mask_counts"]["COAST_LAND"] > 0
    assert summary["mask_counts"]["COAST_OCEAN"] > 0


def test_split_merit_mask_layers_returns_surface_coast_and_river_layers(tmp_path):
    tile = tmp_path / "n20e110.nc"
    _write_merit_fixture(tile)
    window = read_merit_window(tile, bbox=(110.0, 20.0, 110.005, 20.005), stride=1)
    masks, _summary = build_merit_masks([window], r2_width_m=50.0, r3_width_m=300.0, r2_upa_km2=5000.0, r3_upa_km2=50000.0)

    layers = split_merit_mask_layers(masks)

    assert sorted(layers) == ["coast", "river", "surface"]
    assert {feature["properties"]["mask_class"] for feature in layers["river"]["features"]} >= {"R2", "R3"}
    assert {feature["properties"]["mask_class"] for feature in layers["surface"]["features"]} <= {"LAND", "OCEAN"}


def test_write_merit_mask_outputs_can_skip_surface_geojson(tmp_path):
    tile = tmp_path / "n20e110.nc"
    out = tmp_path / "out_skip_surface"
    _write_merit_fixture(tile)

    outputs = write_merit_mask_outputs(
        tmp_path,
        bbox=(110.0, 20.0, 110.005, 20.005),
        output_dir=out,
        stride=1,
        write_surface_mask=False,
    )

    assert outputs["surface_masks"] is None
    assert not (out / "merit_surface_masks.geojson").exists()
    assert outputs["masks"].exists()
    assert outputs["river_masks"].exists()
    assert outputs["coast_masks"].exists()
    assert outputs["summary"].exists()


def test_write_merit_mask_outputs_can_skip_combined_geojson(tmp_path):
    tile = tmp_path / "n20e110.nc"
    out = tmp_path / "out_skip_combined"
    _write_merit_fixture(tile)

    outputs = write_merit_mask_outputs(
        tmp_path,
        bbox=(110.0, 20.0, 110.005, 20.005),
        output_dir=out,
        stride=1,
        write_combined_mask=False,
    )

    assert outputs["masks"] is None
    assert not (out / "merit_masks.geojson").exists()
    assert outputs["river_masks"].exists()
    assert outputs["coast_masks"].exists()
    assert outputs["surface_masks"].exists()
    assert outputs["summary"].exists()


def test_write_merit_mask_outputs_writes_geojson_and_summary(tmp_path):
    tile = tmp_path / "n20e110.nc"
    out = tmp_path / "out"
    _write_merit_fixture(tile)

    outputs = write_merit_mask_outputs(
        tmp_path,
        bbox=(110.0, 20.0, 110.005, 20.005),
        output_dir=out,
        stride=1,
        r2_width_m=50.0,
        r3_width_m=300.0,
        r2_upa_km2=5000.0,
        r3_upa_km2=50000.0,
    )

    assert outputs["masks"].name == "merit_masks.geojson"
    assert outputs["river_masks"].name == "merit_river_masks.geojson"
    assert outputs["coast_masks"].name == "merit_coast_masks.geojson"
    assert outputs["surface_masks"].name == "merit_surface_masks.geojson"
    assert outputs["summary"].name == "merit_mask_summary.json"
    assert all(path.exists() for path in outputs.values())


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


def _write_merit_coast_fixture(path: Path) -> None:
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
        ds.variables["upa"][:, :] = 0.0
        ds.variables["wth"][:, :] = 0.0
        ds.variables["elv"][:, :] = 1.0
        landtype = np.ones((6, 6), dtype="i1")
        landtype[3:, :] = 17
        ds.variables["landtype_igbp"][:, :] = landtype


def test_write_merit_mask_outputs_can_gzip_geojson_layers(tmp_path):
    import gzip

    tile = tmp_path / "n20e110.nc"
    out = tmp_path / "out_gzip"
    _write_merit_fixture(tile)

    outputs = write_merit_mask_outputs(
        tmp_path,
        bbox=(110.0, 20.0, 110.005, 20.005),
        output_dir=out,
        stride=1,
        write_combined_mask=False,
        write_surface_mask=False,
        compress_geojson=True,
    )

    assert outputs["masks"] is None
    assert outputs["surface_masks"] is None
    assert outputs["river_masks"].name == "merit_river_masks.geojson.gz"
    assert outputs["coast_masks"].name == "merit_coast_masks.geojson.gz"
    with gzip.open(outputs["river_masks"], "rt") as handle:
        payload = json.load(handle)
    assert payload["type"] == "FeatureCollection"
    assert outputs["summary"].name == "merit_mask_summary.json"
