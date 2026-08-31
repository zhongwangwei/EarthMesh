# Full-polygon CAT completeness

## Exact family

For a sector with a fixed cyclic boundary

\[
P=(p_0,\ldots,p_{n-1}),
\]

the full-polygon family contains every abstract noncrossing triangulation of
that boundary, with no interior vertex. Source-coordinate visibility and angle
quality are not topology filters.

The interval recurrence is

\[
T(i,j)=\bigcup_{i<k<j}
T(i,k)\oplus T(k,j)\oplus(p_i,p_k,p_j),
\]

with `T(i,j) = {empty}` when `j <= i + 1`.

Every triangulation contains exactly one triangle incident to boundary edge
`(p_i,p_j)`. Its third vertex is some `p_k`; removing that triangle leaves the
two independent intervals `P[i..=k]` and `P[k..=j]`. Induction therefore puts
every triangulation in the recurrence. Conversely, joining two recursively
valid interval triangulations with that triangle produces a noncrossing
triangulation of the parent interval. The recurrence is thus sound and
complete for the stated fixed-boundary family.

PR39 signature aggregation preserves member multiplicity without materializing
topology keys. PR40 must retain every concrete key before any later geometry
search. The Catalan regression for `n=3..9` checks totals
`1, 2, 5, 14, 42, 132, 429`.

## Allowed hard conflicts

A candidate is rejected only for an abstract/topological conflict: repeated
vertices, a degenerate triangle, or use of a non-boundary edge already owned
by the fixed outside mesh. Existing polygon boundary edges remain allowed.
No source visibility, orientation, crossing scan, or angle threshold removes
a member from the exact combinatorial family.

## PR39 reachability boundary

PR39 stores exact per-sector incidence signatures and applies candidate-aware
degree support propagation. Generic anchor-ear deltas are currently a marked
conservative superset; therefore they may preserve extra signatures but may
never justify a family No-Go. `NecessaryFeasible` means the exact incidence
gate did not rule out the family and authorizes PR40 enumeration; it is not a
global-topology existence claim. Exact link-cycle compatibility and concrete
member recovery are closed by the later enumerator/global-merge stages.
