use std::collections::{BTreeMap, BTreeSet};
use std::{fs, io, path::PathBuf};

use earthmesh_geometry::Point;
use earthmesh_mesh::{
    lonlat_degrees_to_unit_xyz, spherical_centroid_degrees, spherical_triangle_area_unit,
    LonLatDegrees,
};
use earthmesh_quality::{
    QualityCell, QualityComputationOptions, QualityMeshInput, QualityThresholds,
};

type Edge = (usize, usize);

#[derive(Clone, Debug, PartialEq)]
struct ResearchTriangle {
    vertices: [usize; 3],
    level: u8,
    parent: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
struct ResearchPrimalMesh {
    points: Vec<LonLatDegrees>,
    triangles: Vec<ResearchTriangle>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TopologyReport {
    euler_characteristic: isize,
    boundary_edge_count: usize,
    max_vertex_valence: usize,
}

fn edge(a: usize, b: usize) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn triangle_edges(vertices: [usize; 3]) -> [Edge; 3] {
    [
        edge(vertices[0], vertices[1]),
        edge(vertices[1], vertices[2]),
        edge(vertices[2], vertices[0]),
    ]
}

fn split_triangle_on_edge(
    vertices: [usize; 3],
    split_edge: Edge,
    midpoint: usize,
) -> Option<[[usize; 3]; 2]> {
    for offset in 0..3 {
        let a = vertices[offset];
        let b = vertices[(offset + 1) % 3];
        let opposite = vertices[(offset + 2) % 3];
        if edge(a, b) == split_edge {
            return Some([[a, midpoint, opposite], [midpoint, b, opposite]]);
        }
    }
    None
}

/// Research-only conforming red/green refinement.
///
/// Marked triangles request all three parent edges. Every triangle incident to
/// one of those edges is then split on the same shared midpoint, so no hanging
/// edge can be created. This intentionally lives in the integration test until
/// the primal/dual split hypothesis is proven on real cases.
fn refine_marked_triangles(
    mesh: &ResearchPrimalMesh,
    marked: &BTreeSet<usize>,
) -> Result<ResearchPrimalMesh, String> {
    refine_marked_triangles_with_midpoints(mesh, marked).map(|(mesh, _)| mesh)
}

fn refine_marked_triangles_with_midpoints(
    mesh: &ResearchPrimalMesh,
    marked: &BTreeSet<usize>,
) -> Result<(ResearchPrimalMesh, BTreeMap<Edge, usize>), String> {
    if let Some(invalid) = marked
        .iter()
        .copied()
        .find(|&triangle| triangle >= mesh.triangles.len())
    {
        return Err(format!("marked triangle {invalid} is out of range"));
    }

    let split_edges = marked
        .iter()
        .flat_map(|&triangle| triangle_edges(mesh.triangles[triangle].vertices))
        .collect::<BTreeSet<_>>();

    let mut points = mesh.points.clone();
    let mut midpoint_ids = BTreeMap::new();
    for &(a, b) in &split_edges {
        let midpoint = spherical_centroid_degrees(&[points[a], points[b]])
            .ok_or_else(|| format!("edge {a}/{b} has no unique spherical midpoint"))?;
        midpoint_ids.insert((a, b), points.len());
        points.push(midpoint);
    }

    let mut triangles = Vec::new();
    for (parent_id, parent) in mesh.triangles.iter().enumerate() {
        let relevant_edges = triangle_edges(parent.vertices)
            .into_iter()
            .filter(|candidate| split_edges.contains(candidate))
            .collect::<BTreeSet<_>>();
        let mut children = vec![parent.vertices];

        for split_edge in relevant_edges.iter().copied() {
            let child_id = children
                .iter()
                .position(|vertices| {
                    vertices.contains(&split_edge.0) && vertices.contains(&split_edge.1)
                })
                .ok_or_else(|| {
                    format!(
                        "parent triangle {parent_id} lost split edge {split_edge:?} during closure"
                    )
                })?;
            let child = children.remove(child_id);
            let split = split_triangle_on_edge(child, split_edge, midpoint_ids[&split_edge])
                .ok_or_else(|| {
                    format!("parent triangle {parent_id} cannot split edge {split_edge:?}")
                })?;
            children.insert(child_id, split[1]);
            children.insert(child_id, split[0]);
        }

        let level = parent.level + u8::from(!relevant_edges.is_empty());
        triangles.extend(children.into_iter().map(|vertices| ResearchTriangle {
            vertices,
            level,
            parent: Some(parent_id),
        }));
    }

    Ok((ResearchPrimalMesh { points, triangles }, midpoint_ids))
}

fn validate_primal(mesh: &ResearchPrimalMesh) -> Result<TopologyReport, String> {
    let mut edges = BTreeMap::<Edge, (usize, isize)>::new();
    let mut vertex_neighbors = vec![BTreeSet::new(); mesh.points.len()];

    for (triangle_id, triangle) in mesh.triangles.iter().enumerate() {
        let [a, b, c] = triangle.vertices;
        if a == b || b == c || c == a {
            return Err(format!("triangle {triangle_id} repeats a vertex"));
        }
        if [a, b, c]
            .into_iter()
            .any(|vertex| vertex >= mesh.points.len())
        {
            return Err(format!(
                "triangle {triangle_id} references an invalid vertex"
            ));
        }
        let area = spherical_triangle_area_unit([
            lonlat_degrees_to_unit_xyz(mesh.points[a]),
            lonlat_degrees_to_unit_xyz(mesh.points[b]),
            lonlat_degrees_to_unit_xyz(mesh.points[c]),
        ]);
        if !area.is_finite() || area <= 1.0e-14 {
            return Err(format!("triangle {triangle_id} has zero spherical area"));
        }

        for (from, to) in [(a, b), (b, c), (c, a)] {
            let key = edge(from, to);
            let direction = if (from, to) == key { 1 } else { -1 };
            let entry = edges.entry(key).or_default();
            entry.0 += 1;
            entry.1 += direction;
            vertex_neighbors[from].insert(to);
            vertex_neighbors[to].insert(from);
        }
    }

    for (edge, &(incidence, direction_balance)) in &edges {
        if incidence > 2 {
            return Err(format!("edge {edge:?} is non-manifold"));
        }
        if incidence == 2 && direction_balance != 0 {
            return Err(format!("edge {edge:?} has inconsistent face orientation"));
        }
    }

    let boundary_edge_count = edges
        .values()
        .filter(|&&(incidence, _)| incidence == 1)
        .count();
    let euler_characteristic =
        mesh.points.len() as isize - edges.len() as isize + mesh.triangles.len() as isize;
    let max_vertex_valence = vertex_neighbors
        .iter()
        .map(BTreeSet::len)
        .max()
        .unwrap_or_default();

    Ok(TopologyReport {
        euler_characteristic,
        boundary_edge_count,
        max_vertex_valence,
    })
}

fn octahedron() -> ResearchPrimalMesh {
    let points = vec![
        LonLatDegrees::new(0.0, 90.0),
        LonLatDegrees::new(0.0, -90.0),
        LonLatDegrees::new(0.0, 0.0),
        LonLatDegrees::new(90.0, 0.0),
        LonLatDegrees::new(180.0, 0.0),
        LonLatDegrees::new(-90.0, 0.0),
    ];
    let faces = [
        [0, 2, 3],
        [0, 3, 4],
        [0, 4, 5],
        [0, 5, 2],
        [1, 3, 2],
        [1, 4, 3],
        [1, 5, 4],
        [1, 2, 5],
    ];
    ResearchPrimalMesh {
        points,
        triangles: faces
            .into_iter()
            .map(|vertices| ResearchTriangle {
                vertices,
                level: 0,
                parent: None,
            })
            .collect(),
    }
}

#[test]
fn marked_triangle_refinement_is_conforming_closed_and_deterministic() {
    let mesh = octahedron();
    assert_eq!(
        validate_primal(&mesh).unwrap(),
        TopologyReport {
            euler_characteristic: 2,
            boundary_edge_count: 0,
            max_vertex_valence: 4,
        }
    );

    let marked = BTreeSet::from([0]);
    let first = refine_marked_triangles(&mesh, &marked).unwrap();
    let second = refine_marked_triangles(&mesh, &marked).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.points.len(), 9);
    assert_eq!(first.triangles.len(), 14);
    assert_eq!(
        validate_primal(&first),
        Ok(TopologyReport {
            euler_characteristic: 2,
            boundary_edge_count: 0,
            max_vertex_valence: 6,
        })
    );
    assert_eq!(
        first
            .triangles
            .iter()
            .filter(|triangle| triangle.parent == Some(0))
            .count(),
        4
    );
    assert!(first
        .triangles
        .iter()
        .all(|triangle| triangle.parent.is_some()));
}

#[test]
fn primal_topology_accepts_dynamic_valence_above_legacy_dual_limit() {
    let mut points = vec![LonLatDegrees::new(0.0, 0.0)];
    points.extend((0..8).map(|slot| {
        let angle = std::f64::consts::TAU * slot as f64 / 8.0;
        LonLatDegrees::new(10.0 * angle.cos(), 10.0 * angle.sin())
    }));
    let triangles = (0..8)
        .map(|slot| ResearchTriangle {
            vertices: [0, slot + 1, (slot + 1) % 8 + 1],
            level: 0,
            parent: None,
        })
        .collect();
    let mesh = ResearchPrimalMesh { points, triangles };

    assert_eq!(
        validate_primal(&mesh),
        Ok(TopologyReport {
            euler_characteristic: 1,
            boundary_edge_count: 8,
            max_vertex_valence: 8,
        })
    );
}

#[test]
fn repeated_refinement_preserves_closed_topology_and_parent_levels() {
    let base = octahedron();
    let level_one = refine_marked_triangles(&base, &BTreeSet::from([0])).unwrap();
    let level_two = refine_marked_triangles(&level_one, &BTreeSet::from([0])).unwrap();

    let report = validate_primal(&level_two).unwrap();
    assert_eq!(report.euler_characteristic, 2);
    assert_eq!(report.boundary_edge_count, 0);
    assert_eq!(
        level_two
            .triangles
            .iter()
            .map(|triangle| triangle.level)
            .max(),
        Some(2)
    );
    assert!(level_two
        .triangles
        .iter()
        .all(|triangle| triangle.parent.is_some()));
}

#[test]
fn shared_edge_midpoints_are_inserted_into_obc_order() {
    let base = ResearchPrimalMesh {
        points: vec![
            LonLatDegrees::new(0.0, 0.0),
            LonLatDegrees::new(1.0, 0.0),
            LonLatDegrees::new(1.0, 1.0),
            LonLatDegrees::new(0.0, 1.0),
        ],
        triangles: vec![
            ResearchTriangle {
                vertices: [0, 1, 2],
                level: 0,
                parent: None,
            },
            ResearchTriangle {
                vertices: [0, 2, 3],
                level: 0,
                parent: None,
            },
        ],
    };
    let source = crate::GridfileMeshPoints {
        m_lon: vec![0.0; 4],
        m_lat: vec![0.0; 4],
        w_lon: vec![0.0, 0.0, 0.0, 1.0, 1.0, 0.0],
        w_lat: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0],
        m_to_w: vec![1, 1, 1, 1, 1, 1, 2, 3, 4, 2, 4, 5],
        m_refine_level: Vec::new(),
        m_refine_level_orig: Vec::new(),
        m_ngr: Vec::new(),
        w_to_m: vec![1, 0, 1, 0, 2, 3, 2, 0, 2, 3, 3, 0],
        w_to_m_width: 2,
        n_w: vec![1, 1, 2, 1, 2, 1],
        w_refine_level: Vec::new(),
        w_refine_level_orig: Vec::new(),
        w_ngr: Vec::new(),
    };
    let orders = earthmesh_mesh::classify_boundary_orders_one_based(
        [4, 1, 1],
        &[1, 2, 3, 4],
        &[vec![], vec![], vec![2], vec![3], vec![4]],
        &[0, 0, 1, 1, 1],
        &[0, 0, 2, 3, 4],
        &[0, 0, 1, 1, 1],
    )
    .unwrap();
    let source_segments = source_obc_segments(&source, &[2, 3, 4, 5], &orders.obc_order).unwrap();
    assert_eq!(source_segments, vec![vec![0, 1, 2]]);

