from __future__ import annotations

import argparse
import html
import json
from pathlib import Path

_CLASS_COLORS = {
    "R2": "#f59e0b",
    "R3": "#dc2626",
}
_DEFAULT_COLOR = "#2563eb"


def render_leaflet_html(collection: dict[str, object], *, title: str = "Hydro Mesh Map") -> str:
    """Render a self-contained HTML shell with embedded GeoJSON for Leaflet QA."""

    title_text = html.escape(title)
    hydro_json = json.dumps(collection, sort_keys=True, ensure_ascii=False)
    feature_count = len(collection.get("features", [])) if isinstance(collection.get("features"), list) else 0
    return f"""<!doctype html>
<html lang=\"en\">
<head>
  <meta charset=\"utf-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
  <title>{title_text}</title>
  <link rel=\"stylesheet\" href=\"https://unpkg.com/leaflet@1.9.4/dist/leaflet.css\" />
  <style>
    html, body, #map {{ height: 100%; margin: 0; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif; }}
    .panel {{
      position: absolute;
      z-index: 1000;
      top: 12px;
      right: 12px;
      max-width: 340px;
      padding: 12px 14px;
      background: rgba(255, 255, 255, 0.94);
      border-radius: 10px;
      box-shadow: 0 8px 24px rgba(15, 23, 42, 0.18);
      line-height: 1.35;
    }}
    .panel h1 {{ font-size: 16px; margin: 0 0 8px; }}
    .panel p {{ margin: 4px 0; font-size: 12px; color: #475569; }}
    .legend-item {{ display: flex; align-items: center; gap: 8px; margin-top: 6px; font-size: 13px; }}
    .swatch {{ width: 14px; height: 14px; border-radius: 50%; display: inline-block; }}
  </style>
</head>
<body>
  <div id=\"map\"></div>
  <aside class=\"panel\">
    <!-- Leaflet QA map -->
    <h1>{title_text}</h1>
    <p>{feature_count} classified CaMa reach candidates embedded in this file.</p>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: #f59e0b\"></span>R2: 2D candidate / medium river</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: #dc2626\"></span>R3: must-split 2D river / estuary</div>
  </aside>
  <script src=\"https://unpkg.com/leaflet@1.9.4/dist/leaflet.js\"></script>
  <script>
    const hydroData = {hydro_json};
    const colors = {{ R2: \"#f59e0b\", R3: \"#dc2626\" }};
    const defaultColor = \"#2563eb\";
    const map = L.map('map', {{ preferCanvas: true }}).setView([31.0, 121.0], 7);
    L.tileLayer('https://{{s}}.tile.openstreetmap.org/{{z}}/{{x}}/{{y}}.png', {{
      maxZoom: 19,
      attribution: '&copy; OpenStreetMap contributors'
    }}).addTo(map);

    function classColor(feature) {{
      const cls = feature && feature.properties ? feature.properties.river_class : undefined;
      return colors[cls] || defaultColor;
    }}

    function popupHtml(feature) {{
      const p = feature.properties || {{}};
      const lines = [
        ['reach_id', p.reach_id],
        ['class', p.river_class],
        ['width_m', p.width_m],
        ['upstream_area_km2', p.upstream_area_km2],
        ['river_length_m', p.river_length_m]
      ];
      return lines
        .filter((row) => row[1] !== undefined && row[1] !== null)
        .map((row) => `<strong>${{row[0]}}</strong>: ${{row[1]}}`)
        .join('<br>');
    }}

    const layer = L.geoJSON(hydroData, {{
      pointToLayer: function(feature, latlng) {{
        const cls = feature.properties ? feature.properties.river_class : undefined;
        return L.circleMarker(latlng, {{
          radius: cls === 'R3' ? 5 : 4,
          color: classColor(feature),
          fillColor: classColor(feature),
          fillOpacity: cls === 'R3' ? 0.85 : 0.65,
          weight: cls === 'R3' ? 2 : 1
        }});
      }},
      style: function(feature) {{
        return {{ color: classColor(feature), weight: 2, opacity: 0.8 }};
      }},
      onEachFeature: function(feature, layer) {{
        layer.bindPopup(popupHtml(feature));
      }}
    }}).addTo(map);

    if (layer.getLayers().length > 0) {{
      map.fitBounds(layer.getBounds(), {{ padding: [20, 20] }});
    }}
  </script>
</body>
</html>
"""


