import json
import math

from netCDF4 import Dataset


def _feature_collection(features):
    return {"type": "FeatureCollection", "features": features}


def _cell_feature(cell_id, ring, area_m2=1_000_000.0):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [ring]},
        "properties": {
            "cell_id": cell_id,
            "grid_kind": "earthmesh_cell",
            "source_areaCell": area_m2,
            "source_areaCell_units": "file_units",
        },
    }


def _corridor_feature(river_class, ring):
    return {
        "type": "Feature",
        "geometry": {"type": "Polygon", "coordinates": [ring]},
        "properties": {"river_class": river_class},
    }


def test_read_mpas_cell_polygons_filters_cells_by_bbox(tmp_path):
    from util.hydro_mesh.earthmesh_intersection import read_mpas_cell_polygons

    mesh_path = tmp_path / "mesh.nc4"
    with Dataset(mesh_path, "w") as ds:
        ds.createDimension("nCells", 2)
        ds.createDimension("nVertices", 8)
        ds.createDimension("maxEdges", 4)
        ds.createVariable("lonCell", "f8", ("nCells",))[:] = [math.radians(120.5), math.radians(130.5)]
        ds.createVariable("latCell", "f8", ("nCells",))[:] = [math.radians(30.5), math.radians(40.5)]
        ds.createVariable("lonVertex", "f8", ("nVertices",))[:] = [math.radians(v) for v in [120, 121, 121, 120, 130, 131, 131, 130]]
        ds.createVariable("latVertex", "f8", ("nVertices",))[:] = [math.radians(v) for v in [30, 30, 31, 31, 40, 40, 41, 41]]
        ds.createVariable("nEdgesOnCell", "i4", ("nCells",))[:] = [4, 4]
        ds.createVariable("verticesOnCell", "i4", ("nCells", "maxEdges"))[:, :] = [[1, 2, 3, 4], [5, 6, 7, 8]]
        ds.createVariable("indexToCellID", "i4", ("nCells",))[:] = [101, 202]
        ds.createVariable("areaCell", "f8", ("nCells",))[:] = [10.0, 20.0]

    cells = read_mpas_cell_polygons(mesh_path, bbox=(118.0, 28.0, 123.0, 33.0))

    assert len(cells["features"]) == 1
    feature = cells["features"][0]
    assert feature["properties"]["cell_id"] == "101"
    assert feature["properties"]["cell_index"] == 0
    assert feature["properties"]["grid_kind"] == "earthmesh_cell"
    assert feature["properties"]["source_areaCell"] == 10.0
    assert feature["properties"]["source_areaCell_units"] == "file_units"
    assert feature["geometry"]["coordinates"][0] == [[120.0, 30.0], [121.0, 30.0], [121.0, 31.0], [120.0, 31.0], [120.0, 30.0]]


def test_earthmesh_cells_to_corridor_intersections_keeps_cell_geometry_with_fraction():
    from util.hydro_mesh.earthmesh_intersection import earthmesh_cells_to_corridor_intersections

    cells = _feature_collection(
        [
            _cell_feature("cell-a", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]], area_m2=2_000_000.0),
            _cell_feature("cell-b", [[2, 0], [3, 0], [3, 1], [2, 1], [2, 0]], area_m2=3_000_000.0),
        ]
    )
    corridors = _feature_collection(
        [
            _corridor_feature("R3", [[0.25, 0.0], [0.75, 0.0], [0.75, 1.0], [0.25, 1.0], [0.25, 0.0]]),
        ]
    )

    intersections = earthmesh_cells_to_corridor_intersections(cells, corridors)

    assert len(intersections["features"]) == 1
    feature = intersections["features"][0]
    assert feature["geometry"] == cells["features"][0]["geometry"]
    assert feature["properties"]["cell_id"] == "cell-a"
    assert feature["properties"]["river_class"] == "R3"
    assert feature["properties"]["grid_kind"] == "earthmesh_cell_preview"
    assert feature["properties"]["corridor_source_geometry"] == "earthmesh_cell_intersection_preview"
    assert feature["properties"]["river_fraction"] == 0.5
    assert feature["properties"]["source_estimated_river_area"] == 1_000_000.0


