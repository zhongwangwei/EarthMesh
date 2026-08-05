use crate::{netcdf_to_io_error, validate_mpas_mesh, MpasMesh};
use earthmesh_mesh::{arc_length_unit_sphere, CartesianPoint};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::path::{Path, PathBuf};

pub const ICON_SPHERE_RADIUS_METERS: f64 = 6_371_229.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconGridWriteReport {
    pub output: PathBuf,
    pub cells: usize,
    pub vertices: usize,
    pub edges: usize,
    pub global_grid: bool,
}

#[derive(Clone)]
struct IconGrid {
    clon: Vec<f64>,
    clat: Vec<f64>,
    vlon: Vec<f64>,
    vlat: Vec<f64>,
    vertex_xyz: Vec<CartesianPoint>,
    elon: Vec<f64>,
    elat: Vec<f64>,
    cell_area: Vec<f64>,
    dual_area: Vec<f64>,
    edge_of_cell: Vec<Vec<i32>>,
    vertex_of_cell: Vec<Vec<i32>>,
    adjacent_cell_of_edge: Vec<Vec<i32>>,
    edge_vertices: Vec<Vec<i32>>,
    cells_of_vertex: Vec<Vec<i32>>,
    edges_of_vertex: Vec<Vec<i32>>,
    vertices_of_vertex: Vec<Vec<i32>>,
    edge_length: Vec<f64>,
    edge_cell_distance: Vec<Vec<f64>>,
    dual_edge_length: Vec<f64>,
    edge_vert_distance: Vec<Vec<f64>>,
    zonal_normal_primal_edge: Vec<f64>,
    meridional_normal_primal_edge: Vec<f64>,
    zonal_normal_dual_edge: Vec<f64>,
    meridional_normal_dual_edge: Vec<f64>,
    orientation_of_normal: Vec<Vec<i32>>,
    neighbor_cell_index: Vec<Vec<i32>>,
    edge_orientation: Vec<Vec<i32>>,
    cell_ctrl: Vec<i32>,
    edge_ctrl: Vec<i32>,
    vertex_ctrl: Vec<i32>,
}

