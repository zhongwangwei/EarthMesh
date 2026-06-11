# Surface-Aware HTML Map Design

**Goal:** Surface-aware delivery packages must be visually inspectable in the same Leaflet HTML file, not only in derived CSV/GeoJSON artifacts.

**Requirements:**
- `render_mesh_leaflet_html()` accepts an optional complete LAND/OCEAN cell mask layer.
- The HTML embeds `surfaceCells`, displays LAND/OCEAN legend entries, and exposes a `complete LAND/OCEAN cell mask` layer toggle.
- Surface cell popups include `surface_class`.
- `mesh_geojson_to_leaflet_html()` accepts `surface_geojson` and passes it through to the renderer.
- `refinement_package.py` must pass the derived `<case>_complete_cell_mask.geojson` into HTML rendering when `--surface-geojson` is supplied.
- Existing package behavior without surface input remains compatible.

**Validation:**
- Unit tests prove the renderer and file wrapper embed optional surface layers.
- Unit tests prove surface-aware packages embed the derived LAND/OCEAN classes into HTML.
- Real N112 smoke verifies the package HTML contains `surfaceCells`, LAND, and OCEAN features.
