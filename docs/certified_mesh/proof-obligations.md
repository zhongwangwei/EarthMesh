# CMRC proof obligations M1--M5

These are gates, not claims inferred from an ordinary floating-point scan.

## M1: mother-grid geometry and topology

For frequency `n`, split each planar icosahedron face into `n^2` integer
barycentric triangles and project every vertex radially to the sphere. The
certificate must establish:

1. every mesh edge is interpreted as the shorter great-circle arc;
2. every spherical triangle angle is inside the internal safety window;
3. the twelve original vertices have degree 5 and every other vertex degree 6;
4. `V = 10n^2 + 2`, `E = 30n^2`, and `F = 20n^2`;
5. Euler is 2 and the degree charge is 12;
6. spherical Delaunay predicates and the Voronoi dual are valid.

Until an all-`n` analytic proof is checked in, the strict API accepts only
`n = 1, 2, 3, 4, 6, 8, 12, 20, 40, 80, 160, 320, 640`. For those values it propagates
outward-rounded coordinate, tangent, dot-product, and squared-norm intervals
and proves the 40.2--79.8 and 40--80 windows by conservative cosine bounds.
The separately reported `f64` extrema are diagnostics, never the proof.

## M2: conservative requirement merge and grading

Every source returns a minimum level over the entire covered patch. At each
vertex, `L_req` is the maximum of all source levels. For `m` graph rings per
level:

```text
L_grade(v) = max_u(L_req(u) - floor(d(u,v) / m))
```

The implementation and tests must establish `L_grade >= L_req`, the adjacent
level bound, input-order independence, and minimality against exhaustive small
graphs. Close regions are therefore merged in one field; they are never meshed
separately and stitched afterward. The `m`-ring rule is a candidate filter, not
an angle proof.

Before final promotion, typed source requirements are conservatively projected
to the actual final Voronoi cells. The general path uses spherical convex
raster/Voronoi overlap and takes the maximum level over every positive-overlap
source cell; a uniform target may use the stronger global raster maximum bound.
The report records physical and adjacent-level witnesses against exact active
cell IDs.

## M3: certified coarsening invariant

Let `C(T)` be the conjunction of primal angle/topology/degree, physical,
balance, Delaunay, and dual certificates. The mother grid starts with `C(T0)`.
Each transaction either restores its bitwise snapshot or commits only after
`C(T_next)` passes. Induction then preserves the invariant for every committed
state. The core finite-cavity transaction checks geometry, exact final-cell
physical/balance requirements, and spherical remap closure before mutating the
delivered level slots. Final evidence and the remap are fingerprint-bound to
the exact target mesh. The CLI publishes both the complete global 2:1 case and
one-level mixed finite-cavity states. A mixed cavity may relocate only its
retained transition block, preserves the original icosahedron protected
one-rings, and is committed only after the same strict certificate passes.

## M4: finite termination

Every committed coarsening removes at least one active vertex. Candidate sets
are finite and stably ordered; each finite-cavity search has a finite state
space or explicit budget; an epoch with no commit terminates the run.
`ProvenInfeasible` is reserved for a completely enumerated finite search.

## M5: conditional strict feasibility

For a closed sphere, finite satisfiable requirements, a sufficient maximum
level and cell budget, and a mother grid that passes the combined certificate,
CMRC returns in finite time with a certified grid. If no coarsening succeeds,
the certified mother grid itself is the valid result.

This does not claim minimum cell count, universally narrow transitions, or
feasibility under an insufficient budget or a non-certifiable physical source.
