from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "docs" / "fortran_to_rust_migration_manifest.json"

REQUIRED_EVIDENCE_FILES = [
    "rust/earthmesh_core/tests/constants.rs",
    "rust/earthmesh_cli/tests/mkgrd_gridinit.rs",
    "rust/earthmesh_cli/tests/mkgrd_refine_prepare_namelist.rs",
    "rust/earthmesh_cli/tests/mkgrd_refine_source_branch_executor.rs",
    "rust/earthmesh_cli/tests/mkgrd_refine_loop_execution.rs",
    "rust/earthmesh_cli/tests/mkgrd_top_level_refine_runner.rs",
    "rust/earthmesh_cli/tests/mkgrd_mask_restart.rs",
    "rust/earthmesh_cli/tests/mkgrd_restart_area_judge.rs",
]

REQUIRED_TEST_ANCHORS = [
    "constants_match_fortran_consts_coms_formulas",
    "grid_memory_allocators_match_mem_grid_zero_initialization",
    "ijtab_allocators_match_mem_ijtabs_defaults",
    "delaunay_memory_allocators_match_mem_delaunay_defaults",
    "runtime_state_wires_configs_and_legacy_memories_without_fortran_globals",
    "runtime_state_records_real_mesh_counts_for_fortran_steps",
    "runtime_state_records_legacy_num_vertex_boundary",
    "runtime_state_records_legacy_scalar_defaults_from_consts_coms_and_mkgrd",
    "runtime_state_derives_num_center_from_previous_step_wp_count",
    "runtime_state_records_legacy_impent_pentagon_indices",
    "runtime_state_records_data_preprocess_source_grid_globals",
    "run_mkgrd_gridinit_global_namelist_writes_initial_gridfile",
    "library_provides_data_preprocess_source_branch_options_from_prepare",
    "source_branch_executor_dispatches_calculated_then_specified_sources",
    "refine_loop_final_domain_contain_records_previous_num_vertex_in_runtime_state",
    "top_level_runner_derives_migrated_source_options_and_runs_standard_stack",
    "top_level_runner_can_use_data_preprocess_landtype_source_state_without_source_state_file",
    "top_level_dispatch_runs_mask_restart_patch_branch_without_gridinit_error",
    "top_level_dispatch_runs_mask_restart_ocean_postproc_branch_without_plan_only",
    "top_level_dispatch_runs_non_ocean_mask_restart_area_judge_continuation_without_plan_only",
    "binary_mask_restart_area_judge_can_generate_earth_final_postproc_outputs",
]

SEARCH_FILES = [
    "rust/earthmesh_core/src/lib.rs",
    "rust/earthmesh_cli/src/lib.rs",
    "docs/fortran_to_rust_migration.md",
    "docs/fortran_to_rust_migration_manifest.json",
    *REQUIRED_EVIDENCE_FILES,
]


def manifest_entry() -> dict:
    manifest = json.loads(MANIFEST.read_text())
    return next(item for item in manifest["fortran_sources"] if item["path"] == "src/consts_coms.F90")


LEGACY_SUBROUTINES = [
    "alloc_xyzem",
    "alloc_xyzew",
    "alloc_grid_lonlatmw",
    "alloc_itabs",
    "alloc_itabsd",
]


def test_consts_coms_completion_gate_has_runtime_state_and_memory_evidence() -> None:
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
