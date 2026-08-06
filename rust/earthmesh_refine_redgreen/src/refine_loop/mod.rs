//! The red-green driver: mark, grow the marking until it is legal, subdivide,
//! close the seams, flip, renumber.
//!
//! Port of `MOD_refine.F90:refine_loop`. Everything it orchestrates already
//! existed kernel by kernel in this crate; what was missing was the order and
//! the bookkeeping between them, which is where the algorithm actually lives.
//!
//! The shape of a round:
//!
//! 1. Drop isolated marks -- a triangle with no marked neighbour is a single
//!    cell of refinement surrounded by coarse ground, which the transition rows
//!    cannot close cleanly and nobody asked for.
//! 2. Grow the marking. `iterB` looks outward from marked triangles, `iterC`
//!    from the polygon cells around them, `iterG` at weak concavities. Each one
//!    can add triangles, and adding triangles can make the others unhappy, so
//!    they run to a joint fixed point. **This is the step that makes an
//!    arbitrary marking legal, and it is why this path refines regions Method-C
//!    refuses**: it grows, it never rejects.
//! 3. Subdivide every marked triangle into four.
//! 4. Build transition rows outward: `max_transition_row` rounds of halving the
//!    triangles along the boundary, forward then reverse, with Lawson flips
//!    after each round to take the angles back.
//! 5. Renumber, which is what turns the sparse working tables into a mesh.

use std::io;

use crate::{
    refine_array_length_calculation_one_based, refine_boundary_segments_make_one_based,
    refine_delaunay_lop_one_based, refine_isreverse_judge_one_based, refine_iter_b_judge_one_based,
    refine_iter_c_judge_one_based, refine_iter_g_judge_one_based, refine_ngr_renew_one_based,
    refine_num_ref_cal_one_based, refine_onedivide_four_connection_one_based,
    refine_onedivide_four_renew_one_based, refine_onedivide_two_one_based,
    refine_sharp_concav_lop_judge_one_based, LonLatDegrees,
};

/// A triangular mesh in the tables this pipeline works in.
///
/// One-based throughout, with slot 0 unused and slot 1 the canonical
/// placeholder, matching every kernel here.
#[derive(Clone, Debug, PartialEq)]
pub struct RedGreenMesh {
    /// Original icosahedron vertices, which no refinement may consume.
    pub num_vertex: usize,
    /// Lowest cell id the refinement may touch; cells below it are original.
    pub num_center: usize,
    /// Triangle centres (`mp`).
    pub triangle_points: Vec<LonLatDegrees>,
    /// Polygon centres, which are the triangles' corners (`wp`).
    pub cell_points: Vec<LonLatDegrees>,
    /// The three cells each triangle stands on (`ngrmw`).
    pub cells_on_triangle: Vec<[usize; 3]>,
    /// The triangles around each cell (`ngrwm`).
    pub triangles_on_cell: Vec<Vec<usize>>,
    /// How many of `triangles_on_cell` are real (`n_ngrwm`).
    pub n_triangles_on_cell: Vec<usize>,
}

impl RedGreenMesh {
    /// Triangle count, as the Fortran's `sjx_points`.
    pub fn triangle_count(&self) -> usize {
        self.cells_on_triangle.len().saturating_sub(1)
    }

    /// Cell count, as the Fortran's `lbx_points`.
    pub fn cell_count(&self) -> usize {
        self.cell_points.len().saturating_sub(1)
    }
}