def render_mesh_leaflet_html(
    background_cells: dict[str, object],
    river_cells: dict[str, object],
    *,
    coast_cells: dict[str, object] | None = None,
    title: str = "EarthMesh Hydro Cells Map",
) -> str:
    """Render a Leaflet QA map with embedded background cells and river-overlap cells."""

    title_text = html.escape(title)
    background_json = json.dumps(background_cells, sort_keys=True, ensure_ascii=False)
    river_json = json.dumps(river_cells, sort_keys=True, ensure_ascii=False)
    coast_cells = coast_cells or {"type": "FeatureCollection", "features": []}
    coast_json = json.dumps(coast_cells, sort_keys=True, ensure_ascii=False)
    background_count = len(background_cells.get("features", [])) if isinstance(background_cells.get("features"), list) else 0
    river_count = len(river_cells.get("features", [])) if isinstance(river_cells.get("features"), list) else 0
    coast_count = len(coast_cells.get("features", [])) if isinstance(coast_cells.get("features"), list) else 0
    return f"""<!doctype html>
<html lang=\"en\">
<head>
  <meta charset=\"utf-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />
  <title>{title_text}</title>
  <link rel=\"stylesheet\" href=\"https://unpkg.com/leaflet@1.9.4/dist/leaflet.css\" />
  <style>
    html, body, #map {{ height: 100%; margin: 0; }}
    body {{ font-family: -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif; }}
    .panel {{
      position: absolute;
      z-index: 1000;
      top: 12px;
      right: 12px;
      max-width: 380px;
      padding: 12px 14px;
      background: rgba(255, 255, 255, 0.94);
      border-radius: 10px;
      box-shadow: 0 8px 24px rgba(15, 23, 42, 0.18);
      line-height: 1.35;
    }}
    .panel h1 {{ font-size: 16px; margin: 0 0 8px; }}
    .panel p {{ margin: 4px 0; font-size: 12px; color: #475569; }}
    .legend-item {{ display: flex; align-items: center; gap: 8px; margin-top: 6px; font-size: 13px; }}
    .swatch {{ width: 18px; height: 12px; border: 1px solid rgba(15,23,42,0.35); display: inline-block; }}
  </style>
</head>
<body>
  <div id=\"map\"></div>
  <aside class=\"panel\">
    <h1>{title_text}</h1>
    <p>{background_count} land/background cells, {river_count} river-overlap cells, and {coast_count} coastal-overlap EarthMesh cells embedded in this file.</p>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(148,163,184,0.22)\"></span>land/background cells</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(245,158,11,0.55)\"></span>R2 river-overlap cells</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(220,38,38,0.62)\"></span>R3 river-overlap cells</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(6,182,212,0.42)\"></span>coastal-overlap EarthMesh cells</div>
  </aside>
  <script src=\"https://unpkg.com/leaflet@1.9.4/dist/leaflet.js\"></script>
  <script>
    const backgroundCells = {background_json};
    const riverCells = {river_json};
    const coastCells = {coast_json};
    const colors = {{ R2: \"#f59e0b\", R3: \"#dc2626\" }};
    const defaultRiverColor = \"#2563eb\";
    const map = L.map('map', {{ preferCanvas: true }}).setView([31.0, 121.0], 7);
    const base = L.tileLayer('https://{{s}}.tile.openstreetmap.org/{{z}}/{{x}}/{{y}}.png', {{
      maxZoom: 19,
      attribution: '&copy; OpenStreetMap contributors'
    }}).addTo(map);

    function riverColor(feature) {{
      const cls = feature && feature.properties ? feature.properties.river_class : undefined;
      return colors[cls] || defaultRiverColor;
    }}

    function riverOpacity(feature) {{
      const p = feature.properties || {{}};
      const fraction = Number(p.river_fraction || 0);
      return Math.min(0.78, Math.max(0.28, 0.28 + fraction * 0.55));
    }}

    function popupHtml(feature) {{
      const p = feature.properties || {{}};
      const lines = [
        ['cell_id', p.cell_id],
        ['cell_index', p.cell_index],
        ['river_class', p.river_class],
        ['river_fraction', p.river_fraction],
        ['mask_class', p.mask_class],
        ['coastal_side', p.coastal_side],
        ['coastal_band_cell_count', p.coastal_band_cell_count],
        ['coastal_fraction', p.coastal_fraction],
        ['overlap_fraction', p.overlap_fraction],
        ['estimated_river_area_m2', p.estimated_river_area_m2],
        ['normalized_cell_area_m2', p.normalized_cell_area_m2],
        ['source_areaCell', p.source_areaCell]
      ];
      return lines
        .filter((row) => row[1] !== undefined && row[1] !== null)
        .map((row) => `<strong>${{row[0]}}</strong>: ${{row[1]}}`)
        .join('<br>');
    }}

    const backgroundLayer = L.geoJSON(backgroundCells, {{
      style: function() {{
        return {{
          color: '#64748b',
          weight: 0.8,
          opacity: 0.48,
          fillColor: '#94a3b8',
          fillOpacity: 0.16
        }};
      }},
      onEachFeature: function(feature, layer) {{
        layer.bindPopup(popupHtml(feature));
      }}
    }}).addTo(map);

    const riverLayer = L.geoJSON(riverCells, {{
      style: function(feature) {{
        return {{
          color: riverColor(feature),
          weight: feature.properties && feature.properties.river_class === 'R3' ? 2.1 : 1.5,
          opacity: 0.95,
          fillColor: riverColor(feature),
          fillOpacity: riverOpacity(feature)
        }};
      }},
      onEachFeature: function(feature, layer) {{
        layer.bindPopup(popupHtml(feature));
      }}
    }}).addTo(map);

    const coastLayer = L.geoJSON(coastCells, {{
      style: function(feature) {{
        const p = feature.properties || {{}};
        const side = p.coastal_side || '';
        return {{
          color: side === 'ocean' ? '#0891b2' : '#0e7490',
          weight: 1.1,
          opacity: 0.82,
          fillColor: '#06b6d4',
          fillOpacity: side === 'ocean' ? 0.26 : 0.38
        }};
      }},
      onEachFeature: function(feature, layer) {{
        layer.bindPopup(popupHtml(feature));
      }}
    }}).addTo(map);

    L.control.layers({{ 'OpenStreetMap': base }}, {{
      'land/background cells': backgroundLayer,
      'coastal-overlap EarthMesh cells': coastLayer,
      'R2/R3 river-overlap cells': riverLayer
    }}, {{ collapsed: false }}).addTo(map);

    const bounds = L.featureGroup([backgroundLayer, coastLayer, riverLayer]).getBounds();
    if (bounds.isValid()) {{
      map.fitBounds(bounds, {{ padding: [20, 20] }});
    }}
  </script>
</body>
</html>
"""


