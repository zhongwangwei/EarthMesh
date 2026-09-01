# Frozen N6 V1/V2 adapter oracle

PR100 runs the known closed Frozen N6 W2 `FaceBandPlan` through both adapters
under the same 4,096-state full-polygon budget.

The oracle requires exact equality for:

- coarse, internal, and fine interface cycles;
- annulus faces and face-band partitions;
- fixed outside boundary contracts and vertex-link contracts;
- sector-family and retained-topology counts;
- states by depth, selected topology keys, and selected ears;
- anchor degrees and ordinary-degree histogram;
- final vertices, edges, faces, Euler characteristic, and charge.

All comparisons pass and both adapters close the same topology. Only the
compatibility representation of deferred geometry guards is excluded from the
topology equality contract.

V1 remains the default; the oracle permits the N12 research runner to opt into
V2 for the fixed-prefix replay. It does not change product behavior or gates.
