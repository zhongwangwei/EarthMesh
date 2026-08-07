use super::*;

pub(crate) fn assign_method_c_triangle_seed_w_ids(
    face_seeds: &[MethodCTriangleSeed],
) -> io::Result<Vec<usize>> {
    let mut assigned = vec![0usize; face_seeds.len()];
    let mut occupied = BTreeSet::<usize>::new();

    for (idx, seed) in face_seeds.iter().enumerate() {
        if seed.target_iw <= 1 {
            continue;
        }
        if !occupied.insert(seed.target_iw) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate Method-C target W id {}", seed.target_iw),
            ));
        }
        assigned[idx] = seed.target_iw;
    }

    let mut next_iw = 2usize;
    for iw in &mut assigned {
        if *iw > 1 {
            continue;
        }
        while occupied.contains(&next_iw) {
            next_iw += 1;
        }
        *iw = next_iw;
        occupied.insert(next_iw);
    }

    Ok(assigned)
}

pub(crate) fn insert_or_attach_method_c_edge(
    u_edges: &mut Vec<IcosahedronUEdge>,
    edge_by_key: &mut BTreeMap<(usize, usize), usize>,
    reserved_u_ids: &BTreeSet<usize>,
    occupied_u_ids: &mut BTreeSet<usize>,
    next_auto_iu: &mut usize,
    iw: usize,
    from: usize,
    to: usize,
    existing_face_edge: usize,
    target_iu: usize,
) -> io::Result<usize> {
    debug_assert_eq!(existing_face_edge, 1);
    let key = method_c_edge_key(from, to);
    if let Some(&iu) = edge_by_key.get(&key) {
        if target_iu > 1 && target_iu != iu {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "W face {iw} target U edge {target_iu} conflicts with existing shared U edge {iu}"
                ),
            ));
        }
        let edge = u_edges.get_mut(iu).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing U edge {iu} while attaching W face {iw}"),
            )
        })?;
        if edge.iw[1] > 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("U edge {iu} has more than two adjacent W faces"),
            ));
        }
        if edge.im != [to, from] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "W face {iw} shares U edge {iu} with inconsistent orientation [{from}, {to}]"
                ),
            ));
        }
        edge.iw[1] = iw;
        return Ok(iu);
    }

    let iu = if target_iu > 1 {
        if !occupied_u_ids.insert(target_iu) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate target U edge id {target_iu} while inserting W face {iw}"),
            ));
        }
        target_iu
    } else {
        while reserved_u_ids.contains(next_auto_iu) || occupied_u_ids.contains(next_auto_iu) {
            *next_auto_iu += 1;
        }
        let iu = *next_auto_iu;
        occupied_u_ids.insert(iu);
        *next_auto_iu += 1;
        iu
    };

    if u_edges.len() <= iu {
        u_edges.resize(iu + 1, IcosahedronUEdge::default());
    }
    let mut edge = IcosahedronUEdge {
        im: [from, to],
        mrlu: 1,
        ..IcosahedronUEdge::default()
    };
    edge.iw[0] = iw;
    u_edges[iu] = edge;
    edge_by_key.insert(key, iu);
    Ok(iu)
}
