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
    let grid = build_icon_grid(mesh)?;
    write_icon_grid(output.as_ref(), &grid, None, &IconFileExtras::default())
}

/// Export an admitted native TRI mesh without dropping cells to satisfy the
/// existing ICON dual-builder/ne=6 limitations. No model solver is run.
pub fn write_icon_from_final_gridfile(
    gridfile: &Path,
    output: &Path,
    nxp: usize,
) -> io::Result<IconGridWriteReport> {
    write_icon_final(gridfile, None, output, nxp)
}

/// Export a whole-triangle selection with metrics from its explicit closed
/// parent. Boundary dual_area retains the full parent dual, not a clipped volume.
pub fn write_icon_from_final_gridfile_with_parent(
    gridfile: &Path,
    parent: &Path,
    output: &Path,
    nxp: usize,
) -> io::Result<IconGridWriteReport> {
    write_icon_final(gridfile, Some(parent), output, nxp)
}

fn write_icon_final(
    gridfile: &Path,
    parent: Option<&Path>,
    output: &Path,
    nxp: usize,
) -> io::Result<IconGridWriteReport> {
    let points = crate::read_gridfile_mesh_points(gridfile)?;
    let input = crate::quality_input_from_gridfile(&points)?;
    let selected = parent
        .map(|parent| {
            let original = crate::read_gridfile_mesh_points(parent)?;
            validate_closed_triangle_parent(&original)?;
            crate::gridfile_lineage::verify_whole_triangle_lineage(parent, gridfile, &points)
        })
        .transpose()?;
    // The parent dual cannot encode distinct regional W vertices at one physical site.
    if parent.is_some() {
        let mut physical_sites = BTreeSet::new();
        let mut used_vertices = BTreeSet::new();
        let bits = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
        for vertex in input
            .cells
            .iter()
            .flat_map(|cell| cell.vertices.iter().copied())
        {
            if used_vertices.insert(vertex) {
                let key = [bits(points.w_lon[vertex]), bits(points.w_lat[vertex])];
                if !physical_sites.insert(key) {
                    return Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "ICON regional adapter cannot represent split vertices",
                    ));
                }
            }
        }
    }
    let mesh = crate::read_unstructured_mesh_netcdf(parent.unwrap_or(gridfile))?;
    // ICON consumes only geometry/connectivity from this existing intermediate:
    // neither its MPAS density nor nominalMinDc is exported as ICON demand.
    let cellwidth = vec![1.0; mesh.w_points.len()];
    let mpas = crate::build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, nxp, 1)?;
    validate_mpas_mesh(&mpas)?;
    let grid = build_icon_grid_selection(&mpas, selected.as_deref())?;
    validate_selected_triangles(&points, &input, &grid)?;
    crate::atomic_output::validate_output_path(gridfile, output)?;
    if let Some(parent) = parent {
        crate::atomic_output::validate_output_path(parent, output)?;
    }
    let mut report = None;
    crate::atomic_output::atomic_write(output, |temporary| {
        let mut written = write_icon_grid(temporary, &grid, parent, &IconFileExtras::default())?;
        crate::open_netcdf(temporary)
            .map_err(netcdf_to_io_error)?
            .close()
            .map_err(netcdf_to_io_error)?;
        written.output = output.to_path_buf();
        report = Some(written);
        Ok(())
    })?;
    report.ok_or_else(|| io::Error::other("ICON publication returned no report"))
}

/// Lateral boundary cell rows flagged in an ICON nest, as the ICON grid
/// generator's `bdy_indexing_depth`. ICON needs at least
/// `nudge_zone_width + 4` (12 by default).
pub const ICON_NEST_BOUNDARY_DEPTH: u32 = 14;

/// One file of an ICON nest set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconNestFileReport {
    pub domain: usize,
    pub parent: usize,
    pub grid_level: u32,
    pub output: PathBuf,
    pub cells: usize,
    pub vertices: usize,
    pub edges: usize,
    pub uuid: String,
}

