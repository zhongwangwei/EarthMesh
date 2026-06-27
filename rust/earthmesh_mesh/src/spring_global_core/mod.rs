use super::*;

/// Pure Rust adapter for the in-memory calculation sequence inside
/// `MOD_grid_preprocess:Springjustment_global`.
///
/// This deliberately excludes NetCDF/file side effects. It wires the migrated
/// kernels in the same order as the Fortran workflow: triangle neighbors,
/// edge/connectivity construction, edge-neighbor topology, global spring
/// dynamics, cell lon/lat refresh, triangle centroid/circumcenter refresh, and
/// final MPAS-style vertex-array ordering.
pub fn springjustment_global_core_fortran_indexed(
    input: SpringjustmentGlobalCoreInput<'_>,
) -> Option<SpringjustmentGlobalCoreOutput> {
    if input.triangle_lonlat.len() != input.cells_on_triangle.len()
        || input.triangles_on_cell.len() != input.n_edges_on_cell.len()
        || input.cell_lonlat.len() != input.n_edges_on_cell.len()
    {
        spring_global_debug("input dimension check failed");
        return None;
    }

    let triangle_neighbors = match triangle_neighbors_from_cell_membership_fortran_indexed(
        input.cells_on_triangle,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("triangle_neighbors_from_cell_membership failed");
            return None;
        }
    };
    let edge_output = match get_edge_production_fortran_indexed(
        &triangle_neighbors,
        input.cells_on_triangle,
        input.triangle_lonlat,
        input.cell_lonlat,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("get_edge_production failed");
            return None;
        }
    };
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
    let geometric_order = order_vertices_on_cell_fortran_indexed(
        &cell_points_for_order,
        &triangle_points_for_order,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    )
    .and_then(|ordered| {
        standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, input.n_edges_on_cell)
    });
    let topological_order = || {
        order_vertices_on_cell_by_shared_edges_fortran_indexed(
            input.triangles_on_cell,
            input.n_edges_on_cell,
            &edge_output.edges_on_vertex,
        )
        .and_then(|ordered| {
            standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, input.n_edges_on_cell)
        })
    };
    let cell_connectivity = match connect_on_cell_fortran_indexed(
        input.n_edges_on_cell,
        &edge_output.cells_on_edge,
        &edge_output.edges_on_vertex,
        input.triangles_on_cell,
    )
    .or_else(|| {
        geometric_order.as_ref().and_then(|ordered| {
            connect_on_cell_fortran_indexed(
                input.n_edges_on_cell,
                &edge_output.cells_on_edge,
                &edge_output.edges_on_vertex,
                ordered,
            )
        })
    })
    .or_else(|| {
        topological_order().and_then(|ordered| {
            connect_on_cell_fortran_indexed(
                input.n_edges_on_cell,
                &edge_output.cells_on_edge,
                &edge_output.edges_on_vertex,
                &ordered,
            )
        })
    }) {
        Some(value) => value,
        None => {
            spring_global_debug("connect_on_cell failed");
            return None;
        }
    };
    let edges_on_edge_tri = match edges_on_edge_tri_fortran_indexed(
        &edge_output.vertices_on_edge,
        &edge_output.edges_on_vertex,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("edges_on_edge_tri failed");
            return None;
        }
    };
    let distance_output =
        match set_dists_on_edge_global_fortran_indexed(SetDistsOnEdgeGlobalInput {
            base_dists_on_edge: input.base_dists_on_edge,
            base_cellwidth: input.base_cellwidth,
            num_rc: input.distance_num_rc,
            spacing: input.distance_spacing,
            triangles_on_cell: input.triangles_on_cell,
            cells_on_triangle: Some(input.cells_on_triangle),
            edges_on_vertex: &edge_output.edges_on_vertex,
            cells_on_edge: &edge_output.cells_on_edge,
            steps: input.distance_steps,
        }) {
            Some(value) => value,
            None => {
                spring_global_debug("set_dists_on_edge_global failed");
                return None;
            }
        };
    let dists_on_edge = distance_output.dists_on_edge;
    let cellwidth = distance_output.cellwidth;

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
    let spring_output = match spring_dynamics_global_fortran_indexed(
        &cell_points,
        input.n_edges_on_cell,
        &cell_connectivity.edges_on_cell,
        &edge_output.cells_on_edge,
        &edges_on_edge_tri,
        &dists_on_edge,
        input.niter_refine,
        input.relax,
        input.radius,
        input.diagnostic_every,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("spring_dynamics_global failed");
            return None;
        }
    };
    let updated_cell_lonlat = spring_output
        .updated_cell_points
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();
    let centroid_lonlat = match centroid_spherical_mesh_fortran_indexed(
        &updated_cell_lonlat,
        input.cells_on_triangle,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("centroid_spherical_mesh failed");
            return None;
        }
    };
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
    let circumcenters = match circumcenter_spherical_mesh_fortran_indexed(
        &centroid_cartesian,
        &spring_output.updated_cell_points,
        input.cells_on_triangle,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("circumcenter_spherical_mesh failed");
            return None;
        }
    };
    let updated_triangle_lonlat = circumcenters
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();
    let updated_triangle_points = updated_triangle_lonlat
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
    let edge_points_cartesian = edge_output
        .edge_points
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
    let final_ordered = match order_vertex_arrays_fortran_indexed(
        &updated_triangle_points,
        &edge_points_cartesian,
        &edge_output.edges_on_vertex,
        &edge_output.vertices_on_edge,
        &edge_output.cells_on_edge,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("order_vertex_arrays failed");
            return None;
        }
    };

    Some(SpringjustmentGlobalCoreOutput {
        updated_triangle_lonlat,
        updated_cell_lonlat,
        triangle_neighbors,
        cells_on_edge: edge_output.cells_on_edge,
        vertices_on_edge: edge_output.vertices_on_edge,
        edges_on_vertex: final_ordered.edges_on_vertex,
        cells_on_vertex: final_ordered.cells_on_vertex,
        edges_on_cell: cell_connectivity.edges_on_cell,
        cells_on_cell: cell_connectivity.cells_on_cell,
        edges_on_edge_tri,
        dists_on_edge,
        cellwidth,
        edge_lonlat: edge_output.edge_points,
        spring: spring_output,
    })
}