pub fn write_icon_grid_netcdf(
    output: impl AsRef<Path>,
    mesh: &MpasMesh,
) -> io::Result<IconGridWriteReport> {
    validate_mpas_mesh(mesh)?;
    let output = output.as_ref();
    crate::ensure_parent_dir(output)?;
    let grid = build_icon_grid(mesh)?;
    let global_grid = grid.adjacent_cell_of_edge.iter().all(|row| row[1] > 0);

    let mut file = crate::create_netcdf(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("cell", grid.clon.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("vertex", grid.vlon.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("edge", grid.elon.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nc", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("nv", 3).map_err(netcdf_to_io_error)?;
    file.add_dimension("ne", 6).map_err(netcdf_to_io_error)?;
    file.add_dimension("no", 4).map_err(netcdf_to_io_error)?;
    file.add_dimension("max_chdom", 1)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("cell_grf", 14)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("edge_grf", 24)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("vert_grf", 13)
        .map_err(netcdf_to_io_error)?;

    write_coord(&mut file, "clon", "cell", &grid.clon, "clon_vertices")?;
    write_coord(&mut file, "clat", "cell", &grid.clat, "clat_vertices")?;
    write_coord(&mut file, "vlon", "vertex", &grid.vlon, "vlon_vertices")?;
    write_coord(&mut file, "vlat", "vertex", &grid.vlat, "vlat_vertices")?;
    write_f64_1d(
        &mut file,
        "cartesian_x_vertices",
        "vertex",
        &grid.vertex_xyz.iter().map(|p| p.x).collect::<Vec<_>>(),
    )?;
    write_f64_1d(
        &mut file,
        "cartesian_y_vertices",
        "vertex",
        &grid.vertex_xyz.iter().map(|p| p.y).collect::<Vec<_>>(),
    )?;
    write_f64_1d(
        &mut file,
        "cartesian_z_vertices",
        "vertex",
        &grid.vertex_xyz.iter().map(|p| p.z).collect::<Vec<_>>(),
    )?;
    write_coord(&mut file, "elon", "edge", &grid.elon, "elon_vertices")?;
    write_coord(&mut file, "elat", "edge", &grid.elat, "elat_vertices")?;

    write_f64_1d(&mut file, "cell_area", "cell", &grid.cell_area)?;
    write_f64_1d(&mut file, "dual_area", "vertex", &grid.dual_area)?;
    write_f64_1d(&mut file, "lon_cell_centre", "cell", &grid.clon)?;
    write_f64_1d(&mut file, "lat_cell_centre", "cell", &grid.clat)?;
    write_f64_1d(&mut file, "longitude_vertices", "vertex", &grid.vlon)?;
    write_f64_1d(&mut file, "latitude_vertices", "vertex", &grid.vlat)?;
    write_f64_1d(&mut file, "lon_edge_centre", "edge", &grid.elon)?;
    write_f64_1d(&mut file, "lat_edge_centre", "edge", &grid.elat)?;

    write_i32_columns(
        &mut file,
        "edge_of_cell",
        &["nv", "cell"],
        &grid.edge_of_cell,
    )?;
    write_i32_columns(
        &mut file,
        "vertex_of_cell",
        &["nv", "cell"],
        &grid.vertex_of_cell,
    )?;
    write_i32_columns(
        &mut file,
        "adjacent_cell_of_edge",
        &["nc", "edge"],
        &grid.adjacent_cell_of_edge,
    )?;
    write_i32_columns(
        &mut file,
        "edge_vertices",
        &["nc", "edge"],
        &grid.edge_vertices,
    )?;
    write_i32_columns(
        &mut file,
        "cells_of_vertex",
        &["ne", "vertex"],
        &grid.cells_of_vertex,
    )?;
    write_i32_columns(
        &mut file,
        "edges_of_vertex",
        &["ne", "vertex"],
        &grid.edges_of_vertex,
    )?;
    write_i32_columns(
        &mut file,
        "vertices_of_vertex",
        &["ne", "vertex"],
        &grid.vertices_of_vertex,
    )?;
    write_f64_1d(&mut file, "cell_area_p", "cell", &grid.cell_area)?;
    write_f64_1d(&mut file, "dual_area_p", "vertex", &grid.dual_area)?;
    write_f64_1d(&mut file, "edge_length", "edge", &grid.edge_length)?;
    write_f64_columns(
        &mut file,
        "edge_cell_distance",
        &["nc", "edge"],
        &grid.edge_cell_distance,
    )?;
    write_f64_1d(
        &mut file,
        "dual_edge_length",
        "edge",
        &grid.dual_edge_length,
    )?;
    write_f64_columns(
        &mut file,
        "edge_vert_distance",
        &["nc", "edge"],
        &grid.edge_vert_distance,
    )?;
    write_f64_1d(
        &mut file,
        "zonal_normal_primal_edge",
        "edge",
        &grid.zonal_normal_primal_edge,
    )?;
    write_f64_1d(
        &mut file,
        "meridional_normal_primal_edge",
        "edge",
        &grid.meridional_normal_primal_edge,
    )?;
    write_f64_1d(
        &mut file,
        "zonal_normal_dual_edge",
        "edge",
        &grid.zonal_normal_dual_edge,
    )?;
    write_f64_1d(
        &mut file,
        "meridional_normal_dual_edge",
        "edge",
        &grid.meridional_normal_dual_edge,
    )?;
    write_i32_columns(
        &mut file,
        "orientation_of_normal",
        &["nv", "cell"],
        &grid.orientation_of_normal,
    )?;

    let clon_vertices = bounds_from_ids(&grid.vertex_of_cell, &grid.vlon, f64::NAN);
    let clat_vertices = bounds_from_ids(&grid.vertex_of_cell, &grid.vlat, f64::NAN);
    write_f64_rows(&mut file, "clon_vertices", &["cell", "nv"], &clon_vertices)?;
    write_f64_rows(&mut file, "clat_vertices", &["cell", "nv"], &clat_vertices)?;
    let elon_vertices = edge_bounds(
        &grid.edge_vertices,
        &grid.adjacent_cell_of_edge,
        &grid.vlon,
        &grid.clon,
        &grid.elon,
    );
    let elat_vertices = edge_bounds(
        &grid.edge_vertices,
        &grid.adjacent_cell_of_edge,
        &grid.vlat,
        &grid.clat,
        &grid.elat,
    );
    write_f64_rows(&mut file, "elon_vertices", &["edge", "no"], &elon_vertices)?;
    write_f64_rows(&mut file, "elat_vertices", &["edge", "no"], &elat_vertices)?;
    let vlon_vertices = padded_bounds_from_ids(&grid.cells_of_vertex, &grid.clon, &grid.vlon);
    let vlat_vertices = padded_bounds_from_ids(&grid.cells_of_vertex, &grid.clat, &grid.vlat);
    write_f64_rows(
        &mut file,
        "vlon_vertices",
        &["vertex", "ne"],
        &vlon_vertices,
    )?;
    write_f64_rows(
        &mut file,
        "vlat_vertices",
        &["vertex", "ne"],
        &vlat_vertices,
    )?;
    write_f64_1d(
        &mut file,
        "quadrilateral_area",
        "edge",
        &vec![0.0; grid.elon.len()],
    )?;

    write_i32_1d(
        &mut file,
        "parent_cell_index",
        "cell",
        &vec![-1; grid.clon.len()],
    )?;
    write_i32_columns(
        &mut file,
        "neighbor_cell_index",
        &["nv", "cell"],
        &grid.neighbor_cell_index,
    )?;
    write_i32_columns(
        &mut file,
        "edge_orientation",
        &["ne", "vertex"],
        &grid.edge_orientation,
    )?;
    write_i32_1d(
        &mut file,
        "edge_system_orientation",
        "edge",
        &vec![1; grid.elon.len()],
    )?;
    write_i32_1d(&mut file, "refin_c_ctrl", "cell", &grid.cell_ctrl)?;
    write_grf_indices(&mut file, "c", grid.clon.len(), &grid.cell_ctrl, 14, 8, 5)?;
    write_i32_1d(&mut file, "refin_e_ctrl", "edge", &grid.edge_ctrl)?;
    write_grf_indices(&mut file, "e", grid.elon.len(), &grid.edge_ctrl, 24, 13, 10)?;
    write_i32_1d(&mut file, "refin_v_ctrl", "vertex", &grid.vertex_ctrl)?;
    write_grf_indices(&mut file, "v", grid.vlon.len(), &grid.vertex_ctrl, 13, 7, 5)?;
    write_i32_1d(
        &mut file,
        "parent_edge_index",
        "edge",
        &vec![-1; grid.elon.len()],
    )?;
    write_i32_1d(
        &mut file,
        "parent_vertex_index",
        "vertex",
        &vec![-1; grid.vlon.len()],
    )?;

    file.add_attribute("title", "ICON grid description")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("institution", "EarthMesh")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("source", "Generated by EarthMesh")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("number_of_grid_used", 0_i32)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("ICON_grid_file_uri", "")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("centre", 255_i32)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("subcentre", 255_i32)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("grid_mapping_name", "lat_long_on_sphere")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("crs_id", "urn:ogc:def:cs:EPSG:6.0:6422")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("crs_name", "Spherical 2D Coordinate System")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("ellipsoid_name", "Sphere")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("semi_major_axis", ICON_SPHERE_RADIUS_METERS)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("inverse_flattening", 0.0_f64)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("grid_level", 0_i32)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("grid_root", 0_i32)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("uuidOfParHGrid", "")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("uuidOfHGrid", "")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("global_grid", i32::from(global_grid))
        .map_err(netcdf_to_io_error)?;

    Ok(IconGridWriteReport {
        output: output.to_path_buf(),
        cells: grid.clon.len(),
        vertices: grid.vlon.len(),
        edges: grid.elon.len(),
        global_grid,
    })
}

fn build_icon_grid(mesh: &MpasMesh) -> io::Result<IconGrid> {
    let cells_on_vertex = derive_cells_on_vertex(mesh)?;
    let valid_cells = (1..cells_on_vertex.len())
        .filter(|&id| cells_on_vertex[id].len() == 3)
        .collect::<Vec<_>>();
    if valid_cells.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ICON export requires at least one complete triangular cell",
        ));
    }
    let used_vertices = valid_cells
        .iter()
        .flat_map(|&id| cells_on_vertex[id].iter().copied())
        .collect::<BTreeSet<_>>();
    let mut cell_map = vec![-1; mesh.cells_on_vertex.len()];
    for (new, old) in valid_cells.iter().copied().enumerate() {
        cell_map[old] = i32::try_from(new + 1).map_err(index_error)?;
    }
    let vertex_old = used_vertices
        .iter()
        .map(|value| usize::try_from(*value).map_err(index_error))
        .collect::<io::Result<Vec<_>>>()?;
    let mut vertex_map = vec![-1; mesh.lon_cell.len()];
    for (new, old) in vertex_old.iter().copied().enumerate() {
        vertex_map[old] = i32::try_from(new + 1).map_err(index_error)?;
    }

    let mut vertex_of_cell = Vec::with_capacity(valid_cells.len());
    let mut edge_of_cell = vec![vec![-1; 3]; valid_cells.len()];
    let mut edges = Vec::<([i32; 2], [i32; 2])>::new();
    let mut edge_ids = BTreeMap::<(i32, i32), i32>::new();
    for (cell_index, old_cell) in valid_cells.iter().copied().enumerate() {
        let mut row = cells_on_vertex[old_cell]
            .iter()
            .map(|value| vertex_map[*value as usize])
            .collect::<Vec<_>>();
        orient_triangle_outward(&mut row, &vertex_old, mesh, old_cell);
        vertex_of_cell.push(row.clone());
        let cell_id = i32::try_from(cell_index + 1).map_err(index_error)?;
        for slot in 0..3 {
            let a = row[slot];
            let b = row[(slot + 1) % 3];
            let key = if a < b { (a, b) } else { (b, a) };
            let edge_id = if let Some(id) = edge_ids.get(&key).copied() {
                let edge = &mut edges[id as usize - 1];
                if edge.1[1] > 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("ICON edge {a}-{b} belongs to more than two cells"),
                    ));
                }
                edge.1[1] = cell_id;
                id
            } else {
                let id = i32::try_from(edges.len() + 1).map_err(index_error)?;
                edge_ids.insert(key, id);
                edges.push(([a, b], [cell_id, -1]));
                id
            };
            edge_of_cell[cell_index][slot] = edge_id;
        }
    }

    let (cells_of_vertex, edges_of_vertex, vertices_of_vertex, edge_orientation) =
        build_icon_vertex_fans(vertex_old.len(), &vertex_of_cell, &edge_of_cell, &edges)?;

    let adjacent_cell_of_edge = edges.iter().map(|edge| edge.1.to_vec()).collect::<Vec<_>>();
    let edge_vertices = edges.iter().map(|edge| edge.0.to_vec()).collect::<Vec<_>>();
    let mut neighbor_cell_index = vec![vec![-1; 3]; valid_cells.len()];
    let mut orientation_of_normal = vec![vec![0; 3]; valid_cells.len()];
    for cell in 0..valid_cells.len() {
        let cell_id = (cell + 1) as i32;
        for slot in 0..3 {
            let edge = edge_of_cell[cell][slot] as usize - 1;
            let adjacent = adjacent_cell_of_edge[edge].as_slice();
            if adjacent[0] == cell_id {
                neighbor_cell_index[cell][slot] = adjacent[1];
                orientation_of_normal[cell][slot] = 1;
            } else {
                neighbor_cell_index[cell][slot] = adjacent[0];
                orientation_of_normal[cell][slot] = -1;
            }
        }
    }

    let clon = valid_cells
        .iter()
        .map(|&id| mesh.lon_vertex[id])
        .collect::<Vec<_>>();
    let clat = valid_cells
        .iter()
        .map(|&id| mesh.lat_vertex[id])
        .collect::<Vec<_>>();
    let vlon = vertex_old
        .iter()
        .map(|&id| mesh.lon_cell[id])
        .collect::<Vec<_>>();
    let vlat = vertex_old
        .iter()
        .map(|&id| mesh.lat_cell[id])
        .collect::<Vec<_>>();
    let vertex_xyz = vertex_old
        .iter()
        .map(|&id| CartesianPoint::new(mesh.x_cell[id], mesh.y_cell[id], mesh.z_cell[id]))
        .collect::<Vec<_>>();
    let cell_xyz = valid_cells
        .iter()
        .map(|&id| CartesianPoint::new(mesh.x_vertex[id], mesh.y_vertex[id], mesh.z_vertex[id]))
        .collect::<Vec<_>>();
    let cell_area = valid_cells
        .iter()
        .map(|&id| mesh.area_triangle[id] * ICON_SPHERE_RADIUS_METERS.powi(2))
        .collect::<Vec<_>>();
    let dual_area = vertex_old
        .iter()
        .map(|&id| mesh.area_cell[id] * ICON_SPHERE_RADIUS_METERS.powi(2))
        .collect::<Vec<_>>();

    let mut elon = Vec::with_capacity(edges.len());
    let mut elat = Vec::with_capacity(edges.len());
    let mut edge_length = Vec::with_capacity(edges.len());
    let mut edge_cell_distance = Vec::with_capacity(edges.len());
    let mut dual_edge_length = Vec::with_capacity(edges.len());
    let mut edge_vert_distance = Vec::with_capacity(edges.len());
    let mut zonal_normal_primal_edge = Vec::with_capacity(edges.len());
    let mut meridional_normal_primal_edge = Vec::with_capacity(edges.len());
    let mut zonal_normal_dual_edge = Vec::with_capacity(edges.len());
    let mut meridional_normal_dual_edge = Vec::with_capacity(edges.len());
    for (vertices, cells) in &edges {
        let a = vertex_xyz[vertices[0] as usize - 1];
        let b = vertex_xyz[vertices[1] as usize - 1];
        let midpoint = normalized(CartesianPoint::new(a.x + b.x, a.y + b.y, a.z + b.z))?;
        let lon = midpoint.y.atan2(midpoint.x);
        let lat = midpoint.z.asin();
        elon.push(lon);
        elat.push(lat);
        edge_length.push(arc_length_unit_sphere(a, b) * ICON_SPHERE_RADIUS_METERS);
        edge_vert_distance.push(vec![
            arc_length_unit_sphere(midpoint, a) * ICON_SPHERE_RADIUS_METERS,
            arc_length_unit_sphere(midpoint, b) * ICON_SPHERE_RADIUS_METERS,
        ]);
        let c1 = cell_xyz[cells[0] as usize - 1];
        let c2 = if cells[1] > 0 {
            cell_xyz[cells[1] as usize - 1]
        } else {
            midpoint
        };
        let d1 = arc_length_unit_sphere(c1, midpoint) * ICON_SPHERE_RADIUS_METERS;
        let d2 = if cells[1] > 0 {
            arc_length_unit_sphere(midpoint, c2) * ICON_SPHERE_RADIUS_METERS
        } else {
            0.0
        };
        edge_cell_distance.push(vec![d1, d2]);
        dual_edge_length.push(d1 + d2);
        let (east_primal, north_primal) = tangent_components(midpoint, c1, c2)?;
        let (east_dual, north_dual) = tangent_components(midpoint, a, b)?;
        zonal_normal_primal_edge.push(east_primal);
        meridional_normal_primal_edge.push(north_primal);
        zonal_normal_dual_edge.push(east_dual);
        meridional_normal_dual_edge.push(north_dual);
    }

    let cell_ctrl = boundary_layers(&neighbor_cell_index, 5);
    let vertex_ctrl = boundary_layers_with_sources(
        &vertices_of_vertex,
        edges_of_vertex.iter().map(|row| {
            row.iter()
                .any(|edge| *edge > 0 && adjacent_cell_of_edge[*edge as usize - 1][1] < 1)
        }),
        5,
    );
    let edge_adjacency = edge_adjacency(&edge_of_cell, &edges_of_vertex, edges.len());
    let edge_ctrl = boundary_layers_with_sources(
        &edge_adjacency,
        adjacent_cell_of_edge.iter().map(|row| row[1] < 1),
        10,
    );

    let grid = IconGrid {
        clon,
        clat,
        vlon,
        vlat,
        vertex_xyz,
        elon,
        elat,
        cell_area,
        dual_area,
        edge_of_cell,
        vertex_of_cell,
        adjacent_cell_of_edge,
        edge_vertices,
        cells_of_vertex,
        edges_of_vertex,
        vertices_of_vertex,
        edge_length,
        edge_cell_distance,
        dual_edge_length,
        edge_vert_distance,
        zonal_normal_primal_edge,
        meridional_normal_primal_edge,
        zonal_normal_dual_edge,
        meridional_normal_dual_edge,
        orientation_of_normal,
        neighbor_cell_index,
        edge_orientation,
        cell_ctrl,
        edge_ctrl,
        vertex_ctrl,
    };
    reorder_icon_grid(grid)
}

