use std::io;
use std::path::{Path, PathBuf};

use earthmesh_mesh::{
    refine_array_length_calculation_fortran_indexed, RefineArrayLengthCalculation,
};

use crate::{write_close_mesh_netcdf, LonLatPoint};

/// One `MOD_refine.F90:Array_length_calculation` close-curve file written via
/// the `MOD_file_preprocess.F90:close_Mesh_Save` compatibility schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineArrayLengthCloseMeshWriteReport {
    pub output: PathBuf,
    pub close_num: usize,
}

/// File-backed side-effect report for `Array_length_calculation` close-mesh
/// outputs. `mask_patch_ndm` mirrors the Fortran `mask_patch_ndm(step)` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineArrayLengthCloseMeshesWriteReport {
    pub mask_patch_ndm: usize,
    pub outputs: Vec<RefineArrayLengthCloseMeshWriteReport>,
}

/// Combined CLI-side evidence for `MOD_refine.F90:Array_length_calculation`:
/// the pure halo/boundary calculation plus the legacy close-mesh scratch files
/// written for downstream mask/refine steps.
#[derive(Debug, Clone, PartialEq)]
pub struct RefineArrayLengthCalculationRunReport {
    pub calculation: RefineArrayLengthCalculation,
    pub close_meshes: RefineArrayLengthCloseMeshesWriteReport,
}

/// Legacy path used by `MOD_refine.F90:Array_length_calculation` for one
/// refinement close-curve scratch file.
pub fn refine_array_length_close_mesh_output_path(
    file_dir: impl AsRef<Path>,
    step: usize,
    curve_id: usize,
) -> PathBuf {
    file_dir
        .as_ref()
        .join("tmpfile")
        .join(format!("mask_patch_close_{step}_{curve_id:03}.nc4"))
}

/// Write the close-mesh side effects produced by
/// `MOD_refine.F90:Array_length_calculation`.
///
/// The pure Rust mesh kernel returns closed curves as one-based vertex ids;
/// this adapter maps those ids through the Fortran-indexed `wp` coordinate table
/// and writes the same `close_Mesh_Save` schema/path family used by the legacy
/// refinement loop.
pub fn write_refine_array_length_close_meshes(
    file_dir: impl AsRef<Path>,
    step: usize,
    calculation: &RefineArrayLengthCalculation,
    wp: &[LonLatPoint],
) -> io::Result<RefineArrayLengthCloseMeshesWriteReport> {
    let num_closed_curve = calculation.boundary.curves.num_closed_curve;
    if calculation.boundary.curves.close_curves.len() < num_closed_curve + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "close_curves must include the placeholder plus num_closed_curve records",
        ));
    }

    let mut outputs = Vec::with_capacity(num_closed_curve);
    for curve_id in 1..=num_closed_curve {
        let curve = &calculation.boundary.curves.close_curves[curve_id];
        if curve.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("close curve {curve_id} must contain at least one vertex"),
            ));
        }
        let mut points = Vec::with_capacity(curve.len());
        for &vertex_id in curve {
            let point = wp.get(vertex_id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "close curve {curve_id} references vertex {vertex_id} without wp coordinate"
                    ),
                )
            })?;
            points.push(*point);
        }
        let output = refine_array_length_close_mesh_output_path(&file_dir, step, curve_id);
        write_close_mesh_netcdf(&output, &points)?;
        outputs.push(RefineArrayLengthCloseMeshWriteReport {
            output,
            close_num: points.len(),
        });
    }

    Ok(RefineArrayLengthCloseMeshesWriteReport {
        mask_patch_ndm: num_closed_curve,
        outputs,
    })
}

/// Run the migrated calculation and file side effects for
/// `MOD_refine.F90:Array_length_calculation`.
///
/// This composes the file-I/O-free `earthmesh_mesh` kernel with the
/// `close_Mesh_Save` compatibility writer so the CLI refine loop can keep the
/// Fortran scratch-file contract while the heavy topology work stays Rust-native.
#[allow(clippy::too_many_arguments)]
pub fn run_refine_array_length_calculation_fortran_indexed(
    file_dir: impl AsRef<Path>,
    step: usize,
    set_dis_in: usize,
    num_vertex: usize,
    num_center: usize,
    sjx_points: usize,
    lbx_points: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    initial_num_transition_row_triangles: usize,
    wp: &[LonLatPoint],
) -> io::Result<RefineArrayLengthCalculationRunReport> {
    let calculation = refine_array_length_calculation_fortran_indexed(
        set_dis_in,
        num_vertex,
        num_center,
        sjx_points,
        lbx_points,
        mrl_new,
        triangle_neighbors,
        cells_on_triangle,
        triangles_on_cell,
        edge_counts,
        initial_num_transition_row_triangles,
    )?;
    let close_meshes = write_refine_array_length_close_meshes(file_dir, step, &calculation, wp)?;
    Ok(RefineArrayLengthCalculationRunReport {
        calculation,
        close_meshes,
    })
}