/// How wide the transition band is and what to do with weak concavities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RedGreenSettings {
    /// `max_transition_row`: rounds of 1→2 closure outside the refined region.
    pub max_transition_row: usize,
    /// `Istransition`: build the transition rows at all. False leaves hanging
    /// nodes and is only meaningful for hexagonal output.
    pub build_transition_rows: bool,
    /// `weak_concav_eliminate`: absorb weak concavities by refining them out
    /// rather than carrying them through the transition rounds.
    ///
    /// **Only half wired.** Turning this on reaches `iterG` in the judge chain,
    /// which is what grows the marking over a weak concavity. It does *not*
    /// reach the transition rounds' half: `MOD_refine.F90:355,477` also call
    /// `weak_concav_segment_make`, `weak_concav_lop_judge` and
    /// `weak_concav_pair_special`, and `close_transition_rows` calls none of
    /// them. Two of those ports (`refine_lop_weak`, `refine_lop_weak_pair`)
    /// additionally carry the same one-based drift this crate's
    /// `refine_lop_sharp` was just converted out of, so wiring them means
    /// converting them first -- the recipe is the same table.
    ///
    /// Left `false` until both halves are there, so it cannot half-run.
    pub eliminate_weak_concavity: bool,
    /// `HALO`: how far inside the previous level's refined region this level
    /// must stay.
    ///
    /// A level that marks a triangle sitting in the previous level's transition
    /// band would refine ground that is still changing resolution, so
    /// `MOD_refine.F90:113-152` erodes the previous region inward this many
    /// rings and cancels any mark that falls in what it ate. Zero means no
    /// protection, which is right for a first level -- there is nothing to
    /// stay inside of.
    pub halo: usize,
}

impl Default for RedGreenSettings {
    fn default() -> Self {
        Self {
            max_transition_row: 3,
            build_transition_rows: true,
            eliminate_weak_concavity: false,
            halo: 3,
        }
    }
}

/// What one round of refinement produced.
#[derive(Clone, Debug, PartialEq)]
pub struct RedGreenOutcome {
    pub mesh: RedGreenMesh,
    /// Triangles the round split into four.
    pub refined_triangle_count: usize,
    /// Triangles the marking gained from the judge chain, over what was asked.
    pub grown_triangle_count: usize,
    /// Triangles dropped for having no marked neighbour.
    pub isolated_dropped_count: usize,
    /// Triangles dropped for lying outside the previous level's halo.
    pub halo_cancelled_count: usize,
    /// Triangles rebuilt by Lawson flips while closing the seams.
    pub flipped_triangle_count: usize,
}

/// Triangle-neighbour slots (`ngrmm`) in the shape the judges want.
fn triangle_neighbor_rows(mesh: &RedGreenMesh) -> io::Result<Vec<Vec<usize>>> {
    let neighbors = earthmesh_mesh::triangle_neighbors_from_cell_membership_one_based(
        &mesh.cells_on_triangle,
        &mesh.triangles_on_cell,
        &mesh.n_triangles_on_cell,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "triangle neighbours could not be derived: cell membership does not close",
        )
    })?;
    Ok(neighbors.into_iter().map(|row| row.to_vec()).collect())
}

/// Drop marks with no marked neighbour.
///
/// A lone refined triangle is not a region, it is a defect: the transition rows
/// around it meet themselves, and no criterion that asked for it meant one cell.
/// Returns how many were dropped so the caller can say so rather than silently
/// refining less than was asked.
fn drop_isolated_marks(
    num_vertex: usize,
    sjx_points: usize,
    triangle_neighbors: &[Vec<usize>],
    ref_sjx: &mut [i32],
) -> usize {
    let isolated: Vec<usize> = (num_vertex + 1..=sjx_points)
        .filter(|&triangle| ref_sjx[triangle] == 1)
        .filter(|&triangle| {
            triangle_neighbors[triangle]
                .iter()
                .all(|&neighbor| ref_sjx.get(neighbor).copied().unwrap_or(0) == 0)
        })
        .collect();
    for triangle in &isolated {
        ref_sjx[*triangle] = 0;
    }
    isolated.len()
}