type IconVertexFans = (Vec<Vec<i32>>, Vec<Vec<i32>>, Vec<Vec<i32>>, Vec<Vec<i32>>);

fn build_icon_vertex_fans(
    vertex_count: usize,
    vertex_of_cell: &[Vec<i32>],
    edge_of_cell: &[Vec<i32>],
    edges: &[([i32; 2], [i32; 2])],
) -> io::Result<IconVertexFans> {
    let mut links = vec![Vec::<(i32, i32, i32)>::new(); vertex_count];
    for (cell, vertices) in vertex_of_cell.iter().enumerate() {
        if vertices.len() != 3 || edge_of_cell[cell].len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ICON triangular connectivity must have width three",
            ));
        }
        let cell_id = i32::try_from(cell + 1).map_err(index_error)?;
        for slot in 0..3 {
            let vertex = usize::try_from(vertices[slot]).map_err(index_error)?;
            if vertex == 0 || vertex > vertex_count {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("ICON vertex id {vertex} is out of range"),
                ));
            }
            links[vertex - 1].push((
                edge_of_cell[cell][(slot + 2) % 3],
                edge_of_cell[cell][slot],
                cell_id,
            ));
        }
    }

    let mut cells_of_vertex = Vec::with_capacity(vertex_count);
    let mut edges_of_vertex = Vec::with_capacity(vertex_count);
    let mut vertices_of_vertex = Vec::with_capacity(vertex_count);
    let mut edge_orientation = Vec::with_capacity(vertex_count);
    for (vertex, vertex_links) in links.into_iter().enumerate() {
        let vertex_id = i32::try_from(vertex + 1).map_err(index_error)?;
        let link_count = vertex_links.len();
        let mut adjacent = BTreeMap::<i32, Vec<(i32, i32)>>::new();
        for (first, second, cell) in vertex_links {
            if first <= 0 || second <= 0 || first == second {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("ICON vertex {vertex_id} has a branched edge fan"),
                ));
            }
            adjacent.entry(first).or_default().push((second, cell));
            adjacent.entry(second).or_default().push((first, cell));
        }
        for neighbors in adjacent.values_mut() {
            neighbors.sort_unstable_by_key(|(other, cell)| (*cell, *other));
            if neighbors.len() > 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("ICON vertex {vertex_id} has a branched edge fan"),
                ));
            }
        }
        let endpoints = adjacent
            .iter()
            .filter(|(_, neighbors)| neighbors.len() == 1)
            .map(|(edge, _)| *edge)
            .collect::<Vec<_>>();
        if !matches!(endpoints.len(), 0 | 2) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("ICON vertex {vertex_id} has a disconnected edge fan"),
            ));
        }
        let Some(start) = endpoints
            .first()
            .copied()
            .or_else(|| adjacent.keys().next().copied())
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("ICON vertex {vertex_id} has no incident cell"),
            ));
        };

        let mut cell_row = Vec::new();
        let mut edge_row = Vec::new();
        let mut neighbor_row = Vec::new();
        let mut orientation_row = Vec::new();
        let mut current = start;
        let mut visited_edges = BTreeSet::new();
        let mut visited_cells = BTreeSet::new();
        loop {
            if !visited_edges.insert(current) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("ICON vertex {vertex_id} edge fan contains a loop before closure"),
                ));
            }
            let edge_index = usize::try_from(current).map_err(index_error)?;
            let endpoints = edges
                .get(edge_index.saturating_sub(1))
                .map(|edge| edge.0)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("ICON edge {current} is out of range"),
                    )
                })?;
            let neighbor = if endpoints[0] == vertex_id {
                endpoints[1]
            } else if endpoints[1] == vertex_id {
                endpoints[0]
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("ICON edge {current} is not incident to vertex {vertex_id}"),
                ));
            };
            edge_row.push(current);
            neighbor_row.push(neighbor);
            orientation_row.push(if endpoints[0] == vertex_id { 1 } else { -1 });
            let choices = adjacent
                .get(&current)
                .into_iter()
                .flatten()
                .filter(|(_, cell)| !visited_cells.contains(cell))
                .copied()
                .collect::<Vec<_>>();
            if choices.len() > 1 && !visited_cells.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("ICON vertex {vertex_id} has a branched edge fan"),
                ));
            }
            let Some((next, cell)) = choices.first().copied() else {
                cell_row.push(-1);
                break;
            };
            visited_cells.insert(cell);
            cell_row.push(cell);
            current = next;
            if current == start {
                break;
            }
        }
        if visited_cells.len() != link_count || visited_edges.len() != adjacent.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("ICON vertex {vertex_id} edge fan is disconnected"),
            ));
        }
        if edge_row.len() > 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "ICON ne=6 cannot represent EarthMesh vertex {vertex_id} with degree {}",
                    edge_row.len()
                ),
            ));
        }
        cell_row.resize(6, -1);
        edge_row.resize(6, -1);
        neighbor_row.resize(6, -1);
        orientation_row.resize(6, 0);
        cells_of_vertex.push(cell_row);
        edges_of_vertex.push(edge_row);
        vertices_of_vertex.push(neighbor_row);
        edge_orientation.push(orientation_row);
    }
    Ok((
        cells_of_vertex,
        edges_of_vertex,
        vertices_of_vertex,
        edge_orientation,
    ))
}

