mod indexing;
mod topology;
mod types;

pub(crate) use indexing::{mesh_canonical_id_for_row, mesh_row_for_canonical_id};
pub use topology::check_unstructured_mesh_topology;
pub(crate) use topology::{unstructured_dimc, validate_unstructured_mesh};
pub use types::{
    GridfileCellKind, GridfileMeshPoints, IapMeshReadPayload, MethodCGridfileMetadataSlices,
    UnstructuredMesh, UnstructuredMeshTopologyReport, UnstructuredMeshWriteReport,
};
