from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_root_makefile_builds_rust_cli_not_legacy_fortran_objects():
    makefile = (ROOT / "Makefile").read_text()

    assert "rust/earthmesh_cli/Cargo.toml" in makefile
    assert "cargo" in makefile.lower()
    assert "all: build" in makefile
    assert ".F90" not in makefile
    assert "include ./Makeoptions" not in makefile
    assert "${FF}" not in makefile


def test_compatibility_scripts_delegate_to_rust_build_path():
    for script in ["make.sh", "make_gnu.sh", "switch_compiler.sh"]:
        text = (ROOT / script).read_text()
        assert "gfortran" not in text.lower()
        assert "ifort" not in text.lower()
        assert "Makeoptions.gnu" not in text

    assert "make clean" in (ROOT / "make_gnu.sh").read_text()
    assert "Rust/Cargo" in (ROOT / "switch_compiler.sh").read_text()
