# Alpha6 legacy/CEC finite-family oracle

PR93 keeps the Alpha5 face-label solver as an explicit small-fixture oracle and
compares its complete accepted plan set with independently enumerated canonical
essential cycles. It does not switch the Alpha6 default solver or alter a
product gate.

## Exhaustible fixtures

The N2, N3, and N4 oracle fixtures use generated mother grids of those source
subdivisions for canonical vertex/face identity and a small synthetic
triangulated annulus with 4, 5, and 6 sectors respectively. Their candidate
graphs contain 4, 5, and 6 edges, so every CEC edge subset is independently
enumerated. The legacy side exhausts every free face-label assignment through
the existing hard validator.

For each fixture:

```text
legacy accepted FaceBandPlan
  -> canonical EssentialCycleKey

all CEC edge subsets
  -> full essential-cycle validation
  -> recovered legacy FaceBandPlan validation
```

The exact key sets are equal:

| Fixture | Candidate edges | Legacy cycle keys | CEC cycle keys |
| --- | ---: | ---: | ---: |
| N2 | 4 | 1 | 1 |
| N3 | 5 | 1 | 1 |
| N4 | 6 | 1 | 1 |

The same equality holds on N3 variants with
`InteriorOfSingleBand`, `OnSingleInterface`, and
`FineCapConnectedToExterior` anchor policies. Clearing the frozen dual seam is
an intentional mutation: CEC then rejects the legacy key, and the oracle test
detects the mismatch. The open-path, multiple-cycle, boundary-touch, and
odd-parity-without-separation mutations remain covered by the PR90 contract
tests.

`enumerate_legacy_face_band_plans` is deliberately documented and exposed as an
Alpha5 regression oracle for small fixtures. It reports whether its supplied
state bound completed; an incomplete enumeration cannot establish set
equality.

This oracle validates the W2 mathematical representation on finite fixtures.
It does not by itself close the Frozen N6 659-family backlog or prove the
40.2–79.8 degree geometry target.
