from util.hydro_mesh.cama_contract import inspect_cama_file_inventory, inspect_cama_variables, parse_cama_params_text


def test_inspect_cama_variables_maps_common_aliases():
    report = inspect_cama_variables(
        [
            "lon",
            "lat",
            "nextxy",
            "uparea",
            "rivwth",
            "rivlen",
            "fldwth",
        ]
    )

    assert report.is_usable
    assert report.canonical_to_source == {
        "lon": "lon",
        "lat": "lat",
        "downstream_topology": "nextxy",
        "upstream_area": "uparea",
        "river_width": "rivwth",
        "river_length": "rivlen",
        "floodplain_width": "fldwth",
    }
    assert report.missing_required == []


def test_inspect_cama_variables_reports_missing_required_fields():
    report = inspect_cama_variables(["lon", "lat", "rivwth"])

    assert not report.is_usable
    assert report.missing_required == [
        "downstream_topology",
        "upstream_area",
        "river_length",
    ]
    assert report.canonical_to_source["river_width"] == "rivwth"


def test_inspect_cama_variables_accepts_case_insensitive_names():
    report = inspect_cama_variables(["XLON", "YLAT", "NEXTXY", "UPAREA", "RIVWTH", "RIVLEN"])

    assert report.is_usable
    assert report.canonical_to_source["lon"] == "XLON"
    assert report.canonical_to_source["downstream_topology"] == "NEXTXY"


def test_inspect_cama_file_inventory_accepts_glb_01min_binary_layout():
    report = inspect_cama_file_inventory(
        [
            "glb_01min/mapdim.txt",
            "glb_01min/params.txt",
            "glb_01min/nextxy.bin",
            "glb_01min/uparea.bin",
            "glb_01min/width.bin",
            "glb_01min/rivlen.bin",
        ]
    )

    assert report.is_usable
    assert report.canonical_to_source == {
        "downstream_topology": "glb_01min/nextxy.bin",
        "upstream_area": "glb_01min/uparea.bin",
        "river_width": "glb_01min/width.bin",
        "river_length": "glb_01min/rivlen.bin",
    }
    assert "using width.bin because rivwth.bin is absent" in report.warnings


def test_parse_cama_params_text_reads_global_one_minute_grid():
    params = parse_cama_params_text(
        "       21600      !! grid number (east-west)\n"
        "       10800      !! grid number (north-south)\n"
        "          10     !! floodplain layer\n"
        "  0.01666667     !! grid size\n"
        "    -180.000     !! west  edge (deg)\n"
        "     180.000     !! east  edge (deg)\n"
        "     -90.000     !! south edge (deg)\n"
        "      90.000     !! north edge (deg)\n"
    )

    assert params == {
        "nx": 21600,
        "ny": 10800,
        "floodplain_layers": 10,
        "grid_size_deg": 0.01666667,
        "west": -180.0,
        "east": 180.0,
        "south": -90.0,
        "north": 90.0,
    }
