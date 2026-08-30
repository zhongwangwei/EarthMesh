# CAT boundary-anchor contract

PR #35 defines the topology-free input to coupled annular transition (CAT).
It extracts existing source-grid rings and boundary incidence obligations; it
does not search for a replacement topology, move vertices, run CBER, or claim
the 40.2--79.8 degree window.

## Nested source and parent grids

For an even source subdivision `n`, the parent subdivision is `n / 2`.
`TriangleAddress::children_2_to_1` supplies the exact four source children of
each parent. Floating-point proximity is never used for hierarchy or seam
identity.

For one connected eligible parent component `D`, let

```text
d(p) = parent-graph distance from p to the complement of D
```

with component-boundary parents at `d = 0`. The extracted rings are:

1. **inner guard** — the coarse-vertex boundary between the deepest parent
   layer and the next layer;
2. **coarse interface** — the coarse-vertex boundary between exact core
   parents and transition parents;
3. **intermediate rings** — fine-source boundaries of the cumulative sets
   `d >= k`, from the core outward;
4. **fine interface** — the fine-source boundary of `D`;
5. **outer guard** — the fine-source boundary of `D` plus its immediate
   outside parent neighbours.

Every boundary edge set must be 2-regular and close as exactly one simple
cycle. Cycles are rotated to a deterministic source-slot order. The coarse and
fine interfaces must be vertex-disjoint; otherwise extraction returns
`UnsupportedIntersectingCycles`. Adjacent guard/intermediate cycles may touch
at source vertices, but their edges remain source-grid edges (coarse rings use
exact two-child parent edges). No new vertex is created.

## Face partition

The mutable annulus is exactly the four source children of every transition
parent. Every other active source face is fixed outside:

```text
annulus_face_slots ∩ fixed_outside_face_slots = ∅
annulus_face_slots ∪ fixed_outside_face_slots = all active source faces
```

The core remains an exact 4-to-1 condensation candidate. Guards are fixed
certification context, not extra topology-search faces.

## Boundary incidence

For a boundary vertex `v`, let:

- `t_A(v)` be its triangle incidence in the mutable annulus;
- `t_ext(v)` be its incidence in `fixed_outside_face_slots`;
- `d_global(v)` be its final closed-sphere degree.

The contract uses triangle incidence consistently:

```text
d_global(v) = t_A(v) + t_ext(v)
```

An original icosahedron vertex is a fixed pentagon anchor:

```text
allowed_global_degree = 5..=5
required_patch_valence = 5 - t_ext
fixed_position = true
```

`t_ext > 5` returns `InvalidAnchorIncidence`.

An ordinary boundary vertex has:

```text
allowed_global_degree = 5..=7
required_patch_valence = max(0, 5 - t_ext)..=7 - t_ext
fixed_position = false
```

`t_ext > 7` is invalid. An `IcosahedronEdge` address is an ordinary movable
seam vertex; the seam label alone never fixes it.

Every anchor occurring on any extracted ring contributes its complete active
five-face star to `anchor_star_guard_face_slots`. This is the geometry/dual
re-certification guard because fixed anchors can still see changed angles and
Voronoi cells when neighbouring ordinary vertices move.

## Frozen Fixture A: ordinary seam annulus

Fixture A was selected deterministically on source `n = 12` / parent `n = 6`:
parent seeds are visited in `TriangleAddress` order, graph balls are widened
until radius 4 with transition width 2 yields a simply connected component
with a nonempty core, two intermediate rings, disjoint coarse/fine interfaces,
no original icosahedron vertex on any ring, and at least one ordinary seam
vertex. The selected seed is:

```text
TriangleAddress { base_face: 0, i: 1, j: 2, n: 6, orientation: Up }
```

The frozen eligible parent addresses are:

```text
base0:
  (0,1,U), (0,1,D), (0,2,U), (0,2,D), (0,3,U), (0,3,D), (0,4,U),
  (1,0,U), (1,0,D), (1,1,U), (1,1,D), (1,2,U), (1,2,D),
  (1,3,U), (1,3,D), (1,4,U),
  (2,0,U), (2,0,D), (2,1,U), (2,1,D), (2,2,U), (2,2,D),
  (2,3,U), (3,0,U), (3,1,U), (3,2,U)
base1:
  (1,0,D), (2,0,U), (2,0,D), (3,0,U), (3,0,D)
```

This freezes `31 parents = 4 core + 27 transition`. It is a development
fixture only and does not replace frozen N6.

## Frozen Fixture B: N6 boundary pentagons

Fixture B remains the PR #34 component:

```text
source n = 6
parent n = 3
32 parents = 10 core + 22 transition
annulus source faces = 88
fixed outside source faces = 632
```

The four exact anchor contracts are:

| source slot | address | `t_ext` | required `t_A` |
|---:|---|---:|---:|
| 29 | `IcosahedronVertex(11)` | 2 | 3 |
| 77 | `IcosahedronVertex(10)` | 2 | 3 |
| 2 | `IcosahedronVertex(0)` | 4 | 1 |
| 155 | `IcosahedronVertex(2)` | 4 | 1 |

All four are fixed, each complete star contains five active faces, and the
four disjoint stars contain 20 faces in total. The regression also freezes the
exact five ring slot sequences so component or source-slot drift fails loudly.

## Typed stop outcomes and stage boundary

PR #35 exposes typed outcomes for invalid anchor incidence, strict-interior
original vertices, multiple cycles, pentagon holes, and intersecting coarse /
fine interfaces. Complement parent components are stably enumerated; when
there is a unique largest outside component, each smaller component is a hole.
An original vertex whose complete parent star lies in such a hole returns
`UnsupportedPentagonHole`. Equal-sized outside candidates remain the more
conservative `UnsupportedMultiCycleAnnulus` classification.

An original icosahedron vertex incident only to mutable annulus faces returns
`UnsupportedInteriorIcosahedronVertex`; this is the mandatory stop condition
before PR #36.

`target_scale = 1.0` is only the extraction-stage identity value. Exact
annular topology belongs to PR #36, free-interface CBER to PR #37, and the
strict frozen-N6 40.2--79.8 certificate to PR #38. NXP80 mixed certified
coarsening remains blocked until that PR #38 gate passes.

Run the PR #35 regression with:

```sh
cargo test -p earthmesh_refine_certified --test cat_annulus -- --nocapture
```
