import json

from util.v3_core.map import canonical_cells_geojson_to_leaflet_html, render_canonical_cells_leaflet_html


def _canonical_collection():
    return {
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[0, 0], [1, 0], [0, 1], [0, 0]]]},
                "properties": {"cell_id": "land", "surface_class": "LAND", "hydro_class": "NONE", "coast_class": "NONE"},
            },
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[1, 0], [2, 0], [1, 1], [1, 0]]]},
                "properties": {"cell_id": "river", "surface_class": "UNKNOWN", "hydro_class": "R3", "coast_class": "NONE"},
            },
            {
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[2, 0], [3, 0], [2, 1], [2, 0]]]},
                "properties": {
                    "cell_id": "missing",
                    "surface_class": "UNKNOWN",
                    "hydro_class": "NONE",
                    "coast_class": "NONE",
                    "quality_flags": ["missing_mask"],
                },
            },
        ],
    }


def test_render_canonical_cells_leaflet_html_embeds_v3_semantic_legend():
    html = render_canonical_cells_leaflet_html(_canonical_collection(), title="V3 QA")

    assert "V3 QA" in html
    assert "const canonicalCells =" in html
    assert "LAND" in html
    assert "OCEAN" in html
    assert "COAST" in html
    assert "R2 river cells" in html
    assert "R3 river cells" in html
    assert "UNKNOWN / missing mask" in html
    assert "missing_mask" in html
    assert "cellSemanticClass" in html
    assert "L.control.layers" in html


def test_canonical_cells_geojson_to_leaflet_html_writes_file(tmp_path):
    geojson = tmp_path / "canonical_cells.geojson"
    html_path = tmp_path / "canonical_cells.html"
    geojson.write_text(json.dumps(_canonical_collection()))

    canonical_cells_geojson_to_leaflet_html(geojson, html_path, title="Written V3 Map")

    text = html_path.read_text()
    assert "Written V3 Map" in text
    assert "land" in text
    assert "river" in text
    assert "missing" in text