/// Write the global grid and its nests as ICON grid files
/// `<stem>_DOM01.nc`, `<stem>_DOM02.nc`, ... in `output_dir`.
///
/// Each nest carries what ICON reads to attach it (guide 11.86):
/// `parent_cell_index` and `parent_edge_index` into its parent's file, a
/// `grid_level` one below its parent's with the same `grid_root`, and
/// `uuidOfParHGrid` equal to the parent's `uuidOfHGrid`. The parent-to-child
/// relations and the parent's negative `refin_ctrl` are computed by ICON at
/// startup and are not written.
pub fn write_icon_nest_set(
    domains: &[earthmesh_mesh::IconNestDomain],
    grid_root: usize,
    output_dir: &Path,
    stem: &str,
) -> io::Result<Vec<IconNestFileReport>> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    let grid_root = i32::try_from(grid_root).map_err(index_error)?;
    std::fs::create_dir_all(output_dir)?;
    struct Written {
        order: IconOrder,
        edges: BTreeMap<(i32, i32), i32>,
        uuid: String,
    }
    let mut written = Vec::<Written>::with_capacity(domains.len());
    let mut reports = Vec::with_capacity(domains.len());
    for (index, domain) in domains.iter().enumerate() {
        if domain.id != index + 1 || (index > 0 && !(1..domain.id).contains(&domain.parent)) {
            return Err(invalid(format!(
                "ICON nest set: domain {} at position {} has parent {}",
                domain.id,
                index + 1,
                domain.parent
            )));
        }
        let geometry = triangle_geometry(&domain.points, &domain.triangles)?;
        let vertex_of_cell = domain
            .triangles
            .iter()
            .map(|tri| {
                tri.iter()
                    .map(|&v| i32::try_from(v + 1).map_err(index_error))
                    .collect::<io::Result<Vec<_>>>()
            })
            .collect::<io::Result<Vec<_>>>()?;
        let ctrl = if domain.parent == 0 {
            IconCtrl::Boundary
        } else {
            IconCtrl::Nest {
                vertex_row: &domain.vertex_row,
                cell_row: &domain.cell_row,
                depth: ICON_NEST_BOUNDARY_DEPTH,
            }
        };
        let (grid, order) = reorder_icon_grid(assemble_icon_grid(vertex_of_cell, geometry, ctrl)?)?;
        let edges = grid
            .edge_vertices
            .iter()
            .enumerate()
            .map(|(e, v)| ((v[0].min(v[1]), v[0].max(v[1])), e as i32 + 1))
            .collect::<BTreeMap<_, _>>();
        let uuid = grid_uuid(&grid.vertex_xyz, domain.id);
        let mut extras = IconFileExtras {
            grid_root,
            grid_level: i32::try_from(domain.depth).map_err(index_error)?,
            uuid: uuid.clone(),
            ..IconFileExtras::default()
        };
        if domain.parent > 0 {
            let parent = &written[domain.parent - 1];
            let parent_domain = &domains[domain.parent - 1];
            extras.parent_uuid = parent.uuid.clone();
            extras.parent_cell_index = Some(
                order
                    .cell_perm
                    .iter()
                    .map(|&old| parent.order.cell_map[domain.parent_triangle[old] + 1])
                    .collect(),
            );
            // A child edge lies on a parent edge (a kept corner and the
            // midpoint of an edge at it) or runs parallel to the one the two
            // midpoints it joins do not share.
            let parent_edge = |a: usize, b: usize| -> io::Result<i32> {
                let (a, b) = (
                    parent.order.vertex_map[a + 1],
                    parent.order.vertex_map[b + 1],
                );
                parent
                    .edges
                    .get(&(a.min(b), a.max(b)))
                    .copied()
                    .ok_or_else(|| {
                        invalid(format!(
                            "ICON nest {}: parent domain {} has no edge {a}-{b}",
                            domain.id, parent_domain.id
                        ))
                    })
            };
            use earthmesh_mesh::NestVertexOrigin as O;
            extras.parent_edge_index = Some(
                grid.edge_vertices
                    .iter()
                    .map(|v| {
                        let origin = |id: i32| domain.vertex_origin[order.vertex_perm[id as usize - 1]];
                        match (origin(v[0]), origin(v[1])) {
                            (O::Parent(k), O::Midpoint(x, y)) | (O::Midpoint(x, y), O::Parent(k))
                                if k == x || k == y =>
                            {
                                parent_edge(x, y)
                            }
                            (O::Midpoint(x, y), O::Midpoint(p, q)) => {
                                let far = [x, y, p, q]
                                    .into_iter()
                                    .filter(|&k| [x, y].contains(&k) != [p, q].contains(&k))
                                    .collect::<Vec<_>>();
                                match far[..] {
                                    [a, b] => parent_edge(a, b),
                                    _ => Err(invalid(format!(
                                        "ICON nest {}: an edge joins midpoints of unrelated parent edges",
                                        domain.id
                                    ))),
                                }
                            }
                            _ => Err(invalid(format!(
                                "ICON nest {}: an edge is not part of a 1->4 bisection",
                                domain.id
                            ))),
                        }
                    })
                    .collect::<io::Result<Vec<_>>>()?,
            );
        }
        let output = output_dir.join(format!("{stem}_DOM{:02}.nc", domain.id));
        let mut report = None;
        crate::atomic_output::atomic_write(&output, |temporary| {
            report = Some(write_icon_grid(temporary, &grid, None, &extras)?);
            Ok(())
        })?;
        let report =
            report.ok_or_else(|| io::Error::other("ICON nest publication returned no report"))?;
        reports.push(IconNestFileReport {
            domain: domain.id,
            parent: domain.parent,
            grid_level: domain.depth,
            output,
            cells: report.cells,
            vertices: report.vertices,
            edges: report.edges,
            uuid: uuid.clone(),
        });
        written.push(Written { order, edges, uuid });
    }
    // Refuse a set ICON would stop on, rather than publish it.
    let files = reports
        .iter()
        .map(|r| (r.output.clone(), r.parent))
        .collect::<Vec<_>>();
    if let Err(error) = validate_icon_nest_set(&files) {
        for (path, _) in &files {
            let _ = std::fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(reports)
}

/// Read an ICON nest set back and check it as ICON does when it loads the
/// domains (guide 11.86), so a set that would stop the model is refused here:
/// - `grid_root` shared, each nest's `grid_level` one below its parent's, its
///   `uuidOfParHGrid` equal to the parent's `uuidOfHGrid`;
/// - cells, edges and vertices sorted boundary rows `1..=max_rl` first, with
///   `start_idx_*`/`end_idx_*` naming exactly those ranges;
/// - every nest flagged at least `nudge_zone_width + 4` (12) cell rows deep;
/// - every parent cell with 0 or 4 children and exactly 3 inner child edges
///   ("Incomplete parent cell", "edge counting went wrong" in ICON);
/// - every child edge's parent edge an edge of its cells' parent cells;
/// - no parent cell shared by two sibling nests.
pub fn validate_icon_nest_set(files: &[(PathBuf, usize)]) -> io::Result<()> {
    let bad = |message: String| io::Error::new(io::ErrorKind::InvalidData, message);
    struct Domain {
        cells: usize,
        edges: usize,
        edge_of_cell: Vec<i32>,
        root: i32,
        level: i32,
        uuid: String,
    }
    let mut loaded = Vec::<Domain>::new();
    let mut claimed = BTreeMap::<(usize, i32), usize>::new();
    for (index, (path, parent)) in files.iter().enumerate() {
        let name = path.display();
        let file = crate::open_netcdf(path).map_err(netcdf_to_io_error)?;
        let dim = |n: &str| {
            file.dimension(n)
                .map(|d| d.len())
                .ok_or_else(|| bad(format!("{name}: no dimension {n}")))
        };
        let (cells, edges, vertices) = (dim("cell")?, dim("edge")?, dim("vertex")?);
        if dim("ne")? != 6 || dim("nv")? != 3 {
            return Err(bad(format!("{name}: ICON needs nv=3 and ne=6")));
        }
        let int = |n: &str| -> io::Result<i32> {
            match file
                .attribute(n)
                .map(|a| a.value())
                .transpose()
                .map_err(netcdf_to_io_error)?
            {
                Some(netcdf::AttributeValue::Int(v)) => Ok(v),
                _ => Err(bad(format!("{name}: attribute {n} must be an int"))),
            }
        };
        let text = |n: &str| -> io::Result<String> {
            match file
                .attribute(n)
                .map(|a| a.value())
                .transpose()
                .map_err(netcdf_to_io_error)?
            {
                Some(netcdf::AttributeValue::Str(v)) => Ok(v),
                _ => Err(bad(format!("{name}: attribute {n} must be a string"))),
            }
        };
        let (root, level, uuid) = (int("grid_root")?, int("grid_level")?, text("uuidOfHGrid")?);
        if uuid.is_empty() {
            return Err(bad(format!("{name}: empty uuidOfHGrid")));
        }
        // Sorted as ICON reads it, with index ranges naming the rows.
        for (suffix, count, min_rl, max_rl) in [
            ("c", cells, -8_i32, 5_i32),
            ("e", edges, -13, 10),
            ("v", vertices, -7, 5),
        ] {
            let ctrl = crate::required_values_i32(&file, &format!("refin_{suffix}_ctrl"))?;
            let start = crate::required_values_i32(&file, &format!("start_idx_{suffix}"))?;
            let end = crate::required_values_i32(&file, &format!("end_idx_{suffix}"))?;
            let slot = |level: i32| (level - min_rl) as usize;
            if start.len() <= slot(max_rl) || end.len() <= slot(max_rl) {
                return Err(bad(format!(
                    "{name}: start/end_idx_{suffix} have fewer than {} levels",
                    slot(max_rl) + 1
                )));
            }
            let reordered = |v: i32| (1..=max_rl).contains(&v);
            let head = ctrl.iter().take_while(|v| reordered(**v)).count();
            if ctrl.len() != count
                || ctrl[head..].iter().any(|v| reordered(*v))
                || ctrl[..head].windows(2).any(|w| w[0] > w[1])
            {
                return Err(bad(format!(
                    "{name}: {suffix} entities are not sorted by refin_{suffix}_ctrl"
                )));
            }
            // Chained ranges, an empty row being `end + 1 .. end`.
            let mut cursor = 0i32;
            for level in 1..=max_rl {
                let rows = ctrl.iter().filter(|&&v| v == level).count() as i32;
                if (start[slot(level)], end[slot(level)]) != (cursor + 1, cursor + rows) {
                    return Err(bad(format!(
                        "{name}: start/end_idx_{suffix} of row {level} do not match"
                    )));
                }
                cursor += rows;
            }
            // The interior follows the rows. The ICON grid generator may end it
            // before the last entity (DWD's limited-area grids leave a few deep
            // rows after it; ICON's whole-grid loops run to the end anyway).
            if start[slot(0)] != cursor + 1 || !(cursor..=count as i32).contains(&end[slot(0)]) {
                return Err(bad(format!(
                    "{name}: start/end_idx_{suffix} of the interior do not match"
                )));
            }
        }
        let edge_of_cell = crate::required_values_i32(&file, "edge_of_cell")?;
        if *parent == 0 {
            if index != 0 {
                return Err(bad(format!(
                    "{name}: only the first domain may lack a parent"
                )));
            }
        } else {
            let up = loaded
                .get(parent - 1)
                .ok_or_else(|| bad(format!("{name}: parent domain {parent} is not before it")))?;
            if root != up.root || level != up.level + 1 || text("uuidOfParHGrid")? != up.uuid {
                return Err(bad(format!(
                    "{name}: grid_root/grid_level/uuidOfParHGrid do not match parent domain {parent}"
                )));
            }
            let rows = crate::required_values_i32(&file, "refin_c_ctrl")?;
            if rows.iter().copied().max().unwrap_or(0) < 12 {
                return Err(bad(format!(
                    "{name}: fewer than 12 boundary cell rows (nudge_zone_width + 4)"
                )));
            }
            let parent_cell = crate::required_values_i32(&file, "parent_cell_index")?;
            let parent_edge = crate::required_values_i32(&file, "parent_edge_index")?;
            let adjacent = crate::required_values_i32(&file, "adjacent_cell_of_edge")?;
            let mut children = BTreeMap::<i32, usize>::new();
            for &p in &parent_cell {
                if !(1..=up.cells as i32).contains(&p) {
                    return Err(bad(format!("{name}: parent_cell_index {p} out of range")));
                }
                *children.entry(p).or_default() += 1;
            }
            if let Some((p, n)) = children.iter().find(|(_, &n)| n != 4) {
                return Err(bad(format!(
                    "{name}: parent cell {p} has {n} children (ICON needs 0 or 4)"
                )));
            }
            let mut inner = BTreeMap::<i32, usize>::new();
            let parent_edges_of =
                |p: i32| (0..3).map(move |k| up.edge_of_cell[k * up.cells + p as usize - 1]);
            for e in 0..edges {
                // Either slot may be the missing one on a boundary edge (the
                // ICON grid generator writes 0 in the first as often).
                let sides = [adjacent[e], adjacent[edges + e]]
                    .into_iter()
                    .filter(|&c| (1..=cells as i32).contains(&c))
                    .map(|c| parent_cell[c as usize - 1])
                    .collect::<Vec<_>>();
                let pe = parent_edge[e];
                if !(1..=up.edges as i32).contains(&pe) {
                    return Err(bad(format!("{name}: parent_edge_index {pe} out of range")));
                }
                if sides.is_empty() {
                    return Err(bad(format!("{name}: child edge {} has no cell", e + 1)));
                }
                let owned = sides.iter().any(|&p| parent_edges_of(p).any(|x| x == pe));
                if let [pa, pb] = sides[..] {
                    if pa == pb {
                        *inner.entry(pa).or_default() += 1;
                    }
                }
                if !owned {
                    return Err(bad(format!(
                        "{name}: child edge {} maps to parent edge {pe}, not an edge of its parent cells",
                        e + 1
                    )));
                }
            }
            if let Some(p) = children
                .keys()
                .find(|p| inner.get(p).copied().unwrap_or(0) != 3)
            {
                return Err(bad(format!(
                    "{name}: parent cell {p} does not have exactly 3 inner edges"
                )));
            }
            for &p in children.keys() {
                if let Some(other) = claimed.insert((*parent, p), index + 1) {
                    return Err(bad(format!(
                        "{name}: parent cell {p} is also refined by sibling domain {other}"
                    )));
                }
            }
        }
        loaded.push(Domain {
            cells,
            edges,
            edge_of_cell,
            root,
            level,
            uuid,
        });
    }
    Ok(())
}

/// Positions and areas of a triangle grid given as unit vectors: cell
/// centres at circumcentres, dual areas from the kites of each triangle.
fn triangle_geometry(points: &[[f64; 3]], triangles: &[[usize; 3]]) -> io::Result<IconGeometry> {
    use earthmesh_mesh::{spherical_kite_area_unit, spherical_triangle_area_unit};
    let r2 = ICON_SPHERE_RADIUS_METERS.powi(2);
    let point = |p: [f64; 3]| CartesianPoint::new(p[0], p[1], p[2]);
    let lonlat = |p: CartesianPoint| (p.y.atan2(p.x), p.z.clamp(-1.0, 1.0).asin());
    let vertex_xyz = points.iter().map(|&p| point(p)).collect::<Vec<_>>();
    let mut cell_xyz = Vec::with_capacity(triangles.len());
    let mut cell_area = Vec::with_capacity(triangles.len());
    let mut dual_area = vec![0.0; points.len()];
    for tri in triangles {
        let [a, b, c] = tri.map(|v| vertex_xyz[v]);
        let (u, w) = (
            CartesianPoint::new(b.x - a.x, b.y - a.y, b.z - a.z),
            CartesianPoint::new(c.x - a.x, c.y - a.y, c.z - a.z),
        );
        let centre = normalized(CartesianPoint::new(
            u.y * w.z - u.z * w.y,
            u.z * w.x - u.x * w.z,
            u.x * w.y - u.y * w.x,
        ))?;
        cell_xyz.push(centre);
        cell_area.push(spherical_triangle_area_unit([a, b, c]) * r2);
        for k in 0..3 {
            let (v, n1, n2) = (tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]);
            let mid = |x: usize, y: usize| {
                let (p, q) = (vertex_xyz[x], vertex_xyz[y]);
                normalized(CartesianPoint::new(p.x + q.x, p.y + q.y, p.z + q.z))
            };
            dual_area[v] +=
                spherical_kite_area_unit(vertex_xyz[v], mid(v, n1)?, mid(v, n2)?, centre) * r2;
        }
    }
    let (clon, clat) = cell_xyz.iter().map(|&p| lonlat(p)).unzip();
    let (vlon, vlat) = vertex_xyz.iter().map(|&p| lonlat(p)).unzip();
    Ok(IconGeometry {
        clon,
        clat,
        vlon,
        vlat,
        vertex_xyz,
        cell_xyz,
        cell_area,
        dual_area,
    })
}

