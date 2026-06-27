use std::io;

use crate::GetContainMeshKind;

pub(crate) fn getcontain_mesh_kind_from_mesh_type(
    mesh_type: &str,
) -> io::Result<GetContainMeshKind> {
    match mesh_type {
        "landmesh" => Ok(GetContainMeshKind::Land),
        "oceanmesh" => Ok(GetContainMeshKind::Ocean),
        "atmosmesh" => Ok(GetContainMeshKind::Atmos),
        "LOCmesh" | "earthmesh" => Ok(GetContainMeshKind::Loc),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported Get_Contain mesh_type {other}"),
        )),
    }
}
