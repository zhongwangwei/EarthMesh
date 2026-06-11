import shutil
import subprocess
from pathlib import Path


def test_rust_geometry_crate_tests_pass_when_cargo_is_available():
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required to verify the Rust geometry MVP"
    manifest = Path("rust/earthmesh_geometry/Cargo.toml")
    assert manifest.exists()

    result = subprocess.run(
        [cargo, "test", "--manifest-path", str(manifest)],
        check=True,
        capture_output=True,
        text=True,
    )

    assert "test result: ok" in result.stdout