/// A UUID-formatted identity for one grid file, derived from its vertex
/// positions and domain id so the same grid always gets the same string.
fn grid_uuid(vertices: &[CartesianPoint], domain: usize) -> String {
    let mut hash = [0xcbf2_9ce4_8422_2325_u64, 0x8422_2325_cbf2_9ce4_u64];
    let mut feed = |bytes: &[u8]| {
        for (lane, h) in hash.iter_mut().enumerate() {
            for &byte in bytes {
                *h ^= u64::from(byte) ^ (lane as u64);
                *h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    };
    feed(&(domain as u64).to_le_bytes());
    for p in vertices {
        for x in [p.x, p.y, p.z] {
            feed(&x.to_bits().to_le_bytes());
        }
    }
    let [hi, lo] = hash;
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        hi >> 32,
        (hi >> 16) & 0xffff,
        hi & 0x0fff,
        (lo >> 48) & 0x3fff | 0x8000,
        lo & 0xffff_ffff_ffff
    )
}

// ICON represents M triangles; a legal degree-four W fan is not a HEX cell.
fn validate_closed_triangle_parent(points: &crate::GridfileMeshPoints) -> io::Result<()> {
    use earthmesh_quality::topology::{
        boundary_topology, connected_component_count, euler_characteristic, MeshTopologyValidator,
        Severity,
    };
    let input = crate::quality_input_from_gridfile(points)?;
    if boundary_topology(&input).edge_count != 0
        || euler_characteristic(&input) != 2
        || connected_component_count(&input) != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ICON regional delivery requires one closed triangular sphere as explicit parent",
        ));
    }
    if let Some(issue) = MeshTopologyValidator::new(&input)
        .validate_all()
        .into_iter()
        .find(|issue| issue.severity == Severity::Fail)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ICON parent topology {}: {}",
                issue.issue_type.as_str(),
                issue.message
            ),
        ));
    }
    Ok(())
}