    let (refined, passes, midpoint_history) =
        refine_to_targets_with_midpoints(&base, &[1, 0]).unwrap();
    assert_eq!(passes, 1);
    let midpoints = &midpoint_history[0];
    let segments = propagate_obc_segments(source_segments, &midpoint_history);
    assert_eq!(
        segments,
        vec![vec![
            0,
            midpoints[&edge(0, 1)],
            1,
            midpoints[&edge(1, 2)],
            2,
        ]]
    );
    validate_obc_segments(&refined, &segments).unwrap();
    let order = fvcom_obc_order(&segments);
    assert_eq!(
        order,
        vec![
            1,
            2,
            midpoints[&edge(0, 1)] + 2,
            3,
            midpoints[&edge(1, 2)] + 2,
            4,
            1,
        ]
    );
    let (mesh, _) = unstructured_mesh_from_research(&refined).unwrap();
    let output = std::env::temp_dir().join(format!(
        "earthmesh-p2-obc-propagation-{}.2dm",
        std::process::id()
    ));
    let report = crate::fvcom_mesh_writer::write_fvcom_mesh_2dm(&output, &mesh, &order).unwrap();
    assert_eq!(report.boundary_segments, 1);
    assert!(fs::read_to_string(&output).unwrap().contains("\nNS "));
    let _ = fs::remove_file(output);
}