def geojson_to_leaflet_html(input_geojson: str | Path, output_html: str | Path, *, title: str = "Hydro Mesh Map") -> str:
    collection = json.loads(Path(input_geojson).read_text())
    rendered = render_leaflet_html(collection, title=title)
    output_path = Path(output_html)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(rendered)
    return rendered


def mesh_geojson_to_leaflet_html(
    background_geojson: str | Path,
    river_geojson: str | Path,
    output_html: str | Path,
    *,
    coast_geojson: str | Path | None = None,
    title: str = "EarthMesh Hydro Cells Map",
) -> str:
    background = json.loads(Path(background_geojson).read_text())
    rivers = json.loads(Path(river_geojson).read_text())
    coast = json.loads(Path(coast_geojson).read_text()) if coast_geojson is not None else None
    rendered = render_mesh_leaflet_html(background, rivers, coast_cells=coast, title=title)
    output_path = Path(output_html)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(rendered)
    return rendered


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Render a classified hydro-mesh GeoJSON file as a Leaflet HTML QA map.")
    parser.add_argument("input_geojson", help="Input GeoJSON FeatureCollection")
    parser.add_argument("output_html", help="Output HTML map file")
    parser.add_argument("--background-geojson", help="Optional land/background cell GeoJSON for a two-layer mesh-cell map")
    parser.add_argument("--coast-geojson", help="Optional coastal-band GeoJSON for a three-layer mesh-cell map")
    parser.add_argument("--title", default="Hydro Mesh Map", help="Map title")
    args = parser.parse_args(argv)
    if args.background_geojson:
        mesh_geojson_to_leaflet_html(
            args.background_geojson,
            args.input_geojson,
            args.output_html,
            coast_geojson=args.coast_geojson,
            title=args.title,
        )
    elif args.coast_geojson:
        raise ValueError("--coast-geojson requires --background-geojson")
    else:
        geojson_to_leaflet_html(args.input_geojson, args.output_html, title=args.title)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
