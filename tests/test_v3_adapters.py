from util.v3_core.adapters import AdapterRegistry, default_adapter_registry
from util.v3_core.schema import CanonicalCell


def test_default_registry_contains_required_adapters():
    registry = default_adapter_registry()

    assert sorted(registry.names()) == ["colm2024", "colm20xx", "fvcom", "generic_esmf", "mpas"]


def test_colm_adapters_accept_shape_independent_cells():
    registry = default_adapter_registry()
    cells = [
        CanonicalCell.minimal("tri", cell_type="TRI"),
        CanonicalCell.minimal("hex", cell_type="HEX"),
    ]

    colm2024 = registry.get("colm2024")
    colm20xx = registry.get("colm20xx")

    assert colm2024.validate_cells(cells) == []
    assert colm20xx.validate_cells(cells) == []


def test_registry_rejects_duplicate_adapter_names():
    registry = AdapterRegistry()
    adapter = default_adapter_registry().get("mpas")
    registry.register(adapter)

    try:
        registry.register(adapter)
    except ValueError as exc:
        assert "duplicate adapter" in str(exc)
    else:
        raise AssertionError("expected duplicate adapter error")
