"""EarthMesh v3 core schemas, recipes, manifests, and adapter boundaries."""

from util.v3_core.adapters import AdapterRegistry, SchemaOnlyAdapter, default_adapter_registry
from util.v3_core.legacy_fortran import LegacyFortranRun, LegacyFortranResult, build_legacy_command
from util.v3_core.manifest import V3RunManifest
from util.v3_core.recipe import ComponentRecipe, MeshRecipe, QARecipe, RegionRecipe, V3Recipe, load_recipe
from util.v3_core.schema import CanonicalCell, ExchangeLink, validate_cell_collection

__all__ = [
    "AdapterRegistry",
    "CanonicalCell",
    "ComponentRecipe",
    "ExchangeLink",
    "LegacyFortranResult",
    "LegacyFortranRun",
    "MeshRecipe",
    "QARecipe",
    "RegionRecipe",
    "SchemaOnlyAdapter",
    "V3Recipe",
    "V3RunManifest",
    "build_legacy_command",
    "default_adapter_registry",
    "load_recipe",
    "validate_cell_collection",
]
