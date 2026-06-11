import json

from util.hydro_mesh.corridor_preview import geojson_points_to_corridors, write_corridor_geojson


def _point_feature(reach_id, river_class, lon=120.0, lat=30.0, width_m=100.0):
    return {
        "type": "Feature",
        "geometry": {"type": "Point", "coordinates": [lon, lat]},
        "properties": {
            "reach_id": reach_id,
            "river_class": river_class,
            "width_m": width_m,
            "upstream_area_km2": 10_000.0,
        },
    }


def test_geojson_points_to_corridors_buffers_only_r2_r3_points():
    collection = {
        "type": "FeatureCollection",
        "features": [
            _point_feature("small", "R1"),
            _point_feature("medium", "R2", width_m=200.0),
            _point_feature("major", "R3", lon=121.0, width_m=3000.0),
        ],
    }

    corridors = geojson_points_to_corridors(collection, segments=16)

    assert corridors["type"] == "FeatureCollection"
    assert [feature["properties"]["reach_id"] for feature in corridors["features"]] == ["medium", "major"]
    assert all(feature["geometry"]["type"] == "Polygon" for feature in corridors["features"])
    assert corridors["features"][1]["properties"]["corridor_radius_m"] > corridors["features"][0]["properties"]["corridor_radius_m"]


def test_corridor_polygon_is_closed_around_point():
    collection = {"type": "FeatureCollection", "features": [_point_feature("r3", "R3", width_m=3000.0)]}

    corridors = geojson_points_to_corridors(collection, segments=12)
    ring = corridors["features"][0]["geometry"]["coordinates"][0]
    lons = [coord[0] for coord in ring]
    lats = [coord[1] for coord in ring]

    assert ring[0] == ring[-1]
    assert len(ring) == 13
    assert min(lons) < 120.0 < max(lons)
    assert min(lats) < 30.0 < max(lats)


def test_write_corridor_geojson_writes_feature_collection(tmp_path):
    input_geojson = tmp_path / "points.geojson"
    output_geojson = tmp_path / "corridors.geojson"
    input_geojson.write_text(json.dumps({"type": "FeatureCollection", "features": [_point_feature("r2", "R2")]}))

    write_corridor_geojson(input_geojson, output_geojson, segments=8)

    written = json.loads(output_geojson.read_text())
    assert written["type"] == "FeatureCollection"
    assert written["features"][0]["properties"]["reach_id"] == "r2"
    assert written["features"][0]["properties"]["corridor_kind"] == "preview_buffer"


def test_write_corridor_preview_png_writes_png_file(tmp_path):
    from util.hydro_mesh.corridor_preview import write_corridor_preview_png

    corridor_geojson = tmp_path / "corridors.geojson"
    png_path = tmp_path / "corridors.png"
    corridor_geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {
                            "type": "Polygon",
                            "coordinates": [
                                [
                                    [120.0, 30.0],
                                    [120.01, 30.0],
                                    [120.01, 30.01],
                                    [120.0, 30.01],
                                    [120.0, 30.0],
                                ]
                            ],
                        },
                        "properties": {"reach_id": "r3", "river_class": "R3"},
                    }
                ],
            }
        )
    )

    write_corridor_preview_png(corridor_geojson, png_path, title="Corridors")

    assert png_path.exists()
    assert png_path.read_bytes().startswith(b"\x89PNG")


def test_geojson_points_to_neighbor_corridors_connects_nearby_same_class_points():
    from util.hydro_mesh.corridor_preview import geojson_points_to_neighbor_corridors

    collection = {
        "type": "FeatureCollection",
        "features": [
            _point_feature("a", "R3", lon=120.0, lat=30.0, width_m=10_000.0),
            _point_feature("b", "R3", lon=120.02, lat=30.0, width_m=10_000.0),
            _point_feature("far", "R3", lon=121.0, lat=30.0, width_m=10_000.0),
            _point_feature("small", "R1", lon=120.01, lat=30.0),
        ],
    }

    corridors = geojson_points_to_neighbor_corridors(collection, max_link_distance_km=4.0, max_radius_m=2_500.0)

    assert len(corridors["features"]) == 1
    feature = corridors["features"][0]
    assert feature["properties"]["from_reach_id"] == "a"
    assert feature["properties"]["to_reach_id"] == "b"
    assert feature["properties"]["corridor_source_geometry"] == "nearest_neighbor_segment"
    assert feature["properties"]["corridor_radius_m"] == 2_500.0
    assert feature["geometry"]["type"] == "Polygon"
    assert feature["geometry"]["coordinates"][0][0] == feature["geometry"]["coordinates"][0][-1]


def test_geojson_points_to_downstream_corridors_uses_cama_indices():
    from util.hydro_mesh.corridor_preview import geojson_points_to_downstream_corridors

    upstream = _point_feature("up", "R3", lon=120.0, lat=30.0, width_m=9000.0)
    upstream["properties"].update({"x_index": 10, "y_index": 20, "downstream_x": 11, "downstream_y": 20})
    downstream = _point_feature("down", "R3", lon=120.02, lat=30.0, width_m=500.0)
    downstream["properties"].update({"x_index": 11, "y_index": 20, "downstream_x": 12, "downstream_y": 20})
    outside = _point_feature("outside", "R3", lon=120.04, lat=30.0, width_m=500.0)
    outside["properties"].update({"x_index": 12, "y_index": 20, "downstream_x": -9999, "downstream_y": -9999})

    corridors = geojson_points_to_downstream_corridors(
        {"type": "FeatureCollection", "features": [upstream, downstream, outside]},
        max_radius_m=2_500.0,
    )

    assert len(corridors["features"]) == 2
    first = corridors["features"][0]
    assert first["properties"]["from_reach_id"] == "up"
    assert first["properties"]["to_reach_id"] == "down"
    assert first["properties"]["corridor_source_geometry"] == "cama_downstream_segment"
    assert first["properties"]["corridor_radius_m"] == 2_500.0


def test_preview_geometry_label_names_downstream_segments():
    from util.hydro_mesh.corridor_preview import preview_geometry_label

    assert preview_geometry_label([{"properties": {"corridor_source_geometry": "cama_downstream_segment"}}]) == "CaMa downstream segment buffers"
    assert preview_geometry_label([{"properties": {"corridor_source_geometry": "nearest_neighbor_segment"}}]) == "nearest-neighbor segment buffers"