/// Erode the previous level's region inward and cancel what falls outside.
///
/// Port of `MOD_refine.F90:113-152`. A cell is on the region's boundary when
/// its incident triangles are neither all marked nor all unmarked; one ring of
/// erosion clears the marks on every such cell's triangles, and `halo` rings of
/// it leave the interior the next level is allowed to touch.
///
/// Returns how many of this level's marks were cancelled, so a caller can say
/// so rather than quietly refining less than was asked.
fn cancel_marks_outside_halo(
    mesh: &RedGreenMesh,
    previous_level_marks: &[i32],
    halo: usize,
    marking: &mut [i32],
) -> io::Result<usize> {
    let sjx_points = mesh.triangle_count();
    if previous_level_marks.len() <= sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "the previous level's marking has {} rows for {sjx_points} triangles",
                previous_level_marks.len()
            ),
        ));
    }
    let mut interior = previous_level_marks[..=sjx_points].to_vec();
    // Original vertices are never inside anything, which is what keeps a deeper
    // level from creeping onto the icosahedron's own points.
    for slot in interior.iter_mut().take(mesh.num_vertex + 1) {
        *slot = 0;
    }

    for _ in 0..halo {
        let boundary: Vec<usize> = (mesh.num_center + 1..=mesh.cell_count())
            .filter(|&cell| {
                let edges = mesh.n_triangles_on_cell[cell];
                if edges == 0 {
                    return false;
                }
                let marked = mesh.triangles_on_cell[cell][..edges]
                    .iter()
                    .filter(|&&triangle| interior.get(triangle).copied().unwrap_or(0) == 1)
                    .count();
                marked > 0 && marked < edges
            })
            .collect();
        if boundary.is_empty() {
            break;
        }
        for cell in boundary {
            let edges = mesh.n_triangles_on_cell[cell];
            for &triangle in &mesh.triangles_on_cell[cell][..edges] {
                if let Some(slot) = interior.get_mut(triangle) {
                    *slot = 0;
                }
            }
        }
    }

    let mut cancelled = 0usize;
    for triangle in mesh.num_vertex + 1..=sjx_points {
        if marking[triangle] != 0 && interior[triangle] != 1 {
            marking[triangle] = 0;
            cancelled += 1;
        }
    }
    Ok(cancelled)
}

/// One red-green refinement round.
///
/// `ref_sjx` is the marking, one entry per triangle, `1` for "split this".
/// Where it came from -- a threshold criterion, a named region, an h-field --
/// is not this function's business, which is the point: any marking is legal
/// input, and the judge chain is what makes it buildable.
pub fn refine_redgreen_round_one_based(
    mesh: &RedGreenMesh,
    ref_sjx: &[i32],
    settings: &RedGreenSettings,
) -> io::Result<RedGreenOutcome> {
    refine_redgreen_round_inside(mesh, ref_sjx, settings, None)
}

