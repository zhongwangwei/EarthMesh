# Continuous feasibility contract

Frozen N6 strict mixed topology is solved, but continuous spherical geometry is
not certified until all hard geometry and final-cell gates pass.

A finite search failure means only that the declared topology/start/domain/solver
budget did not find a certificate. It must be reported as
`ContinuousSearchIncomplete` unless a scoped interval/global proof justifies a
stronger result.

Allowed result meanings:

- `Certified`: a witness passes the 40.2--79.8 degree internal window and all
  downstream geometry gates.
- `ContinuousSearchIncomplete`: bounded numerical search stopped without proof.
- `RequiresDifferentTopology`: evidence specifically shows the topology must
  change; angle-search exhaustion alone is insufficient.
- `CertifiedInfeasibleWithinDomain`: only an interval/global proof may assert
  infeasibility, and only for the named topology/domain/trust bounds.
- `InvalidPatch`: fixture or patch construction is invalid.

NXP80 production runs remain gated behind Frozen N6, then N12, N24, and N40.
