mod indexing;
mod topology;
mod types;

pub(crate) use indexing::{
    gridfile_m_row_layout, gridfile_w_row_layout, mesh_canonical_id_for_row,
    mesh_m_has_two_placeholder_rows, mesh_row_for_canonical_id, mesh_w_has_two_placeholder_rows,
    unstructured_w_row_layout, GridfileRowLayout,
};
pub use topology::{check_unstructured_mesh_topology, validate_published_cell_degrees};
pub(crate) use topology::{
    split_non_manifold_triangle_vertex_fans, unstructured_dimc, validate_unstructured_mesh,
};
pub use types::{
    GridfileCellKind, GridfileMeshPoints, GridfileMetadataSlices, IapMeshReadPayload,
    MethodCGridfileLineages, UnstructuredMesh, UnstructuredMeshTopologyReport,
    UnstructuredMeshWriteReport,
};
