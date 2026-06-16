from __future__ import annotations

import json
from pathlib import Path

LEGACY_SUBROUTINES = [
    "GetRef",
    "GetRef_Lnd",
    "GetRef_Ocn",
    "GetRef_Atmos",
    "GetRef_LOC",
    "mean_std_cal2d",
    "mean_std_cal3d",
]


def test_mod_getref_manifest_is_completed_with_subroutine_anchors():
    manifest = json.loads(Path("docs/fortran_to_rust_migration_manifest.json").read_text())
    entry = next(item for item in manifest["fortran_sources"] if item["path"] == "src/MOD_GetRef.F90")

    assert entry.get("port_status") == "completed"
    assert entry.get("remaining_rust_surfaces") == []

    evidence = "\n".join(
        [
            Path("rust/earthmesh_cli/src/lib.rs").read_text(errors="ignore"),
            Path("docs/fortran_to_rust_migration_manifest.json").read_text(errors="ignore"),
            Path("docs/fortran_to_rust_migration.md").read_text(errors="ignore"),
        ]
    )
    missing = [name for name in LEGACY_SUBROUTINES if name not in evidence]
    assert missing == []
