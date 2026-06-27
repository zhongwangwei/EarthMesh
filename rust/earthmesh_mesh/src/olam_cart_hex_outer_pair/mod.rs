use super::*;

pub(crate) fn order_olam_outer_w_pair_for_fill_rad3(
    w_faces: &[IcosahedronWFace],
    pair: [usize; 2],
    outer_candidates: [usize; 6],
    imx: usize,
) -> io::Result<[usize; 2]> {
    if let Some(ordered) = order_olam_outer_w_pair_candidate(w_faces, pair, outer_candidates, imx)?
    {
        return Ok(ordered);
    }
    if let Some(ordered) =
        order_olam_outer_w_pair_candidate(w_faces, [pair[1], pair[0]], outer_candidates, imx)?
    {
        return Ok(ordered);
    }
    Ok(pair)
}

fn order_olam_outer_w_pair_candidate(
    w_faces: &[IcosahedronWFace],
    pair: [usize; 2],
    outer_candidates: [usize; 6],
    imx: usize,
) -> io::Result<Option<[usize; 2]>> {
    let nwd = w_faces.len().saturating_sub(1);
    require_olam_id("OLAM cart_hex outer W pair", pair[0], nwd)?;
    require_olam_id("OLAM cart_hex outer W pair", pair[1], nwd)?;
    if !w_faces[pair[0]].im.contains(&imx) {
        return Ok(None);
    }
    let (im1, im2) = face_following_two_vertices(w_faces[pair[0]], imx, pair[0])?;
    if w_faces[pair[1]].im.contains(&im2) {
        let im3 = face_following_vertex(w_faces[pair[1]], im2, pair[1])?;
        if im3 != im1 {
            return Ok(Some(pair));
        }
    }
    for iwy in w_faces[pair[0]].iw {
        if iwy <= 1 {
            continue;
        }
        require_olam_id("OLAM cart_hex iwx W neighbor", iwy, nwd)?;
        if iwy != pair[0] && w_faces[iwy].im.contains(&im2) {
            let im3 = face_following_vertex(w_faces[iwy], im2, iwy)?;
            if im3 != im1 {
                return Ok(Some([pair[0], iwy]));
            }
        }
    }
    for iwy in outer_candidates {
        require_olam_id("OLAM cart_hex outer W candidate", iwy, nwd)?;
        if iwy != pair[0] && w_faces[iwy].im.contains(&im2) {
            let im3 = face_following_vertex(w_faces[iwy], im2, iwy)?;
            if im3 != im1 {
                return Ok(Some([pair[0], iwy]));
            }
        }
    }
    Ok(Some(pair))
}
