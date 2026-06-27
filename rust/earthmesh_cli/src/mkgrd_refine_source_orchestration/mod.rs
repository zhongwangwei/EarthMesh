use std::io;

use earthmesh_core::RefineConfig;

use crate::MkgrdRefineLoopPrepareReport;

mod data_preprocess_sources;
mod derived_execution;
mod source_options;

pub use data_preprocess_sources::{
    data_preprocess_source_state_calculated_refine_from_prepare,
    run_mkgrd_refine_compact_source_state_namelist, run_mkgrd_refine_landtype_source_namelist,
    run_mkgrd_refine_loop_execution_with_data_preprocess_source_state,
    with_data_preprocess_source_state_refine_source_branch_options_from_prepare,
};
pub use derived_execution::{
    run_mkgrd_refine_loop_namelist_with_calculated_migrated_executor_and_prepare_hook,
    run_mkgrd_refine_loop_namelist_with_derived_migrated_executor_and_final_domain_contain_and_prepare_hook,
    run_mkgrd_refine_loop_namelist_with_derived_migrated_executor_and_prepare_hook,
    run_mkgrd_refine_loop_namelist_with_specified_migrated_executor_and_prepare_hook,
};
pub use source_options::{
    mkgrd_calculated_refine_source_options_from_prepare,
    mkgrd_refine_source_branch_options_from_prepare,
    mkgrd_specified_refine_source_options_from_prepare,
};

pub(crate) fn runtime_refine_from_prepare(
    prepare: &MkgrdRefineLoopPrepareReport,
) -> io::Result<&RefineConfig> {
    prepare.runtime_state.refine.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "prepared mkgrd runtime state is missing refine config",
        )
    })
}
