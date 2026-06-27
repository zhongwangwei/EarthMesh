use std::io;

use crate::*;

pub(super) fn run_final_regional_spring_from_unstructured_mesh(
    plan: &MkgrdFinalQualityCheckIoPlan,
    mesh: &UnstructuredMesh,
) -> io::Result<UnstructuredMesh> {
    let set_dis = plan.regional_set_dis.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check regional spring requires regional_set_dis",
        )
    })?;
    let regional_spring = plan.regional_spring.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check regional spring requires regional_spring controls",
        )
    })?;
    if set_dis < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check regional_set_dis must be non-negative",
        ));
    }

    let cells_on_triangle = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let source_mask = plan.regional_source_mask.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Final_Grid_Quality_Check regional spring requires source mask classification inputs",
        )
    })?;
    let refined_triangles = final_regional_refined_triangles_from_source_mask(mesh, source_mask)?;
    let move_mask = final_regional_move_mask_from_refined_triangles(
        mesh,
        &cells_on_triangle,
        &refined_triangles,
        set_dis as usize,
    )?;
    let report = run_springjustment_regional_from_unstructured_mesh(
        mesh,
        SpringjustmentRegionalRunOptions {
            move_mask: &move_mask,
            niter_refine: regional_spring.niter_refine,
            radius: regional_spring.radius,
            diagnostic_every: 100,
        },
    )?;
    Ok(report.mesh)
}

fn final_regional_refined_triangles_from_source_mask(
    mesh: &UnstructuredMesh,
    source_mask: &MkgrdFinalQualityRegionalSourceMaskIoPlan,
) -> io::Result<Vec<bool>> {
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    earthmesh_mesh::refine_sjx_regional_make_fortran_indexed(
        earthmesh_mesh::RefineRegionalMaskInput {
            triangle_lonlat: &triangle_lonlat,
            source_lon_vertices: &source_mask.source_lon_vertices,
            source_lat_vertices: &source_mask.source_lat_vertices,
            mask_patch: &source_mask.mask_patch,
            first_triangle_id: source_mask.first_triangle_id,
        },
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to classify final regional refined triangles from source mask",
        )
    })
}

fn final_regional_move_mask_from_refined_triangles(
    mesh: &UnstructuredMesh,
    cells_on_triangle: &[[usize; 3]],
    refined_triangles: &[bool],
    set_dis: usize,
) -> io::Result<Vec<bool>> {
    let triangles_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let mask = earthmesh_mesh::set_dbx_move_regional_step_fortran_indexed(
        earthmesh_mesh::RegionalMoveMaskInput {
            set_dis,
            refined_triangles,
            cells_on_triangle,
            triangles_on_cell: &triangles_on_cell,
            n_edges_on_cell: &n_edges_on_cell,
            protected_seed_cells: &[],
            vertex_protect_layers: 0,
        },
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to derive final regional Springjustment move mask",
        )
    })?;
    Ok(mask.move_mask)
}