#[test]
fn primal_topology_rejects_degenerate_triangles() {
    let mesh = ResearchPrimalMesh {
        points: vec![LonLatDegrees::new(0.0, 0.0), LonLatDegrees::new(10.0, 0.0)],
        triangles: vec![ResearchTriangle {
            vertices: [0, 1, 1],
            level: 0,
            parent: None,
        }],
    };

    assert_eq!(
        validate_primal(&mesh).unwrap_err(),
        "triangle 0 repeats a vertex"
    );
}

fn research_mesh_from_quality(input: &QualityMeshInput) -> Result<ResearchPrimalMesh, String> {
    research_mesh_from_quality_with_sources(input).map(|(mesh, _)| mesh)
}

fn research_mesh_from_quality_with_sources(
    input: &QualityMeshInput,
) -> Result<(ResearchPrimalMesh, Vec<usize>), String> {
    let mut remap = vec![usize::MAX; input.vertices.len()];
    let mut points = Vec::new();
    let mut source_rows = Vec::new();
    let mut triangles = Vec::with_capacity(input.cells.len());
    for (cell_id, cell) in input.cells.iter().enumerate() {
        if cell.vertices.len() != 3 {
            return Err(format!(
                "quality cell {cell_id} has {} vertices instead of 3",
                cell.vertices.len()
            ));
        }
        let mut vertices = [0_usize; 3];
        for (slot, &source) in cell.vertices.iter().enumerate() {
            let point = input
                .vertices
                .get(source)
                .ok_or_else(|| format!("quality cell {cell_id} references vertex {source}"))?;
            if remap[source] == usize::MAX {
                remap[source] = points.len();
                points.push(LonLatDegrees::new(point.x, point.y));
                source_rows.push(source);
            }
            vertices[slot] = remap[source];
        }
        triangles.push(ResearchTriangle {
            vertices,
            level: u8::try_from(cell.refine_level.unwrap_or_default())
                .map_err(|_| format!("quality cell {cell_id} level does not fit u8"))?,
            parent: None,
        });
    }
    Ok((ResearchPrimalMesh { points, triangles }, source_rows))
}

