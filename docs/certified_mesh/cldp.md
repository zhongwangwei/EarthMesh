# Certified Local Decompression and Promotion (CLDP)

CLDP is the CMRC-CAT recovery path used after a compressed mixed mesh misses
the strict angle window. It is not a user-visible backend. The frozen target is
internal `40.2--79.8` degrees and final `40--80` degrees with the existing
topology, primal/dual, physical, balance, and remap certificates.

## Registered recovery order

1. preserve the best certified-topology W2 witness;
2. run the explicit linearized max-min trust solve;
3. map every active angle to finite source-face and hierarchy-parent support;
4. try local topology changes without adding vertices;
5. restore safe source faces locally and expand a finite collar as needed;
6. return the certified safe mother grid rather than publish a bad mesh.

W3 remains `TopologyClosedNoUsableEmbedding`: its initial candidates contain
non-positive faces, crossings, and rotation mismatches before angle
optimization. W4 is therefore disabled.

## Frozen N6 evidence through PR75

- PR61: W3 +1/+2 initial audits report `17/13` crossing pairs and two rotation
  mismatches each.
- PR62: W2 improves from `38.551143486745--81.453074281139` to
  `39.278499430048--80.721500570507` degrees; internal margin improves from
  `-1.653074281139` to `-0.921500570507` degrees.
- PR63: all 173 registered active/worst angles have non-empty source-face and
  parent support; 64 custom topology triangles carry deterministic provenance.
  Their one-ring supports merge into one Frozen N6 promotion component.
- PR64: a deterministic 128-state neighbourhood tried one to three local 2-2
  flips. Of 128 states, 61 failed hard topology gates and 67 reached local
  max-min geometry. None improved the preserved
  `39.278499430048--80.721500570507` degree incumbent, so the bounded search
  ended as `LocalTopologyBudgetExhausted` and CLDP proceeds to source-face
  promotion.
- PR65: the 285 violation-support faces require ten deterministic connector or
  hole-fill faces to form a valid P1 source union. P1 restores 295 exact mother
  faces behind one simple boundary; P2 restores 396 exact mother faces, split
  into 295 interior and 101 one-parent-ring collar faces. Both patches preserve
  exact triangle vertices, orientation, hierarchy address, and coordinates,
  with complete source-face coverage and stable fingerprints.
- PR66: the finite P1/P2/P3/P4 ladder promotes `295/396/468/468` source
  faces. Conservative custom-triangle provenance makes every materialized
  candidate the complete 362-vertex, 720-face N6 mother grid, so each is
  rejected as `NoCompressedExterior` rather than mislabeled adaptive. P5 then
  returns the certified safe mother fallback at
  `54.361673298250--72.000000000000` degrees. No strict mixed collar candidate
  exists under the current support decomposition.
- PR67: the safe fallback passes internal and final angle windows, anchor and
  ordinary degree gates, link/edge/Euler/charge, Delaunay/Voronoi, physical,
  balance, and remap with zero residuals. Its final counts are `V=362`,
  `E=1080`, `F=720`, but it retains zero coarse parents and has compression
  ratio `1.0`. The Frozen N6 strict mixed gate therefore fails only on
  `mixed_levels_delivered`.
- PR68: promotion input is restricted to 109 true strict violations; the 64
  non-violating solver guards remain optimization-only. Exact support shrinks
  from 285 to 275 source faces but still forms one component.
- PR69: the ten-parent retained-core planner exhaustively enumerates all 1024
  subsets. It records 191 connected non-empty candidates, including all ten
  single-release and 45 pair-release cases, in deterministic order.
- PR70: rebuilding every single-release candidate from the source hierarchy
  and the exact W2 face band produces ten exact
  `TopologyFamilyExhaustedNoSolution` outcomes. No candidate reaches geometry.
- PR71: of 45 pair-release candidates, 41 exhaust the registered topology
  family without a solution and four exhaust the one-million-state search
  budget. None closes topology, so geometry is not attempted.
- PR72: promotion patches now identify the fine exterior by an explicit seed,
  preserve protected coarse descendant components, and certify disk, annulus,
  multi-hole, or whole-sphere topology by boundary count and Euler
  characteristic. A constructed annular ring retains its coarse parent with
  two boundary cycles and Euler characteristic zero.
- PR73: the 14 incumbent full-polygon sectors exactly partition all 88 source
  annulus faces and own all 64 custom transition triangles. The 109 strict
  violation angles occupy 89 mixed faces: 45 custom and 44 hierarchy faces.
  Every face maps uniquely to one exact sector or hierarchy leaf, producing 58
  deterministic recovery atoms without changing the incumbent mesh. The 64
  non-violating optimization guards create no recovery atoms.
- PR74: actual mixed-edge, required-active-vertex, and intersecting-boundary
  adjacency clusters the 58 exact atoms into three incumbent-local components.
  Two are one-face disks; the remaining 56-atom component is a 176-source-face
  multi-hole patch that preserves two coarse-parent islands. Explicit selection
  of the largest fine-exterior component avoids the former 275-face conservative
  support collapse and the incorrect 707-face hole fill. No mesh is changed.
- PR75: eight of the 14 violating exact sectors are fine-compatible and can be
  materialized directly as source faces; six return a typed boundary-parent
  peel blocker and none require an untyped global expansion. All direct trials
  preserve the logical outside topology and source-coordinate bits and keep
  closed edge incidence. Four trials enter local max-min, but none certify; the
  best range remains `39.278499430048°–80.721500570507°`.

## Current stop condition

CLDP guarantees a finite certified safe result for the Frozen N6 fixture, but
not a non-trivial mixed result. The retained-core search has no closed
single-release topology and no closed pair-release topology; four pair cases
remain search-budget unknown rather than proven impossible. PR73 establishes a
separate incumbent-preserving local family, PR74 classifies its three exact
local cavities, and PR75 proves direct restoration without exterior drift. The
six typed coarse-interface blockers now advance to the finite PR76 boundary-ear
peel matrix. Larger N12/N24/N40 and NXP80 runs remain gated off until Frozen N6
returns `CertifiedAdaptive`.

The PR63 atlas records every active angle, custom-triangle provenance entry,
violation component, and source-face expansion edge in deterministic JSON. It
does not change mesh topology or coordinates. PR64 never replaces the incumbent
unless a candidate is strictly certified; rollback is exact.