fn derive_cells_on_vertex(mesh: &MpasMesh) -> io::Result<Vec<Vec<i32>>> {
    let mut result = vec![BTreeSet::new(); mesh.lon_vertex.len()];
    for cell in 1..mesh.vertices_on_cell.len() {
        let count = usize::try_from(mesh.n_edges_on_cell[cell]).map_err(index_error)?;
        for &vertex in mesh.vertices_on_cell[cell].iter().take(count) {
            if vertex > 0 {
                let vertex = usize::try_from(vertex).map_err(index_error)?;
                if vertex >= result.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("ICON vertex id {vertex} is out of range"),
                    ));
                }
                result[vertex].insert(i32::try_from(cell).map_err(index_error)?);
            }
        }
    }
    Ok(result
        .into_iter()
        .map(|cells| cells.into_iter().collect())
        .collect())
}

fn orient_triangle_outward(row: &mut [i32], vertex_old: &[usize], mesh: &MpasMesh, cell: usize) {
    let point = |id: i32| {
        let old = vertex_old[id as usize - 1];
        CartesianPoint::new(mesh.x_cell[old], mesh.y_cell[old], mesh.z_cell[old])
    };
    let a = point(row[0]);
    let b = point(row[1]);
    let c = point(row[2]);
    let centre = CartesianPoint::new(
        mesh.x_vertex[cell],
        mesh.y_vertex[cell],
        mesh.z_vertex[cell],
    );
    let ab = CartesianPoint::new(b.x - a.x, b.y - a.y, b.z - a.z);
    let ac = CartesianPoint::new(c.x - a.x, c.y - a.y, c.z - a.z);
    let cross = CartesianPoint::new(
        ab.y * ac.z - ab.z * ac.y,
        ab.z * ac.x - ab.x * ac.z,
        ab.x * ac.y - ab.y * ac.x,
    );
    if cross.x * centre.x + cross.y * centre.y + cross.z * centre.z < 0.0 {
        row.swap(1, 2);
    }
}

