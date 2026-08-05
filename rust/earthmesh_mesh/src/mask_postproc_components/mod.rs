use super::*;

/// Result of pruning a masked domain down to its largest edge-connected piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LargestComponentRetention {
    /// Edge-connected components the carved domain started out with.
    pub component_count: usize,
    /// Cell count of the component that was kept.
    pub retained_cell_count: usize,
    /// Canonical ids of the cells dropped from the domain, ascending.
    pub removed_cell_ids: Vec<usize>,
    /// How many of the dropped cells came from splitting non-manifold vertex fans.
    pub non_manifold_removed_cell_count: usize,
    /// Demanded cells dropped anyway because they were left alone.
    ///
    /// Demand keeps a component whatever its size, but it cannot make a lone
    /// cell usable: with no edge neighbour it exchanges nothing with the rest
    /// of the mesh, and the `orphan_cell` gate rejects it. Such a cell goes,
    /// and this is how many did, so a run can say the region it named came out
    /// too thin to keep rather than lose it in silence.
    pub demanded_isolated_removed_cell_count: usize,
}

/// Keep only the largest edge-connected component of the masked domain.
///
/// A land/sea carve marks cells purely by their own centre sample, so a coastal
/// domain routinely ends up with cells that touch the main body at a single
/// vertex, or not at all — narrow bays and river mouths whose water channel is
/// thinner than one cell. Those pieces fail the `orphan_cell`,
/// `disconnected_mesh`, and `non_manifold_vertex_fan` topology gates, and no
/// amount of refinement repairs them (finer cells resolve *more* small bays).
///
/// Two cells are adjacent here when they share **two** vertices, i.e. a full
/// edge — the same adjacency the quality checker uses, so vertex-only contacts
/// do not hold a piece in the domain. Ties on component size are broken by the
/// smallest canonical cell id so the retained mesh is deterministic.
///
/// Pinch points are handled too: a vertex where the water body touches itself at
/// a single point leaves two incident-cell fans that share no edge, which the
/// `non_manifold_vertex_fan` gate rejects even when both fans reach the main body
/// some other way. Each such vertex keeps only its largest fan. Fan splitting and
/// component selection run alternately until the domain stops changing, since
/// either one can expose new work for the other.
///
/// `is_in_domain` uses the Canonical 1-based convention: entries equal to `1`
/// are in-domain, and dropped cells are set to `-1`, matching
/// [`remove_isolated_ocean_one_based`](super::remove_isolated_ocean_one_based).
pub fn retain_largest_edge_connected_component_one_based(
    is_in_domain: &mut [i32],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
) -> io::Result<LargestComponentRetention> {
    retain_edge_connected_components_with_hard_demand_one_based(
        is_in_domain,
        center_neighbors,
        center_neighbor_counts,
        vertex_neighbors,
        vertex_neighbor_counts,
        &[],
    )
}

/// As [`retain_largest_edge_connected_component_one_based`], but a component
/// holding hard demand is kept whatever its size.
///
/// Component size is a proxy for "this piece is worth simulating", and it is
/// the wrong answer where a run has said outright which cells it wants: a
/// refinement circle over a small bay produces exactly the disjoint piece the
/// largest-component rule deletes, and nothing reports that the region the user
/// named is gone. Demand is not a proxy, so it wins.
///
/// `hard_demand` is indexed by one-based centre id and may be shorter than the
/// domain or empty; anything it does not cover is simply not demanded.
pub fn retain_edge_connected_components_with_hard_demand_one_based(
    is_in_domain: &mut [i32],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    hard_demand: &[bool],
) -> io::Result<LargestComponentRetention> {
    if center_neighbor_counts.len() < center_neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "center_neighbor_counts must cover center_neighbors",
        ));
    }
    if vertex_neighbor_counts.len() < vertex_neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex_neighbor_counts must cover vertex_neighbors",
        ));
    }

    let mut removed_cell_ids: BTreeSet<usize> = BTreeSet::new();
    let mut non_manifold_removed_cell_count = 0usize;
    let mut demanded_isolated_removed_cell_count = 0usize;
    // Reported as the domain's own component count, so it stays a diagnostic of
    // what the carve produced rather than of what pruning converged to.
    let mut initial_component_count = None;
    // Each pass removes at least one cell or terminates, so this converges.
    let latest = loop {
        // Component selection goes first so the reported count is the carve's
        // own, before any fan pruning has reshaped the domain.
        let pass = retain_largest_component_pass_one_based(
            is_in_domain,
            center_neighbors,
            center_neighbor_counts,
            vertex_neighbors,
            vertex_neighbor_counts,
            hard_demand,
        )?;
        removed_cell_ids.extend(pass.removed_cell_ids.iter().copied());
        initial_component_count.get_or_insert(pass.component_count);
        let pass_removed_nothing = pass.removed_cell_ids.is_empty();

        let fan_removed = split_non_manifold_vertex_fans_one_based(
            is_in_domain,
            center_neighbors,
            center_neighbor_counts,
            vertex_neighbors,
            vertex_neighbor_counts,
            hard_demand,
        )?;
        demanded_isolated_removed_cell_count += pass.demanded_isolated_removed_cell_count;
        non_manifold_removed_cell_count += fan_removed.len();
        removed_cell_ids.extend(fan_removed.iter().copied());

        if fan_removed.is_empty() && pass_removed_nothing {
            break pass;
        }
    };

    Ok(LargestComponentRetention {
        component_count: initial_component_count.unwrap_or(latest.component_count),
        retained_cell_count: latest.retained_cell_count,
        removed_cell_ids: removed_cell_ids.into_iter().collect(),
        non_manifold_removed_cell_count,
        demanded_isolated_removed_cell_count,
    })
}