fn validate_selected_triangles(
    points: &crate::GridfileMeshPoints,
    input: &earthmesh_quality::QualityMeshInput,
    grid: &IconGrid,
) -> io::Result<()> {
    let invalid = || {
        io::Error::new(io::ErrorKind::InvalidData,
        "ICON adapter changed the selected native triangle/vertex/edge set; the existing dual adapter cannot represent this selection")
    };
    // Exact coordinate identities, with signed zero equivalent. Coincident
    // physical vertices are ambiguous: reject rather than merge their topology.
    let bits = |x: f64| if x == 0.0 { 0 } else { x.to_bits() };
    let key = |lon: f64, lat: f64| [bits(lon), bits(lat)];
    let radians = |lon: &[f64], lat: &[f64]| {
        let points = lon
            .iter()
            .zip(lat)
            .map(|(&lon_degrees, &lat_degrees)| crate::LonLatDegrees {
                lon_degrees,
                lat_degrees,
            })
            .collect::<Vec<_>>();
        crate::mpas_lat_lon_radians(&points)
    };
    let (lat, lon) = radians(&points.w_lon, &points.w_lat);
    let used = input
        .cells
        .iter()
        .flat_map(|c| c.vertices.iter().copied())
        .collect::<BTreeSet<_>>();
    let expected_vertices = used
        .iter()
        .map(|&i| key(lon[i], lat[i]))
        .collect::<BTreeSet<_>>();
    let vertices = grid
        .vlon
        .iter()
        .zip(&grid.vlat)
        .map(|(&x, &y)| key(x, y))
        .collect::<Vec<_>>();
    if expected_vertices.len() != used.len()
        || vertices.len() != used.len()
        || vertices.iter().copied().collect::<BTreeSet<_>>() != expected_vertices
    {
        return Err(invalid());
    }
    let (mlat, mlon) = radians(&points.m_lon, &points.m_lat);
    let first = crate::gridfile_m_row_layout(points).first_physical_row;
    let mut expected_cells = input
        .cells
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut corners = c
                .vertices
                .iter()
                .map(|&j| key(lon[j], lat[j]))
                .collect::<Vec<_>>();
            corners.sort_unstable();
            (key(mlon[first + i], mlat[first + i]), corners)
        })
        .collect::<Vec<_>>();
    let mut cells = grid
        .vertex_of_cell
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut corners = c
                .iter()
                .map(|&j| vertices[j as usize - 1])
                .collect::<Vec<_>>();
            corners.sort_unstable();
            (key(grid.clon[i], grid.clat[i]), corners)
        })
        .collect::<Vec<_>>();
    expected_cells.sort_unstable();
    cells.sort_unstable();
    if cells != expected_cells {
        return Err(invalid());
    }
    let edge = |a, b| if a < b { (a, b) } else { (b, a) };
    let mut expected_edges = BTreeMap::new();
    for cell in &input.cells {
        for i in 0..3 {
            let a = cell.vertices[i];
            let b = cell.vertices[(i + 1) % 3];
            *expected_edges
                .entry(edge(key(lon[a], lat[a]), key(lon[b], lat[b])))
                .or_insert(0) += 1;
        }
    }
    let mut edges = grid
        .edge_vertices
        .iter()
        .map(|e| edge(vertices[e[0] as usize - 1], vertices[e[1] as usize - 1]))
        .collect::<Vec<_>>();
    edges.sort_unstable();
    if edges != expected_edges.keys().copied().collect::<Vec<_>>()
        || grid
            .adjacent_cell_of_edge
            .iter()
            .filter(|e| e[1] < 1)
            .count()
            != expected_edges.values().filter(|&&n| n == 1).count()
    {
        return Err(invalid());
    }
    Ok(())
}

