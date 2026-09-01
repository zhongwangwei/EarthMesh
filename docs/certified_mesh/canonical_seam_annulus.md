# Canonical Seam Annulus Enumerator (CSAE)

PR105 defines the exact finite family of triangulations for two fixed,
disjoint boundary cycles with no interior vertices. Visibility and geometry do
not filter this family.

## Construction

For each lower/upper root bridge, CSAE cuts the annulus along the bridge and
duplicates its two endpoints. The resulting polygon has `m + n + 2`
occurrences. The complete polygon recurrence enumerates its `m + n`
triangles, after which glue validation checks:

- distinct global vertices per triangle and no duplicate triangle;
- occurrence-edge uniqueness except for the two root seam copies;
- boundary incidence 1 and internal/root-bridge incidence 2;
- one link path at every boundary vertex;
- connectedness, `F = m + n`, `E = 2(m + n)`, and Euler 0;
- forbidden fixed edges;
- canonical-minimum bridge root.

Every accepted topology contains a bridge. Cutting an arbitrary accepted
annular triangulation along its minimum bridge produces a polygon
triangulation covered by the recurrence. Conversely, every glued candidate
passing the listed manifold/link/Euler checks is an annular triangulation.
Thus CSAE is complete for the declared fixed-boundary/no-interior-vertex
family; it makes no claim about added vertices or cross-band edges.

## Independent small exact oracle

A legal-edge-flip closure, whose triangulation flip graph is connected for
this fixed marked annulus, independently expands one valid seed. Its topology
key set exactly matches CSAE:

| boundaries | exact topologies |
| --- | ---: |
| 3+3 | 21 |
| 3+4 | 132 |
| 4+4 | 844 |
| 4+5 | 4,180 |

Evidence: `tests/fixtures/annular_small_exact_oracle.json`. Taskbook SHA-256:
`cb911eef1de3593df10d042bf72ce3707080d2b521ceb074d36b8b05cfe4b63e`.

This passes the PR105 gate only. No N12 topology, geometry, product output, or
gate change is claimed.