fn reorder_icon_grid(mut grid: IconGrid) -> io::Result<IconGrid> {
    let cell_perm = permutation(&grid.cell_ctrl);
    let edge_perm = permutation(&grid.edge_ctrl);
    let vertex_perm = permutation(&grid.vertex_ctrl);
    let cell_map = inverse_map(&cell_perm)?;
    let edge_map = inverse_map(&edge_perm)?;
    let vertex_map = inverse_map(&vertex_perm)?;

    grid.clon = permute(&grid.clon, &cell_perm);
    grid.clat = permute(&grid.clat, &cell_perm);
    grid.cell_area = permute(&grid.cell_area, &cell_perm);
    grid.cell_ctrl = permute(&grid.cell_ctrl, &cell_perm);
    grid.edge_of_cell = remap_rows(&permute(&grid.edge_of_cell, &cell_perm), &edge_map);
    grid.vertex_of_cell = remap_rows(&permute(&grid.vertex_of_cell, &cell_perm), &vertex_map);
    grid.neighbor_cell_index =
        remap_rows(&permute(&grid.neighbor_cell_index, &cell_perm), &cell_map);

    grid.vlon = permute(&grid.vlon, &vertex_perm);
    grid.vlat = permute(&grid.vlat, &vertex_perm);
    grid.vertex_xyz = permute(&grid.vertex_xyz, &vertex_perm);
    grid.dual_area = permute(&grid.dual_area, &vertex_perm);
    grid.vertex_ctrl = permute(&grid.vertex_ctrl, &vertex_perm);
    grid.cells_of_vertex = remap_rows(&permute(&grid.cells_of_vertex, &vertex_perm), &cell_map);
    grid.edges_of_vertex = remap_rows(&permute(&grid.edges_of_vertex, &vertex_perm), &edge_map);
    grid.vertices_of_vertex = remap_rows(
        &permute(&grid.vertices_of_vertex, &vertex_perm),
        &vertex_map,
    );

    grid.elon = permute(&grid.elon, &edge_perm);
    grid.elat = permute(&grid.elat, &edge_perm);
    grid.edge_length = permute(&grid.edge_length, &edge_perm);
    grid.edge_cell_distance = permute(&grid.edge_cell_distance, &edge_perm);
    grid.dual_edge_length = permute(&grid.dual_edge_length, &edge_perm);
    grid.edge_vert_distance = permute(&grid.edge_vert_distance, &edge_perm);
    grid.zonal_normal_primal_edge = permute(&grid.zonal_normal_primal_edge, &edge_perm);
    grid.meridional_normal_primal_edge = permute(&grid.meridional_normal_primal_edge, &edge_perm);
    grid.zonal_normal_dual_edge = permute(&grid.zonal_normal_dual_edge, &edge_perm);
    grid.meridional_normal_dual_edge = permute(&grid.meridional_normal_dual_edge, &edge_perm);
    grid.edge_ctrl = permute(&grid.edge_ctrl, &edge_perm);
    grid.adjacent_cell_of_edge =
        remap_rows(&permute(&grid.adjacent_cell_of_edge, &edge_perm), &cell_map);
    grid.edge_vertices = remap_rows(&permute(&grid.edge_vertices, &edge_perm), &vertex_map);

    grid.orientation_of_normal = grid
        .edge_of_cell
        .iter()
        .enumerate()
        .map(|(cell, edges)| {
            edges
                .iter()
                .map(|edge| {
                    let adjacent = &grid.adjacent_cell_of_edge[*edge as usize - 1];
                    if adjacent[0] == cell as i32 + 1 {
                        1
                    } else {
                        -1
                    }
                })
                .collect()
        })
        .collect();
    grid.edge_orientation = grid
        .edges_of_vertex
        .iter()
        .enumerate()
        .map(|(vertex, edges)| {
            edges
                .iter()
                .map(|edge| {
                    if *edge < 1 {
                        0
                    } else if grid.edge_vertices[*edge as usize - 1][0] == vertex as i32 + 1 {
                        1
                    } else {
                        -1
                    }
                })
                .collect()
        })
        .collect();
    Ok(grid)
}

