# Hierarchy elastic targets contract

This is the PR46 gate contract, not an implementation claim.

Future elastic target work must derive edge, area, and degree-angle targets from
hierarchy levels instead of treating failed trial geometry as the only target.
Cross-level edge targets should use geometric interpolation between level
scales. Invalid reference Voronoi areas must not block the angle phase.

Go requires deterministic A/B evidence against the PR45 Frozen N6 baseline:

- same topology order;
- same start set;
- same iteration budget;
- improved `best_signed_margin_deg`, or no default switch;
- no regression in orientation/crossing evidence;
- no regression for already certified simple fixtures.