def test_unit_sphere_area_normalization_adds_true_m2_estimate():
    from util.hydro_mesh.earthmesh_intersection import EARTH_RADIUS_M, earthmesh_cells_to_corridor_intersections

    cells = _feature_collection(
        [
            _cell_feature("cell-a", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]], area_m2=2.0),
        ]
    )
    corridors = _feature_collection(
        [
            _corridor_feature("R3", [[0, 0], [0.25, 0], [0.25, 1], [0, 1], [0, 0]]),
        ]
    )

    intersections = earthmesh_cells_to_corridor_intersections(cells, corridors, unit_sphere_area=True)

    feature = intersections["features"][0]
    assert feature["properties"]["area_normalization"] == "unit_sphere_area_to_m2"
    assert feature["properties"]["normalized_cell_area_m2"] == 2.0 * EARTH_RADIUS_M * EARTH_RADIUS_M
    assert feature["properties"]["estimated_river_area_m2"] == 0.5 * EARTH_RADIUS_M * EARTH_RADIUS_M


def test_domain_geojson_clips_corridor_before_cell_intersection():
    from util.hydro_mesh.earthmesh_intersection import earthmesh_cells_to_corridor_intersections

    cells = _feature_collection(
        [
            _cell_feature("cell-a", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]], area_m2=100.0),
        ]
    )
    corridors = _feature_collection(
        [
            _corridor_feature("R3", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]),
        ]
    )
    domain = _feature_collection(
        [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [0.5, 0], [0.5, 1], [0, 1], [0, 0]]]},
                "properties": {"domain": "left-half"},
            }
        ]
    )

    intersections = earthmesh_cells_to_corridor_intersections(cells, corridors, domain=domain)

    feature = intersections["features"][0]
    assert feature["properties"]["river_fraction"] == 0.5
    assert feature["properties"]["domain_clip_applied"] is True


def test_write_earthmesh_intersection_geojson_accepts_cell_geojson(tmp_path):
    from util.hydro_mesh.earthmesh_intersection import write_earthmesh_intersection_geojson

    cells_path = tmp_path / "cells.geojson"
    corridors_path = tmp_path / "corridors.geojson"
    output_path = tmp_path / "intersection.geojson"
    cells_path.write_text(
        json.dumps(
            _feature_collection(
                [_cell_feature("cell-a", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]], area_m2=10.0)]
            )
        )
    )
    corridors_path.write_text(
        json.dumps(_feature_collection([_corridor_feature("R2", [[0, 0], [0.5, 0], [0.5, 1], [0, 1], [0, 0]])]))
    )

    write_earthmesh_intersection_geojson(corridors_path, output_path, cell_geojson=cells_path)

    written = json.loads(output_path.read_text())
    assert written["type"] == "FeatureCollection"
    assert written["features"][0]["properties"]["cell_id"] == "cell-a"
    assert written["features"][0]["properties"]["river_class"] == "R2"


def test_write_earthmesh_intersection_geojson_accepts_domain_geojson_and_unit_sphere_area(tmp_path):
    from util.hydro_mesh.earthmesh_intersection import write_earthmesh_intersection_geojson

    cells_path = tmp_path / "cells.geojson"
    corridors_path = tmp_path / "corridors.geojson"
    domain_path = tmp_path / "domain.geojson"
    output_path = tmp_path / "intersection.geojson"
    cells_path.write_text(
        json.dumps(
            _feature_collection(
                [_cell_feature("cell-a", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]], area_m2=1.0)]
            )
        )
    )
    corridors_path.write_text(json.dumps(_feature_collection([_corridor_feature("R3", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]])])))
    domain_path.write_text(
        json.dumps(
            _feature_collection(
                [
                    {
                        "type": "Feature",
                        "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [0.5, 0], [0.5, 1], [0, 1], [0, 0]]]},
                        "properties": {},
                    }
                ]
            )
        )
    )

    write_earthmesh_intersection_geojson(
        corridors_path,
        output_path,
        cell_geojson=cells_path,
        domain_geojson=domain_path,
        unit_sphere_area=True,
    )

    written = json.loads(output_path.read_text())
    properties = written["features"][0]["properties"]
    assert properties["domain_clip_applied"] is True
    assert properties["area_normalization"] == "unit_sphere_area_to_m2"