/// What distinguishes one file of an ICON grid set from a lone grid.
#[derive(Default)]
struct IconFileExtras {
    parent_cell_index: Option<Vec<i32>>,
    parent_edge_index: Option<Vec<i32>>,
    grid_root: i32,
    grid_level: i32,
    uuid: String,
    parent_uuid: String,
}

fn write_icon_grid(
    output: &Path,
    grid: &IconGrid,
    parent: Option<&Path>,
    extras: &IconFileExtras,
) -> io::Result<IconGridWriteReport> {
    crate::ensure_parent_dir(output)?;
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
        extras
            .parent_cell_index
            .as_deref()
            .unwrap_or(&vec![-1; grid.clon.len()]),
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
        extras
            .parent_edge_index
            .as_deref()
            .unwrap_or(&vec![-1; grid.elon.len()]),
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
    file.add_attribute("grid_level", extras.grid_level)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("grid_root", extras.grid_root)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("uuidOfParHGrid", extras.parent_uuid.as_str())
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("uuidOfHGrid", extras.uuid.as_str())
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("global_grid", i32::from(global_grid))
        .map_err(netcdf_to_io_error)?;
    if let Some(parent) = parent {
        file.add_attribute(
            "earthmesh_geometry_parent",
            parent.to_string_lossy().as_ref(),
        )
        .map_err(netcdf_to_io_error)?;
        file.add_attribute("earthmesh_dual_area_scope", "full_parent_dual")
            .map_err(netcdf_to_io_error)?;
    }
    file.close().map_err(netcdf_to_io_error)?;

    Ok(IconGridWriteReport {
        output: output.to_path_buf(),
        cells: grid.clon.len(),
        vertices: grid.vlon.len(),
        edges: grid.elon.len(),
        global_grid,
    })
}

