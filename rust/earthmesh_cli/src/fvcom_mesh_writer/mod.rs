mod write;

pub(crate) use write::write_fvcom_ns_records;
pub use write::{
    fvcom_mesh_2dm_output_path, write_fvcom_mesh_2dm, write_fvcom_mesh_save_outputs,
    FvcomMesh2dmWriteReport,
};
