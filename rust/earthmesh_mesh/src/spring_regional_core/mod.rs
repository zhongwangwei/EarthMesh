use super::*;

/// Pure Rust adapter for the in-memory calculation sequence inside
/// `MOD_grid_preprocess:Springjustment_regional_step`.
pub fn springjustment_regional_core_one_based(
    input: SpringjustmentRegionalCoreInput<'_>,
) -> Option<SpringjustmentRegionalCoreOutput> {
    if input.triangle_lonlat.len() != input.cells_on_triangle.len()
        || input.triangles_on_cell.len() != input.n_edges_on_cell.len()
        || input.cell_lonlat.len() != input.n_edges_on_cell.len()
        || input.move_mask.len() != input.n_edges_on_cell.len()
    {
        return None;
    }

    let triangle_neighbors = triangle_neighbors_from_cell_membership_one_based(
        input.cells_on_triangle,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    )?;
    let edge_connectivity =
        get_edge_connectivity_one_based(&triangle_neighbors, input.cells_on_triangle)?;
    let vertices_on_edge = order_vertices_on_edge_one_based(
        input.triangle_lonlat,
        input.cell_lonlat,
        &edge_connectivity.cells_on_edge,
        &edge_connectivity.vertices_on_edge,
    )?;
    let triangle_points_for_order = input
        .triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let cell_points_for_order = input
        .cell_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let geometric_order = order_vertices_on_cell_one_based(
        &cell_points_for_order,
        &triangle_points_for_order,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    )
    .and_then(|ordered| {
        standardize_vertices_on_cell_rotation_one_based(&ordered, input.n_edges_on_cell)
    });
    let topological_order = || {
        order_vertices_on_cell_by_shared_edges_one_based(
            input.triangles_on_cell,
            input.n_edges_on_cell,
            &edge_connectivity.edges_on_vertex,
            &triangle_points_for_order,
            &cell_points_for_order,
        )
        .and_then(|ordered| {
            standardize_vertices_on_cell_rotation_one_based(&ordered, input.n_edges_on_cell)
        })
    };
    let cell_connectivity = connect_on_cell_one_based(
        input.n_edges_on_cell,
        &edge_connectivity.cells_on_edge,
        &edge_connectivity.edges_on_vertex,
        input.triangles_on_cell,
    )
    .or_else(|| {
        geometric_order.as_ref().and_then(|ordered| {
            connect_on_cell_one_based(
                input.n_edges_on_cell,
                &edge_connectivity.cells_on_edge,
                &edge_connectivity.edges_on_vertex,
                ordered,
            )
        })
    })
    .or_else(|| {
        topological_order().and_then(|ordered| {
            connect_on_cell_one_based(
                input.n_edges_on_cell,
                &edge_connectivity.cells_on_edge,
                &edge_connectivity.edges_on_vertex,
                &ordered,
            )
        })
    })?;

    let cell_points = input
        .cell_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let regional = spring_dynamics_regional_one_based(
        &cell_points,
        input.n_edges_on_cell,
        &cell_connectivity.cells_on_cell,
        input.move_mask,
        input.niter_refine,
        input.radius,
        input.diagnostic_every,
    )?;
    let updated_cell_lonlat = regional
        .updated_cell_points
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();
    let centroid_lonlat =
        centroid_spherical_mesh_one_based(&updated_cell_lonlat, input.cells_on_triangle)?;
    let centroid_cartesian = centroid_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let circumcenters = circumcenter_spherical_mesh_one_based(
        &centroid_cartesian,
        &regional.updated_cell_points,
        input.cells_on_triangle,
    )?;
    let updated_triangle_lonlat = circumcenters
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();

    Some(SpringjustmentRegionalCoreOutput {
        triangle_neighbors,
        cells_on_edge: edge_connectivity.cells_on_edge,
        vertices_on_edge,
        edges_on_vertex: edge_connectivity.edges_on_vertex,
        edges_on_cell: cell_connectivity.edges_on_cell,
        cells_on_cell: cell_connectivity.cells_on_cell,
        updated_cell_lonlat,
        updated_triangle_lonlat,
        regional,
    })
}
