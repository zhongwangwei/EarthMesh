use earthmesh_core::deg_to_rad;

use crate::coordinates::{cross, dot, magnitude, vector_between};
use crate::{arc_length_unit_sphere, CartesianPoint};

/// Port of `MOD_grid_preprocess:planeAngle`.
pub fn plane_angle_signed(
    point_a: CartesianPoint,
    point_b: CartesianPoint,
    point_c: CartesianPoint,
    normal: CartesianPoint,
) -> Option<f64> {
    let ab = vector_between(point_a, point_b);
    let ac = vector_between(point_a, point_c);
    let mab = magnitude(ab);
    let mac = magnitude(ac);
    if mab == 0.0 || mac == 0.0 {
        return None;
    }

    let cos_angle = (dot(ab, ac) / (mab * mac)).clamp(-1.0, 1.0);
    let signed = if dot(cross(ab, ac), normal) >= 0.0 {
        cos_angle.acos()
    } else {
        -cos_angle.acos()
    };
    Some(signed)
}

/// Output of `MOD_grid_preprocess:Get_Edge_DIS_Angle`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDistanceAngleOutput {
    pub dc_edge: Vec<f64>,
    pub dv_edge: Vec<f64>,
    pub angle_edge: Vec<f64>,
}

/// Port of `MOD_grid_preprocess:Get_Edge_DIS_Angle`.
pub fn edge_distance_angle_one_based(
    vertices: &[CartesianPoint],
    cells: &[CartesianPoint],
    edge_points: &[CartesianPoint],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
    lat_vertex_degrees: &[f64],
    lon_edge_degrees: &[f64],
    lat_edge_degrees: &[f64],
) -> Option<EdgeDistanceAngleOutput> {
    if cells_on_edge.len() != vertices_on_edge.len()
        || edge_points.len() < vertices_on_edge.len()
        || lon_edge_degrees.len() < vertices_on_edge.len()
        || lat_edge_degrees.len() < vertices_on_edge.len()
    {
        return None;
    }

    let mut dc_edge = vec![0.0; vertices_on_edge.len()];
    let mut dv_edge = vec![0.0; vertices_on_edge.len()];
    let mut angle_edge = vec![0.0; vertices_on_edge.len()];
    let pi = std::f64::consts::PI;

    for edge_id in 2..vertices_on_edge.len() {
        let vertex_ids = vertices_on_edge[edge_id];
        let cell_ids = cells_on_edge[edge_id];
        if vertex_ids[0] == 0 || vertex_ids[1] == 0 || cell_ids[0] == 0 {
            return None;
        }
        let vertex1 = *vertices.get(vertex_ids[0])?;
        let vertex2 = *vertices.get(vertex_ids[1])?;
        let cell1 = *cells.get(cell_ids[0])?;

        dv_edge[edge_id] = arc_length_unit_sphere(vertex1, vertex2);
        if dv_edge[edge_id] == 0.0 {
            return None;
        }
        dc_edge[edge_id] = if cell_ids[1] == 0 {
            // Match MpasMeshConverter.x for a non-periodic boundary edge,
            // where the exterior cell is represented by a midpoint ghost.
            3.0_f64.sqrt() * dv_edge[edge_id]
        } else {
            let cell2 = *cells.get(cell_ids[1])?;
            arc_length_unit_sphere(cell1, cell2)
        };

        let mut angle = (deg_to_rad(*lat_vertex_degrees.get(vertex_ids[1])?)
            - deg_to_rad(*lat_vertex_degrees.get(vertex_ids[0])?))
            / dv_edge[edge_id];
        angle = angle.clamp(-1.0, 1.0).acos();

        let edge_point = *edge_points.get(edge_id)?;
        let lon_north = deg_to_rad(lon_edge_degrees[edge_id]);
        let lat_north = deg_to_rad(lat_edge_degrees[edge_id] + 0.05);
        let north_point = CartesianPoint::new(
            lat_north.cos() * lon_north.cos(),
            lat_north.cos() * lon_north.sin(),
            lat_north.sin(),
        );
        let mut sign = plane_angle_signed(edge_point, north_point, vertex2, edge_point)?;
        if sign.abs() > 1.0e-14 {
            sign /= sign.abs();
        } else {
            sign = 1.0;
        }

        angle *= sign;
        if angle > pi {
            angle -= 2.0 * pi;
        }
        if angle < -pi {
            angle += 2.0 * pi;
        }
        angle_edge[edge_id] = angle;
    }

    Some(EdgeDistanceAngleOutput {
        dc_edge,
        dv_edge,
        angle_edge,
    })
}