fn build_icon_grid(mesh: &MpasMesh) -> io::Result<IconGrid> {
    build_icon_grid_selection(mesh, None)
}

fn build_icon_grid_selection(mesh: &MpasMesh, selected: Option<&[usize]>) -> io::Result<IconGrid> {
    let cells_on_vertex = derive_cells_on_vertex(mesh)?;
    let valid_cells = if let Some(selected) = selected {
        let mut seen = BTreeSet::new();
        for &id in selected {
            if id == 0
                || !seen.insert(id)
                || cells_on_vertex
                    .get(id)
                    .is_none_or(|corners| corners.len() != 3)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "ICON selection requires unique complete parent triangles",
                ));
            }
        }
        selected.to_vec()
    } else {
        (1..cells_on_vertex.len())
            .filter(|&id| cells_on_vertex[id].len() == 3)
            .collect::<Vec<_>>()
    };
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
    for old_cell in valid_cells.iter().copied() {
        let mut row = cells_on_vertex[old_cell]
            .iter()
            .map(|value| vertex_map[*value as usize])
            .collect::<Vec<_>>();
        orient_triangle_outward(&mut row, &vertex_old, mesh, old_cell);
        vertex_of_cell.push(row);
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
    let geometry = IconGeometry {
        clon,
        clat,
        vlon,
        vlat,
        vertex_xyz,
        cell_xyz,
        cell_area,
        dual_area,
    };
    let grid = assemble_icon_grid(vertex_of_cell, geometry, IconCtrl::Boundary)?;
    Ok(reorder_icon_grid(grid)?.0)
}

