import importlib
import subprocess
import sys
from math import isclose

from util.v3_core.geometry import MaskFeature
from util.v3_core.geometry_backend import (
    PythonGeometryBackend,
    RustGeometryBackend,
    get_geometry_backend,
)
from util.v3_core.schema import CanonicalCell


def develop_rust_geometry_extension() -> None:
    subprocess.run(
        [
            sys.executable,
            "-m",
            "maturin",
            "develop",
            "--manifest-path",
            "rust/earthmesh_geometry/Cargo.toml",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    importlib.invalidate_caches()


def test_get_geometry_backend_returns_python_backend_by_default():
    backend = get_geometry_backend()

    assert isinstance(backend, PythonGeometryBackend)


def test_python_backend_overlays_cells_with_masks():
    backend = PythonGeometryBackend()
    cell = CanonicalCell.minimal("cell", cell_type="POLYGON")
    mask = MaskFeature("land", "LAND", 1, [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)])

    results = backend.overlay_cells([cell], [mask])

    assert len(results) == 1
    assert results[0].winning_class == "LAND"
    assert results[0].class_fractions == {"LAND": 1.0}


def test_get_geometry_backend_returns_rust_backend_when_requested():
    develop_rust_geometry_extension()

    backend = get_geometry_backend("rust")

    assert isinstance(backend, RustGeometryBackend)
    assert backend.name == "rust_pyo3"


def test_rust_backend_matches_python_backend_for_overlay_fixture():
    develop_rust_geometry_extension()
    cell = CanonicalCell(
        cell_id="cell",
        cell_index=0,
        cell_type="POLYGON",
        center_lon=1.0,
        center_lat=1.0,
        area_m2=4.0,
        vertices=[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
    )
    masks = [
        MaskFeature(
            "coast",
            "COAST_LAND",
            10,
            [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
        ),
        MaskFeature(
            "river",
            "R3",
            30,
            [(1.0, 0.0), (2.0, 0.0), (2.0, 2.0), (1.0, 2.0)],
        ),
    ]

    python_result = PythonGeometryBackend().overlay_cells([cell], masks)[0]
    rust_result = RustGeometryBackend().overlay_cells([cell], masks)[0]

    assert rust_result.winning_class == python_result.winning_class
    assert rust_result.winning_priority == python_result.winning_priority
    assert rust_result.source_feature_ids == python_result.source_feature_ids
    assert rust_result.quality_flags == python_result.quality_flags
    assert rust_result.class_fractions == python_result.class_fractions


def test_rust_backend_overlays_concave_hydro_mask_via_pyo3_runtime():
    develop_rust_geometry_extension()
    cell = CanonicalCell(
        cell_id="cell",
        cell_index=0,
        cell_type="POLYGON",
        center_lon=1.5,
        center_lat=1.5,
        area_m2=4.0,
        vertices=[(0.5, 0.5), (2.5, 0.5), (2.5, 2.5), (0.5, 2.5)],
    )
    mask = MaskFeature(
        "concave-river",
        "R2",
        20,
        [(0.0, 0.0), (3.0, 0.0), (3.0, 1.0), (1.0, 1.0), (1.0, 3.0), (0.0, 3.0)],
    )

    result = RustGeometryBackend().overlay_cells([cell], [mask])[0]

    assert result.winning_class == "R2"
    assert result.winning_priority == 20
    assert result.source_feature_ids == ["concave-river"]
    assert result.quality_flags == []
    assert result.class_fractions == {"R2": 0.4375}


def test_rust_pyo3_overlay_cell_returns_classification_payload_for_concave_masks():
    develop_rust_geometry_extension()
    rust_geometry = importlib.import_module("earthmesh_geometry")

    winning_class, winning_priority, fractions, source_ids, quality_flags = rust_geometry.overlay_cell(
        [(0.5, 0.5), (2.5, 0.5), (2.5, 2.5), (0.5, 2.5)],
        [
            (
                "coast",
                "COAST_LAND",
                10,
                [(0.5, 0.5), (2.5, 0.5), (2.5, 2.5), (0.5, 2.5)],
            ),
            (
                "river",
                "R2",
                30,
                [(0.0, 0.0), (3.0, 0.0), (3.0, 1.0), (1.0, 1.0), (1.0, 3.0), (0.0, 3.0)],
            ),
        ],
    )

    fraction_map = dict(fractions)
    assert winning_class == "R2"
    assert winning_priority == 30
    assert source_ids == ["coast", "river"]
    assert quality_flags == []
    assert isclose(fraction_map["COAST_LAND"], 1.0)
    assert isclose(fraction_map["R2"], 0.4375)


def test_rust_pyo3_overlay_cells_batches_multiple_cells_with_ids():
    develop_rust_geometry_extension()
    rust_geometry = importlib.import_module("earthmesh_geometry")

    results = rust_geometry.overlay_cells(
        [
            ("wet-cell", [(0.5, 0.5), (2.5, 0.5), (2.5, 2.5), (0.5, 2.5)]),
            ("dry-cell", [(5.0, 5.0), (6.0, 5.0), (6.0, 6.0), (5.0, 6.0)]),
        ],
        [
            (
                "river",
                "R2",
                30,
                [(0.0, 0.0), (3.0, 0.0), (3.0, 1.0), (1.0, 1.0), (1.0, 3.0), (0.0, 3.0)],
            )
        ],
    )

    assert results[0][0] == "wet-cell"
    assert results[0][1] == "R2"
    assert results[0][2] == 30
    assert dict(results[0][3]) == {"R2": 0.4375}
    assert results[0][4] == ["river"]
    assert results[0][5] == []
    assert results[1] == (
        "dry-cell",
        "UNKNOWN",
        0,
        [("UNKNOWN", 1.0)],
        [],
        ["missing_mask"],
    )


def test_rust_backend_batches_multiple_cells_through_pyo3_runtime():
    develop_rust_geometry_extension()
    cells = [
        CanonicalCell(
            cell_id="wet-cell",
            cell_index=0,
            cell_type="POLYGON",
            center_lon=1.5,
            center_lat=1.5,
            area_m2=4.0,
            vertices=[(0.5, 0.5), (2.5, 0.5), (2.5, 2.5), (0.5, 2.5)],
        ),
        CanonicalCell(
            cell_id="dry-cell",
            cell_index=1,
            cell_type="POLYGON",
            center_lon=5.5,
            center_lat=5.5,
            area_m2=1.0,
            vertices=[(5.0, 5.0), (6.0, 5.0), (6.0, 6.0), (5.0, 6.0)],
        ),
    ]
    masks = [
        MaskFeature(
            "river",
            "R2",
            30,
            [(0.0, 0.0), (3.0, 0.0), (3.0, 1.0), (1.0, 1.0), (1.0, 3.0), (0.0, 3.0)],
        )
    ]

    results = RustGeometryBackend().overlay_cells(cells, masks)

    assert [result.cell_id for result in results] == ["wet-cell", "dry-cell"]
    assert results[0].winning_class == "R2"
    assert results[0].class_fractions == {"R2": 0.4375}
    assert results[1].winning_class == "UNKNOWN"
    assert results[1].quality_flags == ["missing_mask"]
