import json


def _polygon_feature(river_class, coordinates):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [coordinates]},
        "properties": {"river_class": river_class},
    }


def test_parse_class_refine_maps_class_names_to_refinement_degrees():
    from util.hydro_mesh.refine_mask_export import parse_class_refine

    assert parse_class_refine(["R2=1", "R3=2"]) == {"R2": 1, "R3": 2}


def test_geojson_to_close_mask_specs_omits_duplicate_ring_closure():
    from util.hydro_mesh.refine_mask_export import geojson_to_close_mask_specs

    collection = {
        "type": "FeatureCollection",
        "features": [
            _polygon_feature("R1", [[10, 0], [11, 0], [11, 1], [10, 1], [10, 0]]),
            _polygon_feature("R3", [[120, 30], [121, 30], [121, 31], [120, 31], [120, 30]]),
        ],
    }

    specs = geojson_to_close_mask_specs(collection, class_refine={"R2": 1, "R3": 2})

    assert len(specs) == 2
    spec = specs[0]
    assert spec.river_class == "R3"
    assert [spec.refine_degree for spec in specs] == [1, 2]
    assert spec.coordinates == [(120.0, 30.0), (121.0, 30.0), (121.0, 31.0), (120.0, 31.0)]
    assert specs[1].coordinates == spec.coordinates


def test_write_close_mask_nmls_writes_earthmesh_close_format(tmp_path):
    from util.hydro_mesh.refine_mask_export import write_close_mask_nmls

    input_geojson = tmp_path / "corridors.geojson"
    output_prefix = tmp_path / "refine_spc_hydro"
    input_geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    _polygon_feature("R2", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]),
                    _polygon_feature("R3", [[2, 0], [3, 0], [3, 1], [2, 1], [2, 0]]),
                ],
            }
        )
    )

    paths = write_close_mask_nmls(input_geojson, output_prefix, class_refine={"R2": 1, "R3": 2})

    assert [path.name for path in paths] == [
        "refine_spc_hydro_R2_d1_001.nml",
        "refine_spc_hydro_R3_d1_001.nml",
        "refine_spc_hydro_R3_d2_001.nml",
    ]
    assert paths[0].read_text().splitlines() == [
        "close_num = 4",
        "close_refine = 1",
        "0.00000000 0.00000000",
        "1.00000000 0.00000000",
        "1.00000000 1.00000000",
        "0.00000000 1.00000000",
    ]
    assert paths[1].read_text().splitlines()[1] == "close_refine = 1"
    assert paths[2].read_text().splitlines()[1] == "close_refine = 2"


def test_write_close_mask_nmls_removes_stale_prefix_files(tmp_path):
    from util.hydro_mesh.refine_mask_export import write_close_mask_nmls

    input_geojson = tmp_path / "corridors.geojson"
    output_prefix = tmp_path / "refine_spc_hydro"
    stale_path = tmp_path / "refine_spc_hydro_R2_100.nml"
    stale_path.write_text("close_num = 4\nclose_refine = 1\n")
    input_geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [_polygon_feature("R2", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]])],
            }
        )
    )

    write_close_mask_nmls(input_geojson, output_prefix, class_refine={"R2": 1})

    assert not stale_path.exists()


def test_geojson_to_close_mask_specs_caps_each_refinement_degree_at_earthmesh_two_digit_limit():
    from util.hydro_mesh.refine_mask_export import geojson_to_close_mask_specs

    features = []
    for index in range(101):
        size = 2.0 if index == 100 else 1.0
        features.append(
            _polygon_feature(
                "R2",
                [[index, 0], [index + size, 0], [index + size, size], [index, size], [index, 0]],
            )
        )

    specs = geojson_to_close_mask_specs({"type": "FeatureCollection", "features": features}, class_refine={"R2": 1})

    assert len(specs) == 99
    assert any(spec.source_feature_index == 100 for spec in specs)


def test_geojson_to_close_mask_specs_emits_higher_target_classes_cumulatively_and_prioritizes_them():
    from util.hydro_mesh.refine_mask_export import geojson_to_close_mask_specs

    features = [
        _polygon_feature("R3", [[200, 0], [201, 0], [201, 1], [200, 1], [200, 0]]),
    ]
    for index in range(100):
        features.append(
            _polygon_feature(
                "R2",
                [[index, 0], [index + 2, 0], [index + 2, 2], [index, 2], [index, 0]],
            )
        )

    specs = geojson_to_close_mask_specs(
        {"type": "FeatureCollection", "features": features},
        class_refine={"R2": 1, "R3": 2},
    )

    degree1 = [spec for spec in specs if spec.refine_degree == 1]
    degree2 = [spec for spec in specs if spec.refine_degree == 2]
    assert len(degree1) == 99
    assert len(degree2) == 1
    assert any(spec.river_class == "R3" for spec in degree1)
    assert degree2[0].river_class == "R3"


def test_geojson_to_close_mask_specs_can_buffer_refinement_envelope():
    from util.hydro_mesh.refine_mask_export import geojson_to_close_mask_specs

    collection = {
        "type": "FeatureCollection",
        "features": [_polygon_feature("R2", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]])],
    }

    specs = geojson_to_close_mask_specs(collection, class_refine={"R2": 1}, buffer_deg=0.1)

    lons = [lon for lon, _ in specs[0].coordinates]
    lats = [lat for _, lat in specs[0].coordinates]
    assert min(lons) < 0.0
    assert min(lats) < 0.0
    assert max(lons) > 1.0
    assert max(lats) > 1.0
