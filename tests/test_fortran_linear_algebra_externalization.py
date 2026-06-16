from __future__ import annotations

import json
from pathlib import Path


def test_vendored_blas_lapack_are_archived_and_externalized_in_manifest():
    manifest = json.loads(Path("docs/fortran_to_rust_migration_manifest.json").read_text())
    by_path = {entry["path"]: entry for entry in manifest["fortran_sources"]}

    assert not Path("src/blas.F90").exists()
    assert not Path("src/lapack.F90").exists()
    for source in ["src/blas.F90", "src/lapack.F90"]:
        entry = by_path[source]
        assert entry["strategy"] == "external_crate"
        assert entry["port_status"] == "completed"
        assert entry["remaining_rust_surfaces"] == []
        assert "externalized" in " ".join(entry["completed_rust_surfaces"]).lower()