def test_write_earthmesh_cell_preview_png_draws_background_cells(tmp_path):
    from util.hydro_mesh.earthmesh_intersection import write_earthmesh_cell_preview_png

    background_path = tmp_path / "all_cells.geojson"
    overlap_path = tmp_path / "overlap.geojson"
    png_path = tmp_path / "preview.png"
    background_path.write_text(
        json.dumps(
            _feature_collection(
                [
                    _cell_feature("land", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]),
                    _cell_feature("river", [[1, 0], [2, 0], [2, 1], [1, 1], [1, 0]]),
                ]
            )
        )
    )
    overlap_path.write_text(
        json.dumps(
            _feature_collection(
                    [
                    {
                        "type": "Feature",
                        "geometry": {"type": "Polygon", "coordinates": [[[1, 0], [2, 0], [2, 1], [1, 1], [1, 0]]]},
                        "properties": {
                            "cell_id": "river",
                            "river_class": "R3",
                            "river_fraction": 0.5,
                            "corridor_source_geometry": "earthmesh_cell_intersection_preview",
                        },
                    }
                ]
            )
        )
    )

    write_earthmesh_cell_preview_png(overlap_path, png_path, background_cell_geojson=background_path, title="With land cells")

    assert png_path.exists()
    assert png_path.read_bytes().startswith(b"\x89PNG")


def test_earthmesh_cells_to_corridor_intersections_can_embed_coast_mask_as_mesh_cells():
    from util.hydro_mesh.earthmesh_intersection import earthmesh_cells_to_corridor_intersections

    cells = _feature_collection(
        [
            _cell_feature("coast-cell", [[0, 0], [2, 0], [2, 1], [0, 1], [0, 0]], area_m2=100.0),
        ]
    )
    coast_band = _feature_collection(
        [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]]]},
                "properties": {"mask_class": "COAST", "coastal_side": "land"},
            }
        ]
    )

    intersections = earthmesh_cells_to_corridor_intersections(cells, coast_band, include_classes={"COAST"})

    assert len(intersections["features"]) == 1
    feature = intersections["features"][0]
    assert feature["geometry"] == cells["features"][0]["geometry"]
    assert feature["properties"]["cell_id"] == "coast-cell"
    assert feature["properties"]["mask_class"] == "COAST"
    assert feature["properties"]["coastal_fraction"] == 0.5
    assert "river_class" not in feature["properties"]


def test_write_earthmesh_intersection_geojson_reads_gzipped_corridor(tmp_path):
    import gzip

    from util.hydro_mesh.earthmesh_intersection import write_earthmesh_intersection_geojson

    cells_path = tmp_path / "cells.geojson"
    corridors_path = tmp_path / "corridors.geojson.gz"
    output_path = tmp_path / "intersection.geojson"
    cells_path.write_text(json.dumps(_feature_collection([_cell_feature("cell-a", [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]], area_m2=10.0)])))
    with gzip.open(corridors_path, "wt") as handle:
        json.dump(_feature_collection([_corridor_feature("R2", [[0, 0], [0.5, 0], [0.5, 1], [0, 1], [0, 0]])]), handle)

    write_earthmesh_intersection_geojson(corridors_path, output_path, cell_geojson=cells_path)

    written = json.loads(output_path.read_text())
    assert written["features"][0]["properties"]["river_class"] == "R2"