/// The same round, held inside the region a previous level refined.
///
/// `previous_level_marks` is that level's marking, one entry per triangle.
/// Every mark this level made that does not survive eroding it by
/// `settings.halo` rings is cancelled, so a level never refines ground the
/// level above it is still transitioning across.
pub fn refine_redgreen_round_inside(
    mesh: &RedGreenMesh,
    ref_sjx: &[i32],
    settings: &RedGreenSettings,
    previous_level_marks: Option<&[i32]>,
) -> io::Result<RedGreenOutcome> {
    let sjx_points = mesh.triangle_count();
    let lbx_points = mesh.cell_count();
    if ref_sjx.len() <= sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "the marking has {} rows for {sjx_points} triangles",
                ref_sjx.len()
            ),
        ));
    }
    if settings.max_transition_row == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_transition_row must be at least one",
        ));
    }

    let triangle_neighbors = triangle_neighbor_rows(mesh)?;
    let mut marking = ref_sjx.to_vec();
    let halo_cancelled_count = match previous_level_marks {
        Some(previous) => cancel_marks_outside_halo(mesh, previous, settings.halo, &mut marking)?,
        None => 0,
    };
    let isolated_dropped_count = drop_isolated_marks(
        mesh.num_vertex,
        sjx_points,
        &triangle_neighbors,
        &mut marking,
    );
    let asked_for = (mesh.num_vertex + 1..=sjx_points)
        .filter(|&triangle| marking[triangle] == 1)
        .count();
    if asked_for == 0 {
        return Ok(RedGreenOutcome {
            mesh: mesh.clone(),
            refined_triangle_count: 0,
            grown_triangle_count: 0,
            isolated_dropped_count,
            halo_cancelled_count,
            flipped_triangle_count: 0,
        });
    }

    // `mrl_new` is the refinement state: 1 unrefined, 4 split into four, 2 a
    // transition triangle. `ref_lbx` remembers which polygon cells have a split
    // triangle beside them, which is what iterC reads.
    let mut mrl_new = vec![1i32; sjx_points + 1];
    let mut ref_lbx = vec![0i32; lbx_points + 1];
    let mut segment = marking.clone();
    let mut num_mp = vec![0usize, sjx_points];
    let mut num_wp = vec![0usize, lbx_points];

    // Applying a judge's proposal: reserve the ids its four-way splits will
    // need, then fold the proposal into the refinement state.
    #[allow(clippy::too_many_arguments)]
    fn apply_marking(
        num_vertex: usize,
        sjx_points: usize,
        cells_on_triangle: &[[usize; 3]],
        marking: &[i32],
        num_ref: usize,
        mrl_new: &mut [i32],
        ref_lbx: &mut [i32],
        num_mp: &mut Vec<usize>,
        num_wp: &mut Vec<usize>,
    ) -> io::Result<()> {
        let last_mp = *num_mp.last().expect("seeded");
        let last_wp = *num_wp.last().expect("seeded");
        num_mp.push(last_mp + 4 * num_ref);
        num_wp.push(last_wp + 3 * num_ref);
        refine_onedivide_four_connection_one_based(
            num_vertex,
            sjx_points,
            cells_on_triangle,
            marking,
            ref_lbx,
            mrl_new,
        )
    }

    apply_marking(
        mesh.num_vertex,
        sjx_points,
        &mesh.cells_on_triangle,
        &marking,
        asked_for,
        &mut mrl_new,
        &mut ref_lbx,
        &mut num_mp,
        &mut num_wp,
    )?;

    // The judge chain, to a joint fixed point. Each judge proposes triangles;
    // whatever it adds is applied and the chain restarts, because a triangle
    // added for iterB's reason can create the configuration iterC objects to.
    // Bounded because every round strictly grows a finite marking.
    let mut grown_triangle_count = 0usize;
    for _ in 0..(sjx_points + 1) {
        let mut grew = false;

        loop {
            marking = refine_iter_b_judge_one_based(
                settings.max_transition_row,
                mesh.num_vertex,
                &triangle_neighbors,
                &mrl_new,
            )?;
            let added =
                refine_num_ref_cal_one_based(mesh.num_vertex, sjx_points, &marking, &mut segment)?;
            if added == 0 {
                break;
            }
            grown_triangle_count += added;
            grew = true;
            apply_marking(
                mesh.num_vertex,
                sjx_points,
                &mesh.cells_on_triangle,
                &marking,
                added,
                &mut mrl_new,
                &mut ref_lbx,
                &mut num_mp,
                &mut num_wp,
            )?;
        }

        loop {
            marking = refine_iter_c_judge_one_based(
                settings.max_transition_row,
                mesh.num_vertex,
                mesh.num_center,
                lbx_points,
                &triangle_neighbors,
                &mesh.triangles_on_cell,
                &mesh.n_triangles_on_cell,
                &mrl_new,
                &ref_lbx,
            )?;
            let added =
                refine_num_ref_cal_one_based(mesh.num_vertex, sjx_points, &marking, &mut segment)?;
            if added == 0 {
                break;
            }
            grown_triangle_count += added;
            grew = true;
            apply_marking(
                mesh.num_vertex,
                sjx_points,
                &mesh.cells_on_triangle,
                &marking,
                added,
                &mut mrl_new,
                &mut ref_lbx,
                &mut num_mp,
                &mut num_wp,
            )?;
        }

        if grew {
            continue;
        }
        if !settings.eliminate_weak_concavity {
            break;
        }
        marking = refine_iter_g_judge_one_based(
            mesh.num_center,
            lbx_points,
            &mesh.triangles_on_cell,
            &mesh.n_triangles_on_cell,
            &mrl_new,
        )?;
        let added =
            refine_num_ref_cal_one_based(mesh.num_vertex, sjx_points, &marking, &mut segment)?;
        if added == 0 {
            break;
        }
        grown_triangle_count += added;
        apply_marking(
            mesh.num_vertex,
            sjx_points,
            &mesh.cells_on_triangle,
            &marking,
            added,
            &mut mrl_new,
            &mut ref_lbx,
            &mut num_mp,
            &mut num_wp,
        )?;
    }

    let refined_triangle_count = (mesh.num_vertex + 1..=sjx_points)
        .filter(|&triangle| segment[triangle] == 1)
        .count();

    // How much room the round needs, including the transition band the halo
    // sizing walks outward.
    let sizing = refine_array_length_calculation_one_based(
        settings.max_transition_row,
        mesh.num_vertex,
        mesh.num_center,
        sjx_points,
        lbx_points,
        &mrl_new,
        &triangle_neighbors,
        &mesh.cells_on_triangle,
        &mesh.triangles_on_cell,
        &mesh.n_triangles_on_cell,
        refined_triangle_count,
    )?;
    let room = sizing.halo.num_transition_row_triangles * 4;

    let mut triangle_points = mesh.triangle_points.clone();
    triangle_points.resize(sjx_points + room + 1, LonLatDegrees::new(0.0, 0.0));
    let mut cell_points = mesh.cell_points.clone();
    cell_points.resize(lbx_points + room + 1, LonLatDegrees::new(0.0, 0.0));
    let mut cells_on_triangle_new = mesh.cells_on_triangle.clone();
    cells_on_triangle_new.resize(sjx_points + room + 1, [1usize; 3]);

    let subdivision_iter = num_mp.len() - 1;
    refine_onedivide_four_renew_one_based(
        subdivision_iter,
        mesh.num_vertex,
        &num_mp,
        &num_wp,
        &mesh.cells_on_triangle,
        &segment,
        &mut triangle_points,
        &mut cell_points,
        &mut cells_on_triangle_new,
    )?;

    let mut flipped_triangle_count = 0usize;
    if settings.build_transition_rows {
        // The curve table is fixed-width with a placeholder row; the segment
        // builder wants each curve's real vertices and nothing else, so trim to
        // the recorded length. Curves shorter than a triangle are dropped
        // rather than passed on -- they are boundary fragments the halo sizing
        // could not close, and the builder rejects them one layer later with
        // less to say about where they came from.
        let curves = &sizing.boundary.curves;
        let closed_curves: Vec<Vec<usize>> = (1..=curves.num_closed_curve)
            .filter_map(|index| {
                let length = *curves.n_close_curve.get(index)?;
                let row = curves.close_curves.get(index)?;
                (length >= 3).then(|| row[..length.min(row.len())].to_vec())
            })
            .collect();
        if closed_curves.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "the refined region has no closed boundary the transition rows can follow                      ({} curve(s) found, none with three cells)",
                    curves.num_closed_curve
                ),
            ));
        }
        flipped_triangle_count = close_transition_rows(
            mesh,
            settings,
            &triangle_neighbors,
            &closed_curves,
            &mut mrl_new,
            &mut num_mp,
            &mut num_wp,
            &mut triangle_points,
            &mut cell_points,
            &mut cells_on_triangle_new,
        )?;
    }

    let renewed = refine_ngr_renew_one_based(
        num_mp.len() - 1,
        mesh.num_vertex,
        &num_mp,
        &num_wp,
        &triangle_points,
        &cell_points,
        &cells_on_triangle_new,
        &sizing.halo.boundary_refine,
        &sizing.halo.boundary_refine_transition,
    )?;

    Ok(RedGreenOutcome {
        mesh: RedGreenMesh {
            num_vertex: mesh.num_vertex,
            num_center: mesh.num_center,
            triangle_points: renewed.triangle_points,
            cell_points: renewed.cell_points,
            cells_on_triangle: renewed.cells_on_triangle,
            triangles_on_cell: renewed.triangles_on_cell,
            n_triangles_on_cell: renewed.n_triangles_on_cell,
        },
        refined_triangle_count,
        grown_triangle_count,
        isolated_dropped_count,
        halo_cancelled_count,
        flipped_triangle_count,
    })
}

