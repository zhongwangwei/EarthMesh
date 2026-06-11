"""EarthMesh v3 core schemas, recipes, manifests, and adapter boundaries."""

from util.v3_core.components import ComponentProduct, ComponentResult, ComponentRunContext
from util.v3_core.adapters import AdapterExportPlan, AdapterRegistry, SchemaOnlyAdapter, default_adapter_registry
from util.v3_core.demo import DemoInputs, build_demo_inputs
from util.v3_core.geometry import (
    MaskFeature,
    OverlayResult,
    apply_overlay_to_cell,
    overlay_cell_with_masks,
    polygon_area,
    polygon_clip_convex,
    summarize_overlay_results,
)
from util.v3_core.geometry_backend import PythonGeometryBackend, get_geometry_backend
from util.v3_core.geojson_io import (
    canonical_cells_to_geojson,
    geojson_cells_to_canonical,
    geojson_masks_to_features,
    load_cells_geojson,
    load_masks_geojson,
    write_cells_geojson,
)
from util.v3_core.legacy_fortran import LegacyFortranRun, LegacyFortranResult, build_legacy_command
from util.v3_core.manifest import V3RunManifest
from util.v3_core.map import canonical_cells_geojson_to_leaflet_html, render_canonical_cells_leaflet_html
from util.v3_core.pipeline import V3PipelineResult, build_v3_pipeline_result
from util.v3_core.recipe import ComponentRecipe, MeshRecipe, QARecipe, RegionRecipe, V3Recipe, load_recipe
from util.v3_core.schema import CanonicalCell, ExchangeLink, validate_cell_collection

__all__ = [
    "AdapterRegistry",
    "AdapterExportPlan",
    "CanonicalCell",
    "ComponentProduct",
    "ComponentResult",
    "ComponentRunContext",
    "ComponentRecipe",
    "DemoInputs",
    "ExchangeLink",
    "LegacyFortranResult",
    "LegacyFortranRun",
    "MaskFeature",
    "MeshRecipe",
    "OverlayResult",
    "PythonGeometryBackend",
    "QARecipe",
    "RegionRecipe",
    "SchemaOnlyAdapter",
    "V3Recipe",
    "V3RunManifest",
    "V3PipelineResult",
    "apply_overlay_to_cell",
    "build_v3_pipeline_result",
    "canonical_cells_to_geojson",
    "canonical_cells_geojson_to_leaflet_html",
    "build_legacy_command",
    "build_demo_inputs",
    "default_adapter_registry",
    "geojson_cells_to_canonical",
    "geojson_masks_to_features",
    "get_geometry_backend",
    "load_cells_geojson",
    "load_masks_geojson",
    "load_recipe",
    "overlay_cell_with_masks",
    "polygon_area",
    "polygon_clip_convex",
    "render_canonical_cells_leaflet_html",
    "summarize_overlay_results",
    "validate_cell_collection",
    "write_cells_geojson",
]
