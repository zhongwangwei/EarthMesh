import importlib
import subprocess
import sys

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
