//! Between the red-green refinement tables and the gridfile's mesh.
//!
//! They are the same five tables under different names -- the writer's
//! `m_to_w`/`w_to_m` are the pipeline's `ngrmw`/`ngrwm` -- so this is a
//! conversion rather than a translation. What it does have to do is police the
//! width: ids are `usize` on one side and `i32` on the other, and a mesh too
//! large to address in `i32` has to say so here rather than wrap silently into
//! a gridfile that reads as valid.

use std::io;

use earthmesh_refine_redgreen::RedGreenMesh;
use rayon::prelude::*;

use crate::coordinate_types::LonLatPoint;
use crate::unstructured_mesh_support::UnstructuredMesh;

fn narrow(id: usize, role: &str) -> io::Result<i32> {
    i32::try_from(id).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{role} id {id} exceeds what a gridfile can address"),
        )
    })
}

/// The refined mesh, in the shape the gridfile writer takes.
pub fn unstructured_mesh_from_redgreen(mesh: &RedGreenMesh) -> io::Result<UnstructuredMesh> {
    let point = |p: &earthmesh_mesh::LonLatDegrees| LonLatPoint {
        lon: p.lon_degrees,
        lat: p.lat_degrees,
    };
    let mut m_to_w = Vec::with_capacity(mesh.cells_on_triangle.len());
    for corners in &mesh.cells_on_triangle {
        m_to_w.push([
            narrow(corners[0], "cell")?,
            narrow(corners[1], "cell")?,
            narrow(corners[2], "cell")?,
        ]);
    }
    let mut w_to_m = Vec::with_capacity(mesh.triangles_on_cell.len());
    for row in &mesh.triangles_on_cell {
        let mut converted = Vec::with_capacity(row.len());
        for &triangle in row {
            converted.push(narrow(triangle, "triangle")?);
        }
        w_to_m.push(converted);
    }
    let mut n_w_to_m = Vec::with_capacity(mesh.n_triangles_on_cell.len());
    for &count in &mesh.n_triangles_on_cell {
        n_w_to_m.push(narrow(count, "triangle count")?);
    }

    Ok(UnstructuredMesh {
        m_points: mesh.triangle_points.iter().map(point).collect(),
        w_points: mesh.cell_points.iter().map(point).collect(),
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refined_mesh_arrives_with_every_table_intact() {
        // The tables are the same five under different names, so the test that
        // matters is that none of them loses a row or a slot on the way.
        let mesh = earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        let redgreen = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&mesh, &neighbors)
            .expect("bridge in");

        let written = unstructured_mesh_from_redgreen(&redgreen).expect("bridge out");

        assert_eq!(written.m_points.len(), redgreen.triangle_points.len());
        assert_eq!(written.w_points.len(), redgreen.cell_points.len());
        assert_eq!(written.m_to_w.len(), redgreen.cells_on_triangle.len());
        assert_eq!(written.w_to_m.len(), redgreen.triangles_on_cell.len());
        assert_eq!(
            written.m_to_w[2],
            redgreen.cells_on_triangle[2].map(|id| id as i32),
            "a triangle's corners survive the narrowing"
        );
        assert_eq!(
            written.n_w_to_m[2] as usize,
            redgreen.n_triangles_on_cell[2]
        );
    }

    #[test]
    fn an_id_too_wide_for_the_gridfile_is_refused_rather_than_wrapped() {
        // Silently wrapping would produce a gridfile that reads as valid and
        // points at the wrong cells.
        let mut mesh = RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points: vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 3],
            cell_points: vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 3],
            cells_on_triangle: vec![[1, 1, 1]; 3],
            triangles_on_cell: vec![Vec::new(); 3],
            n_triangles_on_cell: vec![0; 3],
        };
        mesh.cells_on_triangle[2] = [1, 1, i32::MAX as usize + 1];

        let error = unstructured_mesh_from_redgreen(&mesh)
            .expect_err("an unaddressable id must not wrap into a gridfile");
        assert!(
            error.to_string().contains("exceeds what a gridfile"),
            "{error}"
        );
    }
}

