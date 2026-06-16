from __future__ import annotations

import json
from pathlib import Path


def test_fortran_to_rust_manifest_tracks_archived_legacy_fortran_sources():
    manifest_path = Path("docs/fortran_to_rust_migration_manifest.json")
    assert manifest_path.exists(), "migration manifest is required for full Fortran-to-Rust refactor"

    manifest = json.loads(manifest_path.read_text())
    entries = manifest["fortran_sources"]

    assert entries
    assert not list(Path("src").glob("*.F90")), "legacy Fortran sources should stay outside the active tree"
    for entry in entries:
        assert entry["path"].startswith("src/")
        assert entry["path"].endswith(".F90")
        assert entry["line_count"] > 0
        assert entry["rust_target"].startswith("rust/") or entry["strategy"] == "external_crate"
        assert entry["migration_phase"] in {"phase_0", "phase_1", "phase_2", "phase_3", "phase_4", "phase_5"}
        assert entry["strategy"] in {"port", "wrap_then_port", "external_crate", "replace_with_v3_pipeline"}
        assert entry["parity_gate"]


def test_started_manifest_entries_have_explicit_remaining_surfaces():
    manifest_path = Path("docs/fortran_to_rust_migration_manifest.json")
    manifest = json.loads(manifest_path.read_text())

    ambiguous = [
        entry["path"]
        for entry in manifest["fortran_sources"]
        if entry.get("port_status") == "started" and not entry.get("remaining_rust_surfaces")
    ]

    assert ambiguous == []
