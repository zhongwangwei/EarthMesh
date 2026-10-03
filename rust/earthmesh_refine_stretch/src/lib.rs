//! The Stretch backend (`NL%refine_backend = 'stretch'`, guide 11.79).
//!
//! A closed ICON grid cannot take a cell inserted anywhere (guide 11.76), so
//! this backend keeps the icosahedral grid's topology and moves its vertices
//! instead: a Schmidt transform toward one focus, finer there and coarser
//! opposite it. Given each vertex's target level, it picks the focus and the
//! factor; the Schmidt kernels themselves are in `earthmesh_mesh`, which
//! CMRC's stretched mother shares.

/// A unit vector.
pub type P = [f64; 3];

/// The stretch a demand asks for, and what it cannot serve.
#[derive(Clone, Debug, PartialEq)]
pub struct StretchPlan {
    /// The focus the grid is stretched toward.
    pub focus: P,
    /// The stretch factor: cells finer by it at the focus, coarser by it
    /// opposite.
    pub factor: f64,
    /// `2^deepest`, what the focus alone would need.
    pub focus_factor: f64,
    /// Vertices asking for a level above 0.
    pub demanded: usize,
    /// Demanded vertices no factor can serve from this focus.
    pub unreachable: usize,
    /// The farthest of them from the focus, degrees.
    pub farthest_unreachable_deg: f64,
    /// Whether the cap of four times `2^deepest`, not the demand, set the
    /// factor.
    pub capped: bool,
}

/// Plan the stretch for unit `points` with target `levels` (0 where nothing
/// is asked), on a grid whose median edge is `h0_radians`: the focus is the
/// demand's depth-weighted centre, and the factor the one the farthest
/// demanded point needs -- half a base cell beyond it, so the cells over the
/// demand's edge qualify too -- at most four times `2^deepest`. `None` when
/// no vertex is demanded.
pub fn plan_stretch(
    points: &[P],
    levels: &[usize],
    h0_radians: Option<f64>,
    max_level: usize,
) -> Option<StretchPlan> {
    let (focus, focus_factor) = earthmesh_mesh::schmidt_focus_for_levels(
        points,
        levels,
        2f64.powi(max_level.min(16) as i32),
    )?;
    let needed = earthmesh_mesh::schmidt_factor_for_levels(
        points,
        levels,
        focus,
        h0_radians.unwrap_or(0.0) / 2.0,
        4.0 * focus_factor,
    );
    Some(StretchPlan {
        focus,
        factor: needed.factor,
        focus_factor,
        demanded: levels.iter().filter(|&&level| level > 0).count(),
        unreachable: needed.unreachable,
        farthest_unreachable_deg: needed.farthest_unreachable_deg,
        capped: needed.capped,
    })
}

/// Move unit `points` by the planned stretch.
pub fn apply_stretch(points: &mut [P], plan: &StretchPlan) {
    earthmesh_mesh::schmidt_stretch(points, plan.focus, plan.factor);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(lon: f64, lat: f64) -> P {
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    }

    /// No demand, no stretch; a demand pulls the focus to it, and the
    /// stretch makes cells there finer: neighbours close in.
    #[test]
    fn a_demand_stretches_the_grid_toward_it() {
        let points = (-80..=80)
            .step_by(10)
            .flat_map(|lat| {
                (-170..=180)
                    .step_by(10)
                    .map(move |lon| unit(lon as f64, lat as f64))
            })
            .collect::<Vec<_>>();
        assert!(plan_stretch(&points, &vec![0; points.len()], Some(0.17), 2).is_none());

        let target = unit(30.0, 20.0);
        let levels = points
            .iter()
            .map(|p| {
                let cos = p[0] * target[0] + p[1] * target[1] + p[2] * target[2];
                usize::from(cos > 15f64.to_radians().cos()) * 2
            })
            .collect::<Vec<_>>();
        let plan = plan_stretch(&points, &levels, Some(0.17), 2).unwrap();
        let cos = plan.focus[0] * target[0] + plan.focus[1] * target[1] + plan.focus[2] * target[2];
        assert!(cos > 10f64.to_radians().cos(), "focus {:?}", plan.focus);
        assert!(plan.factor > 1.0 && plan.demanded > 0, "{plan:?}");

        let mut moved = points.clone();
        apply_stretch(&mut moved, &plan);
        let gap = |a: P, b: P| {
            (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
                .clamp(-1.0, 1.0)
                .acos()
        };
        let near = points
            .iter()
            .position(|p| (p[0] * target[0] + p[1] * target[1] + p[2] * target[2]) > 0.999)
            .unwrap();
        let next = near + 1;
        assert!(gap(moved[near], moved[next]) < gap(points[near], points[next]));
    }
}
