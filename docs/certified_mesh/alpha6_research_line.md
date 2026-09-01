# Alpha6 research line

`v3.0.0-alpha6` starts at the frozen Alpha5 commit
`ca7e5449ce7cf5305af8af2205c4fd20848eeb10`.

Alpha5 remains the reproducible safe baseline recorded in
[`alpha5_baseline_manifest.json`](alpha5_baseline_manifest.json). Experimental
topology solvers, N12 fixtures, and validation-governance changes belong only
to Alpha6. They must not alter Alpha5 results or product gates.

Alpha6 research is fail-closed:

- N12 probes are research-only and cannot publish a product grid.
- `SearchIncomplete` is not `ExactNoSolution`.
- The frozen `40.2°–79.8°` internal geometry contract is unchanged.
- N12 success does not unlock NXP80 without the N12 → N24 → N40 ladder.