/// Positions and areas of a triangle grid, in the order of its cells and
/// vertices before the ICON reordering.
struct IconGeometry {
    clon: Vec<f64>,
    clat: Vec<f64>,
    vlon: Vec<f64>,
    vlat: Vec<f64>,
    vertex_xyz: Vec<CartesianPoint>,
    cell_xyz: Vec<CartesianPoint>,
    cell_area: Vec<f64>,
    dual_area: Vec<f64>,
}

/// How the lateral-boundary flags `refin_*_ctrl` are set.
enum IconCtrl<'a> {
    /// Rows counted from an open boundary, capped at ICON's reordered rows
    /// (the regional adapter's convention).
    Boundary,
    /// A nest's rows, flagged to `depth` cell rows as the current ICON grid
    /// generator does with `bdy_indexing_depth = 14` (guide 11.86; every
    /// cell, edge and vertex equal on six DWD nests and limited-area grids):
    /// a vertex's row is its distance from the boundary in vertex rows (1 on
    /// it), a cell's the least of its vertices', an edge 1 on the boundary,
    /// `2r` inside row `r` and `2r + 1` between rows `r` and `r + 1`; flagged
    /// up to `depth` (cells, vertices) and `2 depth` (edges), 0 beyond.
    Nest {
        vertex_row: &'a [u32],
        cell_row: &'a [u32],
        depth: u32,
    },
}

