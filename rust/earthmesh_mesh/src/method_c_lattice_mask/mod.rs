//! Refine from a face mask instead of from regions.
//!
//! `spawn_nest` takes shapes and searches for a legal patch that covers them,
//! which is where a criteria-driven region gets refused: the shape came from
//! data and need not sit on anything the perimeter walk can close. This module
//! offers the other direction, for a caller that already has a face set and
//! wants it refined without a search.
//!
//! It does **not** offer legality by construction. That was the premise of the
//! nest-compiler design, and it was measured and did not hold -- see
//! `docs/experiments/2026-08_lattice_invariants.md`. A union of whole rad3
//! footprints on the stride-3 lattice, every seed held more than three M rings
//! from any defect, closes as triplets 95% of the time at two seeds, 66% at
//! five, and 21% at nine; a contiguous run of four, which is the shape a real
//! refinement region has, closes 40% of the time. So a caller has to ask
//! `method_c_mask_closes` and be ready for a no.
//!
//! What is measured and does hold is the single footprint: beyond three rings
//! from a defect, 362 of 362 on the base mesh and 135 of 135 on the child
//! generation. Within three rings the footprint is a different shape -- 50 faces
//! and a 16 or 17 point ring, against 54 and 18 out in the regular field -- and
//! better than half break. Both generations behave the same way, so the rule is
//! about distance from a defect and not about depth.

use std::collections::{BTreeSet, VecDeque};
use std::io;

use super::*;

/// Rings a seed has to keep from a defect for its own footprint to be regular.
///
/// Measured: beyond three rings, 362 of 362 single footprints on the base mesh
/// and 135 of 135 on the child generation close as triplets, with no failure at
/// any greater distance. Within three, better than half break. Three is also
/// what `method_c_perimeter_mrows` writes as `old_row > -3`, which is the
/// clearance the kernel already demands between a child patch and its parent's
/// fine edge.
///
/// It buys the single footprint and nothing more. Unions still break, at a rate
/// that grows with how many seeds they hold.
pub const METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS: usize = 3;

impl TriangularMesh {
    /// M rings from each point to the nearest defect.
    ///
    /// A defect is one of the twelve icosahedral pentagons or any point the
    /// transition band left without a six-edge ring. `usize::MAX` marks a point
    /// no walk reached.
    pub fn method_c_defect_ring_distance(
        &self,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let mut distance = vec![usize::MAX; self.nmd + 1];
        let mut queue = VecDeque::new();
        for im in 2..=self.nmd {
            if self.impent.contains(&im) || m_neighbors[im].npoly != 6 {
                distance[im] = 0;
                queue.push_back(im);
            }
        }
        while let Some(im) = queue.pop_front() {
            let neighbors = m_neighbors[im];
            for j in 0..neighbors.npoly.min(6) {
                let Ok(next) = self.other_m_endpoint(neighbors.iu[j], im) else {
                    continue;
                };
                if next > 1 && next <= self.nmd && distance[next] == usize::MAX {
                    distance[next] = distance[im] + 1;
                    queue.push_back(next);
                }
            }
        }
        Ok(distance)
    }

    /// Stride-3 lattice seeds reachable from `start`, none of them nearer a
    /// defect than `clearance` rings.
    ///
    /// The walk is the selection walker's own `thirdm` step, so the lattice is
    /// the one the kernel already agrees with rather than a second definition
    /// of the same thing. `cap` bounds the enumeration; ordering is by id, so
    /// the result does not depend on traversal order.
    pub fn method_c_lattice_seeds_with_clearance(
        &self,
        start: usize,
        cap: usize,
        clearance: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        require_method_c_id("Method-C lattice start M point", start, self.nmd)?;
        let distance = self.method_c_defect_ring_distance(m_neighbors)?;
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut reached = BTreeSet::new();
        let mut queue = VecDeque::new();
        reached.insert(start);
        queue.push_back(start);
        while let Some(im) = queue.pop_front() {
            if reached.len() >= cap {
                break;
            }
            let Ok(thirds) = self.method_c_thirdm_neighbors_canonical_with_neighbors(
                im,
                &mut jdone,
                m_neighbors,
            ) else {
                continue;
            };
            for third in thirds {
                if third > 1 && third <= self.nmd && reached.insert(third) {
                    queue.push_back(third);
                }
            }
        }
        Ok(reached
            .into_iter()
            .filter(|&im| distance[im] > clearance)
            .collect())
    }

    /// The union of the seeds' whole rad3 footprints.
    ///
    /// Whole footprints, never single faces. Dropping one face from a mask that
    /// closes usually breaks it, so a repair chain working at face granularity
    /// is operating outside the class the count holds over. Whole footprints are
    /// the better granularity; they are not a guarantee.
    pub fn method_c_footprint_mask(
        &self,
        seeds: &[usize],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<bool>> {
        let mut selected = vec![false; self.nwd + 1];
        for &seed in seeds {
            self.mark_fill_rad3_faces_with_neighbors(seed, &mut selected, m_neighbors)?;
        }
        Ok(selected)
    }

    /// Whether a mask closes: every perimeter walks, and every ring is a
    /// multiple of three.
    ///
    /// The two gates a mask has to satisfy before the kernel will take it. A
    /// compiler runs this as its own check; a caller building masks by hand
    /// runs it to find out whether it built one the kernel can use.
    pub fn method_c_mask_closes(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> bool {
        self.method_c_perimeters_from_selected_faces(selected, m_neighbors)
            .map(|perimeters| Self::method_c_perimeters_are_triplets(&perimeters))
            .unwrap_or(false)
    }

    /// Refine `passes` generations, taking each generation's face mask from
    /// `mask_for`.
    ///
    /// The mask for pass k indexes the faces of the mesh pass k-1 produced, and
    /// that mesh does not exist until pass k-1 has run. So the masks arrive
    /// through a callback rather than as a vector prepared up front, which is
    /// the same interleaving `spawn_nest_internal` already has.
    ///
    /// No region search, no radius ladder, no annealing. A mask that the kernel
    /// refuses is returned as the kernel's own error, naming the gate: this
    /// entry point does not repair, because a caller that needs repair has
    /// produced a mask outside the class and should be told so.
    pub fn spawn_nest_from_face_masks<F>(
        &self,
        passes: usize,
        max_mrows: usize,
        mut mask_for: F,
    ) -> io::Result<Self>
    where
        F: FnMut(&Self, usize) -> io::Result<Vec<bool>>,
    {
        let mut mesh = self.clone();
        for pass in 0..passes {
            // Grid numbers are one based and the base mesh is 1, so the first
            // refined generation is 2.
            let child_level = pass + 2;
            let selected = mask_for(&mesh, child_level)?;
            require_method_c_len("selected_faces", selected.len(), mesh.nwd + 1)?;
            if !selected.iter().any(|&face| face) {
                // Nothing asked for at this depth, and nothing deeper will ask
                // either. Stopping is the answer; refining nothing and saying
                // so is not.
                break;
            }
            mesh = mesh.spawn_nest_pass_with_max_mrows(&selected, child_level, max_mrows, true)?;
        }
        Ok(mesh)
    }
}