fn boundary_layers(adjacency: &[Vec<i32>], max_layer: i32) -> Vec<i32> {
    boundary_layers_with_sources(
        adjacency,
        adjacency.iter().map(|row| row.iter().any(|id| *id < 1)),
        max_layer,
    )
}

fn boundary_layers_with_sources(
    adjacency: &[Vec<i32>],
    sources: impl Iterator<Item = bool>,
    max_layer: i32,
) -> Vec<i32> {
    let mut layers = vec![0; adjacency.len()];
    let mut queue = VecDeque::new();
    for (index, boundary) in sources.enumerate() {
        if boundary {
            layers[index] = 1;
            queue.push_back(index);
        }
    }
    while let Some(index) = queue.pop_front() {
        let next = layers[index] + 1;
        if next > max_layer {
            continue;
        }
        for neighbor in &adjacency[index] {
            if *neighbor > 0 && layers[*neighbor as usize - 1] == 0 {
                layers[*neighbor as usize - 1] = next;
                queue.push_back(*neighbor as usize - 1);
            }
        }
    }
    layers
}

fn edge_adjacency(
    edges_of_cell: &[Vec<i32>],
    edges_of_vertex: &[Vec<i32>],
    edge_count: usize,
) -> Vec<Vec<i32>> {
    let mut adjacency = vec![BTreeSet::new(); edge_count];
    for row in edges_of_cell.iter().chain(edges_of_vertex) {
        let ids = row.iter().copied().filter(|id| *id > 0).collect::<Vec<_>>();
        for &a in &ids {
            for &b in &ids {
                if a != b {
                    adjacency[a as usize - 1].insert(b);
                }
            }
        }
    }
    adjacency
        .into_iter()
        .map(|neighbors| neighbors.into_iter().collect())
        .collect()
}