/// Edges, vertex fans, metrics and boundary flags of a grid whose cells are
/// `vertex_of_cell` (1-based, counter-clockwise seen from outside).
fn assemble_icon_grid(
    vertex_of_cell: Vec<Vec<i32>>,
    geometry: IconGeometry,
    ctrl: IconCtrl<'_>,
) -> io::Result<IconGrid> {
    let IconGeometry {
        clon,
        clat,
        vlon,
        vlat,
        vertex_xyz,
        cell_xyz,
        cell_area,
        dual_area,
    } = geometry;
    let mut edge_of_cell = vec![vec![-1; 3]; vertex_of_cell.len()];
    let mut edges = Vec::<([i32; 2], [i32; 2])>::new();
    let mut edge_ids = BTreeMap::<(i32, i32), i32>::new();
    for (cell_index, row) in vertex_of_cell.iter().enumerate() {
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
        build_icon_vertex_fans(vertex_xyz.len(), &vertex_of_cell, &edge_of_cell, &edges)?;

    let adjacent_cell_of_edge = edges.iter().map(|edge| edge.1.to_vec()).collect::<Vec<_>>();
    let edge_vertices = edges.iter().map(|edge| edge.0.to_vec()).collect::<Vec<_>>();
    let mut neighbor_cell_index = vec![vec![-1; 3]; vertex_of_cell.len()];
    let mut orientation_of_normal = vec![vec![0; 3]; vertex_of_cell.len()];
    for cell in 0..vertex_of_cell.len() {
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

    let (cell_ctrl, vertex_ctrl, edge_ctrl) = match ctrl {
        IconCtrl::Boundary => {
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
            (cell_ctrl, vertex_ctrl, edge_ctrl)
        }
        IconCtrl::Nest {
            vertex_row,
            cell_row,
            depth,
        } => {
            let flag = |row: u32, limit: u32| {
                if row <= limit {
                    row as i32
                } else {
                    0
                }
            };
            let cell_ctrl = cell_row.iter().map(|&r| flag(r, depth)).collect::<Vec<_>>();
            let vertex_ctrl = vertex_row
                .iter()
                .map(|&r| flag(r, depth))
                .collect::<Vec<_>>();
            let edge_ctrl = adjacent_cell_of_edge
                .iter()
                .map(|cells| {
                    let first = cell_row[cells[0] as usize - 1];
                    if cells[1] < 1 {
                        return 1;
                    }
                    let second = cell_row[cells[1] as usize - 1];
                    let (low, high) = (first.min(second), first.max(second));
                    let value = if low == high {
                        low.saturating_mul(2)
                    } else {
                        low.saturating_mul(2).saturating_add(1)
                    };
                    flag(value, 2 * depth)
                })
                .collect::<Vec<_>>();
            (cell_ctrl, vertex_ctrl, edge_ctrl)
        }
    };

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
    Ok(grid)
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

/// Where the ICON reordering moved each entity: `*_perm[new] = old` and
/// `*_map[old + 1] = new + 1` (0-based positions, 1-based ids).
struct IconOrder {
    cell_perm: Vec<usize>,
    vertex_perm: Vec<usize>,
    cell_map: Vec<i32>,
    vertex_map: Vec<i32>,
}

/// Sort cells, edges and vertices as ICON reads them: boundary rows
/// `1..=max_rl` first in ascending order, then everything else.
fn reorder_icon_grid(mut grid: IconGrid) -> io::Result<(IconGrid, IconOrder)> {
    let cell_perm = permutation(&grid.cell_ctrl, 5);
    let edge_perm = permutation(&grid.edge_ctrl, 10);
    let vertex_perm = permutation(&grid.vertex_ctrl, 5);
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
    let order = IconOrder {
        cell_perm,
        vertex_perm,
        cell_map,
        vertex_map,
    };
    Ok((grid, order))
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

fn permutation(groups: &[i32], max_rl: i32) -> Vec<usize> {
    let mut ids = (0..groups.len()).collect::<Vec<_>>();
    ids.sort_by_key(|index| {
        (
            if (1..=max_rl).contains(&groups[*index]) {
                groups[*index]
            } else {
                i32::MAX
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
    // The ranges chain as the ICON grid generator writes them: rows
    // 1..=max_rl in order, then the interior (level 0, which also holds rows
    // beyond max_rl), each starting one past the previous end -- an empty row
    // is `start = end + 1` at that point, so a grid without a boundary has
    // row 1 as 1..0 and its interior from 1 (DWD's global R02B05 grid). The
    // levels past the interior (child overlap, halos) are empty after it.
    let count_i32 = i32::try_from(count).map_err(index_error)?;
    let mut start = vec![count_i32 + 1; grf_count];
    let mut end = vec![count_i32; grf_count];
    let mut cursor = 0i32;
    for group in 1..=boundary_slots {
        let rows = controls
            .iter()
            .filter(|value| **value == group as i32)
            .count();
        let slot = interior_slot + group;
        start[slot] = cursor + 1;
        cursor += i32::try_from(rows).map_err(index_error)?;
        end[slot] = cursor;
    }
    start[interior_slot] = cursor + 1;
    end[interior_slot] = count_i32;
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
    fn degree_seven_is_rejected_instead_of_truncated_to_ne_six() {
        let vertices = (0..7)
            .map(|i| vec![1, i + 2, (i + 1) % 7 + 2])
            .collect::<Vec<_>>();
        let cell_edges = (0..7)
            .map(|i| vec![i + 1, i + 8, (i + 1) % 7 + 1])
            .collect::<Vec<_>>();
        let mut edges = (0..7)
            .map(|i| ([1, i + 2], [(i + 6) % 7 + 1, i + 1]))
            .collect::<Vec<_>>();
        edges.extend((0..7).map(|i| ([i + 2, (i + 1) % 7 + 2], [i + 1, -1])));
        let err = build_icon_vertex_fans(8, &vertices, &cell_edges, &edges).unwrap_err();
        assert!(
            err.to_string()
                .contains("ICON ne=6 cannot represent EarthMesh vertex 1 with degree 7"),
            "{err}"
        );
    }

    #[test]
    fn selected_triangle_guard_rejects_changed_sets_even_with_equal_counts() {
        let path = std::env::temp_dir().join(format!("icon_set_guard_{}.nc4", std::process::id()));
        let state = earthmesh_mesh::gridinit_voronoi_state_canonical(1, 0, 1.0, 0.25, 100).unwrap();
        let mesh = crate::gridfile_mesh_from_one_based_state(&state.grid, &state.tabs).unwrap();
        crate::write_unstructured_mesh_netcdf(&path, &mesh).unwrap();
        let points = crate::read_gridfile_mesh_points(&path).unwrap();
        let input = crate::quality_input_from_gridfile(&points).unwrap();
        let mpas = crate::build_mpas_mesh_from_unstructured_one_based(
            &mesh,
            &vec![1.0; mesh.w_points.len()],
            1,
            1,
        )
        .unwrap();
        let grid = build_icon_grid(&mpas).unwrap();
        validate_selected_triangles(&points, &input, &grid).unwrap();
        for case in 0..5 {
            let mut changed = grid.clone();
            match case {
                0 => {
                    changed.vertex_of_cell.pop();
                }
                1 => changed.vertex_of_cell[0] = changed.vertex_of_cell[1].clone(),
                2 => changed.vlon[0] += 0.01,
                3 => changed.edge_vertices[0] = changed.edge_vertices[1].clone(),
                _ => changed.adjacent_cell_of_edge[0][1] = -1,
            }
            assert!(
                validate_selected_triangles(&points, &input, &changed).is_err(),
                "case {case}"
            );
        }
        std::fs::remove_file(path).unwrap();
    }

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
