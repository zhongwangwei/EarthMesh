from util.hydro_mesh.cama_binary import CamaGridSpec


def test_surface_mask_geojson_labels_land_and_ocean_cells_without_dissolve():
    from util.hydro_mesh.cama_surface_mask import surface_mask_geojson

    grid = CamaGridSpec(nx=2, ny=1, west=10.0, south=20.0, grid_size_deg=0.5)
    collection = surface_mask_geojson(
        grid,
        x_start=0,
        y_start=0,
        land_mask=[[True, False]],
        dissolve=False,
    )

    assert [feature["properties"]["surface_class"] for feature in collection["features"]] == ["LAND", "OCEAN"]
    assert collection["features"][0]["geometry"]["coordinates"][0] == [[10.0, 20.0], [10.5, 20.0], [10.5, 20.5], [10.0, 20.5], [10.0, 20.0]]
    assert collection["features"][1]["properties"]["x_index"] == 1
