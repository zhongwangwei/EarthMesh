import struct

from util.hydro_mesh.cama_binary import CamaGridSpec, read_binary_window


def _write_float_grid(path, nx, ny):
    values = [float(i) for i in range(nx * ny)]
    path.write_bytes(struct.pack(f"<{len(values)}f", *values))


def _write_int_grid(path, nx, ny, components):
    values = list(range(nx * ny * components))
    path.write_bytes(struct.pack(f"<{len(values)}i", *values))


def test_read_binary_window_reads_float32_rows_without_loading_full_grid(tmp_path):
    binary = tmp_path / "uparea.bin"
    _write_float_grid(binary, nx=5, ny=4)

    window = read_binary_window(
        binary,
        CamaGridSpec(nx=5, ny=4, west=-180.0, south=-90.0, grid_size_deg=1.0),
        x_start=1,
        y_start=1,
        width=3,
        height=2,
        dtype="float32",
    )

    assert window == [
        [6.0, 7.0, 8.0],
        [11.0, 12.0, 13.0],
    ]


def test_read_binary_window_reads_component_from_int32_topology_file(tmp_path):
    binary = tmp_path / "nextxy.bin"
    _write_int_grid(binary, nx=4, ny=3, components=2)

    next_y = read_binary_window(
        binary,
        CamaGridSpec(nx=4, ny=3, west=-180.0, south=-90.0, grid_size_deg=1.0),
        x_start=1,
        y_start=0,
        width=2,
        height=2,
        dtype="int32",
        components=2,
        component_index=1,
    )

    assert next_y == [
        [3, 5],
        [11, 13],
    ]


def test_read_binary_window_rejects_out_of_bounds_window(tmp_path):
    binary = tmp_path / "uparea.bin"
    _write_float_grid(binary, nx=3, ny=2)

    try:
        read_binary_window(
            binary,
            CamaGridSpec(nx=3, ny=2, west=-180.0, south=-90.0, grid_size_deg=1.0),
            x_start=2,
            y_start=0,
            width=2,
            height=1,
            dtype="float32",
        )
    except ValueError as exc:
        assert "outside grid" in str(exc)
    else:
        raise AssertionError("expected out-of-bounds window to fail")


def test_grid_spec_converts_bbox_to_clipped_window():
    grid = CamaGridSpec(nx=360, ny=180, west=-180.0, south=-90.0, grid_size_deg=1.0)

    assert grid.window_for_bbox(west=100.0, east=102.0, south=20.0, north=22.0) == (280, 110, 2, 2)
    assert grid.window_for_bbox(west=-200.0, east=-179.0, south=-95.0, north=-89.0) == (0, 0, 1, 1)


def test_read_binary_window_honors_y_reversed_storage(tmp_path):
    binary = tmp_path / "uparea.bin"
    _write_float_grid(binary, nx=3, ny=3)
    grid = CamaGridSpec(
        nx=3,
        ny=3,
        west=0.0,
        south=0.0,
        grid_size_deg=1.0,
        y_reversed_storage=True,
    )

    south_row = read_binary_window(
        binary,
        grid,
        x_start=0,
        y_start=0,
        width=3,
        height=1,
        dtype="float32",
    )

    assert south_row == [[6.0, 7.0, 8.0]]