/// The green step: `max_transition_row` rounds of halving the boundary.
///
/// Each round takes one triangle off the head of every boundary segment,
/// splits it in two forward, then again in reverse where the segment's
/// orientation calls for it, and flips the sharp corners the split left. The
/// segments shorten as they are consumed, so the band walks outward one row per
/// round and stops when they are empty.
#[allow(clippy::too_many_arguments)]
fn close_transition_rows(
    mesh: &RedGreenMesh,
    settings: &RedGreenSettings,
    triangle_neighbors: &[Vec<usize>],
    closed_curves: &[Vec<usize>],
    mrl_new: &mut [i32],
    num_mp: &mut Vec<usize>,
    num_wp: &mut Vec<usize>,
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle_new: &mut [[usize; 3]],
) -> io::Result<usize> {
    let sjx_points = mesh.triangle_count();
    let mut segments = refine_boundary_segments_make_one_based(
        settings.max_transition_row,
        closed_curves,
        &mesh.triangles_on_cell,
        &mesh.n_triangles_on_cell,
        mrl_new,
    )?;
    if segments.num_bdy_refine_segment == 0 {
        return Ok(0);
    }

    let mut sjx_child = vec![[0usize; 2]; sjx_points + 1];
    let mut flipped = 0usize;

    for _ in 0..settings.max_transition_row {
        // The judge compares the segments before and after this row was
        // consumed, so the snapshot has to be taken before the heads are eaten.
        let segments_before = segments.bdy_refine_segment.clone();
        let mut row_marking = vec![0i32; sjx_points + 1];
        let mut row_count = 0usize;
        for index in 0..segments.num_bdy_refine_segment {
            if segments.n_bdy_refine_segment[index] == 0 {
                continue;
            }
            // A segment row is as long as the boundary there allowed, which is
            // not always the full transition width -- a short stretch of
            // boundary carries fewer rows, and reading past it is reading
            // another segment's storage.
            let row = &segments.bdy_refine_segment[index];
            for slot in 0..settings.max_transition_row {
                let triangle = row[slot];
                if triangle == 1 {
                    break;
                }
                if triangle <= sjx_points && row_marking[triangle] == 0 {
                    row_marking[triangle] = 1;
                    row_count += 1;
                }
            }
            segments.n_bdy_refine_segment[index] -= 1;
        }
        if row_count == 0 {
            break;
        }

        let last_mp = *num_mp.last().expect("seeded");
        let last_wp = *num_wp.last().expect("seeded");
        num_mp.push(last_mp + 2 * row_count);
        num_wp.push(last_wp + row_count);
        refine_onedivide_two_one_based(
            num_mp.len() - 1,
            false,
            mesh.num_vertex,
            num_mp,
            num_wp,
            triangle_neighbors,
            &mesh.cells_on_triangle,
            &row_marking,
            mrl_new,
            triangle_points,
            cell_points,
            cells_on_triangle_new,
            &mut sjx_child,
        )?;
        for triangle in mesh.num_vertex + 1..=sjx_points {
            if row_marking[triangle] == 1 {
                mrl_new[triangle] = 4;
            }
        }

        // `MOD_refine.F90:446`: which triangles split the *other* way next
        // round. Without it the band only ever grows forward and the segments
        // never present their reverse side, so the row after this one has
        // nothing to halve.
        let reverse_marking = refine_isreverse_judge_one_based(
            settings.max_transition_row,
            segments.num_bdy_refine_segment,
            triangle_neighbors,
            mrl_new,
            &mut segments.bdy_refine_segment,
            &segments.n_bdy_refine_segment,
        )?;
        let reverse_count = (mesh.num_vertex + 1..=sjx_points)
            .filter(|&triangle| reverse_marking[triangle] == 1)
            .count();
        if reverse_count > 0 {
            let last_mp = *num_mp.last().expect("seeded");
            let last_wp = *num_wp.last().expect("seeded");
            num_mp.push(last_mp + 2 * reverse_count);
            num_wp.push(last_wp + reverse_count);
            refine_onedivide_two_one_based(
                num_mp.len() - 1,
                true,
                mesh.num_vertex,
                num_mp,
                num_wp,
                triangle_neighbors,
                &mesh.cells_on_triangle,
                &reverse_marking,
                mrl_new,
                triangle_points,
                cell_points,
                cells_on_triangle_new,
                &mut sjx_child,
            )?;
            for triangle in mesh.num_vertex + 1..=sjx_points {
                if reverse_marking[triangle] == 1 {
                    mrl_new[triangle] = 4;
                }
            }
        }

        // Sharp corners the row left behind, taken back by Lawson flips.
        // `MOD_refine.F90:341,487` -- num_end, and one row per segment plus one
        // per weak concavity. Weak concavities are not carried here, so the
        // second term is zero, and the row count is the segment count exactly:
        // these tables are indexed by a count, not by an entity id.
        let num_end = 4.max(4 * settings.max_transition_row.saturating_sub(1));
        let mut lop_candidates = vec![1usize; 0];
        let mut lop_counts = vec![0usize; segments.num_bdy_refine_segment];
        let mut lop_rows = vec![vec![1usize; num_end]; segments.num_bdy_refine_segment];
        let mut flips = 0usize;
        refine_sharp_concav_lop_judge_one_based(
            &mut flips,
            segments.num_bdy_refine_segment,
            mrl_new,
            triangle_neighbors,
            cells_on_triangle_new,
            &sjx_child,
            &segments.bdy_refine_segment,
            &segments_before,
            &segments.n_bdy_refine_segment,
            &mut lop_rows,
            &mut lop_counts,
        )?;
        if flips > 0 {
            for (index, count) in lop_counts.iter().enumerate() {
                lop_candidates.extend_from_slice(&lop_rows[index][..(*count).min(num_end)]);
            }
            let last_mp = *num_mp.last().expect("seeded");
            let last_wp = *num_wp.last().expect("seeded");
            num_mp.push(last_mp + flips);
            num_wp.push(last_wp);
            refine_delaunay_lop_one_based(
                num_mp.len() - 1,
                flips,
                num_mp,
                num_wp,
                triangle_points,
                cell_points,
                cells_on_triangle_new,
                &lop_candidates,
            )?;
            flipped += flips;
        }
        sjx_child.iter_mut().for_each(|row| *row = [0, 0]);
    }

    Ok(flipped)
}

