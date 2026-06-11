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


def geojson_to_leaflet_html(input_geojson: str | Path, output_html: str | Path, *, title: str = "Hydro Mesh Map") -> str:
    collection = json.loads(Path(input_geojson).read_text())
    rendered = render_leaflet_html(collection, title=title)
    output_path = Path(output_html)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(rendered)
    return rendered


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Render a classified hydro-mesh GeoJSON file as a Leaflet HTML QA map.")
    parser.add_argument("input_geojson", help="Input GeoJSON FeatureCollection")
    parser.add_argument("output_html", help="Output HTML map file")
    parser.add_argument("--title", default="Hydro Mesh Map", help="Map title")
    args = parser.parse_args(argv)
    geojson_to_leaflet_html(args.input_geojson, args.output_html, title=args.title)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
