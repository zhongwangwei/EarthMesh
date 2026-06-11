import json

from util.hydro_mesh.geojson_map import geojson_to_leaflet_html, render_leaflet_html


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
