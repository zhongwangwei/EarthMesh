use super::*;

fn to_usize_index(value: isize) -> Option<usize> {
    if value < 0 {
        None
    } else {
        Some(value as usize)
    }
}

fn fill_diamond_fortran_indexed(
    u_edges: &mut [IcosahedronUEdge],
    w_faces: &mut [IcosahedronWFace],
    im_left: usize,
    im_right: usize,
    im_top: usize,
    im_bot: usize,
    iu0: usize,
    iu1: usize,
    iu2: usize,
    iu3: usize,
    iu4: usize,
    iw1: usize,
    iw2: usize,
) -> Option<()> {
    let edge0 = u_edges.get_mut(iu0)?;
    edge0.im = [im_left, im_right];
    edge0.iw[0] = iw1;
    edge0.iw[1] = iw2;
    edge0.mrlu = 1;

    let edge1 = u_edges.get_mut(iu1)?;
    edge1.im = [im_left, im_bot];
    edge1.iw[1] = iw1;
    edge1.mrlu = 1;

    u_edges.get_mut(iu2)?.iw[0] = iw1;

    let edge3 = u_edges.get_mut(iu3)?;
    edge3.im = [im_top, im_left];
    edge3.iw[1] = iw2;
    edge3.mrlu = 1;

    u_edges.get_mut(iu4)?.iw[0] = iw2;

    let face1 = w_faces.get_mut(iw1)?;
    face1.iu = [iu0, iu1, iu2];
    face1.mrlw = 1;
    face1.mrlw_orig = 1;
    face1.ngr = 1;

    let face2 = w_faces.get_mut(iw2)?;
    face2.iu = [iu0, iu4, iu3];
    face2.mrlw = 1;
    face2.mrlw_orig = 1;
    face2.ngr = 1;

    Some(())
}

/// Port of the `fill_diamond` invocation loop inside
/// `icosahedron.F90:icosahedron`.
///
/// This preserves Fortran's 1-based allocated-array convention by returning
/// vectors with indices `0` and `1` unused/defaulted. It only covers the fields
/// explicitly written by `fill_diamond`; `tri_neighbors` is responsible for
/// later reciprocal U/W/M neighbor completion.
pub fn icosahedron_fill_diamonds_fortran(nxp0: usize) -> Option<IcosahedronDiamondConnectivity> {
    let counts = icosahedron_counts_fortran(nxp0)?;
    let mut u_edges = vec![IcosahedronUEdge::default(); counts.nud + 1];
    let mut w_faces = vec![IcosahedronWFace::default(); counts.nwd + 1];
    let ibigd_ne = [6isize, 7, 8, 9, 10, 7, 8, 9, 10, 6];
    let ibigd_se = [2isize, 3, 4, 5, 1, 2, 3, 4, 5, 1];
    let n = nxp0 as isize;
    let n2 = n * n;

    for ibigd in 1..=10isize {
        for j in 1..=n {
            for i in 1..=n {
                let idiamond = (ibigd - 1) * n2 + (j - 1) * n + i;
                let im_left = to_usize_index(idiamond + 2)?;
                let iu0 = to_usize_index(3 * idiamond)?;
                let iu1 = to_usize_index(3 * idiamond - 1)?;
                let iu3 = to_usize_index(3 * idiamond + 1)?;
                let iw1 = to_usize_index(2 * idiamond)?;
                let iw2 = to_usize_index(2 * idiamond + 1)?;

                let (im_right, im_top, im_bot, iu2, iu4) = if ibigd < 6 {
                    let idiamond_top = if i < n {
                        idiamond + 1
                    } else {
                        (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (j - 1) * n + 1
                    };
                    let im_top = idiamond_top + 2;
                    let iu4 = 3 * idiamond_top - 1;

                    let (idiamond_right, mut iu2) = if j > 1 && i < n {
                        (idiamond - n + 1, 0)
                    } else if j == 1 {
                        let right = (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (i - 1) * n + 1;
                        (right, 3 * right - 1)
                    } else {
                        (
                            (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (j - 2) * n + 1,
                            0,
                        )
                    };
                    let im_right = idiamond_right + 2;

                    let idiamond_bot = if j > 1 {
                        let bottom = idiamond - n;
                        iu2 = 3 * bottom + 1;
                        bottom
                    } else {
                        (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (i - 2) * n + 1
                    };
                    let mut im_bot = idiamond_bot + 2;
                    if i == 1 && j == 1 {
                        im_bot = 2;
                    }
                    (im_right, im_top, im_bot, iu2, iu4)
                } else {
                    let (idiamond_top, mut iu4) = if i < n {
                        let top = idiamond + 1;
                        (top, 3 * top - 1)
                    } else {
                        (
                            (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + j + 1,
                            0,
                        )
                    };
                    let mut im_top = idiamond_top + 2;

                    let idiamond_right = if j > 1 && i < n {
                        idiamond - n + 1
                    } else if j == 1 && i < n {
                        (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + i + 1
                    } else {
                        let right = (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + j;
                        iu4 = 3 * right + 1;
                        right
                    };
                    let im_right = idiamond_right + 2;

                    let idiamond_bot = if j > 1 {
                        idiamond - n
                    } else {
                        (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + i
                    };
                    let im_bot = idiamond_bot + 2;
                    let iu2 = 3 * idiamond_bot + 1;

                    if i == n && j == n {
                        im_top = 10 * n2 + 3;
                    }
                    (im_right, im_top, im_bot, iu2, iu4)
                };

                fill_diamond_fortran_indexed(
                    &mut u_edges,
                    &mut w_faces,
                    im_left,
                    to_usize_index(im_right)?,
                    to_usize_index(im_top)?,
                    to_usize_index(im_bot)?,
                    iu0,
                    iu1,
                    to_usize_index(iu2)?,
                    iu3,
                    to_usize_index(iu4)?,
                    iw1,
                    iw2,
                )?;
            }
        }
    }

    Some(IcosahedronDiamondConnectivity { u_edges, w_faces })
}