/// The engine's refinement settings, as this level's red-green run reads them.
///
/// `halo` and `max_transition_row` are per-level in the namelist -- v2's
/// `HALO = 3, 3, 3` -- so the level picks its own entry. A level past the end of
/// the array reuses the last one that was given rather than silently falling
/// back to a default: the array is how the user said "these levels", and
/// running a deeper level on a number nobody wrote would be inventing one.
pub fn redgreen_settings_for_level(
    refine: &earthmesh_core::RefineConfig,
    level: usize,
) -> earthmesh_refine_redgreen::RedGreenSettings {
    let at_level = |values: &[i32; 10]| -> usize {
        let index = level.max(1).min(values.len()) - 1;
        let chosen = values[index..]
            .iter()
            .rev()
            .find(|&&value| value > 0)
            .copied()
            .unwrap_or(values[index]);
        let value = if values[index] > 0 {
            values[index]
        } else {
            chosen
        };
        value.max(0) as usize
    };
    earthmesh_refine_redgreen::RedGreenSettings {
        max_transition_row: at_level(&refine.max_transition_row).max(1),
        build_transition_rows: refine.is_transition,
        eliminate_weak_concavity: refine.weak_concav_eliminate,
        halo: at_level(&refine.halo),
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn each_level_reads_its_own_halo_and_transition_width() {
        // The namelist gives these per level -- v2's HALO = 3, 3, 3 -- so a
        // three-level run that narrows the band as it deepens has to be read
        // that way, not collapsed to one number.
        let refine = earthmesh_core::RefineConfig {
            halo: [4, 3, 2, 0, 0, 0, 0, 0, 0, 0],
            max_transition_row: [3, 2, 1, 0, 0, 0, 0, 0, 0, 0],
            ..earthmesh_core::RefineConfig::default()
        };

        assert_eq!(redgreen_settings_for_level(&refine, 1).halo, 4);
        assert_eq!(redgreen_settings_for_level(&refine, 2).halo, 3);
        assert_eq!(redgreen_settings_for_level(&refine, 3).halo, 2);
        assert_eq!(
            redgreen_settings_for_level(&refine, 3).max_transition_row,
            1
        );
    }

    #[test]
    fn a_transition_width_of_zero_still_leaves_one_row() {
        // Zero rows is not a mesh this path can build -- the driver rejects it
        // -- and the namelist's zeros are "levels not configured", not "no
        // transition". Clamping here keeps that from reading as a request.
        let refine = earthmesh_core::RefineConfig::default();
        assert!(redgreen_settings_for_level(&refine, 9).max_transition_row >= 1);
    }
}

/// Which triangles a level's regions ask for, one entry per triangle.
///
/// The marking is the whole interface between "what the project wants" and
/// "what red-green builds": any set of triangles is legal input, and the judge
/// chain grows it until the triangulation closes. That is why this can be a
/// containment test and nothing more -- there is no shape to satisfy.
///
/// A triangle is asked for when its own centre falls inside a region. Centre
/// sampling is the same rule the ocean carve uses, so a cell is refined and
/// kept, or neither, rather than refined and then carved away.
pub fn redgreen_marking_from_regions(
    mesh: &earthmesh_refine_redgreen::RedGreenMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    level: usize,
) -> Vec<i32> {
    let mut marking = vec![0i32; mesh.triangle_count() + 1];
    if regions.is_empty() {
        return marking;
    }
    let region_index = earthmesh_mesh::RefinementRegionIndex::new(regions);
    marking
        .par_iter_mut()
        .enumerate()
        .skip(mesh.num_vertex + 1)
        .for_each(|(triangle, mark)| {
            let centre = mesh.triangle_points[triangle];
            *mark = i32::from(region_index.contains_lonlat_canonical(centre, level));
        });
    marking
}

#[cfg(test)]
mod marking_tests {
    use super::*;
    use earthmesh_mesh::{LonLatDegrees, RefinementRegion};

    fn base() -> earthmesh_refine_redgreen::RedGreenMesh {
        let mesh = earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&mesh, &neighbors).expect("bridge")
    }

    #[test]
    fn a_circle_marks_the_triangles_whose_centres_it_holds() {
        let mesh = base();
        let marking = redgreen_marking_from_regions(
            &mesh,
            &[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 2_000_000.0,
                level: 1,
            }],
            1,
        );

        let marked = marking.iter().filter(|&&value| value == 1).count();
        assert!(marked > 0, "a circle this size must hold some triangle");
        assert!(
            marked < mesh.triangle_count(),
            "and must not hold the whole globe: {marked} of {}",
            mesh.triangle_count()
        );
        assert_eq!(marking[0], 0, "slot 0 is not a triangle");
        assert_eq!(
            marking[1], 0,
            "slot 1 is the canonical placeholder and is never asked for"
        );
    }

    #[test]
    fn a_region_shallower_than_this_level_asks_for_nothing_here() {
        // A level-1 circle is served by level 1 and must not reappear at level
        // 2, or every level would refine everything the one above it did.
        let mesh = base();
        let regions = [RefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 2_000_000.0,
            level: 1,
        }];

        assert!(redgreen_marking_from_regions(&mesh, &regions, 1).contains(&1));
        assert!(redgreen_marking_from_regions(&mesh, &regions, 2)
            .iter()
            .all(|&value| value == 0));
    }

    #[test]
    fn marking_is_identical_across_thread_counts() {
        let mesh = base();
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(179.0, 0.0),
                radius_meters: 2_000_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(-45.0, 80.0),
                radius_meters: 1_000_000.0,
                level: 1,
            },
        ];
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| redgreen_marking_from_regions(&mesh, &regions, 1))
        };

        assert_eq!(run(1), run(4));
    }
}

