"""EarthMesh v3 core schemas, recipes, manifests, and adapter boundaries."""

from util.v3_core.components import ComponentProduct, ComponentResult, ComponentRunContext
from util.v3_core.adapters import AdapterExportPlan, AdapterRegistry, SchemaOnlyAdapter, default_adapter_registry
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
from util.v3_core.legacy_fortran import LegacyFortranRun, LegacyFortranResult, build_legacy_command
from util.v3_core.manifest import V3RunManifest
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
    "apply_overlay_to_cell",
    "build_legacy_command",
    "default_adapter_registry",
    "get_geometry_backend",
    "load_recipe",
    "overlay_cell_with_masks",
    "polygon_area",
    "polygon_clip_convex",
    "summarize_overlay_results",
    "validate_cell_collection",
]