/// One largest-component selection pass; see the wrapper for the full contract.
fn retain_largest_component_pass_one_based(
    is_in_domain: &mut [i32],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    hard_demand: &[bool],
) -> io::Result<LargestComponentRetention> {
    let domain_cells: Vec<usize> = (0..is_in_domain.len())
        .filter(|&cell_id| is_in_domain[cell_id] == 1)
        .collect();
    if domain_cells.is_empty() {
        return Ok(LargestComponentRetention {
            component_count: 0,
            retained_cell_count: 0,
            removed_cell_ids: Vec::new(),
            non_manifold_removed_cell_count: 0,
            demanded_isolated_removed_cell_count: 0,
        });
    }

    let mut component_of: BTreeMap<usize, usize> = BTreeMap::new();
    let mut component_sizes: Vec<usize> = Vec::new();
    for &seed in &domain_cells {
        if component_of.contains_key(&seed) {
            continue;
        }
        let component_id = component_sizes.len();
        let mut size = 0usize;
        let mut queue = vec![seed];
        component_of.insert(seed, component_id);
        while let Some(cell_id) = queue.pop() {
            size += 1;
            for neighbor in edge_neighbors_one_based(
                cell_id,
                is_in_domain,
                center_neighbors,
                center_neighbor_counts,
                vertex_neighbors,
                vertex_neighbor_counts,
            )? {
                if component_of.contains_key(&neighbor) {
                    continue;
                }
                component_of.insert(neighbor, component_id);
                queue.push(neighbor);
            }
        }
        component_sizes.push(size);
    }

    // Largest component wins; the smallest seed id breaks ties because
    // `domain_cells` is ascending and component ids are handed out in that order.
    let retained_component = component_sizes
        .iter()
        .enumerate()
        .max_by_key(|(component_id, size)| (**size, std::cmp::Reverse(*component_id)))
        .map(|(component_id, _)| component_id)
        .expect("non-empty domain yields at least one component");

    let demanded = |cell_id: usize| hard_demand.get(cell_id).copied().unwrap_or(false);
    let mut component_has_demand = vec![false; component_sizes.len()];
    for (&cell_id, &component_id) in &component_of {
        if demanded(cell_id) {
            component_has_demand[component_id] = true;
        }
    }

    let mut removed_cell_ids = Vec::new();
    let mut retained_cell_count = 0usize;
    let mut demanded_isolated_removed_cell_count = 0usize;
    for (&cell_id, &component_id) in &component_of {
        // A one-cell component is an orphan by definition. Demand is why a
        // small component survives at all, but it cannot buy this one a
        // neighbour, and keeping it only moves the failure to the quality gate.
        let alone = component_sizes[component_id] < 2;
        let demanded = component_has_demand[component_id];
        if component_id == retained_component || (demanded && !alone) {
            retained_cell_count += 1;
        } else {
            if demanded && alone {
                demanded_isolated_removed_cell_count += 1;
            }
            is_in_domain[cell_id] = -1;
            removed_cell_ids.push(cell_id);
        }
    }

    Ok(LargestComponentRetention {
        component_count: component_sizes.len(),
        retained_cell_count,
        removed_cell_ids,
        non_manifold_removed_cell_count: 0,
        demanded_isolated_removed_cell_count,
    })
}

