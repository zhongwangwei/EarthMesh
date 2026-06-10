from util.hydro_mesh.cama_binary import CamaGridSpec
from util.hydro_mesh.cama_inventory import build_reach_inventory, read_reach_inventory_window


def test_build_reach_inventory_skips_empty_cells_and_preserves_metadata():
    grid = CamaGridSpec(nx=4, ny=3, west=100.0, south=20.0, grid_size_deg=0.25)
    records = build_reach_inventory(
        grid,
        x_start=1,
        y_start=1,
        target_dx_km=10.0,
        uparea_km2=[
            [0.0, 2000.0],
            [12000.0, -9999.0],
        ],
        width_m=[
            [0.0, 80.0],
            [300.0, 40.0],
        ],
        rivlen_m=[
            [0.0, 1000.0],
            [1500.0, 800.0],
        ],
        next_x=[
            [0, 3],
            [2, 0],
        ],
        next_y=[
            [0, 1],
            [2, 0],
        ],
    )

    assert [record.reach.reach_id for record in records] == ["cama-1-2", "cama-2-1"]
    assert records[0].reach.upstream_area_km2 == 2000.0
    assert records[0].reach.width_m == 80.0
    assert records[0].reach.floodplain_width_m == 0.0
    assert records[0].river_length_m == 1000.0
    assert records[0].lon == 100.625
    assert records[0].lat == 20.375
    assert records[0].downstream_x == 3
    assert records[0].downstream_y == 1


def test_build_reach_inventory_marks_window_edge_ocean_outlet_as_estuary_candidate():
    grid = CamaGridSpec(nx=2, ny=2, west=0.0, south=0.0, grid_size_deg=1.0)
    records = build_reach_inventory(
        grid,
        x_start=0,
        y_start=0,
        target_dx_km=5.0,
        uparea_km2=[[60000.0]],
        width_m=[[200.0]],
        rivlen_m=[[1000.0]],
        next_x=[[0]],
        next_y=[[0]],
    )

    assert records[0].reach.is_estuary


def test_read_reach_inventory_window_reads_required_binary_fields(tmp_path):
    import struct

    nx, ny = 3, 2
    (tmp_path / "uparea.bin").write_bytes(struct.pack("<6f", 0.0, 2000.0, 0.0, 12000.0, 0.0, 0.0))
    (tmp_path / "width.bin").write_bytes(struct.pack("<6f", 0.0, 80.0, 0.0, 300.0, 0.0, 0.0))
    (tmp_path / "rivlen.bin").write_bytes(struct.pack("<6f", 0.0, 1000.0, 0.0, 1500.0, 0.0, 0.0))
    # nextxy stores two int32 components per cell: x, y
    (tmp_path / "nextxy.bin").write_bytes(
        struct.pack(
            "<12i",
            0, 0,
            2, 0,
            0, 0,
            1, 1,
            0, 0,
            0, 0,
        )
    )
    grid = CamaGridSpec(nx=nx, ny=ny, west=0.0, south=0.0, grid_size_deg=1.0)

    records = read_reach_inventory_window(
        tmp_path,
        grid,
        x_start=0,
        y_start=0,
        width=3,
        height=2,
        target_dx_km=10.0,
    )

    assert [record.reach.reach_id for record in records] == ["cama-0-1", "cama-1-0"]
    assert records[0].downstream_x == 2
    assert records[0].downstream_y == 0


def test_build_reach_inventory_converts_uparea_units_to_km2():
    grid = CamaGridSpec(nx=1, ny=1, west=0.0, south=0.0, grid_size_deg=1.0)
    records = build_reach_inventory(
        grid,
        x_start=0,
        y_start=0,
        target_dx_km=10.0,
        uparea_km2=[[2_000_000.0]],
        width_m=[[50.0]],
        rivlen_m=[[1000.0]],
        next_x=[[1]],
        next_y=[[1]],
        uparea_to_km2=1e-6,
    )

    assert records[0].reach.upstream_area_km2 == 2.0
