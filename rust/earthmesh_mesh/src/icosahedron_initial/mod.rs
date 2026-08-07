use super::*;

/// `mem_ijtabs:mloops` used by `mdloopf`, `udloopf`, and `wdloopf`.
pub const ICOSAHEDRON_MLOOPS: usize = 7;
pub const METHOD_C_CANONICAL_EARTH_RADIUS_METERS: f64 = earthmesh_core::EARTH_RADIUS_METERS;
// Exact Method-C Canonical single-precision literal; `f32::consts::PI` differs in the last
// digits and would break bit-for-bit Canonical parity, so keep the literal as-is.
#[allow(clippy::approx_constant)]
const METHOD_C_CANONICAL_PI2: f32 = 3.1415927_f32 * 2.0;

pub fn canonical_global_dist00(beta: f64, radius: f64, nxp: usize) -> f64 {
    ((beta as f32) * METHOD_C_CANONICAL_PI2 * (radius as f32) / (5.0 * nxp as f32)) as f64
}

/// Port of the `nmd/nud/nwd` sizing formulas in
/// `icosahedron.F90:icosahedron`.
pub fn icosahedron_counts_canonical(nxp0: usize) -> Option<IcosahedronCounts> {
    if nxp0 == 0 {
        return None;
    }
    let nn10 = nxp0.checked_mul(nxp0)?.checked_mul(10)?;
    Some(IcosahedronCounts {
        nmd: nn10 + 3,
        nud: 3 * nn10 + 1,
        nwd: 2 * nn10 + 1,
    })
}

/// Port of the big-diamond corner coordinate initialization in
/// `icosahedron.F90:icosahedron`.
pub fn icosahedron_diamond_corners_canonical() -> [IcosahedronDiamondCorners; 10] {
    let radius = METHOD_C_CANONICAL_EARTH_RADIUS_METERS as f32;
    let erador5 = radius / 5.0_f32.sqrt();
    let full_turn = METHOD_C_CANONICAL_PI2;

    std::array::from_fn(|slot| {
        let id = slot + 1;
        if id <= 5 {
            let angle_n = 0.2_f32 * (id - 1) as f32 * full_turn;
            let angle_w = angle_n - 0.1_f32 * full_turn;
            let angle_e = angle_n + 0.1_f32 * full_turn;
            IcosahedronDiamondCorners {
                south: CartesianPoint::new(0.0, 0.0, -radius as f64),
                north: CartesianPoint::new(
                    (erador5 * 2.0 * angle_n.cos()) as f64,
                    (erador5 * 2.0 * angle_n.sin()) as f64,
                    erador5 as f64,
                ),
                west: CartesianPoint::new(
                    (erador5 * 2.0 * angle_w.cos()) as f64,
                    (erador5 * 2.0 * angle_w.sin()) as f64,
                    -erador5 as f64,
                ),
                east: CartesianPoint::new(
                    (erador5 * 2.0 * angle_e.cos()) as f64,
                    (erador5 * 2.0 * angle_e.sin()) as f64,
                    -erador5 as f64,
                ),
            }
        } else {
            let angle_s = 0.2_f32 * (id - 6) as f32 * full_turn + 0.1_f32 * full_turn;
            let angle_w = angle_s - 0.1_f32 * full_turn;
            let angle_e = angle_s + 0.1_f32 * full_turn;
            IcosahedronDiamondCorners {
                south: CartesianPoint::new(
                    (erador5 * 2.0 * angle_s.cos()) as f64,
                    (erador5 * 2.0 * angle_s.sin()) as f64,
                    -erador5 as f64,
                ),
                north: CartesianPoint::new(0.0, 0.0, radius as f64),
                west: CartesianPoint::new(
                    (erador5 * 2.0 * angle_w.cos()) as f64,
                    (erador5 * 2.0 * angle_w.sin()) as f64,
                    erador5 as f64,
                ),
                east: CartesianPoint::new(
                    (erador5 * 2.0 * angle_e.cos()) as f64,
                    (erador5 * 2.0 * angle_e.sin()) as f64,
                    erador5 as f64,
                ),
            }
        }
    })
}