/// One red-green level, from the regions that asked for it to a mesh the
/// gridfile writer takes.
///
/// `previous_level_marks` is the level above's marking **in this mesh's
/// numbering**. A round renumbers, so a caller chaining levels has to carry it
/// through `RefineNgrRenewCore::vertex_mapping` rather than reusing the array
/// it built last time -- which is why this takes one level and returns, instead
/// of looping internally over a mapping it would have to guess at.
pub fn refine_redgreen_level(
    mesh: &earthmesh_refine_redgreen::RedGreenMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    refine: &earthmesh_core::RefineConfig,
    level: usize,
    previous_level_marks: Option<&[i32]>,
    preserve_locality: bool,
) -> io::Result<(UnstructuredMesh, earthmesh_refine_redgreen::RedGreenOutcome)> {
    let marking = redgreen_marking_from_regions(mesh, regions, level);
    let mut settings = redgreen_settings_for_level(refine, level);
    let mut outcome = earthmesh_refine_redgreen::refine_redgreen_round_inside(
        mesh,
        &marking,
        &settings,
        previous_level_marks,
    )?;
    let refinable = mesh.triangle_count().saturating_sub(mesh.num_vertex);
    if preserve_locality
        && settings.eliminate_weak_concavity
        && outcome.grown_triangle_count > 0
        && outcome.refined_triangle_count == refinable
    {
        settings.eliminate_weak_concavity = false;
        outcome = earthmesh_refine_redgreen::refine_redgreen_round_inside(
            mesh,
            &marking,
            &settings,
            previous_level_marks,
        )?;
        eprintln!(
            "earthmesh_cli: Red-Green weak-concavity elimination reached the whole triangular domain; carrying the boundary concavities instead"
        );
    }
    let written = unstructured_mesh_from_redgreen(&outcome.mesh)?;
    Ok((written, outcome))
}

#[cfg(test)]
mod level_tests {
    use super::*;
    use earthmesh_mesh::{LonLatDegrees, RefinementRegion};

    #[test]
    fn a_named_circle_refines_and_arrives_as_a_writable_mesh() {
        // The chain end to end: regions -> marking -> round -> gridfile tables.
        // Every link has its own test; this is the one that says they compose.
        let base = earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        let before = mesh.triangle_count();

        let (written, outcome) = refine_redgreen_level(
            &mesh,
            &[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            }],
            &earthmesh_core::RefineConfig::default(),
            1,
            None,
            false,
        )
        .expect("one red-green level");

