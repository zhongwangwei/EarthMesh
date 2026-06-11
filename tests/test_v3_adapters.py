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


def test_write_adapter_mesh_artifact_writes_mpas_netcdf(tmp_path):
    import netCDF4

    from util.v3_core.adapters import write_adapter_mesh_artifact

    cells = [
        CanonicalCell(
            cell_id="hex-1",
            cell_index=4,
            cell_type="HEX",
            center_lon=120.0,
            center_lat=30.0,
            area_m2=100.0,
            vertices=[(119.9, 29.9), (120.1, 29.9), (120.2, 30.0), (120.1, 30.1), (119.9, 30.1), (119.8, 30.0)],
            surface_class="LAND",
            hydro_class="R3",
            coast_class="NONE",
        )
    ]

    output = write_adapter_mesh_artifact("mpas", cells, tmp_path)

    assert output.name == "adapter_mpas_mesh.nc"
    with netCDF4.Dataset(output) as ds:
        assert ds.getncattr("adapter_name") == "mpas"
        assert ds.getncattr("kind") == "earthmesh_v3_adapter_mesh_metadata"
        assert ds.dimensions["cell"].size == 1
        assert ds.dimensions["vertex"].size == 6
        assert list(ds.variables["cell_index"][:]) == [4]
        assert list(ds.variables["center_lon"][:]) == [120.0]
        assert list(ds.variables["vertex_count"][:]) == [6]
        assert list(ds.variables["hydro_class_code"][:]) == [3]


def test_write_adapter_mesh_artifact_writes_fvcom_dat(tmp_path):
    from util.v3_core.adapters import write_adapter_mesh_artifact

    cells = [
        CanonicalCell(
            cell_id="tri-1",
            cell_index=1,
            cell_type="TRI",
            center_lon=120.0,
            center_lat=30.0,
            area_m2=50.0,
            vertices=[(119.9, 29.9), (120.1, 29.9), (120.0, 30.1)],
            surface_class="OCEAN",
            hydro_class="NONE",
            coast_class="COAST_OCEAN",
        )
    ]

    output = write_adapter_mesh_artifact("fvcom", cells, tmp_path)

    assert output.name == "adapter_fvcom_mesh.dat"
    lines = output.read_text().splitlines()
    assert lines[0] == "# EarthMesh v3 FVCOM mesh metadata"
    assert lines[1] == "# cell_id cell_index cell_type center_lon center_lat area_m2 vertex_count surface_class hydro_class coast_class vertices"
    assert lines[2].startswith("tri-1 1 TRI 120.0 30.0 50.0 3 OCEAN NONE COAST_OCEAN")
    assert "119.9,29.9;120.1,29.9;120.0,30.1" in lines[2]


def test_write_adapter_model_artifacts_writes_colm20xx_exchange_netcdf(tmp_path):
    import netCDF4

    from util.v3_core.adapters import write_adapter_model_artifacts

    cell = CanonicalCell(
        cell_id="delta-cell",
        cell_index=9,
        cell_type="POLYGON",
        center_lon=121.0,
        center_lat=31.0,
        area_m2=250.0,
        vertices=[(120.9, 30.9), (121.1, 30.9), (121.1, 31.1), (120.9, 31.1)],
        surface_class="COAST",
        hydro_class="R3",
        coast_class="DELTA",
        component_roles=["colm_land", "colm_ocean", "cama_river", "exchange_cell"],
        source_fractions={"LAND": 0.4, "OCEAN": 0.5, "R3": 0.1},
    )

    artifacts = write_adapter_model_artifacts("colm20xx", [cell], tmp_path)

    assert sorted(artifacts) == ["exchange"]
    output = artifacts["exchange"]
    assert output.name == "adapter_colm20xx_exchange.nc"
    with netCDF4.Dataset(output) as ds:
        assert ds.getncattr("kind") == "earthmesh_colm20xx_exchange_netcdf"
        assert ds.getncattr("adapter_name") == "colm20xx"
        assert ds.getncattr("schema_version") == "0.1"
        assert ds.dimensions["cell"].size == 1
        assert list(ds.variables["cell_index"][:]) == [9]
        assert list(ds.variables["surface_class_code"][:]) == [3]
        assert list(ds.variables["hydro_class_code"][:]) == [3]
        assert list(ds.variables["coast_class_code"][:]) == [5]
        assert list(ds.variables["land_fraction"][:]) == [0.4]
        assert list(ds.variables["ocean_fraction"][:]) == [0.5]
        assert list(ds.variables["river_fraction"][:]) == [0.1]
        assert list(ds.variables["supports_land_ocean_exchange"][:]) == [1]
        assert list(ds.variables["supports_river_land_exchange"][:]) == [1]
        assert list(ds.variables["supports_river_ocean_exchange"][:]) == [1]


def test_colm20xx_adapter_contract_requires_component_roles_for_exchange_cells():
    plan = default_adapter_registry().get("colm20xx").plan_export([CanonicalCell.minimal("cell")])

    assert "component_roles" in plan.required_fields
    assert "source_fractions" in plan.required_fields
    assert plan.output_format == "colm20xx_mesh_coupling_contract"
