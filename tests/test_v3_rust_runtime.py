import importlib
import subprocess
import sys
from pathlib import Path


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


def test_pyo3_extension_exports_geometry_functions():
    assert Path("rust/earthmesh_geometry/Cargo.toml").exists()
    develop_rust_geometry_extension()

    rust_geometry = importlib.import_module("earthmesh_geometry")

    assert rust_geometry.polygon_area([(0.0, 0.0), (2.0, 0.0), (0.0, 2.0)]) == 2.0
    assert rust_geometry.intersection_area(
        [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
        [(1.0, 0.0), (3.0, 0.0), (3.0, 2.0), (1.0, 2.0)],
    ) == 2.0
