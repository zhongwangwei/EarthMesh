# Annular degree/link reachability

PR106 adds a V3 reachability gate before concrete annular topology recovery.
It consumes `AnnularCellDomain` directly and never calls the legacy sector or
monotone-connector path.

## Signature DP

For each canonical root bridge, an interval DP over the occurrence-space cut
polygon aggregates only:

- triangle incidence count at every global boundary vertex;
- the boundary-link path endpoints and length;
- root bridge;
- represented cut-state count.

Fixed outside triangle counts, the ordinary degree ceiling 7, the anchor
ceiling 5, the minimum contribution of the other W2 cell, and any declared ear
delta are applied before a state is retained. Concrete triangle families are
not stored.

The DP is deliberately a necessary relaxation of CSAE: a glue-invalid cut
state may share a signature with valid states, but every CSAE topology must
project into the DP domain. Exhausting this superset with no degree/link
support is therefore an exact negative result; retaining support is only a
necessary-feasible result.

## W2 AC-3

AC-3 repeatedly removes a cell signature unless every shared vertex has
support in the other cell for:

- final degree 5--7, or exactly 5 for a pentagon anchor;
- matching link-path endpoints;
- two path providers when the ear domain is exactly zero, so the final link is
  one cycle.

The coarse boundary source paths are contracted before endpoint comparison.
This maps each length-two source path endpoint to the opposite coarse topology
vertex and preserves the frozen N6 selected topology.

## Typed stop semantics

- `NecessaryFeasible`: exhaustive signature relaxation retains W2 support;
- `ProvenImpossibleWithinDeclaredAnnularFamily`: exhaustive relaxation has no
  support, hence neither can any concrete CSAE topology;
- `SearchIncomplete`: the signature-state budget or checked count arithmetic
  stopped enumeration. It is never reported as no-solution.

The frozen N6 legacy selected topology survives the V3 caps and AC-3 as two
singleton annular signatures. Raw N6 DP with 4,096 signature states reports
`SearchIncomplete`, not a false rejection. This PR does not recover a concrete
V3 topology, run Lifted-N12, run geometry, or change product gates.