/// Point-coordinate portion of `icosahedron.F90:icosahedron`.
///
/// This initializes the allocated point counts, the 12 pentagonal M-point
/// indices, the 10 big-diamond corner coordinates, and the pre-spring M-point
/// coordinates. Connectivity construction (`fill_diamond`/`tri_neighbors`) and
/// spring relaxation remain separate architecture surfaces.
pub fn icosahedron_initial_grid_canonical(nxp0: usize) -> Option<IcosahedronInitialGrid> {
    let counts = icosahedron_counts_canonical(nxp0)?;
    let diamond_corners = icosahedron_diamond_corners_canonical();
    let mut impent = [0usize; 12];
    let mut m_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); counts.nmd + 1];
    let pwrd = 0.9_f32;
    let radius = METHOD_C_CANONICAL_EARTH_RADIUS_METERS as f32;

    impent[0] = 2;
    impent[11] = counts.nmd;

    for ibigd in 1..=10 {
        let corners = diamond_corners[ibigd - 1];
        for j in 1..=nxp0 {
            for i in 1..=nxp0 {
                let idiamond = (ibigd - 1) * nxp0 * nxp0 + (j - 1) * nxp0 + i;
                let im_left = idiamond + 2;
                if i == 1 && j == nxp0 {
                    impent[ibigd] = im_left;
                }

                let (mut wts, mut wtn, wtw0, wte0) = if i + j <= nxp0 {
                    (
                        ((nxp0 + 1 - i - j) as f32 / nxp0 as f32).clamp(0.0, 1.0),
                        0.0,
                        (j as f32 / (i + j - 1) as f32).clamp(0.0, 1.0),
                        1.0 - (j as f32 / (i + j - 1) as f32).clamp(0.0, 1.0),
                    )
                } else {
                    let wte0 = ((nxp0 - j) as f32 / (2 * nxp0 + 1 - i - j) as f32).clamp(0.0, 1.0);
                    (
                        0.0,
                        ((i + j - nxp0 - 1) as f32 / nxp0 as f32).clamp(0.0, 1.0),
                        1.0 - wte0,
                        wte0,
                    )
                };

                let mut wtw = (1.0 - wts - wtn) * wtw0;
                let mut wte = (1.0 - wts - wtn) * wte0;
                let sumwt = wts.powf(pwrd) + wtn.powf(pwrd) + wtw.powf(pwrd) + wte.powf(pwrd);
                if sumwt == 0.0 {
                    return None;
                }
                wts = wts.powf(pwrd) / sumwt;
                wtn = wtn.powf(pwrd) / sumwt;
                wtw = wtw.powf(pwrd) / sumwt;
                wte = wte.powf(pwrd) / sumwt;

                let point = CartesianPoint::new(
                    (wts * corners.south.x as f32
                        + wtn * corners.north.x as f32
                        + wtw * corners.west.x as f32
                        + wte * corners.east.x as f32) as f64,
                    (wts * corners.south.y as f32
                        + wtn * corners.north.y as f32
                        + wtw * corners.west.y as f32
                        + wte * corners.east.y as f32) as f64,
                    (wts * corners.south.z as f32
                        + wtn * corners.north.z as f32
                        + wtw * corners.west.z as f32
                        + wte * corners.east.z as f32) as f64,
                );
                let norm = ((point.x as f32).powi(2)
                    + (point.y as f32).powi(2)
                    + (point.z as f32).powi(2))
                .sqrt();
                if norm == 0.0 {
                    return None;
                }
                let expansion = radius / norm;
                m_points[im_left] = CartesianPoint::new(
                    (point.x as f32 * expansion) as f64,
                    (point.y as f32 * expansion) as f64,
                    (point.z as f32 * expansion) as f64,
                );
            }
        }
    }

    m_points[2] = CartesianPoint::new(0.0, 0.0, -radius as f64);
    m_points[counts.nmd] = CartesianPoint::new(0.0, 0.0, radius as f64);

    Some(IcosahedronInitialGrid {
        nmd: counts.nmd,
        nud: counts.nud,
        nwd: counts.nwd,
        impent,
        diamond_corners,
        m_points,
    })
}