/// Build the red-green tables from a Method-C mesh.
///
/// The two representations are duals of each other's naming, which is the only
/// thing that makes this look like work: Method-C's W faces are the triangles
/// this pipeline calls `sjx`, and Method-C's M points are the polygon centres
/// it calls `wp` -- the triangles' corners. A triangle has no stored coordinate
/// in Method-C, so its centre is built from its three corners here.
pub fn redgreen_mesh_from_method_c(
    mesh: &earthmesh_mesh::MethodCDelaunayMesh,
    m_neighbors: &[earthmesh_mesh::IcosahedronMPointNeighbors],
) -> io::Result<RedGreenMesh> {
    if m_neighbors.len() < mesh.nmd + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "M-point neighbours have {} rows for {} points",
                m_neighbors.len(),
                mesh.nmd
            ),
        ));
    }
    let cell_points: Vec<LonLatDegrees> = mesh
        .m_points
        .iter()
        .map(|point| earthmesh_mesh::xyz_to_lonlat_degrees(*point))
        .collect();

    let mut cells_on_triangle = vec![[1usize; 3]; mesh.nwd + 1];
    let mut triangle_points = vec![LonLatDegrees::new(0.0, 0.0); mesh.nwd + 1];
    for iw in 2..=mesh.nwd {
        let corners = mesh.w_faces[iw].im;
        cells_on_triangle[iw] = corners;
        let centre = crate::average_lonlat3(
            cell_points[corners[0]],
            cell_points[corners[1]],
            cell_points[corners[2]],
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("W face {iw} has no representable centre"),
            )
        })?;
        triangle_points[iw] = centre;
    }

    let mut triangles_on_cell = vec![Vec::new(); mesh.nmd + 1];
    let mut n_triangles_on_cell = vec![0usize; mesh.nmd + 1];
    for im in 2..=mesh.nmd {
        let neighbors = m_neighbors[im];
        triangles_on_cell[im] = neighbors.iw[..neighbors.npoly.min(7)].to_vec();
        n_triangles_on_cell[im] = neighbors.npoly.min(7);
    }

    Ok(RedGreenMesh {
        // Slot 1 is the canonical placeholder and the only protected row on a
        // mesh this path has not refined yet.
        num_vertex: 1,
        num_center: 1,
        triangle_points,
        cell_points,
        cells_on_triangle,
        triangles_on_cell,
        n_triangles_on_cell,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icosahedron(nxp: usize) -> RedGreenMesh {
        let mesh = earthmesh_mesh::MethodCDelaunayMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0)
            .expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        redgreen_mesh_from_method_c(&mesh, &neighbors).expect("bridge")
    }

    #[test]
    fn an_unmarked_mesh_comes_back_unchanged() {
        let mesh = icosahedron(6);
        let marking = vec![0i32; mesh.triangle_count() + 1];

        let outcome =
            refine_redgreen_round_one_based(&mesh, &marking, &RedGreenSettings::default())
                .expect("nothing to do");

        assert_eq!(outcome.refined_triangle_count, 0);
        assert_eq!(outcome.mesh, mesh, "an empty marking must not move a point");
    }

    #[test]
    fn a_deeper_level_is_held_inside_the_one_above_it() {
        // A level marking ground that the level above is still transitioning
        // across would refine a resolution that has not settled. The halo
        // erodes the previous region and cancels what falls outside -- and the
        // count says so, because a silent cancel reads as "refined what you
        // asked for".
        let mesh = icosahedron(6);
        let previous: Vec<i32> = (0..=mesh.triangle_count())
            .map(|triangle| i32::from((40..70).contains(&triangle)))
            .collect();
        let mut marking = vec![0i32; mesh.triangle_count() + 1];
        for triangle in 40..70 {
            marking[triangle] = 1;
        }

        let held = refine_redgreen_round_inside(
            &mesh,
            &marking,
            &RedGreenSettings {
                halo: 2,
                ..RedGreenSettings::default()
            },
            Some(&previous),
        )
        .expect("a held round");
        let free = refine_redgreen_round_inside(
            &mesh,
            &marking,
            &RedGreenSettings {
                halo: 0,
                ..RedGreenSettings::default()
            },
            Some(&previous),
        )
        .expect("an unheld round");

        assert!(
            held.halo_cancelled_count > free.halo_cancelled_count,
            "eroding two rings must cancel more than eroding none: {held:?} vs {free:?}"
        );
        assert!(
            held.refined_triangle_count < free.refined_triangle_count,
            "and so refine less: {held:?} vs {free:?}"
        );
    }

    #[test]
    fn a_lone_marked_triangle_is_dropped_and_reported() {
        // One cell of refinement in a coarse field is not a region: the
        // transition rows around it would meet themselves. Dropping it is the
        // right answer, and saying so is the rest of the right answer -- a
        // silent drop reads as "refined what you asked for".
        let mesh = icosahedron(6);
        let mut marking = vec![0i32; mesh.triangle_count() + 1];
        marking[40] = 1;

        let outcome =
            refine_redgreen_round_one_based(&mesh, &marking, &RedGreenSettings::default())
                .expect("a lone mark is not an error");

        assert_eq!(outcome.isolated_dropped_count, 1);
        assert_eq!(outcome.refined_triangle_count, 0);
    }

    /// The property the whole path exists for: a patch nobody shaped for the
    /// algorithm refines, rather than being refused for its shape.
    ///
    /// This was blocked on three LOP judges that had transcribed Fortran's
    /// `do i = 1, num` as `1..=num` without the array gaining a slot to spare.
    /// `MOD_refine.F90:1411` allocates the segment tables with the column
    /// dimension *equal* to the count, so `size == num` and there is no
    /// placeholder column; the canonical `n + 1` convention applies to tables
    /// indexed by an entity id, and both dimensions of a segment table are
    /// counts.
    ///
    /// The driver also had to call `ref_sjx_isreverse_judge` between the
    /// forward and reverse halves of each transition round, which it did not.
    #[test]
    fn an_arbitrary_patch_is_grown_until_it_is_legal_rather_than_refused() {
        // The property this whole path exists for. Method-C would judge the
        // shape of this patch and could refuse it; here the judges add whatever
        // the triangulation needs and the round produces a mesh.
        let mesh = icosahedron(6);
        let mut marking = vec![0i32; mesh.triangle_count() + 1];
        let patch: Vec<usize> = (40..56).collect();
        for triangle in &patch {
            marking[*triangle] = 1;
        }

        let outcome =
            refine_redgreen_round_one_based(&mesh, &marking, &RedGreenSettings::default())
                .expect("an arbitrary patch must not be refused");

        assert!(
            outcome.refined_triangle_count >= patch.len() - outcome.isolated_dropped_count,
            "the round must split at least what survived the isolation drop: {outcome:?}"
        );
        assert!(
            outcome.mesh.triangle_count() > mesh.triangle_count(),
            "a refined mesh has more triangles than it started with: {} vs {}",
            outcome.mesh.triangle_count(),
            mesh.triangle_count()
        );
    }
}
