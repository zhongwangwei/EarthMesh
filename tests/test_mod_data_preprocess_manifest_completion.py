from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs" / "fortran_to_rust_migration_manifest.json"

REQUIRED_EVIDENCE_FILES = [
    "rust/earthmesh_cli/tests/data_preprocess_landtype.rs",
    "rust/earthmesh_cli/tests/data_preprocess_threshold_readers.rs",
    "rust/earthmesh_cli/tests/data_preprocess_merit_netcdf.rs",
    "rust/earthmesh_cli/tests/data_preprocess_cama_binary.rs",
    "rust/earthmesh_cli/tests/data_preprocess_v3_geojson_sources.rs",
    "rust/earthmesh_cli/tests/hydro_close_mask_nml_cli.rs",
    "rust/earthmesh_cli/tests/hydro_close_recipe_cli.rs",
    "rust/earthmesh_cli/tests/hydro_composite_close_mask_cli.rs",
    "rust/earthmesh_cli/tests/colm_coupling_netcdf_cli.rs",
    "tests/test_v3_rust_geometry.py",
    "tests/test_v3_hydro_merit_pipeline.py",
]

REQUIRED_TEST_ANCHORS = [
    "data_preprocess_onelayer_and_twolayer_read_fortran_windows",
    "threshold_read_lnd_and_ocn_follow_enabled_flag_pairs",
    "binary_exports_geojson_linestring_as_buffered_close_mask",
    "binary_exports_geojson_multilinestring_as_cumulative_buffered_close_masks",
    "binary_exports_crossing_order_non_rectilinear_holes_as_split_slab_close_masks",
    "binary_preserves_non_rectilinear_multi_component_union_hole_without_bbox",
    "binary_dissolves_shared_edge_non_rectangular_polygons_without_bbox",
    "binary_dissolves_partial_shared_edge_polygons_without_bbox",
    "binary_dissolves_chained_non_rectangular_polygons_without_bbox",
    "binary_does_not_dissolve_bbox_overlapping_disjoint_non_rectangular_polygons",
    "binary_dissolves_overlapping_non_rectilinear_convex_polygons",
    "binary_writes_colm_coupling_netcdf_from_package_csv",
    "overlay_cell",
    "test_run_merit_v3_pipeline_records_effective_rust_geometry_backend",
]

SEARCH_FILES = [
    "rust/earthmesh_cli/src/lib.rs",
    "docs/fortran_to_rust_migration.md",
    "docs/fortran_to_rust_migration_manifest.json",
    *REQUIRED_EVIDENCE_FILES,
]


def manifest_entry() -> dict:
    manifest = json.loads(MANIFEST.read_text())
    return next(item for item in manifest["fortran_sources"] if item["path"] == "src/MOD_data_preprocess.F90")


LEGACY_SUBROUTINES = [
    "data_preprocess",
    "Threshold_Read_Lnd",
    "Threshold_Read_Ocn",
    "Threshold_Read_Atmos",
    "data_read_onelayer",
    "data_read_twolayer",
]


def test_mod_data_preprocess_completion_gate_has_threshold_hydro_colm_and_v3_evidence() -> None:
    corpus = "\n".join((ROOT / path).read_text(errors="ignore") for path in SEARCH_FILES if (ROOT / path).exists())

    missing_subroutines = [name for name in LEGACY_SUBROUTINES if name not in corpus]
    assert missing_subroutines == []

    missing_files = [path for path in REQUIRED_EVIDENCE_FILES if not (ROOT / path).exists()]
    assert missing_files == []

    missing_test_anchors = [name for name in REQUIRED_TEST_ANCHORS if name not in corpus]
    assert missing_test_anchors == []

    entry = manifest_entry()
    assert entry["remaining_rust_surfaces"] == []
    assert entry["port_status"] == "completed"
