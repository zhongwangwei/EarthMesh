import json

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


def test_adapter_export_plan_summarizes_topology_and_warnings():
    registry = default_adapter_registry()
    cells = [
        CanonicalCell.minimal("tri", cell_type="TRI"),
        CanonicalCell.minimal("hex", cell_type="HEX"),
    ]

    mpas_plan = registry.get("mpas").plan_export(cells)
    colm_plan = registry.get("colm2024").plan_export(cells)

    assert mpas_plan.adapter_name == "mpas"
    assert mpas_plan.output_format == "mpas_unstructured_mesh_contract"
    assert mpas_plan.cell_type_counts == {"TRI": 1, "HEX": 1}
    assert mpas_plan.required_fields[:3] == ["cell_id", "cell_index", "cell_type"]
    assert mpas_plan.warnings == ["mpas does not support cell_type=TRI for cell_id=tri"]
    assert colm_plan.warnings == []


def test_adapter_export_plan_writes_deterministic_json(tmp_path):
    plan = default_adapter_registry().get("fvcom").plan_export([CanonicalCell.minimal("tri", cell_type="TRI")])

    output = plan.write_json(tmp_path / "fvcom_adapter_plan.json")
    payload = json.loads(output.read_text())

    assert payload["adapter_name"] == "fvcom"
    assert payload["output_format"] == "fvcom_tri_mesh_contract"
    assert payload["cell_type_counts"] == {"TRI": 1}
    assert payload["files"] == {}
