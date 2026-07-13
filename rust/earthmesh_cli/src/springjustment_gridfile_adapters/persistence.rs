use crate::write_cellwidth_netcdf;
use crate::write_dists_on_edge_netcdf;
use crate::CellwidthMesh;
use crate::DistsOnEdgeMesh;
use crate::SpringjustmentGlobalPersistenceReport;
use earthmesh_mesh::LonLatDegrees;
use std::fs;
use std::io;
use std::path::Path;

use super::conversion::lonlat_degrees_to_lonlat_point;

/// Persist the file side effects produced near the start of
/// `MOD_grid_preprocess.F90:Springjustment_global`.
pub fn write_springjustment_global_persistence(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    cell_points_for_cellwidth: &[LonLatDegrees],
    output: &earthmesh_mesh::SpringjustmentGlobalCoreOutput,
) -> io::Result<SpringjustmentGlobalPersistenceReport> {
    let file_dir = file_dir.as_ref();
    let result_dir = file_dir.join("result");
    fs::create_dir_all(&result_dir)?;

    let edge_points = output
        .edge_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_lonlat_point)
        .collect::<Vec<_>>();
    let dists_on_edge = write_dists_on_edge_netcdf(
        result_dir.join(format!("distsOnEdge_NXP{nxp:04}_{step:02}_global.nc4")),
        &DistsOnEdgeMesh {
            edge_points,
            dists_on_edge: output.dists_on_edge.clone(),
        },
    )?;

    let cellwidth = if let Some(cellwidth) = &output.cellwidth {
        let cell_points = cell_points_for_cellwidth
            .iter()
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect::<Vec<_>>();
        Some(write_cellwidth_netcdf(
            result_dir.join(format!("cellwidth_NXP{nxp:04}_global.nc4")),
            &CellwidthMesh {
                cell_points,
                cellwidth: cellwidth.clone(),
            },
        )?)
    } else {
        None
    };

    Ok(SpringjustmentGlobalPersistenceReport {
        dists_on_edge,
        cellwidth,
    })
}