fn permutation(groups: &[i32]) -> Vec<usize> {
    let mut ids = (0..groups.len()).collect::<Vec<_>>();
    ids.sort_by_key(|index| {
        (
            if groups[*index] == 0 {
                i32::MAX
            } else {
                groups[*index]
            },
            *index,
        )
    });
    ids
}

fn inverse_map(permutation: &[usize]) -> io::Result<Vec<i32>> {
    let mut map = vec![-1; permutation.len() + 1];
    for (new, old) in permutation.iter().copied().enumerate() {
        map[old + 1] = i32::try_from(new + 1).map_err(index_error)?;
    }
    Ok(map)
}

fn permute<T: Clone>(values: &[T], permutation: &[usize]) -> Vec<T> {
    permutation
        .iter()
        .map(|&index| values[index].clone())
        .collect()
}

fn remap_rows(rows: &[Vec<i32>], map: &[i32]) -> Vec<Vec<i32>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|id| if *id < 1 { -1 } else { map[*id as usize] })
                .collect()
        })
        .collect()
}

fn normalized(point: CartesianPoint) -> io::Result<CartesianPoint> {
    let magnitude = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
    if !magnitude.is_finite() || magnitude == 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot normalize ICON spherical point",
        ));
    }
    Ok(CartesianPoint::new(
        point.x / magnitude,
        point.y / magnitude,
        point.z / magnitude,
    ))
}

fn tangent_components(
    midpoint: CartesianPoint,
    from: CartesianPoint,
    to: CartesianPoint,
) -> io::Result<(f64, f64)> {
    let lon = midpoint.y.atan2(midpoint.x);
    let lat = midpoint.z.asin();
    let east = CartesianPoint::new(-lon.sin(), lon.cos(), 0.0);
    let north = CartesianPoint::new(-lat.sin() * lon.cos(), -lat.sin() * lon.sin(), lat.cos());
    let delta = CartesianPoint::new(to.x - from.x, to.y - from.y, to.z - from.z);
    let e = delta.x * east.x + delta.y * east.y + delta.z * east.z;
    let n = delta.x * north.x + delta.y * north.y + delta.z * north.z;
    let magnitude = (e * e + n * n).sqrt();
    if !magnitude.is_finite() || magnitude == 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot derive ICON edge tangent",
        ));
    }
    Ok((e / magnitude, n / magnitude))
}

