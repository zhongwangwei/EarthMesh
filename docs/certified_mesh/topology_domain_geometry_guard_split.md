# Topology-domain / geometry-guard split

PR99 adds a V2 face-band adapter that constructs the topology domain directly
from the plan, component hierarchy, and fixed outside incidence. It never calls
legacy coupled-annulus extraction and therefore does not require a single
`inner_guard` or `outer_guard` cycle.

`TransitionTopologyDomain` freezes:

- annulus face labels;
- hierarchical coarse interface;
- canonical internal interfaces;
- fine interface;
- fixed outside faces and link/degree incidence contracts;
- an exact topology-domain key.

`GeometryGuardRegion` is a separate, explicit object. It keeps anchors and
physical fixed sources immovable and covers every face incident to a movable
vertex. Its diagnostic boundary graphs may have multiple components. The CEC
topology evaluator does not construct it.

The V1 path remains available as
`build_stratified_annulus_from_face_bands_v1`; production behavior still uses
that path until the Frozen N6 oracle is complete. The V2 entry point is
`build_stratified_topology_domain_v2`.

The frozen Lifted legacy plan now builds a topology domain without the old
`inner_guard` error. Its next V2 rejection is a plan-dependent
`UnsupportedNonDiskBandComponent`, which is intentionally left for the fixed
CEC-prefix replay to classify across actual cycles rather than being hidden as
a geometry-guard failure.

No geometry, product artifact, or gate changes in this PR.