fn quality_input_from_research(mesh: &ResearchPrimalMesh) -> QualityMeshInput {
    let vertices = mesh
        .points
        .iter()
        .map(|point| Point::new(point.lon_degrees, point.lat_degrees))
        .collect::<Vec<_>>();
    let mut cells = mesh
        .triangles
        .iter()
        .map(|triangle| QualityCell {
            vertices: triangle.vertices.to_vec(),
            refine_level: Some(u32::from(triangle.level)),
            neighbors: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut edge_cells = BTreeMap::<Edge, Vec<usize>>::new();
    for (cell_id, triangle) in mesh.triangles.iter().enumerate() {
        for edge in triangle_edges(triangle.vertices) {
            edge_cells.entry(edge).or_default().push(cell_id);
        }
    }
    for incident in edge_cells.values().filter(|incident| incident.len() == 2) {
        let [left, right] = [incident[0], incident[1]];
        cells[left].neighbors.push(right);
        cells[right].neighbors.push(left);
    }
    QualityMeshInput { vertices, cells }
}

fn unstructured_mesh_from_research(
    mesh: &ResearchPrimalMesh,
) -> io::Result<(crate::unstructured_mesh_support::UnstructuredMesh, Vec<i32>)> {
    let canonical_triangles = mesh.triangles.len() + 1;
    let canonical_vertices = mesh.points.len() + 1;
    let origin = LonLatDegrees::new(0.0, 0.0);
    let mut m_points_canonical = vec![origin; canonical_triangles + 1];
    let mut m_to_w_canonical = vec![[1_usize; 3]; canonical_triangles + 1];
    let mut m_points = vec![crate::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }];
    let mut m_to_w = vec![[1_i32; 3]];
    let mut m_refine_levels = vec![0_i32];

    for (triangle, cell) in mesh.triangles.iter().enumerate() {
        let canonical_id = triangle + 2;
        let vertices = cell.vertices.map(|vertex| vertex + 2);
        let centroid = spherical_centroid_degrees(&cell.vertices.map(|vertex| mesh.points[vertex]))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("triangle {triangle} has no unique spherical centroid"),
                )
            })?;
        m_points_canonical[canonical_id] = centroid;
        m_to_w_canonical[canonical_id] = vertices;
        m_points.push(crate::coordinate_types::LonLatPoint {
            lon: centroid.lon_degrees,
            lat: centroid.lat_degrees,
        });
        m_to_w.push([
            crate::usize_to_i32("P2 W vertex id", vertices[0])?,
            crate::usize_to_i32("P2 W vertex id", vertices[1])?,
            crate::usize_to_i32("P2 W vertex id", vertices[2])?,
        ]);
        m_refine_levels.push(i32::from(cell.level));
    }

    let mut w_points = vec![crate::coordinate_types::LonLatPoint { lon: 0.0, lat: 0.0 }];
    w_points.extend(
        mesh.points
            .iter()
            .map(|point| crate::coordinate_types::LonLatPoint {
                lon: point.lon_degrees,
                lat: point.lat_degrees,
            }),
    );
    let (w_to_m, n_w_to_m) = crate::derive_iap_w_to_m_one_based(
        canonical_vertices,
        &m_to_w_canonical,
        &m_points_canonical,
    )?;
    Ok((
        crate::unstructured_mesh_support::UnstructuredMesh {
            m_points,
            w_points,
            m_to_w,
            w_to_m,
            n_w_to_m,
        },
        m_refine_levels,
    ))
}

