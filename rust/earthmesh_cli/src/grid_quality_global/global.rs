use crate::write_quality_global_netcdf;
use crate::GlobalQualityMesh;
use crate::GlobalQualityWriteReport;
use crate::QualityClassMetrics;
use std::io;
use std::path::Path;

/// Convert the pure `Grid_Quality_Check_Global` Rust kernel output into the
/// `quality_save_global` writer payload.
pub fn global_quality_mesh_from_grid_quality(
    quality: &earthmesh_mesh::GridQualityGlobalOutput,
) -> GlobalQualityMesh {
    GlobalQualityMesh {
        sjx: quality_class_from_triangle_quality(&quality.triangle),
        wbx: quality.pentagon.as_ref().map_or_else(
            || empty_quality_class(5),
            quality_class_from_polygon_quality,
        ),
        lbx: quality.hexagon.as_ref().map_or_else(
            || empty_quality_class(6),
            quality_class_from_polygon_quality,
        ),
        qbx: quality
            .heptagon
            .as_ref()
            .map(quality_class_from_polygon_quality),
    }
}

/// Compose the current `Grid_Quality_Check_Global` pure output with the
/// `quality_save_global` NetCDF side effect.
pub fn write_grid_quality_global_netcdf(
    output: impl AsRef<Path>,
    quality: &earthmesh_mesh::GridQualityGlobalOutput,
) -> io::Result<GlobalQualityWriteReport> {
    let mesh = global_quality_mesh_from_grid_quality(quality);
    write_quality_global_netcdf(output, &mesh)
}

fn quality_class_from_triangle_quality(
    output: &earthmesh_mesh::TriangleMeshQualityCanonicalOutput,
) -> QualityClassMetrics {
    QualityClassMetrics {
        length: output.length_cache.iter().map(|row| row.to_vec()).collect(),
        angle: output.angle_cache.iter().map(|row| row.to_vec()).collect(),
        extr: [
            output.extreme_angles_degrees.0,
            output.extreme_angles_degrees.1,
        ],
        eavg: [
            output.average_min_max_angles_degrees.0,
            output.average_min_max_angles_degrees.1,
        ],
        savg: output.angle_stddev_degrees,
        less: bool_flags_to_i32(&output.angle_less_flags),
        more: bool_flags_to_i32(&output.angle_more_flags),
    }
}

fn quality_class_from_polygon_quality(
    output: &earthmesh_mesh::PolygonMeshQualityCanonicalOutput,
) -> QualityClassMetrics {
    QualityClassMetrics {
        length: output.length_cache.clone(),
        angle: output.angle_cache.clone(),
        extr: [
            output.extreme_angles_degrees.0,
            output.extreme_angles_degrees.1,
        ],
        eavg: [
            output.average_min_max_angles_degrees.0,
            output.average_min_max_angles_degrees.1,
        ],
        savg: output.angle_stddev_degrees,
        less: bool_flags_to_i32(&output.angle_less_flags),
        more: bool_flags_to_i32(&output.angle_more_flags),
    }
}

fn empty_quality_class(_width: usize) -> QualityClassMetrics {
    QualityClassMetrics {
        length: Vec::new(),
        angle: Vec::new(),
        extr: [0.0, 0.0],
        eavg: [0.0, 0.0],
        savg: 0.0,
        less: Vec::new(),
        more: Vec::new(),
    }
}

fn bool_flags_to_i32(flags: &[bool]) -> Vec<i32> {
    flags.iter().map(|flag| i32::from(*flag)).collect()
}
