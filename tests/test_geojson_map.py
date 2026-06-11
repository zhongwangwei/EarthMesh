import json

from util.hydro_mesh.geojson_map import geojson_to_leaflet_html, mesh_geojson_to_leaflet_html, render_leaflet_html


def test_render_leaflet_html_embeds_geojson_and_legend():
    collection = {
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {"type": "Point", "coordinates": [121.0, 31.0]},
                "properties": {"reach_id": "r2", "river_class": "R2", "width_m": 200.0},
            },
            {
                "type": "Feature",
                "geometry": {"type": "Point", "coordinates": [122.0, 32.0]},
                "properties": {"reach_id": "r3", "river_class": "R3", "width_m": 3000.0},
            },
        ],
    }

    html = render_leaflet_html(collection, title="Test Map")

    assert "Test Map" in html
    assert "Leaflet" in html
    assert "const hydroData =" in html
    assert "R2" in html
    assert "R3" in html
    assert "#f59e0b" in html
    assert "#dc2626" in html


def test_geojson_to_leaflet_html_writes_html_file(tmp_path):
    geojson = tmp_path / "sample.geojson"
    html_path = tmp_path / "sample.html"
    geojson.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {"type": "Point", "coordinates": [121.0, 31.0]},
                        "properties": {"reach_id": "r3", "river_class": "R3"},
                    }
                ],
            }
        )
    )

    geojson_to_leaflet_html(geojson, html_path, title="Sample")

    assert html_path.exists()
    text = html_path.read_text()
    assert "Sample" in text
    assert "r3" in text


def test_render_mesh_leaflet_html_embeds_background_and_river_cell_layers():
    from util.hydro_mesh.geojson_map import render_mesh_leaflet_html

    background = {
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[121, 31], [122, 31], [122, 32], [121, 32], [121, 31]]]},
                "properties": {"cell_id": "land-1"},
            }
        ],
    }
    rivers = {
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[121.2, 31.2], [121.4, 31.2], [121.4, 31.4], [121.2, 31.4], [121.2, 31.2]]]},
                "properties": {"cell_id": "river-1", "river_class": "R3", "river_fraction": 0.5},
            }
        ],
    }

    html = render_mesh_leaflet_html(background, rivers, title="Mesh Map")

    assert "Mesh Map" in html
    assert "const backgroundCells =" in html
    assert "const riverCells =" in html
    assert "land/background cells" in html
    assert "R2 river-overlap cells" in html
    assert "R3 river-overlap cells" in html
    assert "L.control.layers" in html
    assert "river_fraction" in html


def test_mesh_geojson_to_leaflet_html_writes_two_layer_map(tmp_path):
    background = tmp_path / "background.geojson"
    rivers = tmp_path / "rivers.geojson"
    html_path = tmp_path / "mesh.html"
    background.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {"type": "Polygon", "coordinates": [[[121, 31], [122, 31], [122, 32], [121, 32], [121, 31]]]},
                        "properties": {"cell_id": "land-1"},
                    }
                ],
            }
        )
    )
    rivers.write_text(
        json.dumps(
            {
                "type": "FeatureCollection",
                "features": [
                    {
                        "type": "Feature",
                        "geometry": {"type": "Polygon", "coordinates": [[[121.2, 31.2], [121.4, 31.2], [121.4, 31.4], [121.2, 31.4], [121.2, 31.2]]]},
                        "properties": {"cell_id": "river-1", "river_class": "R2", "river_fraction": 0.25},
                    }
                ],
            }
        )
    )

    mesh_geojson_to_leaflet_html(background, rivers, html_path, title="Two Layer Mesh")

    text = html_path.read_text()
    assert "Two Layer Mesh" in text
    assert "land-1" in text
    assert "river-1" in text
