from __future__ import annotations

import json
from pathlib import Path


def test_fortran_to_rust_manifest_tracks_every_fortran_source_file():
    manifest_path = Path("docs/fortran_to_rust_migration_manifest.json")
    assert manifest_path.exists(), "migration manifest is required for full Fortran-to-Rust refactor"

    manifest = json.loads(manifest_path.read_text())
    entries = manifest["fortran_sources"]
    tracked = {entry["path"] for entry in entries}
    actual = {str(path) for path in Path("src").glob("*.F90")}

    assert tracked == actual
    for entry in entries:
        assert entry["rust_target"].startswith("rust/") or entry["strategy"] == "external_crate"
        assert entry["migration_phase"] in {"phase_0", "phase_1", "phase_2", "phase_3", "phase_4", "phase_5"}
        assert entry["strategy"] in {"port", "wrap_then_port", "external_crate", "replace_with_v3_pipeline"}
        assert entry["parity_gate"]
