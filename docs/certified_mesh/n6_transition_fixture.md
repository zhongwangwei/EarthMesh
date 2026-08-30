# N6 hidden mixed transition fixture

PR #34 freezes the smallest strict mixed fixture without copying the CLI,
namelist, NetCDF circle mask, raster projection, or production publication path.

## Source

- `MotherGrid::generate(6)`
- Parent/coarse subdivision: `n = 3`
- Parent requirements include every valid `n=3` parent exactly once.
- `available = true` for every parent.
- `maximum_required_level = 0` only for the 32 parents below; all other parents
  use `maximum_required_level = 1`.
- Component planner call:

```rust
plan_hierarchy_components_from_parent_requirements(&source, &requirements, 0, 1)
```

This must produce one component:

```text
parents = 32
core_parents = 10
transition_parents = 22
```

## Frozen level-0 parent addresses

```text
base0:  (1,0,U), (1,0,D), (2,0,U)
base3:  (0,1,U), (0,1,D), (0,2,U)
base4:  (0,0,U), (0,0,D), (0,1,U), (0,1,D), (0,2,U),
        (1,0,U), (1,0,D), (1,1,U), (2,0,U)
base6:  (2,0,U)
base7:  (0,0,U), (0,0,D), (0,1,U), (0,1,D), (0,2,U),
        (1,0,U), (1,0,D), (1,1,U), (2,0,U)
base8:  (0,0,U)
base16: (0,1,U), (0,1,D), (0,2,U)
base17: (0,1,D), (0,2,U), (1,1,U)
```

## Regression owner

`rust/earthmesh_refine_certified/tests/n6_transition_no_go.rs` rebuilds this
fixture directly from `MotherGrid::generate(6)` and asserts the component
shape above before any legacy topology evidence is accepted.
