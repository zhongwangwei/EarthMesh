import json
import struct

from util.hydro_mesh.cama_binary import CamaGridSpec


def test_coastal_band_cells_include_land_and_ocean_sides():
    from util.hydro_mesh.coastal_band import coastal_band_cells

    land = [
        [True, True, False],
        [True, True, False],
        [True, True, False],
    ]

    band = coastal_band_cells(land, radius_cells=1)

    assert band == [
        [False, True, True],
        [False, True, True],
        [False, True, True],
    ]


def test_coastal_band_geojson_dissolves_band_cells():
    from util.hydro_mesh.coastal_band import coastal_band_geojson, coastal_band_cells

    grid = CamaGridSpec(nx=3, ny=3, west=0.0, south=0.0, grid_size_deg=1.0)
    land = [
        [True, True, False],
        [True, True, False],
        [True, True, False],
    ]
    band = coastal_band_cells(land, radius_cells=1)

    collection = coastal_band_geojson(grid, x_start=0, y_start=0, band=band, land_mask=land, dissolve=True)

    assert collection["type"] == "FeatureCollection"
    assert len(collection["features"]) == 1
    feature = collection["features"][0]
    assert feature["properties"]["mask_class"] == "COAST"
    assert feature["properties"]["coastal_band_cell_count"] == 6
    assert feature["properties"]["land_side_cell_count"] == 3
    assert feature["properties"]["ocean_side_cell_count"] == 3
    assert feature["geometry"]["type"] in {"Polygon", "MultiPolygon"}


def test_write_coastal_band_geojson_reads_elevtn_window(tmp_path):
    from util.hydro_mesh.coastal_band import write_coastal_band_geojson

    map_dir = tmp_path / "map"
    map_dir.mkdir()
    (map_dir / "params.txt").write_text(
        "           3      !! grid number (east-west)\n"
        "           3      !! grid number (north-south)\n"
        "          10     !! floodplain layer\n"
        "   1.0000000     !! grid size\n"
        "       0.000     !! west  edge (deg)\n"
        "       3.000     !! east  edge (deg)\n"
        "       0.000     !! south edge (deg)\n"
        "       3.000     !! north edge (deg)\n"
    )
    # Left two columns are valid land/elevation; right column is CaMa undef ocean.
    values = [
        10.0, 11.0, -9999.0,
        12.0, 13.0, -9999.0,
        14.0, 15.0, -9999.0,
    ]
    (map_dir / "elevtn.bin").write_bytes(struct.pack("<9f", *values))
    output = tmp_path / "coast.geojson"

    collection = write_coastal_band_geojson(map_dir, output, bbox=(0.0, 0.0, 3.0, 3.0), radius_cells=1, y_reversed_storage=False)

    assert output.exists()
    written = json.loads(output.read_text())
    assert written == collection
    assert written["features"][0]["properties"]["coastal_band_cell_count"] == 6
