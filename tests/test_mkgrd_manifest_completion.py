from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs" / "fortran_to_rust_migration_manifest.json"

REQUIRED_EVIDENCE_FILES = [
    "rust/earthmesh_cli/tests/mkgrd_gridinit.rs",
    "rust/earthmesh_cli/tests/mkgrd_initial_quality_check.rs",
    "rust/earthmesh_cli/tests/mkgrd_final_quality_check_executor.rs",
    "rust/earthmesh_cli/tests/mkgrd_refine_prepare_namelist.rs",
    "rust/earthmesh_cli/tests/mkgrd_refine_source_branch_executor.rs",
    "rust/earthmesh_cli/tests/mkgrd_refine_loop_execution.rs",
    "rust/earthmesh_cli/tests/mkgrd_refine_final_handoff.rs",
    "rust/earthmesh_cli/tests/mkgrd_top_level_refine_runner.rs",
    "rust/earthmesh_cli/tests/mkgrd_mask_restart.rs",
    "rust/earthmesh_cli/tests/mkgrd_restart_area_judge.rs",
    "rust/earthmesh_cli/tests/mask_postproc_atmos_mpas_simple.rs",
    "rust/earthmesh_cli/tests/mask_postproc_earth_runner.rs",
    "rust/earthmesh_cli/tests/mask_postproc_land_runner.rs",
    "rust/earthmesh_cli/tests/mask_postproc_ocean_runner.rs",
    "rust/earthmesh_cli/tests/mpas_full_builder.rs",
    "rust/earthmesh_cli/tests/mpas_simple_builder.rs",
]

REQUIRED_TEST_ANCHORS = [
    "run_mkgrd_gridinit_global_namelist_writes_initial_gridfile",
    "run_mkgrd_gridinit_global_matches_fortran_nxp64_gridfile_fixture",
    "initial_grid_quality_check_reads_gridfile_and_writes_orial_quality",
    "working_state_executor_restores_compact_one_based_final_quality_gridfile_shape",
    "regional_final_quality_runs_with_source_mask_classification_inputs",
    "library_provides_data_preprocess_source_branch_options_from_prepare",
    "source_branch_executor_dispatches_calculated_then_specified_sources",
    "refine_loop_final_domain_contain_records_previous_num_vertex_in_runtime_state",
    "top_level_runner_derives_migrated_source_options_and_runs_standard_stack",
    "binary_source_state_land_reports_patchtype_output",
    "binary_source_state_can_run_ocean_final_domain_postproc",
    "binary_source_state_atmos_full_mpas_reports_mesh_and_graph_outputs",
    "binary_source_state_earth_reports_patchtype_and_info_outputs",
    "top_level_runner_can_use_data_preprocess_landtype_source_state_without_source_state_file",
    "library_landtype_source_runner_can_execute_atmos_mpas_simple_final_postproc",
    "binary_landtype_source_atmos_full_mpas_reports_mesh_and_graph_outputs",
    "binary_landtype_source_runs_ocean_final_domain_postproc",
    "binary_landtype_source_runs_land_final_domain_postproc",
    "binary_default_entry_runs_refine_landtype_source_without_explicit_mode_flag",
    "top_level_dispatch_runs_mask_restart_patch_branch_without_gridinit_error",
    "top_level_dispatch_runs_mask_restart_ocean_postproc_branch_without_plan_only",
    "top_level_dispatch_runs_non_ocean_mask_restart_area_judge_continuation_without_plan_only",
    "default_restart_dispatch_runs_atmos_mpas_simple_final_postproc_when_num_vertex_is_supplied",
    "default_restart_dispatch_runs_atmos_mpas_final_postproc_when_num_vertex_is_supplied",
    "binary_mask_restart_area_judge_can_generate_earth_final_postproc_outputs",
    "restart_area_judge_refine_handoff_runs_migrated_refine_loop_from_restart_state",
    "binary_restart_refine_source_state_atmos_full_mpas_reports_mesh_and_graph_outputs",
    "binary_restart_refine_source_state_earth_reports_patchtype_and_info_outputs",
    "binary_default_restart_refine_source_state_earth_hex_reports_patchtype_and_info_outputs",
    "binary_default_restart_refine_source_state_land_uses_source_state_num_vertex_for_postproc",
    "binary_default_restart_refine_source_state_ocean_uses_source_state_num_vertex_for_postproc",
    "binary_default_restart_refine_landtype_ocean_uses_mode_grid_num_vertex_for_postproc",
    "binary_default_restart_refine_landtype_atmos_full_mpas_reports_mesh_and_graph_outputs",
    "binary_default_restart_refine_landtype_earth_hex_reports_patchtype_and_info_outputs",
    "binary_default_restart_refine_requires_source_state_or_landtype_file",
]

SEARCH_FILES = [
    "rust/earthmesh_cli/src/lib.rs",
    "rust/earthmesh_cli/src/main.rs",
    "docs/fortran_to_rust_migration.md",
    "docs/fortran_to_rust_migration_manifest.json",
    *REQUIRED_EVIDENCE_FILES,
]


def manifest_entry() -> dict:
    manifest = json.loads(MANIFEST.read_text())
    return next(item for item in manifest["fortran_sources"] if item["path"] == "src/mkgrd.F90")


LEGACY_SUBROUTINES = [
    "Inital_Grid_Quality_Check",
    "Final_Grid_Quality_Check",
    "mode4mesh_make",
    "read_nl",
    "Mask_make",
    "bbox_mask_make",
    "lamb_mask_make",
    "circle_mask_make",
    "close_mask_make",
    "init_consts",
    "gridinit",
    "gridfile_write",
    "CHECK",
    "voronoi",
    "pcvt",
    "grid_xyz2lonlat",
]


def test_mkgrd_completion_gate_has_restart_refine_continue_matrix_evidence() -> None:
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