fn bounds_from_ids(rows: &[Vec<i32>], values: &[f64], missing: f64) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|id| {
                    if *id > 0 {
                        values[*id as usize - 1]
                    } else {
                        missing
                    }
                })
                .collect()
        })
        .collect()
}

fn padded_bounds_from_ids(rows: &[Vec<i32>], values: &[f64], fallback: &[f64]) -> Vec<Vec<f64>> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let last = row
                .iter()
                .rev()
                .find(|id| **id > 0)
                .map(|id| values[*id as usize - 1])
                .unwrap_or(fallback[index]);
            row.iter()
                .map(|id| {
                    if *id > 0 {
                        values[*id as usize - 1]
                    } else {
                        last
                    }
                })
                .collect()
        })
        .collect()
}

fn edge_bounds(
    edge_vertices: &[Vec<i32>],
    adjacent_cells: &[Vec<i32>],
    vertex_values: &[f64],
    cell_values: &[f64],
    edge_values: &[f64],
) -> Vec<Vec<f64>> {
    edge_vertices
        .iter()
        .zip(adjacent_cells)
        .enumerate()
        .map(|(index, (vertices, cells))| {
            vec![
                vertex_values[vertices[0] as usize - 1],
                vertex_values[vertices[1] as usize - 1],
                if cells[1] > 0 {
                    cell_values[cells[1] as usize - 1]
                } else {
                    edge_values[index]
                },
                cell_values[cells[0] as usize - 1],
            ]
        })
        .collect()
}

fn write_grf_indices(
    file: &mut netcdf::FileMut,
    suffix: &str,
    count: usize,
    controls: &[i32],
    grf_count: usize,
    interior_slot: usize,
    boundary_slots: usize,
) -> io::Result<()> {
    let count_i32 = i32::try_from(count).map_err(index_error)?;
    let mut start = vec![count_i32 + 1; grf_count];
    let mut end = vec![count_i32; grf_count];
    let boundary_count = controls.iter().take_while(|value| **value > 0).count();
    if boundary_count < count {
        start[interior_slot] = i32::try_from(boundary_count + 1).map_err(index_error)?;
        end[interior_slot] = count_i32;
    }
    for group in 1..=boundary_slots {
        let first = controls.iter().position(|value| *value == group as i32);
        let last = controls.iter().rposition(|value| *value == group as i32);
        let slot = interior_slot + group;
        if let (Some(first), Some(last)) = (first, last) {
            start[slot] = i32::try_from(first + 1).map_err(index_error)?;
            end[slot] = i32::try_from(last + 1).map_err(index_error)?;
        }
    }
    let dim = match suffix {
        "c" => "cell_grf",
        "e" => "edge_grf",
        _ => "vert_grf",
    };
    write_i32_columns(
        file,
        &format!("start_idx_{suffix}"),
        &["max_chdom", dim],
        &[start],
    )?;
    write_i32_columns(
        file,
        &format!("end_idx_{suffix}"),
        &["max_chdom", dim],
        &[end],
    )
}

fn write_coord(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[f64],
    bounds: &str,
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_attribute("units", "radian")
        .map_err(netcdf_to_io_error)?;
    let standard_name = if name.ends_with("lat") {
        "grid_latitude"
    } else {
        "grid_longitude"
    };
    var.put_attribute("standard_name", standard_name)
        .map_err(netcdf_to_io_error)?;
    var.put_attribute("bounds", bounds)
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

fn write_i32_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[i32],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

fn write_f64_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[f64],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

fn write_i32_columns(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<i32>],
) -> io::Result<()> {
    let flat = transpose_rows(rows);
    let mut var = file
        .add_variable::<i32>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flat, (.., ..)).map_err(netcdf_to_io_error)
}

fn write_f64_columns(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<f64>],
) -> io::Result<()> {
    let flat = transpose_rows(rows);
    let mut var = file
        .add_variable::<f64>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flat, (.., ..)).map_err(netcdf_to_io_error)
}

fn write_f64_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<f64>],
) -> io::Result<()> {
    let flat = rows
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect::<Vec<_>>();
    let mut var = file
        .add_variable::<f64>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flat, (.., ..)).map_err(netcdf_to_io_error)
}

fn transpose_rows<T: Copy>(rows: &[Vec<T>]) -> Vec<T> {
    let width = rows.first().map(Vec::len).unwrap_or(0);
    (0..width)
        .flat_map(|column| rows.iter().map(move |row| row[column]))
        .collect()
}

fn index_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("ICON index conversion failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_vertex_fan_keeps_cell_between_its_two_edges() {
        let vertices = vec![vec![1, 2, 3]];
        let cell_edges = vec![vec![1, 2, 3]];
        let edges = vec![([1, 2], [1, -1]), ([2, 3], [1, -1]), ([3, 1], [1, -1])];
        let (cells, vertex_edges, neighbors, orientations) =
            build_icon_vertex_fans(3, &vertices, &cell_edges, &edges).expect("open fan");

        assert_eq!(cells[0], vec![1, -1, -1, -1, -1, -1]);
        assert_eq!(vertex_edges[0], vec![1, 3, -1, -1, -1, -1]);
        assert_eq!(neighbors[0], vec![2, 3, -1, -1, -1, -1]);
        assert_eq!(orientations[0], vec![1, -1, 0, 0, 0, 0]);
    }
}