        assert!(
            outcome.refined_triangle_count > 0,
            "the circle asked for triangles: {outcome:?}"
        );
        assert!(
            outcome.mesh.triangle_count() > before,
            "a refined mesh has more triangles: {} vs {before}",
            outcome.mesh.triangle_count()
        );
        assert_eq!(
            written.m_to_w.len(),
            outcome.mesh.cells_on_triangle.len(),
            "and arrives whole"
        );
    }

    /// A refined mesh has to reach the writer with both arrays on the same row
    /// layout.
    ///
    /// A gridfile reader picks between the compact layout (id = row + 1) and the
    /// two-placeholder one (id = row) by whether rows 0 and 1 sit at the origin
    /// -- and it picks per array. The renewal left slot 0 of the cell points at
    /// its 9999 "no vertex here yet" sentinel, so the cell array read as compact
    /// while the triangle array read as two-placeholder. The file opened, passed
    /// its checks, and resolved every connectivity id one row off.
    #[test]
    fn a_refined_mesh_keeps_both_arrays_on_the_same_row_layout() {
        let base = earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");

        let (written, _) = refine_redgreen_level(
            &mesh,
            &[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            }],
            // With the transition rows off the round leaves hanging nodes by
            // design -- only the hexagonal dual reads such a mesh -- so the
            // triangular view is only a mesh to check when they are on.
            &earthmesh_core::RefineConfig {
                is_transition: true,
                ..earthmesh_core::RefineConfig::default()
            },
            1,
            None,
            false,
        )
        .expect("one red-green level");

        let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&written);
        assert!(
            topology.is_consistent(),
            "a refined red-green mesh must reach the writer as one mesh: {:?}",
            &topology.violations[..topology.violations.len().min(4)]
        );
    }

    /// How a caller chains levels, and why the previous marking is asked for
    /// again rather than carried.
    ///
    /// A round renumbers *triangles*, so the array handed to level 1 indexes a
    /// mesh that no longer exists by level 2 -- it is not even the right
    /// length. `RedGreenOutcome::cell_renumbering` cannot bridge that: it maps
    /// cells, and a marking is per triangle. Asking the regions again on the
    /// refined mesh is the same question in the numbering the next level will
    /// be asked about.
    #[test]
    fn a_deeper_level_is_held_inside_the_one_above_it() {
        let base = earthmesh_mesh::TriangularMesh::from_icosahedron(9, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        // Both levels ask for the same disc, so level 2 reaches all the way out
        // to level 1's boundary and the halo has something to cancel.
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 2,
            },
        ];
        // The transition rows have to be built for a level to be chainable at
        // all -- without them the round leaves hanging nodes and the next one
        // cannot even derive the triangle neighbours -- and the halo is what
        // holds the deeper level inside.
        let refine = earthmesh_core::RefineConfig {
            is_transition: true,
            halo: [3; 10],
            max_transition_row: [3; 10],
            ..earthmesh_core::RefineConfig::default()
        };

        let (_, first) =
            refine_redgreen_level(&mesh, &regions, &refine, 1, None, false).expect("level one");
        let previous = redgreen_marking_from_regions(&first.mesh, &regions, 1);
        assert_eq!(
            previous.len(),
            first.mesh.triangle_count() + 1,
            "the marking a level reads must be sized to the mesh that level sees"
        );
        assert_ne!(
            previous.len(),
            first.cell_renumbering.len(),
            "and cell_renumbering is not that mapping -- it is per cell"
        );

        let (_, held) =
            refine_redgreen_level(&first.mesh, &regions, &refine, 2, Some(&previous), false)
                .expect("level two, held inside level one");
        let (_, free) = refine_redgreen_level(&first.mesh, &regions, &refine, 2, None, false)
            .expect("level two, free");

        assert!(
            held.halo_cancelled_count > 0,
            "a level reaching its parent's boundary must be pulled back inside: {held:?}"
        );
        assert_eq!(free.halo_cancelled_count, 0, "and only when asked to be");
        assert!(
            held.refined_triangle_count < free.refined_triangle_count,
            "so it refines less: {} vs {}",
            held.refined_triangle_count,
            free.refined_triangle_count
        );
    }

    #[test]
    fn triangular_red_green_does_not_turn_distributed_demand_into_global_refinement() {
        let base = earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        let refine = earthmesh_core::RefineConfig::default();
        let refinable = mesh.triangle_count() - mesh.num_vertex;
        let mut fixture = None;
        'search: for step in [45, 30, 20] {
            for radius_meters in [1_500_000.0, 2_000_000.0, 2_500_000.0] {
                let regions = [-60.0, 0.0, 60.0]
                    .into_iter()
                    .flat_map(|lat| {
                        (-180..180)
                            .step_by(step)
                            .map(move |lon| RefinementRegion::Circle {
                                center: LonLatDegrees::new(f64::from(lon), lat),
                                radius_meters,
                                level: 1,
                            })
                    })
                    .collect::<Vec<_>>();
                let (_, filled) = refine_redgreen_level(&mesh, &regions, &refine, 1, None, false)
                    .expect("filled run");
                if filled.refined_triangle_count == refinable && filled.grown_triangle_count > 0 {
                    fixture = Some((regions, filled));
                    break 'search;
                }
            }
        }
        let (regions, filled) = fixture.expect("a distributed marking must exercise the fallback");

        let (written, local) =
            refine_redgreen_level(&mesh, &regions, &refine, 1, None, true).expect("local run");
        assert!(
            local.refined_triangle_count < filled.refined_triangle_count,
            "distributed local demand must retain a coarse exterior"
        );
        let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&written);
        assert!(
            topology
                .violations
                .iter()
                .all(|violation| !violation.starts_with("misoriented_shared_edge")),
            "the carried transition must stay consistently oriented: {:?}",
            topology.violations
        );
    }
}
