# Alpha6 N12 legacy baseline

PR88 ran the frozen Alpha5 W2 face-label solver with 16,384 face-band states
and 4,096 downstream topology states. The runner is research-only and cannot
write a gridfile, ready marker, product outcome, or gate change.

| fixture | face-band result | downstream result | research outcome |
|---|---|---|---|
| N12-Lifted-N6 | closed in 265 states | the inherited stratified-annulus evaluator cannot form a closed 2-regular inner guard | `ResearchSearchIncomplete` |
| N12-Interior-Control | W2 family exhausted during propagation | not entered | `ResearchExactNoSolution` for this declared legacy W2 family only |

The lifted result is not an N12 no-go: a valid W2 face labeling was found, but
the Alpha5 downstream representation rejected the exact lifted component. No
geometry run was permitted because no downstream topology closed.

The canonical machine-readable evidence is stored at
`rust/earthmesh_refine_certified/tests/fixtures/n12_legacy_baseline.json`.
