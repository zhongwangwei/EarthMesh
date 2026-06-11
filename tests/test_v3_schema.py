import pytest

from util.v3_core.schema import CanonicalCell, ExchangeLink, validate_cell_collection


def test_canonical_cell_accepts_hex_triangle_and_polygon_types():
    hex_cell = CanonicalCell(
        cell_id="h1",
        cell_index=1,
        cell_type="HEX",
        center_lon=113.5,
        center_lat=22.5,
        area_m2=1000.0,
        vertices=[(113.0, 22.0), (113.5, 22.0), (114.0, 22.25), (114.0, 22.75), (113.5, 23.0), (113.0, 22.75)],
        neighbors=["h2"],
        surface_class="COAST",
        hydro_class="ESTUARY",
        coast_class="ESTUARY",
        mesh_priority=3,
        component_roles=["colm_land", "colm_ocean", "exchange_cell"],
        source_fractions={"land": 0.45, "ocean": 0.55},
        quality_flags=[],
    )
    tri_cell = CanonicalCell(
        cell_id="t1",
        cell_index=2,
        cell_type="TRI",
        center_lon=114.0,
        center_lat=22.0,
        area_m2=500.0,
        vertices=[(113.8, 21.8), (114.2, 21.8), (114.0, 22.2)],
        neighbors=[],
        surface_class="OCEAN",
        hydro_class="NONE",
        coast_class="NONE",
        mesh_priority=0,
        component_roles=["fvcom_ocean"],
        source_fractions={"ocean": 1.0},
        quality_flags=[],
    )

    assert hex_cell.cell_type == "HEX"
    assert tri_cell.cell_type == "TRI"
    assert hex_cell.is_exchange_cell is True
    assert tri_cell.is_exchange_cell is False


def test_cell_rejects_invalid_cell_type():
    with pytest.raises(ValueError, match="cell_type"):
        CanonicalCell(
            cell_id="bad",
            cell_index=9,
            cell_type="SQUARE",
            center_lon=0.0,
            center_lat=0.0,
            area_m2=1.0,
            vertices=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
            neighbors=[],
            surface_class="LAND",
            hydro_class="NONE",
            coast_class="NONE",
            mesh_priority=0,
            component_roles=[],
            source_fractions={"land": 1.0},
            quality_flags=[],
        )


def test_validate_cell_collection_requires_unique_ids():
    first = CanonicalCell.minimal("same", cell_type="POLYGON")
    second = CanonicalCell.minimal("same", cell_type="POLYGON")

    with pytest.raises(ValueError, match="duplicate cell_id"):
        validate_cell_collection([first, second])


def test_exchange_link_records_shape_independent_coupling():
    link = ExchangeLink(
        source_cell_id="river-1",
        target_cell_id="ocean-1",
        source_role="river",
        target_role="ocean",
        interface_type="river_ocean",
        exchange_area_m2=125.0,
        exchange_fraction=0.25,
        weight=0.75,
        conservative=True,
        quality_flags=[],
    )

    assert link.interface_type == "river_ocean"
    assert link.conservative is True
