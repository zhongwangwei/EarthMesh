from __future__ import annotations

import html
import json
from pathlib import Path


def render_canonical_cells_leaflet_html(collection: dict[str, object], *, title: str = "EarthMesh v3 Canonical Cells") -> str:
    title_text = html.escape(title)
    canonical_json = json.dumps(collection, sort_keys=True, ensure_ascii=False)
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
      max-width: 390px;
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
    <p>{feature_count} v3 canonical cells embedded in this file.</p>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(34,197,94,0.46)\"></span>LAND</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(14,165,233,0.40)\"></span>OCEAN</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(6,182,212,0.54)\"></span>COAST / shelf / estuary</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(245,158,11,0.62)\"></span>R2 river cells</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(220,38,38,0.68)\"></span>R3 river cells</div>
    <div class=\"legend-item\"><span class=\"swatch\" style=\"background: rgba(148,163,184,0.42)\"></span>UNKNOWN / missing mask</div>
  </aside>
  <script src=\"https://unpkg.com/leaflet@1.9.4/dist/leaflet.js\"></script>
  <script>
    const canonicalCells = {canonical_json};
    const map = L.map('map', {{ preferCanvas: true }}).setView([31.0, 121.0], 7);
    const base = L.tileLayer('https://{{s}}.tile.openstreetmap.org/{{z}}/{{x}}/{{y}}.png', {{
      maxZoom: 19,
      attribution: '&copy; OpenStreetMap contributors'
    }}).addTo(map);

    function cellSemanticClass(feature) {{
      const p = feature.properties || {{}};
      const flags = Array.isArray(p.quality_flags) ? p.quality_flags : [];
      if (flags.includes('missing_mask')) return 'MISSING';
      if (p.hydro_class === 'R3') return 'R3';
      if (p.hydro_class === 'R2') return 'R2';
      if (p.coast_class && p.coast_class !== 'NONE') return 'COAST';
      if (p.surface_class === 'LAND') return 'LAND';
      if (p.surface_class === 'OCEAN') return 'OCEAN';
      return 'UNKNOWN';
    }}

    function styleFor(feature) {{
      const cls = cellSemanticClass(feature);
      const colors = {{
        LAND: ['#16a34a', '#22c55e', 0.46],
        OCEAN: ['#0284c7', '#0ea5e9', 0.40],
        COAST: ['#0891b2', '#06b6d4', 0.54],
        R2: ['#d97706', '#f59e0b', 0.62],
        R3: ['#b91c1c', '#dc2626', 0.68],
        MISSING: ['#475569', '#94a3b8', 0.42],
        UNKNOWN: ['#64748b', '#cbd5e1', 0.30]
      }};
      const selected = colors[cls] || colors.UNKNOWN;
      return {{ color: selected[0], weight: cls === 'R3' ? 1.8 : 1.0, opacity: 0.92, fillColor: selected[1], fillOpacity: selected[2] }};
    }}

    function popupHtml(feature) {{
      const p = feature.properties || {{}};
      const lines = [
        ['cell_id', p.cell_id],
        ['cell_type', p.cell_type],
        ['surface_class', p.surface_class],
        ['hydro_class', p.hydro_class],
        ['coast_class', p.coast_class],
        ['mesh_priority', p.mesh_priority],
        ['quality_flags', Array.isArray(p.quality_flags) ? p.quality_flags.join(',') : p.quality_flags],
        ['source_fractions', p.source_fractions ? JSON.stringify(p.source_fractions) : undefined]
      ];
      return lines
        .filter((row) => row[1] !== undefined && row[1] !== null && row[1] !== '')
        .map((row) => `<strong>${{row[0]}}</strong>: ${{row[1]}}`)
        .join('<br>');
    }}

    const canonicalLayer = L.geoJSON(canonicalCells, {{
      style: styleFor,
      onEachFeature: function(feature, layer) {{
        layer.bindPopup(popupHtml(feature));
      }}
    }}).addTo(map);

    L.control.layers({{ 'OpenStreetMap': base }}, {{ 'v3 canonical cells': canonicalLayer }}, {{ collapsed: false }}).addTo(map);

    if (canonicalLayer.getLayers().length > 0) {{
      map.fitBounds(canonicalLayer.getBounds(), {{ padding: [20, 20] }});
    }}
  </script>
</body>
</html>
"""


def canonical_cells_geojson_to_leaflet_html(
    input_geojson: str | Path,
    output_html: str | Path,
    *,
    title: str = "EarthMesh v3 Canonical Cells",
) -> str:
    collection = json.loads(Path(input_geojson).read_text())
    rendered = render_canonical_cells_leaflet_html(collection, title=title)
    output_path = Path(output_html)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(rendered)
    return rendered
