from __future__ import annotations

import json
from pathlib import Path

LEGACY_SUBROUTINES = [
    "Get_Contain",
    "IsInArea_ustr_Calculation",
    "Contain_Calculation",
    "Data_Updata",
]


def test_mod_getcontain_manifest_is_completed_with_subroutine_anchors():
    manifest_text = Path("docs/fortran_to_rust_migration_manifest.json").read_text()
    manifest = json.loads(manifest_text)
    entry = next(item for item in manifest["fortran_sources"] if item["path"] == "src/MOD_GetContain.F90")

    assert entry.get("port_status") == "completed"
    assert entry.get("remaining_rust_surfaces") == []

    evidence = "\n".join(
        [
            Path("rust/earthmesh_cli/src/lib.rs").read_text(errors="ignore"),
            Path("rust/earthmesh_mesh/src/lib.rs").read_text(errors="ignore"),
            manifest_text,
            Path("docs/fortran_to_rust_migration.md").read_text(errors="ignore"),
        ]
    )
    missing = [name for name in LEGACY_SUBROUTINES if name not in evidence]
    assert missing == []
