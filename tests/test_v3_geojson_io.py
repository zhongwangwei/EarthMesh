from util.v3_core.geojson_io import canonical_cells_to_geojson, geojson_cells_to_canonical, geojson_masks_to_features
from util.v3_core.schema import CanonicalCell


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


def test_canonical_cells_to_geojson_preserves_v3_semantics():
    cell = CanonicalCell(
        cell_id="r3-cell",
        cell_index=2,
        cell_type="HEX",
        center_lon=0.5,
        center_lat=0.5,
        area_m2=1.0,
        vertices=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
        surface_class="OCEAN",
        hydro_class="R3",
        coast_class="NONE",
        mesh_priority=30,
        source_fractions={"R3": 1.0},
        quality_flags=["example"],
    )

    collection = canonical_cells_to_geojson([cell])

    feature = collection["features"][0]
    assert collection["type"] == "FeatureCollection"
    assert feature["geometry"]["coordinates"] == [[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]]
    assert feature["properties"]["cell_id"] == "r3-cell"
    assert feature["properties"]["surface_class"] == "OCEAN"
    assert feature["properties"]["hydro_class"] == "R3"
    assert feature["properties"]["source_fractions"] == {"R3": 1.0}
