# OLAM Method-C Replacement Plan

## Goal

Replace the Rust specified-region `spawn_nest` kernel with a faithful OLAM Method-C implementation, retiring the older generic conforming-split approximation as the refinement algorithm.

## Acceptance

- Same-pass specified regions are not blindly unioned into one region; disjoint regions run as independent OLAM grids, while overlapping regions are not re-applied on an already spawned same-level transition mesh.
- Region levels remain independently selectable in `1..=5`, and the GUI default `max_iter_spc` remains `5`.
- The Method-C pass must stop depending on arbitrary single-edge/two-edge conforming triangle seeds as the main algorithm.
- The implementation must preserve OLAM's M/U/W topology constraints: every active M point has `3..=7` neighbors, every U edge has two W faces, and every W face is triangular after rebuild.
- Existing `olam_spawn_nest` tests plus focused GUI refinement tests must pass before claiming completion.

## Work Order

1. Lock current regressions with tests for independent region execution and M-point refinement metadata.
2. Replace `spawn_nest_internal` region scheduling with OLAM-style per-pass/per-grid behavior.
3. Replace `spawn_nest_pass_method_c` with a Method-C-specific table path based on `iwnew/iunew/imnew`, `nest_ud`, `nest_wd`, perimeter triples, and transition-row rebuild.
4. Verify mesh topology and GUI/CLI refinement handling with targeted tests before full case smoke tests.
