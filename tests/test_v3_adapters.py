import csv
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


def test_write_adapter_cell_table_writes_stable_csv(tmp_path):
    from util.v3_core.adapters import write_adapter_cell_table

    cell = CanonicalCell(
        cell_id="cell-1",
        cell_index=7,
        cell_type="POLYGON",
        center_lon=113.5,
        center_lat=22.5,
        area_m2=12.0,
        vertices=[(113.0, 22.0), (114.0, 22.0), (114.0, 23.0), (113.0, 23.0)],
        surface_class="LAND",
        hydro_class="R2",
        coast_class="COAST_LAND",
        mesh_priority=20,
        source_fractions={"R2": 0.5, "LAND": 0.5},
        quality_flags=["refined_from_mask"],
        geometry_ref="base-cell",
        source_mesh_type="bbox_grid_refined",
    )

    output = write_adapter_cell_table("colm2024", [cell], tmp_path)

    assert output.name == "adapter_colm2024_cells.csv"
    lines = output.read_text().splitlines()
    assert lines[0] == (
        "adapter_name,cell_id,cell_index,cell_type,center_lon,center_lat,area_m2,"
        "surface_class,hydro_class,coast_class,mesh_priority,source_mesh_type,geometry_ref,"
        "vertex_count,vertices_json,source_fractions_json,quality_flags_json"
    )
    assert "colm2024,cell-1,7,POLYGON,113.5,22.5,12.0,LAND,R2,COAST_LAND,20,bbox_grid_refined,base-cell,4" in lines[1]
    row = next(csv.DictReader(output.open()))
    assert row["source_fractions_json"] == '{"LAND": 0.5, "R2": 0.5}'
    assert row["quality_flags_json"] == '["refined_from_mask"]'