#[test]
fn research_primal_gridfile_round_trip_preserves_topology_and_levels() {
    let refined = refine_marked_triangles(&octahedron(), &BTreeSet::from([0])).unwrap();
    let (mesh, levels) = unstructured_mesh_from_research(&refined).unwrap();
    let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&mesh);
    assert!(topology.is_consistent(), "{:?}", topology.violations);
    assert_eq!(topology.euler_characteristic, Some(2));

    let root = std::env::temp_dir().join(format!(
        "earthmesh-p2-gridfile-roundtrip-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let output = root.join("gridfile.nc4");
    crate::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_refine_levels(
        &output,
        &mesh,
        Some(&levels),
        None,
    )
    .unwrap();

    let reloaded = crate::grid_quality_inputs::read_gridfile_mesh_points(&output).unwrap();
    let input = crate::grid_quality_inputs::quality_input_from_gridfile(&reloaded).unwrap();
    assert_eq!(input.cells.len(), refined.triangles.len());
    assert_eq!(input.vertices.len(), refined.points.len() + 1);
    assert_eq!(
        input
            .cells
            .iter()
            .map(|cell| cell.refine_level.unwrap())
            .collect::<Vec<_>>(),
        refined
            .triangles
            .iter()
            .map(|triangle| u32::from(triangle.level))
            .collect::<Vec<_>>()
    );
    let report = earthmesh_quality::compute_with_options(
        &input,
        &QualityThresholds::default(),
        QualityComputationOptions {
            expected_euler_characteristic: Some(2),
            masked_subset: false,
        },
    );
    assert_eq!(report.topology.euler_characteristic_mismatch_count, 0);
    assert!(report.topology_issues.is_empty());
    let _ = fs::remove_dir_all(root);
}

fn target_above_actual(input: &QualityMeshInput, targets: &[u32]) -> BTreeSet<usize> {
    input
        .cells
        .iter()
        .zip(targets)
        .enumerate()
        .filter_map(|(cell_id, (cell, target))| {
            (*target > cell.refine_level.unwrap_or_default()).then_some(cell_id)
        })
        .collect()
}

fn raise_targets_for_unit_jumps(input: &QualityMeshInput, targets: &[u32]) -> Vec<u32> {
    let mut levels = targets.to_vec();
    loop {
        let snapshot = levels.clone();
        let mut changed = false;
        for (cell, neighbors) in input.cells.iter().enumerate() {
            for &neighbor in &neighbors.neighbors {
                let required = snapshot[neighbor].saturating_sub(1);
                if levels[cell] < required {
                    levels[cell] = required;
                    changed = true;
                }
            }
        }
        if !changed {
            return levels;
        }
    }
}

fn refine_to_targets(
    mesh: &ResearchPrimalMesh,
    targets: &[u32],
) -> Result<(ResearchPrimalMesh, usize), String> {
    refine_to_targets_with_midpoints(mesh, targets).map(|(mesh, passes, _)| (mesh, passes))
}

fn refine_to_targets_with_midpoints(
    mesh: &ResearchPrimalMesh,
    targets: &[u32],
) -> Result<(ResearchPrimalMesh, usize, Vec<BTreeMap<Edge, usize>>), String> {
    if mesh.triangles.len() != targets.len() {
        return Err("research mesh/target lengths differ".to_string());
    }
    let mut mesh = mesh.clone();
    let mut targets = targets.to_vec();
    let mut passes = 0;
    let mut midpoint_history = Vec::new();
    loop {
        let marked = mesh
            .triangles
            .iter()
            .zip(&targets)
            .enumerate()
            .filter_map(|(triangle, (cell, target))| {
                (*target > u32::from(cell.level)).then_some(triangle)
            })
            .collect::<BTreeSet<_>>();
        if marked.is_empty() {
            return Ok((mesh, passes, midpoint_history));
        }
        let (refined, midpoint_ids) = refine_marked_triangles_with_midpoints(&mesh, &marked)?;
        targets = refined
            .triangles
            .iter()
            .map(|triangle| targets[triangle.parent.expect("refined child has parent")])
            .collect();
        mesh = refined;
        midpoint_history.push(midpoint_ids);
        passes += 1;
        if passes > 8 {
            return Err("research refinement did not reach its inherited targets".to_string());
        }
    }
}

fn source_obc_segments(
    gridfile_mesh: &crate::GridfileMeshPoints,
    source_vertex_rows: &[usize],
    source_obc_order: &[usize],
) -> Result<Vec<Vec<usize>>, String> {
    let row_layout = crate::gridfile_w_row_layout(gridfile_mesh);
    let row_to_research = source_vertex_rows
        .iter()
        .enumerate()
        .map(|(research, &row)| (row, research))
        .collect::<BTreeMap<_, _>>();
    let mut segments = Vec::new();
    let mut segment = Vec::new();
    for &canonical in source_obc_order {
        if canonical == 1 {
            if !segment.is_empty() {
                segments.push(std::mem::take(&mut segment));
            }
            continue;
        }
        let canonical = i32::try_from(canonical)
            .map_err(|_| format!("OBC vertex id {canonical} does not fit i32"))?;
        let row = row_layout
            .physical_row_for_canonical_id(canonical, gridfile_mesh.w_lon.len())
            .ok_or_else(|| format!("OBC vertex id {canonical} has no physical W row"))?;
        let research = row_to_research.get(&row).copied().ok_or_else(|| {
            format!("OBC W row {row} is not referenced by the source triangle mesh")
        })?;
        segment.push(research);
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    Ok(segments)
}

fn propagate_obc_segments(
    mut segments: Vec<Vec<usize>>,
    midpoint_history: &[BTreeMap<Edge, usize>],
) -> Vec<Vec<usize>> {
    for midpoints in midpoint_history {
        for segment in &mut segments {
            if segment.len() < 2 {
                continue;
            }
            let mut expanded = Vec::with_capacity(segment.len() * 2);
            expanded.push(segment[0]);
            for pair in segment.windows(2) {
                if let Some(&midpoint) = midpoints.get(&edge(pair[0], pair[1])) {
                    expanded.push(midpoint);
                }
                expanded.push(pair[1]);
            }
            *segment = expanded;
        }
    }
    segments
}

fn validate_obc_segments(mesh: &ResearchPrimalMesh, segments: &[Vec<usize>]) -> Result<(), String> {
    let mut incidence = BTreeMap::<Edge, usize>::new();
    for triangle in &mesh.triangles {
        for edge in triangle_edges(triangle.vertices) {
            *incidence.entry(edge).or_default() += 1;
        }
    }
    for segment in segments {
        if segment.len() < 2 {
            return Err("OBC segment has fewer than two vertices".to_string());
        }
        for pair in segment.windows(2) {
            let boundary_edge = edge(pair[0], pair[1]);
            if incidence.get(&boundary_edge) != Some(&1) {
                return Err(format!(
                    "OBC segment edge {boundary_edge:?} is not a final boundary edge"
                ));
            }
        }
    }
    Ok(())
}

fn fvcom_obc_order(segments: &[Vec<usize>]) -> Vec<usize> {
    let mut order = vec![1];
    for segment in segments {
        order.extend(segment.iter().map(|vertex| vertex + 2));
        order.push(1);
    }
    order
}

#[test]
fn hard_targets_are_only_raised_to_form_unit_level_jumps() {
    let cells = vec![
        QualityCell {
            neighbors: vec![1],
            ..QualityCell::default()
        },
        QualityCell {
            neighbors: vec![0, 2],
            ..QualityCell::default()
        },
        QualityCell {
            neighbors: vec![1],
            ..QualityCell::default()
        },
    ];
    let input = QualityMeshInput {
        vertices: Vec::new(),
        cells,
    };
    assert_eq!(raise_targets_for_unit_jumps(&input, &[3, 0, 0]), [3, 2, 1]);
}

#[test]
fn inherited_targets_drive_only_the_required_number_of_passes() {
    let base = octahedron();
    let mut targets = vec![0; base.triangles.len()];
    targets[0] = 2;
    let (refined, passes) = refine_to_targets(&base, &targets).unwrap();
    assert_eq!(passes, 2);
    assert_eq!(
        refined
            .triangles
            .iter()
            .map(|triangle| triangle.level)
            .max(),
        Some(2)
    );
    assert_eq!(validate_primal(&refined).unwrap().euler_characteristic, 2);
}

#[test]
#[ignore = "research-only real OBC fixture; set EARTHMESH_P2_OBC_SOURCE_GRIDFILE and related paths"]
fn clean_ocean_produces_real_obc_fixture_for_p2() {
    let source = PathBuf::from(
        std::env::var_os("EARTHMESH_P2_OBC_SOURCE_GRIDFILE")
            .expect("EARTHMESH_P2_OBC_SOURCE_GRIDFILE"),
    );
    let close_nml = PathBuf::from(
        std::env::var_os("EARTHMESH_P2_OBC_CLOSE_NML").expect("EARTHMESH_P2_OBC_CLOSE_NML"),
    );
    let landtype = PathBuf::from(
        std::env::var_os("EARTHMESH_P2_OBC_LANDTYPE").expect("EARTHMESH_P2_OBC_LANDTYPE"),
    );
    let namelist = fs::read_to_string(
        std::env::var_os("EARTHMESH_P2_OBC_NAMELIST").expect("EARTHMESH_P2_OBC_NAMELIST"),
    )
    .expect("read OBC fixture namelist");
    let work_dir = PathBuf::from(
        std::env::var_os("EARTHMESH_P2_OBC_WORKDIR").expect("EARTHMESH_P2_OBC_WORKDIR"),
    );
    let close = crate::parse_close_mask_nml(close_nml, usize::MAX)
        .expect("parse OBC close mask")
        .expect("OBC close mask enabled");
    let config = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&namelist)
        .expect("parse OBC fixture namelist");
    let nxp = usize::try_from(config.nxp).expect("OBC fixture NXP fits usize");
    let gridnum_perdegree = crate::mkgrd_gridinit_driver::landtype_gridnum_perdegree(&landtype)
        .expect("read OBC landtype resolution");
    let plan = crate::regional_gridfile_writers::write_clean_regional_ocean_gridfile(
        &source,
        &close.points,
        &landtype,
        nxp,
        gridnum_perdegree,
        config.mask_sea_ratio,
        &work_dir,
    )
    .expect("write clean-ocean P2 fixture");
    let obc = plan.obc_output.as_ref().expect("OBC fixture output");
    let order = crate::read_obc_order_netcdf(obc).expect("read OBC fixture order");
    assert!(order.iter().any(|&vertex| vertex != 1));

    let result = serde_json::json!({
        "gridfile": plan.result_gridfile,
        "obc": obc,
        "obc_order_entries": order.len(),
    });
    if let Some(path) = std::env::var_os("EARTHMESH_P2_OBC_OUTPUT") {
        fs::write(path, serde_json::to_vec_pretty(&result).unwrap())
            .expect("write OBC fixture result");
    }
    eprintln!("{}", serde_json::to_string_pretty(&result).unwrap());
}

/// Real-case P2 feasibility test. It remains ignored and requires an existing
/// immutable Case 9 gridfile/source-demand pair; no production path calls it.
#[test]
#[ignore = "research-only real Tri A/B; set EARTHMESH_P2_GRIDFILE and EARTHMESH_P2_NAMELIST"]
fn unmet_tri_case_conforming_primal_refinement_closes_hard_coverage() {
    let gridfile =
        PathBuf::from(std::env::var_os("EARTHMESH_P2_GRIDFILE").expect("EARTHMESH_P2_GRIDFILE"));
    let namelist_path =
        PathBuf::from(std::env::var_os("EARTHMESH_P2_NAMELIST").expect("EARTHMESH_P2_NAMELIST"));
    let namelist = fs::read_to_string(&namelist_path).expect("read Case 9 namelist");
    let gridfile_mesh =
        crate::grid_quality_inputs::read_gridfile_mesh_points(&gridfile).expect("read Case 9 mesh");
    let base_input = crate::grid_quality_inputs::quality_input_from_gridfile(&gridfile_mesh)
        .expect("build Case 9 triangle view");
    let demand = crate::source_demand_artifact::load_hfield_source_demand(&gridfile, &namelist)
        .expect("load immutable Case 9 source demand");
    let (base_targets, base_coverage) =
        crate::grid_quality_inputs::hfield_support_coverage::target_levels_with_hard_coverage(
            &base_input,
            demand.nlon,
            demand.nlat,
            &demand.hard_levels,
            &demand.hard_levels,
            &demand.intended_output_support,
        )
        .expect("evaluate Case 9 baseline coverage");
    let marked = target_above_actual(&base_input, &base_targets);
    assert!(
        !marked.is_empty(),
        "Case 9 baseline unexpectedly has full coverage"
    );

    let (base, source_vertex_rows) =
        research_mesh_from_quality_with_sources(&base_input).expect("build research primal");
    let base_topology = validate_primal(&base).expect("validate baseline primal");
    let base_quality = earthmesh_quality::compute(&base_input, &QualityThresholds::default());
    let graded_targets = raise_targets_for_unit_jumps(&base_input, &base_targets);
    let marked_centers = marked
        .iter()
        .map(|&cell| {
            let points = base_input.cells[cell]
                .vertices
                .iter()
                .map(|&vertex| {
                    let point = base_input.vertices[vertex];
                    LonLatDegrees::new(point.x, point.y)
                })
                .collect::<Vec<_>>();
            let center =
                spherical_centroid_degrees(&points).expect("target cell has spherical centroid");
            [center.lon_degrees, center.lat_degrees]
        })
        .collect::<Vec<_>>();

    let (refined, passes, midpoint_history) =
        refine_to_targets_with_midpoints(&base, &graded_targets).expect("refine Case 9 primal");
    let (repeated, repeated_passes, repeated_midpoints) =
        refine_to_targets_with_midpoints(&base, &graded_targets).expect("repeat Case 9 primal");
    assert_eq!(passes, repeated_passes);
    assert_eq!(midpoint_history, repeated_midpoints);
    assert_eq!(refined, repeated, "P2 refinement is not deterministic");
    let refined_topology = validate_primal(&refined).expect("validate refined primal");
    assert_eq!(
        refined_topology.euler_characteristic,
        base_topology.euler_characteristic
    );

    let refined_input = quality_input_from_research(&refined);
    let (refined_targets, refined_coverage) =
        crate::grid_quality_inputs::hfield_support_coverage::target_levels_with_hard_coverage(
            &refined_input,
            demand.nlon,
            demand.nlat,
            &demand.hard_levels,
            &demand.hard_levels,
            &demand.intended_output_support,
        )
        .expect("evaluate refined Case 9 coverage");
    let remaining = target_above_actual(&refined_input, &refined_targets);
    let quality = earthmesh_quality::compute_with_options(
        &refined_input,
        &QualityThresholds::default(),
        QualityComputationOptions {
            expected_euler_characteristic: Some(base_topology.euler_characteristic),
            masked_subset: true,
        },
    );

    assert_eq!(
        refined_coverage.adequately_covered_bin_count, refined_coverage.active_bin_count,
        "P2 did not cover every active hard bin"
    );
    assert!(
        remaining.is_empty(),
        "P2 left {} cells below target",
        remaining.len()
    );
    assert_eq!(quality.topology.euler_characteristic_mismatch_count, 0);
    assert_eq!(
        quality.topology.connected_component_count,
        base_quality.topology.connected_component_count
    );
    assert_eq!(
        quality.topology.boundary_loop_count,
        base_quality.topology.boundary_loop_count
    );
    assert_eq!(quality.topology.non_manifold_vertex_fan_count, 0);
    assert_eq!(quality.topology.dangling_edge_count, 0);
    assert_eq!(quality.topology.misoriented_shared_edge_count, 0);
    assert_eq!(quality.geometry.self_intersection_count, 0);
    assert_eq!(quality.geometry.invalid_polygon_count, 0);
    assert!(
        quality.topology_issues.is_empty(),
        "P2 topology issues: {:?}",
        quality
            .topology_issues
            .iter()
            .map(|issue| (issue.issue_type.as_str(), issue.message.as_str()))
            .collect::<Vec<_>>()
    );

    let written_gridfile = std::env::var_os("EARTHMESH_P2_WRITTEN_GRIDFILE").map(|path| {
        let path = PathBuf::from(path);
        let (mesh, levels) =
            unstructured_mesh_from_research(&refined).expect("build research gridfile mesh");
        let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&mesh);
        assert!(
            topology.is_consistent(),
            "research gridfile topology: {:?}",
            topology.violations
        );
        assert_eq!(
            topology.euler_characteristic,
            Some(base_topology.euler_characteristic)
        );
        crate::unstructured_mesh_io::write_unstructured_mesh_netcdf_with_refine_levels(
            &path,
            &mesh,
            Some(&levels),
            None,
        )
        .expect("write research gridfile");

        let reloaded_mesh = crate::grid_quality_inputs::read_gridfile_mesh_points(&path)
            .expect("reload research gridfile");
        let reloaded = crate::grid_quality_inputs::quality_input_from_gridfile(&reloaded_mesh)
            .expect("build reloaded triangle view");
        assert_eq!(reloaded.cells.len(), refined_input.cells.len());
        let (reloaded_targets, reloaded_coverage) =
            crate::grid_quality_inputs::hfield_support_coverage::target_levels_with_hard_coverage(
                &reloaded,
                demand.nlon,
                demand.nlat,
                &demand.hard_levels,
                &demand.hard_levels,
                &demand.intended_output_support,
            )
            .expect("evaluate reloaded coverage");
        let reloaded_remaining = target_above_actual(&reloaded, &reloaded_targets);
        let reloaded_quality = earthmesh_quality::compute_with_options(
            &reloaded,
            &QualityThresholds::default(),
            QualityComputationOptions {
                expected_euler_characteristic: Some(base_topology.euler_characteristic),
                masked_subset: true,
            },
        );
        assert_eq!(
            reloaded_coverage.adequately_covered_bin_count,
            reloaded_coverage.active_bin_count
        );
        assert!(reloaded_remaining.is_empty());
        assert_eq!(
            reloaded_quality.topology.connected_component_count,
            quality.topology.connected_component_count
        );
        assert_eq!(
            reloaded_quality.topology.boundary_loop_count,
            quality.topology.boundary_loop_count
        );
        assert_eq!(
            reloaded_quality
                .topology
                .euler_characteristic_mismatch_count,
            0
        );
        assert_eq!(reloaded_quality.geometry.self_intersection_count, 0);
        assert_eq!(reloaded_quality.geometry.invalid_polygon_count, 0);
        assert!(reloaded_quality.topology_issues.is_empty());
        let source_obc = std::env::var_os("EARTHMESH_P2_SOURCE_OBC").map(PathBuf::from);
        let fvcom = std::env::var_os("EARTHMESH_P2_WRITTEN_FVCOM").map(|output| {
            let (report, obc_segments) = if let Some(source_obc) = source_obc.as_ref() {
                let source_order =
                    crate::read_obc_order_netcdf(source_obc).expect("read source OBC order");
                let segments =
                    source_obc_segments(&gridfile_mesh, &source_vertex_rows, &source_order)
                        .expect("map source OBC order");
                let segments = propagate_obc_segments(segments, &midpoint_history);
                validate_obc_segments(&refined, &segments).expect("validate refined OBC order");
                let order = fvcom_obc_order(&segments);
                (
                    crate::fvcom_mesh_writer::write_fvcom_mesh_2dm(output, &mesh, &order)
                        .expect("write FVCOM research adapter output with OBC"),
                    segments.len(),
                )
            } else {
                (
                    crate::regional_gridfile_writers::write_standard_fvcom_from_gridfile(
                        &path, output,
                    )
                    .expect("write FVCOM research adapter output"),
                    0,
                )
            };
            assert_eq!(report.triangles, reloaded.cells.len());
            assert_eq!(report.nodes + 1, reloaded.vertices.len());
            serde_json::json!({
                "path": report.output,
                "triangles": report.triangles,
                "nodes": report.nodes,
                "obc_segments": obc_segments,
                "boundary_segments": report.boundary_segments,
            })
        });
        serde_json::json!({
            "path": path,
            "cells": reloaded.cells.len(),
            "active_hard_bins": reloaded_coverage.active_bin_count,
            "adequately_covered_hard_bins": reloaded_coverage.adequately_covered_bin_count,
            "target_above_actual_cells": reloaded_remaining.len(),
            "euler_characteristic": reloaded_quality.topology.euler_characteristic,
            "boundary_edges": reloaded_quality.topology.boundary_edge_count,
            "verdict": format!("{:?}", reloaded_quality.verdict).to_ascii_lowercase(),
            "fvcom_adapter": fvcom,
        })
    });

    let result = serde_json::json!({
        "kind": "earthmesh_p2_primal_refine_research",
        "gridfile": gridfile,
        "baseline": {
            "cells": base_input.cells.len(),
            "active_hard_bins": base_coverage.active_bin_count,
            "adequately_covered_hard_bins": base_coverage.adequately_covered_bin_count,
            "target_above_actual_cells": marked.len(),
            "target_above_actual_centers": marked_centers,
            "graded_target_above_actual_cells": target_above_actual(&base_input, &graded_targets).len(),
        },
        "refined": {
            "cells": refined_input.cells.len(),
            "added_cells": refined_input.cells.len() - base_input.cells.len(),
            "passes": passes,
            "active_hard_bins": refined_coverage.active_bin_count,
            "adequately_covered_hard_bins": refined_coverage.adequately_covered_bin_count,
            "target_above_actual_cells": remaining.len(),
            "euler_characteristic": quality.topology.euler_characteristic,
            "boundary_edges": quality.topology.boundary_edge_count,
            "max_vertex_valence": refined_topology.max_vertex_valence,
            "verdict": format!("{:?}", quality.verdict).to_ascii_lowercase(),
        },
        "written_gridfile": written_gridfile,
    });
    if let Some(path) = std::env::var_os("EARTHMESH_P2_OUTPUT") {
        fs::write(path, serde_json::to_vec_pretty(&result).unwrap()).expect("write P2 result");
    }
    eprintln!("{}", serde_json::to_string_pretty(&result).unwrap());
}

