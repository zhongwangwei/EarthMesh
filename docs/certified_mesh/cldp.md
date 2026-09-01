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

## Frozen N6 evidence through PR65

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

The PR63 atlas records every active angle, custom-triangle provenance entry,
violation component, and source-face expansion edge in deterministic JSON. It
does not change mesh topology or coordinates. PR64 never replaces the incumbent
unless a candidate is strictly certified; rollback is exact.
