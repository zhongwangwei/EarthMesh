from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs" / "fortran_to_rust_migration_manifest.json"

REQUIRED_EVIDENCE_FILES = [
    "rust/earthmesh_cli/tests/mod_file_preprocess_bbox_circle_mesh.rs",
    "rust/earthmesh_cli/tests/close_mesh_io.rs",
    "rust/earthmesh_cli/tests/fvcom_mesh_save_writer.rs",
    "rust/earthmesh_cli/tests/iap_mesh_read.rs",
    "rust/earthmesh_cli/tests/mpas_full_writer.rs",
    "rust/earthmesh_cli/tests/mpas_full_builder.rs",
    "rust/earthmesh_cli/tests/mpas_simple_writer.rs",
    "rust/earthmesh_cli/tests/mpas_simple_builder.rs",
    "rust/earthmesh_cli/tests/mpas_graph_info_writer.rs",
    "rust/earthmesh_cli/tests/mpas_edge_reference_reader.rs",
    "rust/earthmesh_cli/tests/mode4mesh_make.rs",
]

SEARCH_FILES = [
    "rust/earthmesh_cli/src/lib.rs",
    "docs/fortran_to_rust_migration.md",
    "docs/fortran_to_rust_migration_manifest.json",
    *REQUIRED_EVIDENCE_FILES,
]


def manifest_entry() -> dict:
    manifest = json.loads(MANIFEST.read_text())
    return next(item for item in manifest["fortran_sources"] if item["path"] == "src/MOD_file_preprocess.F90")


LEGACY_SUBROUTINES = [
    "distsOnEdge_save",
    "cellwidth_save",
    "cellwidth_read",
    "data_read",
    "FVCOM_Mesh_Read",
    "FVCOM_Mesh_Save",
    "IAP_Mesh_Read",
    "MPAS_Mesh_Read",
    "MPAS_Mesh_Save",
    "MPAS_info_Save",
    "MPAS_Mesh_Simple_Save",
    "Mode4_Mesh_Read",
    "Mode4_Mesh_Save",
    "Unstructured_Mesh_Read",
    "Unstructured_Mesh_Save",
    "bbox_Mesh_Read",
    "bbox_Mesh_Save",
    "circle_Mesh_Read",
    "circle_Mesh_Save",
    "close_Mesh_Read",
    "close_Mesh_Save",
    "Contain_Read",
    "Contain_Save",
    "LOCmesh_info_save",
    "quality_save_global",
]


def test_mod_file_preprocess_completion_gate_has_all_subroutine_anchors_and_fixture_evidence() -> None:
    corpus = "\n".join((ROOT / path).read_text(errors="ignore") for path in SEARCH_FILES if (ROOT / path).exists())
    missing = [name for name in LEGACY_SUBROUTINES if name not in corpus]
    assert missing == []

    missing_files = [path for path in REQUIRED_EVIDENCE_FILES if not (ROOT / path).exists()]
    assert missing_files == []

    entry = manifest_entry()
    assert entry["remaining_rust_surfaces"] == []
    assert entry["port_status"] == "completed"