#[test]
#[ignore = "research-only real tri negative control; set EARTHMESH_P2_CONTROL_GRIDFILE and EARTHMESH_P2_CONTROL_NAMELIST"]
fn satisfied_tri_control_is_a_topology_preserving_noop() {
    let gridfile = PathBuf::from(
        std::env::var_os("EARTHMESH_P2_CONTROL_GRIDFILE").expect("EARTHMESH_P2_CONTROL_GRIDFILE"),
    );
    let namelist_path = PathBuf::from(
        std::env::var_os("EARTHMESH_P2_CONTROL_NAMELIST").expect("EARTHMESH_P2_CONTROL_NAMELIST"),
    );
    let gridfile_mesh = crate::grid_quality_inputs::read_gridfile_mesh_points(&gridfile)
        .expect("read control mesh");
    let input = crate::grid_quality_inputs::quality_input_from_gridfile(&gridfile_mesh)
        .expect("build control triangle view");
    let namelist = fs::read_to_string(namelist_path).expect("read control namelist");
    let demand = crate::source_demand_artifact::load_hfield_source_demand(&gridfile, &namelist)
        .expect("load control source demand");
    let (targets, coverage) =
        crate::grid_quality_inputs::hfield_support_coverage::target_levels_with_hard_coverage(
            &input,
            demand.nlon,
            demand.nlat,
            &demand.hard_levels,
            &demand.hard_levels,
            &demand.intended_output_support,
        )
        .expect("evaluate control coverage");
    assert!(
        target_above_actual(&input, &targets).is_empty(),
        "negative control unexpectedly requires P2 refinement"
    );
    assert_eq!(
        coverage.adequately_covered_bin_count, coverage.active_bin_count,
        "negative control has unmet hard demand"
    );

    let base = research_mesh_from_quality(&input).expect("build control research primal");
    let topology = validate_primal(&base).expect("validate control primal");
    let graded_targets = raise_targets_for_unit_jumps(&input, &targets);
    let (refined, passes) =
        refine_to_targets(&base, &graded_targets).expect("evaluate control refinement");
    assert_eq!(passes, 0);
    assert_eq!(refined, base);
    assert_eq!(validate_primal(&refined), Ok(topology));

    let result = serde_json::json!({
        "kind": "earthmesh_p2_primal_refine_negative_control",
        "gridfile": gridfile,
        "cells": input.cells.len(),
        "active_hard_bins": coverage.active_bin_count,
        "adequately_covered_hard_bins": coverage.adequately_covered_bin_count,
        "passes": passes,
        "changed": refined != base,
        "euler_characteristic": topology.euler_characteristic,
        "boundary_edges": topology.boundary_edge_count,
        "max_vertex_valence": topology.max_vertex_valence,
    });
    if let Some(path) = std::env::var_os("EARTHMESH_P2_CONTROL_OUTPUT") {
        fs::write(path, serde_json::to_vec_pretty(&result).unwrap())
            .expect("write P2 control result");
    }
    eprintln!("{}", serde_json::to_string_pretty(&result).unwrap());
}