/// Drop the smaller incident-cell fans at every non-manifold in-domain vertex.
///
/// A vertex is non-manifold when its in-domain cells fall into more than one
/// edge-connected fan — the mesh pinches to a point there. Keeping the largest
/// fan is what makes the pinch go away; component selection alone cannot, since
/// the fans often rejoin elsewhere. Returns the dropped cell ids, ascending.
fn split_non_manifold_vertex_fans_one_based(
    is_in_domain: &mut [i32],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    hard_demand: &[bool],
) -> io::Result<Vec<usize>> {
    let mut removed_cell_ids = BTreeSet::new();
    for vertex_id in 0..vertex_neighbors.len() {
        let center_row = &vertex_neighbors[vertex_id];
        let center_count = vertex_neighbor_counts[vertex_id];
        if center_count > center_row.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} center count exceeds row width"),
            ));
        }
        let incident: Vec<usize> = center_row
            .iter()
            .take(center_count)
            .copied()
            .filter(|&cell_id| cell_id < is_in_domain.len() && is_in_domain[cell_id] == 1)
            .collect();
        if incident.len() < 2 {
            continue;
        }

        let vertices_of = |cell_id: usize| -> io::Result<BTreeSet<usize>> {
            let row = center_neighbors.get(cell_id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("center {cell_id} missing neighbor row"),
                )
            })?;
            let count = center_neighbor_counts[cell_id];
            if count > row.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("center {cell_id} neighbor count exceeds row width"),
                ));
            }
            Ok(row.iter().take(count).copied().collect())
        };

        // Fans are edge-connected runs within the cells incident to this vertex.
        let mut fan_of: BTreeMap<usize, usize> = BTreeMap::new();
        let mut fans: Vec<Vec<usize>> = Vec::new();
        for &seed in &incident {
            if fan_of.contains_key(&seed) {
                continue;
            }
            let fan_id = fans.len();
            let mut members = Vec::new();
            let mut queue = vec![seed];
            fan_of.insert(seed, fan_id);
            while let Some(cell_id) = queue.pop() {
                members.push(cell_id);
                let cell_vertices = vertices_of(cell_id)?;
                for &other in &incident {
                    if other == cell_id || fan_of.contains_key(&other) {
                        continue;
                    }
                    if cell_vertices.intersection(&vertices_of(other)?).count() >= 2 {
                        fan_of.insert(other, fan_id);
                        queue.push(other);
                    }
                }
            }
            fans.push(members);
        }
        if fans.len() < 2 {
            continue;
        }

        // Only one fan can survive a pinch, so demand decides which before size
        // does. Choosing purely by size would delete the cells the component
        // pass had just protected -- the caller promises a demanded region
        // survives, and this runs inside the same loop.
        let kept_fan = fans
            .iter()
            .enumerate()
            .max_by_key(|(fan_id, members)| {
                let demanded = members
                    .iter()
                    .filter(|&&cell_id| hard_demand.get(cell_id).copied().unwrap_or(false))
                    .count();
                (demanded, members.len(), std::cmp::Reverse(*fan_id))
            })
            .map(|(fan_id, _)| fan_id)
            .expect("at least two fans");
        for (fan_id, members) in fans.iter().enumerate() {
            if fan_id == kept_fan {
                continue;
            }
            for &cell_id in members {
                is_in_domain[cell_id] = -1;
                removed_cell_ids.insert(cell_id);
            }
        }
    }
    Ok(removed_cell_ids.into_iter().collect())
}

/// In-domain cells sharing a full edge (two vertices) with `cell_id`.
fn edge_neighbors_one_based(
    cell_id: usize,
    is_in_domain: &[i32],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
) -> io::Result<Vec<usize>> {
    let vertex_row = center_neighbors.get(cell_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("center {cell_id} missing neighbor row"),
        )
    })?;
    let vertex_count = center_neighbor_counts[cell_id];
    if vertex_count > vertex_row.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("center {cell_id} neighbor count exceeds row width"),
        ));
    }

    let mut shared_vertex_counts: BTreeMap<usize, usize> = BTreeMap::new();
    for &vertex_id in vertex_row.iter().take(vertex_count) {
        require_vertex_count(vertex_id, vertex_neighbor_counts)?;
        let center_row = vertex_neighbors.get(vertex_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} missing vertex_neighbors row"),
            )
        })?;
        let center_count = vertex_neighbor_counts[vertex_id];
        if center_count > center_row.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} center count exceeds row width"),
            ));
        }
        for &other_cell_id in center_row.iter().take(center_count) {
            if other_cell_id == cell_id {
                continue;
            }
            if other_cell_id >= is_in_domain.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} canonicals center {other_cell_id}, outside is_in_domain"
                    ),
                ));
            }
            if is_in_domain[other_cell_id] != 1 {
                continue;
            }
            *shared_vertex_counts.entry(other_cell_id).or_insert(0) += 1;
        }
    }

    Ok(shared_vertex_counts
        .into_iter()
        .filter(|&(_, shared)| shared >= 2)
        .map(|(other_cell_id, _)| other_cell_id)
        .collect())
}
