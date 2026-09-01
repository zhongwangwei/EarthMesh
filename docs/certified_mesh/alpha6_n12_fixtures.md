# Alpha6 N12 research fixtures

Alpha6 freezes two research-only N12 fixtures. Neither fixture changes a
product gate or writes a deliverable grid.

## N12-Lifted-N6

Every Frozen N6 N3 parent is replaced by its four exact N6 children while the
source grid moves from N6 to N12. The frozen counts are:

| quantity | count |
|---|---:|
| source vertices | 1442 |
| source edges | 4320 |
| source faces | 2880 |
| component parents | 128 |
| core parents | 40 |
| transition parents | 88 |

The original icosahedron anchors remain vertices 0, 2, 10, and 11.

## N12-Interior-Control

The initial deterministic search selected and then froze a ten-parent patch in
base face 0: one core parent and nine transition parents. Its transition and
two outside guard rings contain no original icosahedron vertex. The exact
addresses are stored in
`rust/earthmesh_refine_certified/tests/fixtures/n12_research_fixtures.json`.

A single N6 base face contains only 36 possible parents, so the taskbook's
40-core/88-transition target cannot be matched inside one base face. The
control is therefore an anchor-free scale diagnostic, not an equal-size copy
of the lifted fixture; representativeness telemetry reports that difference
instead of hiding it.

`n12_research_fixture_report_json()` emits the complete manifests and the
dimensionless curvature, band-width, pentagon-distance, area, boundary, and
fixed/movable ratios.
