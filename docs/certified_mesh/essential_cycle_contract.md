# Canonical essential-cycle contract for W2 face bands

Alpha6 represents a valid two-band (`W2`) interface by its canonical primal
cycle instead of by runtime face slots. This PR adds construction and lossless
conversion only; it does not add a cycle search, change a product gate, or
unlock N12/NXP80.

## Equivalence

Let `K` be the fixed triangulated transition annulus, with coarse boundary
faces `Bc` and fine boundary faces `Bf`. A valid legacy W2 plan labels every
transition face `0` or `1`, fixes `Bc` to `0` and `Bf` to `1`, keeps each label
connected and annular, and makes the unlike-label primal edges one internal
simple cycle that satisfies all anchor policies.

For such a plan, collecting every primal edge whose incident faces have
different labels produces one simple cycle: the legacy hard gate already
requires degree two at every interface vertex and one connected interface.
Because coarse and fine boundary faces lie on opposite sides, the cycle is
essential in the annulus.

Conversely, remove an internal simple cycle from the face-dual graph. Flooding
from `Bc` uniquely labels its component `0`; the remaining connected component
containing `Bf` is uniquely labelled `1`. The recovered plan is accepted only
if the existing legacy W2 validator confirms both annular strips and every
anchor rule. Therefore the conversion does not weaken the established hard
contract.

The canonical cycle key starts at the least canonical vertex and chooses the
lexicographically smaller traversal direction. Runtime face and vertex slots
are not part of the key.

## Candidate graph and dual seam

Candidate primal edges are canonical edges between two transition faces. Edges
touching a fixed coarse/fine boundary vertex or an anchor that must remain
inside one band are excluded. Each candidate records its two incident
canonical faces and its incident candidate vertices.

A deterministic breadth-first path in the dual graph connects `Bc` to `Bf`.
The candidate primal edges crossed by this path form the frozen dual seam. A
closed candidate cycle must cross that seam an odd number of times. This
mod-two test is only a cheap necessary prune: final acceptance still removes
the selected edges and proves by dual flood that the coarse and fine boundary
sets lie in exactly two connected sides.

## Public conversion surface

- `build_essential_cycle_problem`
- `essential_cycle_from_face_band_plan`
- `face_band_plan_from_essential_cycle`
- `validate_selected_essential_cycle`
- `essential_cycle_seam_parity`

The Frozen N6 F0 W2 plan and the N12-Lifted-N6 W2 plan both round-trip with
identical labels, interface edges, band counts, and legacy fingerprint. Tests
also reject an open path, multiple cycles, a contractible cycle, and a cycle
touching a fixed boundary. An odd-parity mutation is rejected by the final
dual flood, preserving the distinction between a prune and a certificate.
