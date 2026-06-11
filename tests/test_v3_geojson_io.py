from util.v3_core.geojson_io import geojson_cells_to_canonical, geojson_masks_to_features


def test_geojson_cells_to_canonical_reads_polygon_cells():
    collection = {
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[120.0, 30.0], [121.0, 30.0], [120.0, 31.0], [120.0, 30.0]]]},
                "properties": {
                    "cell_id": "cell-a",
                    "cell_index": 7,
                    "cell_type": "TRI",
                    "area_m2": 123.0,
                    "surface_class": "LAND",
                },
            }
        ],
    }

    cells = geojson_cells_to_canonical(collection)

    assert len(cells) == 1
    assert cells[0].cell_id == "cell-a"
    assert cells[0].cell_index == 7
    assert cells[0].cell_type == "TRI"
    assert cells[0].area_m2 == 123.0
    assert cells[0].vertices == [(120.0, 30.0), (121.0, 30.0), (120.0, 31.0)]
    assert cells[0].surface_class == "LAND"


def test_geojson_masks_to_features_reads_mask_priority_and_class():
    collection = {
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]]},
                "properties": {"feature_id": "river-a", "river_class": "R3"},
            },
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[2.0, 0.0], [3.0, 0.0], [2.0, 1.0], [2.0, 0.0]]]},
                "properties": {"cell_id": "coast-a", "mask_class": "COAST"},
            },
        ],
    }

    masks = geojson_masks_to_features(collection)

    assert [mask.feature_id for mask in masks] == ["river-a", "coast-a"]
    assert [mask.mask_class for mask in masks] == ["R3", "COAST"]
    assert [mask.priority for mask in masks] == [30, 10]
    assert masks[0].polygon == [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]
