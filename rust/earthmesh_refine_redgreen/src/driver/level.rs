//! One refinement level: the settings it runs with, the triangles it marks,
//! and the round that splits them.

use std::io;

use rayon::prelude::*;

// Geometry contract, independent of the configurable warn/fail quality policy.
const TRIANGLE_SHAPE_FLOOR_DEG: f64 = 25.0;
/// Green floor for a demand that nests by construction (the h-field, and
/// named regions with criteria circles once every level's circles are marked
/// together, 2026-09-28). The
/// derived floor -- half the red leaves' smallest angle, 26.31 degrees on the
/// global coast case -- rejected about half the greens and each rejection
/// cascaded outward as red splits; the final angle-window repair now owns the
/// published angles, so closure only has to avoid needles. Measured on Case9
/// (guide 11.71): 151,427 -> 131,752 cells, finer than target 27,641 -> 2,812,
/// coarser 262 -> 402, still 35-85 after repair. Criteria circles marked one
/// level at a time did not nest, and there the over-refinement was holding up
/// deeper demand (coarser than target rose 32,829 -> 44,975 at 20 degrees);
/// marked together they nest and take this floor too (guide 11.78).
const NESTED_DEMAND_GREEN_FLOOR_DEG: f64 = 20.0;

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
) -> crate::RedGreenSettings {
    let defaults = crate::RedGreenSettings::default();
    let at_level = |values: &[i32; 10], fallback: usize| -> usize {
        let index = level.max(1).min(values.len() - 1);
        (values[index] > 0)
            .then_some(values[index])
            .or_else(|| {
                values[1..index]
                    .iter()
                    .rev()
                    .find(|&&value| value > 0)
                    .copied()
            })
            .map(|value| value as usize)
            .unwrap_or(fallback)
    };
    crate::RedGreenSettings {
        max_transition_row: at_level(&refine.max_transition_row, defaults.max_transition_row),
        build_transition_rows: refine.is_transition,
        eliminate_weak_concavity: refine.weak_concav_eliminate,
        halo: at_level(&refine.halo, defaults.halo),
        protect_triangle_quality: false,
        min_triangle_angle_deg: defaults.min_triangle_angle_deg,
        green_floor_deg: defaults.green_floor_deg,
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
            halo: [0, 4, 3, 2, 0, 0, 0, 0, 0, 0],
            max_transition_row: [0, 3, 2, 1, 0, 0, 0, 0, 0, 0],
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
    fn omitted_redgreen_bands_use_the_algorithm_defaults() {
        let refine = earthmesh_core::RefineConfig::default();
        let settings = redgreen_settings_for_level(&refine, 1);
        assert_eq!(settings.halo, 3);
        assert_eq!(settings.max_transition_row, 3);
    }

    #[test]
    fn levels_past_the_configured_prefix_reuse_the_last_value() {
        let refine = earthmesh_core::RefineConfig {
            halo: [0, 4, 2, 0, 0, 0, 0, 0, 0, 0],
            max_transition_row: [0, 5, 3, 0, 0, 0, 0, 0, 0, 0],
            ..earthmesh_core::RefineConfig::default()
        };

        let settings = redgreen_settings_for_level(&refine, 4);
        assert_eq!(settings.halo, 2);
        assert_eq!(settings.max_transition_row, 3);
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
    mesh: &crate::RedGreenMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    level: usize,
) -> Vec<i32> {
    redgreen_marking(
        mesh,
        &earthmesh_refine::RegionTargets::new(regions),
        level,
        false,
    )
    .expect("region containment cannot fail")
}

/// Mark every triangle whose centre the demand asks to be at least `level`
/// deep. The demand is read only as a point query, so named regions, criteria
/// circles and the h-field all mark the same way.
///
/// With `split_children`, a triangle is also marked when the demand holds at
/// the centre of any half a green closure could cut it into. An unmarked
/// triangle beside the refinement is bisected, and its halves are faces of the
/// final mesh at its own depth: the h-field's quality check reads the target
/// at every face centre (a hex cell's corners), so a half whose centre falls
/// inside the demand is a cell below its target even though the parent's
/// centre was outside -- the one warn left on a global 1000 km circle.
pub fn redgreen_marking(
    mesh: &crate::RedGreenMesh,
    targets: &dyn earthmesh_refine::TargetLevelField,
    level: usize,
    split_children: bool,
) -> io::Result<Vec<i32>> {
    let mut marking = vec![0i32; mesh.triangle_count() + 1];
    if !targets.demands_anywhere(level) {
        return Ok(marking);
    }
    marking
        .par_iter_mut()
        .enumerate()
        .skip(mesh.num_vertex + 1)
        .try_for_each(|(triangle, mark)| -> io::Result<()> {
            let centre = mesh.triangle_points[triangle];
            if targets.demands(centre, level)? {
                *mark = 1;
                return Ok(());
            }
            if split_children {
                let corners = mesh.cells_on_triangle[triangle].map(|cell| mesh.cell_points[cell]);
                for k in 0..3 {
                    let (a, b, c) = (corners[k], corners[(k + 1) % 3], corners[(k + 2) % 3]);
                    let Some(middle) = earthmesh_mesh::spherical_centroid_degrees(&[a, b]) else {
                        continue;
                    };
                    for half in [[a, middle, c], [middle, b, c]] {
                        if let Some(point) = earthmesh_mesh::spherical_centroid_degrees(&half) {
                            if targets.demands(point, level)? {
                                *mark = 1;
                                return Ok(());
                            }
                        }
                    }
                }
            }
            Ok(())
        })?;
    Ok(marking)
}

#[cfg(test)]
mod marking_tests {
    use super::*;
    use earthmesh_mesh::{LonLatDegrees, RefinementRegion};

    fn base() -> crate::RedGreenMesh {
        let mesh =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        crate::redgreen_mesh_from_triangular(&mesh, &neighbors).expect("bridge")
    }

    #[test]
    fn split_children_add_the_rim_triangles_a_green_half_would_reach() {
        let mesh = base();
        let regions = [RefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 2_000_000.0,
            level: 1,
        }];
        let targets = earthmesh_refine::RegionTargets::new(&regions);
        let centres = redgreen_marking(&mesh, &targets, 1, false).unwrap();
        let halves = redgreen_marking(&mesh, &targets, 1, true).unwrap();
        // Every triangle the centre rule marks stays marked, and the rim gains
        // the triangles whose centre is outside but one of whose halves is in.
        assert!(centres
            .iter()
            .zip(&halves)
            .all(|(&centre, &half)| centre <= half));
        let (a, b) = (
            centres.iter().filter(|&&m| m == 1).count(),
            halves.iter().filter(|&&m| m == 1).count(),
        );
        assert!(b > a, "{a} centre marks, {b} with halves");
        assert_eq!(&halves[..=mesh.num_vertex], &centres[..=mesh.num_vertex]);
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
/// `previous_level_marks` is the level above's settled red interior **in this
/// mesh's numbering**. Transition children are deliberately excluded so a
/// deeper round cannot split them again.
/// The settings one round runs with. TRI can publish variable-width W fans and
/// closes with shape-checked greens; HEX still needs the canonical dual.
fn redgreen_round_settings(
    refine: &earthmesh_core::RefineConfig,
    level: usize,
    preserve_locality: bool,
    demand_nests: bool,
) -> crate::RedGreenSettings {
    let mut settings = redgreen_settings_for_level(refine, level);
    settings.protect_triangle_quality = preserve_locality;
    if preserve_locality {
        settings.min_triangle_angle_deg = TRIANGLE_SHAPE_FLOOR_DEG;
        if demand_nests {
            settings.green_floor_deg = Some(NESTED_DEMAND_GREEN_FLOOR_DEG);
        }
    }
    settings
}

#[cfg(test)]
mod round_settings_tests {
    use super::*;
    use earthmesh_refine::TargetLevelField;

    #[test]
    fn only_triangle_output_from_a_nesting_demand_fixes_the_green_floor() {
        let refine = earthmesh_core::RefineConfig::default();
        let floor = |tri, nests| redgreen_round_settings(&refine, 1, tri, nests).green_floor_deg;
        assert_eq!(floor(true, true), Some(NESTED_DEMAND_GREEN_FLOOR_DEG));
        assert_eq!(
            floor(true, false),
            None,
            "a demand that does not nest keeps the derived floor"
        );
        assert_eq!(floor(false, true), None, "hex closes with transition rows");
        assert_eq!(floor(false, false), None);
        // Regions are asked for at `>= level`, so they nest.
        assert!(earthmesh_refine::RegionTargets::new(&[]).nests_by_construction());
    }
}

pub fn refine_redgreen_level(
    mesh: &crate::RedGreenMesh,
    targets: &dyn earthmesh_refine::TargetLevelField,
    refine: &earthmesh_core::RefineConfig,
    level: usize,
    previous_level_marks: Option<&[i32]>,
    preserve_locality: bool,
    split_children: bool,
) -> io::Result<crate::RedGreenOutcome> {
    let marking = redgreen_marking(mesh, targets, level, split_children)?;
    let settings = redgreen_round_settings(
        refine,
        level,
        preserve_locality,
        targets.nests_by_construction(),
    );
    let outcome =
        crate::refine_redgreen_round_inside(mesh, &marking, &settings, previous_level_marks)?;
    Ok(outcome)
}

#[cfg(test)]
mod level_tests {
    use super::*;
    use earthmesh_mesh::{LonLatDegrees, RefinementRegion};

    /// A deeper level must stay inside the red children actually produced by
    /// its parent. Re-testing region geometry would also include green
    /// transition children and let later levels split them into slivers.
    #[test]
    fn a_deeper_level_is_held_inside_the_one_above_it() {
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(9, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = crate::redgreen_mesh_from_triangular(&base, &neighbors).expect("bridge in");
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

        let first = refine_redgreen_level(
            &mesh,
            &earthmesh_refine::RegionTargets::new(&regions),
            &refine,
            1,
            None,
            false,
            false,
        )
        .expect("level one");
        let previous = first.interior_marks.clone();
        assert_eq!(
            previous.len(),
            first.mesh.triangle_count() + 1,
            "the carried interior must use the mesh numbering the next level sees"
        );
        assert_ne!(
            previous.len(),
            first.cell_renumbering.len(),
            "and cell_renumbering is not that mapping -- it is per cell"
        );

        let held = refine_redgreen_level(
            &first.mesh,
            &earthmesh_refine::RegionTargets::new(&regions),
            &refine,
            2,
            Some(&previous),
            false,
            false,
        )
        .expect("level two, held inside level one");
        let free = refine_redgreen_level(
            &first.mesh,
            &earthmesh_refine::RegionTargets::new(&regions),
            &refine,
            2,
            None,
            false,
            false,
        )
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
}
