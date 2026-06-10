from util.hydro_mesh.cama_contract import inspect_cama_variables


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
