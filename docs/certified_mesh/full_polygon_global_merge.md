# Full-polygon global exact merge (PR41)

Taskbook: `EarthMesh_CMRC_CAT_Full_Polygon_Freedom_Ladder_Taskbook.md`
SHA-256: `953b213e7eba617926c50b70faa9a97eb49ac80c7d9167936b5624f3d38034ad`

## Exact family

The search consumes every PR40 full-polygon member retained by the sound PR39 AC-3 degree gate. It does not call the legacy two-chain solver and does not use a fixed-width topology bitmask.

Frozen N6 family counts:

```text
full     [5, 5, 132, 132, 5, 5, 132, 132, 14, 14, 14, 14, 132, 132]
retained [5, 5,  84,  84, 5, 5,  51,  51, 14, 14, 14, 14,  51,  51]
```

The retained set preserves every concrete topology member whose exact incidence signature survives PR39. Search uses deterministic MRV, open-edge provider forcing, exact edge incidence, partial link-cycle checks, dynamic degree support, and generic anchor ears. Source-coordinate visibility is never a topology filter.

## Frozen N6 topology gate

Release evidence:

```text
topology states: 29
ear states: 0
closed candidates: 1
anchors: {2:5, 29:5, 77:5, 155:5}
ordinary degree histogram: {5:25, 6:289, 7:17}
V/E/F: 335/999/666
source V/F: 362/720
Euler: 2
charge: 12
```

The final candidate also passes the actual single-cycle link and two-face edge-incidence gates, materializes as a mixed hierarchy mesh, and reduces both vertices and faces.

## Outcome semantics

- `Closed` is emitted only after generic ear completion, the shared final degree/link/edge/Euler/charge gate, and hierarchy materialization.
- `SearchBudgetExhausted` remains unknown.
- `TopologyFamilyExhaustedNoSolution` is available only after the finite retained product and generic ear states are exhausted.
- This result is topology-only. It does not claim spherical orientation, 40°–80° angles, Delaunay/Voronoi, physical, balance, or remap certification; those remain PR43–PR45 work.
