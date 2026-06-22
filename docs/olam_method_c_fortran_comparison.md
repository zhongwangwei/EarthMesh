# OLAM Method-C Rust/Fortran comparison status

This note records the current evidence for the OLAM Method-C port and the
remaining gap for direct Fortran executable comparison.

## Rust-side verified cases

Command:

```sh
cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib -- --nocapture
```

Result:

```text
77 passed; 0 failed; 1 ignored
```

The ignored case is
`olam_method_c_olamin_style_multilevel_corridor_outputs_closed_mesh`, because it
runs three 5000-iteration atmosphere spring passes. The default fast coverage is
the table-only OLAMIN-style corridor case.

Covered Method-C behavior includes:

- `ngr_area` circle/corridor/bbox/polygon semantics using Fortran polar-stereographic distance.
- `thirdm` traversal and `jdone` behavior.
- `fill_rad3` immediate and distant W-face marking.
- Concavity closure threshold `nw >= npoly - 1`.
- `perim_map2`, `perim_ngr`, perimeter triples, and center-segment suppression.
- `perim_mrow` transition-row propagation and previous-boundary crossing rejection.
- `perim_fill3` transition coordinates and endpoint/W-face slot updates.
- Fortran `imnew/iunew/iwnew` allocation order and prognostic partner remapping.
- Method-C parent-MRL selection from the current starting M point.
- cart_hex periodic-copy W-face handling in Method-C M-neighbor rings.
- spring_dynamics_nest movement masks, mrow use, and parent-grid immobility.

OLAMIN-style table case:

- Test: `olam_method_c_olamin_style_multilevel_corridor_table_outputs_closed_mesh`
- Expected table sizes: `nmd=84099`, `nud=252289`, `nwd=168193`
- Expected W-face `ngr` counts: `{1: 76426, 2: 11468, 3: 15114, 4: 65184}`
- Expected atmosphere `mrow` envelope: first `-13`, last `13`, count `50069`
- Topology validation passes.

CLI-level OLAM cases:

```sh
cargo test --manifest-path rust/earthmesh_cli/Cargo.toml --lib olam_ -- --nocapture
```

Result:

```text
13 passed; 0 failed
```

Top-level dispatcher cases:

```sh
cargo test --manifest-path rust/earthmesh_cli/Cargo.toml --test mkgrd_top_level_refine_runner default_dispatcher_ -- --nocapture --test-threads=1
```

Result:

```text
5 passed; 0 failed
```

The HDF5-DIAG messages in dispatcher tests are nonfatal missing-file probes; the
tests pass.

## Direct Fortran executable comparison status

Current direct Fortran comparison is not yet complete.

One reduced direct Fortran comparison is now available for a small Method-C
case. The comparison is direct in the sense that the Fortran numbers came from
compiled OLAM Fortran routines in a temporary reduced probe, not from Rust.

The source tree does not currently expose a ready `olam-7.0` executable under
`/Users/zhongwangwei/Desktop/olam-model-code-r1095-trunk/build_olam_test`.
It does contain `OLAMIN`, `Makefile`, `include.mk`, and many old object/module
files.

The original `include.mk` is configured for Intel-style build commands:

- `F_COMP=h5pfc`
- `C_COMP=icc`
- Intel flags such as `-xHost`, `-assume norealloc_lhs`, `-qopenmp`
- NCAR/HDF5/NetCDF link dependencies

Local tool availability found:

- `gfortran`: `/opt/homebrew/bin/gfortran`
- `mpifort`: `/opt/homebrew/bin/mpifort`
- `h5fc`: `/Users/zhongwangwei/miniforge3/bin/h5fc`
- `nf-config`: `/opt/homebrew/bin/nf-config`
- `nc-config`: `/Users/zhongwangwei/miniforge3/bin/nc-config`
- NCAR tools: `/opt/homebrew/ncl-6.6.2/bin/ncargf90`

However, the local `h5fc` wrapper points to a missing compiler:

```text
arm64-apple-darwin20.0.0-gfortran: command not found
```

A non-destructive GNU build probe was created under `/tmp/olam-fortran-probe`,
with the build directory copied and source directories symlinked back to the
original OLAM tree. The probe compiled part of the tree:

```text
131 .o files
105 .mod files
```

The first hard gfortran error was outside Method-C, in `lagpart/mem_lp.f90`:

```text
Error: IMPORT statement only permitted in an INTERFACE body
```

This blocks full OLAM executable comparison with the current GNU setup. It does
not by itself indicate a Method-C algorithm mismatch.

## Direct reduced Fortran comparison: NXP=6 single atmosphere circle

Temporary probe location:

```text
/tmp/olam-reduced-probe
```

Fortran path:

1. `init_consts(0, 0, 0.0)`
2. Set `mdomain=0`, `nxp=6`, `ngrids=2`
3. Set grid 2 as one circle: `lat=25.0`, `lon=115.0`, `radius=2500000.0`
4. Call `icosahedron(nxp)`
5. Call `spawn_nest(.true.)`
6. Print compact W-table summaries after `perim_mrow`; spring is stubbed no-op
   in the reduced probe.

Fortran output:

```text
summary nmd nud nwd         435        1297         865
summary ngr counts           0         864           0           0           0           0
summary mrow min max count          -6          12         864
```

Rust regression test:

```text
olam_method_c_matches_reduced_fortran_nxp6_single_circle_summary
```

Rust command:

```sh
cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_matches_reduced_fortran_nxp6_single_circle_summary -- --nocapture
```

Result:

```text
1 passed; 0 failed
```

Important setup detail:

- The Fortran call is `spawn_nest(.true.)`, so it uses atmosphere
  `max_mrows=13`.
- Rust must therefore use `spawn_nest_as_atmosmesh`, not the default surface
  `max_mrows=7`.

## Next useful direct-comparison path

The most practical next step is not to keep forcing a full OLAM build first.
Instead:

1. Build a reduced Fortran Method-C probe that excludes lagpart/radiation/CMAQ
   where possible and links only the grid-construction modules needed for
   `gridinit`, `cart_hex`, `spawn_nest`, `triangle_utils`, and
   `spring_dynamics`.
2. Have the probe print compact table summaries:
   `nmd/nud/nwd`, per-`ngr` W counts, `mrow` envelope/count, and topology
   sentinel counts.
3. Compare those summaries against the Rust cases already asserted in
   `earthmesh_mesh` tests.

Until that reduced Fortran probe exists, the current evidence is source-level
algorithmic equivalence plus Rust regression tests, not a direct Fortran binary
run.

## Reduced Fortran probe dependency sketch

The first reduced probe should avoid `olammain.F90` and full-model packages such
as lagpart, radiation, sea, land, CMAQ, and UMWM.

Target routines for a Method-C table comparison:

- `omodel/icosahedron.f90`
- `omodel/cart_hex.f90`
- `omodel/expand_global.f90`
- `omodel/spawn_nest.f90`
- `omodel/triangle_utils.f90`
- `omodel/spring_dynamics.f90`
- `omodel/fill_itabs.f90`

Core state modules needed by those routines:

- `modules/mem_delaunay.f90`
- `modules/mem_ijtabs.f90`
- `modules/mem_grid.f90`
- `modules/misc_coms.f90`
- `modules/consts_coms.f90`
- `modules/max_dims.f90`
- `modules/oname_coms.f90`

Additional lightweight dependencies observed from `use` statements:

- `outils/map_proj.f90` for `ec_ps`
- `outils/map_proj_ps.f90` for `ll_ps`, `ec_ps`, `ps_ll`
- `modules/oplot_coms.f90` for `op`
- `sfcgrid/mem_sfcg.f90` for surface-grid globals referenced by `spawn_nest`
- `modules/mem_para.F90` only for `olam_stop`/finalize stubs unless full MPI is
  retained

The reduced probe can likely replace some full modules with tiny stubs:

- `mem_para`: provide `olam_stop` that prints and stops.
- `oplot_coms`: provide an `op` object with the fields used by Method-C paths.
- `mem_sfcg`: provide zero/default surface-grid counters when testing
  atmosphere Method-C only.

The first probe should print only compact summaries, not HDF5:

- `nmd`, `nud`, `nwd`
- W-face counts by `ngr`
- `mrow` minimum, maximum, and nonzero count
- counts of invalid placeholder ids in active topology

## Reduced Fortran direct checks added after the first NXP6 case

### NXP6 two nested circles, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=6`, `ngrids=3`.
- Grid 2: circle centered at lon `115`, lat `25`, radius `4,000,000`, parent level `1`.
- Grid 3: circle centered at lon `115`, lat `25`, radius `1,000,000`, child level `2`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         624        1864        1243
summary ngr counts           0         154        1088           0           0           0
summary mrow min max count          -6          11        1242
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp6_two_circle_summary`
- Uses `spawn_nest_as_atmosmesh` so the Rust `max_mrows` path matches Fortran atmosphere `spawn_nest(.true.)`.
- Current result matches the reduced Fortran summary exactly for `nmd/nud/nwd`, `ngr` counts, and `mrow` min/max/count.

### NXP6 two nested circles, rejected boundary case

Fortran reduced probe setup:
- Same center and `nxp=6`, `ngrids=3`.
- Grid 2 radius `2,500,000`; grid 3 radius `1,000,000`.

Fortran reduced probe result:

```text
Current nested grid 3 crosses (or is too close to) the next coarser grid boundary...
```

Rust regression test:
- `olam_method_c_rejects_reduced_fortran_nxp6_two_circle_too_close_boundary`
- Rust rejects the case through a boundary-style error before the Rust-only perimeter-triple grouping guard.

Interpretation:
- Behavior-level agreement: both implementations reject this invalid/tight two-level circle case.
- Rejection-path agreement is now closer: Rust no longer reaches the later perimeter grouping invariant first for this reduced negative case.

### NXP6 single corridor, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=6`, `ngrids=2`.
- Grid 2 uses the Fortran `ngrdll > 1` corridor path with two connected points.
- Point 1: lon `115`, lat `25`, radius `2,500,000`.
- Point 2: lon `130`, lat `25`, radius `2,500,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         474        1414         943
summary ngr counts           0         942           0           0           0           0
summary mrow min max count          -6          12         942
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp6_corridor_summary`
- Uses `OlamRefinementRegion::Corridor` with matching two points and endpoint radii.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- A shorter two-point corridor `(115,25) -> (116,26)` with the same radius produced the same coarse-grid summary as the single-circle case, so it was not useful as an independent positive comparison.
- Narrower `nxp=6` corridor radii around `1,500,000` to `2,250,000` triggered Fortran's own `STOP stop tri_neighbors npoly`, so they were not used as positive equivalence tests.

### NXP6 variable-radius corridor, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=6`, `ngrids=2`.
- Grid 2 uses the Fortran `ngrdll > 1` corridor path with endpoint radius interpolation.
- Point 1: lon `115`, lat `25`, radius `2,500,000`.
- Point 2: lon `130`, lat `25`, radius `1,250,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         435        1297         865
summary ngr counts           0         864           0           0           0           0
summary mrow min max count          -6          12         864
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp6_variable_radius_corridor_summary`
- Uses the same two corridor points with endpoint radii `2,500,000` and `1,250,000`.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- The broader variable-radius case `2,500,000 -> 1,500,000` completed in Fortran but produced the same summary as the equal-radius long corridor, so it was less useful as an additional regression.
- The narrower variable-radius cases with the second endpoint at `500,000` or `1,000,000` triggered Fortran's own `STOP stop tri_neighbors npoly` on this coarse `nxp=6` grid.

### NXP6 three-point corridor, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=6`, `ngrids=2`.
- Grid 2 uses the Fortran `ngrdll > 1` corridor path with three connected points and two searched segments.
- Point 1: lon `115`, lat `25`, radius `2,500,000`.
- Point 2: lon `130`, lat `25`, radius `2,500,000`.
- Point 3: lon `150`, lat `0`, radius `2,500,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         552        1648        1099
summary ngr counts           0        1098           0           0           0           0
summary mrow min max count          -9          12        1098
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp6_three_point_corridor_summary`
- Uses the same three corridor points with equal endpoint radii.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- A milder three-point bend `(115,25) -> (130,25) -> (130,35)` completed in Fortran but produced the same summary as the two-point long corridor.
- Extending the third point to `(145,35)` still produced the same summary on coarse `nxp=6`.
- The `(150,0)` third point is the first tried three-point case that changed the reduced Fortran topology summary and therefore gives useful coverage of the multi-segment `ngrdll=3` path.

### NXP6 two-level corridor, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=6`, `ngrids=3`.
- Grid 2 uses the Fortran `ngrdll > 1` corridor path.
- Grid 2 point 1: lon `115`, lat `25`, radius `6,000,000`.
- Grid 2 point 2: lon `130`, lat `25`, radius `6,000,000`.
- Grid 3 uses a shorter child corridor inside the parent corridor.
- Grid 3 point 1: lon `120`, lat `25`, radius `1,000,000`.
- Grid 3 point 2: lon `125`, lat `25`, radius `1,000,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         783        2341        1561
summary ngr counts           0         294        1266           0           0           0
summary mrow min max count          -6          11        1560
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp6_two_level_corridor_summary`
- Uses matching parent and child corridor regions.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- A child corridor spanning the full parent segment `(115,25) -> (130,25)` with radius `1,000,000` was rejected by Fortran as crossing or being too close to the next coarser grid boundary, even when the parent radius was increased from `4,000,000` to `6,000,000`.
- Shortening the child to `(120,25) -> (125,25)` produced a stable positive two-level corridor comparison.

### NXP6 two-level corridor, rejected boundary case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=6`, `ngrids=3`.
- Grid 2: corridor `(115,25) -> (130,25)`, endpoint radii `6,000,000`.
- Grid 3: same-length child corridor `(115,25) -> (130,25)`, endpoint radii `1,000,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe result:

```text
Current nested grid 3 crosses (or is too close to) the next coarser grid boundary...
```

Rust regression test:
- `olam_method_c_rejects_reduced_fortran_nxp6_two_level_corridor_too_close_boundary`
- Rust rejects this invalid same-length child corridor through a boundary-style error before the Rust-only perimeter-triple grouping guard.

Interpretation:
- Behavior-level agreement: both implementations reject the invalid/tight two-level corridor case.
- Rejection-path agreement is now closer: Rust no longer reaches the later perimeter grouping invariant first for this reduced negative case.
- No core mesh construction change was made for this diagnostic-path difference.

### NXP7 single circle, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=7`, `ngrids=2`.
- Grid 2: circle centered at lon `115`, lat `25`, radius `2,500,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         565        1687        1125
summary ngr counts          57        1067           0           0           0           0
summary mrow min max count          -6          13        1067
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp7_single_circle_summary`
- Uses the same circle specification on `from_icosahedron(7, 0, 1.0, 0.25, 100)`.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- This case extends the direct comparison beyond `nxp=6`.
- Unlike the `nxp=6` single-circle case, the Fortran output retains `57` active W faces at `ngr=1` and `1067` at `ngr=2`, so it also gives extra coverage of post-spawn grid-number bookkeeping.

### NXP7 single corridor, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=7`, `ngrids=2`.
- Grid 2 uses the Fortran `ngrdll > 1` corridor path.
- Point 1: lon `115`, lat `25`, radius `2,500,000`.
- Point 2: lon `130`, lat `25`, radius `2,500,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         643        1921        1281
summary ngr counts          25        1255           0           0           0           0
summary mrow min max count          -8          13        1255
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp7_corridor_summary`
- Uses the same two-point corridor on `from_icosahedron(7, 0, 1.0, 0.25, 100)`.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- This case extends direct comparison of the `ngrdll > 1` corridor branch beyond `nxp=6`.
- The output retains both `ngr=1` and `ngr=2` active W faces, so it also checks grid-number bookkeeping for non-circle refinement at `nxp=7`.

### NXP7 two nested circles, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=7`, `ngrids=3`.
- Grid 2: circle centered at lon `115`, lat `25`, radius `3,000,000`.
- Grid 3: circle centered at lon `115`, lat `25`, radius `1,000,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         754        2254        1503
summary ngr counts           3         335        1164           0           0           0
summary mrow min max count          -6          13        1499
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp7_two_circle_summary`
- Uses matching parent/child circle regions on `from_icosahedron(7, 0, 1.0, 0.25, 100)`.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- Reusing the `nxp=6` accepted two-circle radii at `nxp=7` with parent radius `4,000,000` caused the reduced Fortran probe to SIGBUS during the first spawn, so that parameter set is not used as a positive equivalence case.
- The parent radius `3,000,000` with child radius `1,000,000` completed cleanly and gives useful coverage because active W faces remain split across `ngr=1`, `ngr=2`, and `ngr=3`.

### NXP7 two-level corridor, accepted case

Fortran reduced probe setup:
- `init_consts(0,0,0.0)`, `mdomain=0`, `nxp=7`, `ngrids=3`.
- Grid 2 uses the Fortran `ngrdll > 1` corridor path.
- Grid 2 point 1: lon `115`, lat `25`, radius `2,500,000`.
- Grid 2 point 2: lon `130`, lat `25`, radius `2,500,000`.
- Grid 3 uses a shorter child corridor inside the parent corridor.
- Grid 3 point 1: lon `120`, lat `25`, radius `500,000`.
- Grid 3 point 2: lon `125`, lat `25`, radius `500,000`.
- `spawn_nest(.true.)`; `spring_dynamics` stubbed as a no-op in the reduced probe.

Fortran reduced probe summary:

```text
summary nmd nud nwd         715        2137        1425
summary ngr counts          25         287        1112           0           0           0
summary mrow min max count          -6          13        1399
```

Rust regression test:
- `olam_method_c_matches_reduced_fortran_nxp7_two_level_corridor_summary`
- Uses matching parent and child corridor regions on `from_icosahedron(7, 0, 1.0, 0.25, 100)`.
- Uses `spawn_nest_as_atmosmesh` to match Fortran atmosphere `spawn_nest(.true.)`.

Notes:
- Reusing the `nxp=6` accepted two-level corridor parent radius `6,000,000` at `nxp=7` triggered Fortran's own `STOP stop tri_neighbors npoly` during the first spawn.
- Parent radius `4,000,000` triggered a Fortran SIGBUS during the first spawn.
- Parent radius `2,500,000` with child radius `500,000` completed cleanly and gives useful `ngr=1/2/3` coverage.

## Current direct-comparison audit and decision gate

As of the latest audit, the Rust Method-C implementation has direct reduced-Fortran behavioral coverage for:
- `nxp=6` and `nxp=7` base grids.
- Circle and corridor refinement regions.
- Single-level and two-level nests.
- Equal-radius, variable-radius, and multi-segment corridor paths.
- Accepted topology-generation cases and rejected too-close boundary cases.

Targeted verification commands and latest observed results:

```text
cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_matches_reduced_fortran -- --nocapture
# 10 passed; 0 failed

cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib rejects_reduced_fortran -- --nocapture
# 2 passed; 0 failed
```

Current behavioral conclusion:
- The accepted reduced-Fortran topology summaries match Rust exactly for `nmd/nud/nwd`, active W-face `ngr` counts, and nonzero `mrow` min/max/count.
- The reduced-Fortran rejected too-close boundary cases are also rejected by Rust.

Resolved rejection-path non-equivalence:
- Fortran rejects the too-close boundary cases earlier through parent-boundary / next-coarser-grid checks.
- Rust now performs the corresponding parent `mrlw` consistency check after selected-face / `fill_rad3` closure and before Method-C perimeter triple grouping.
- The two reduced-Fortran rejected cases are covered by Rust tests that require a boundary-style error and reject the later `cannot be grouped into transition triples` path.

## Re-running reduced Fortran probes

The reduced Fortran probe workflow is captured in:

```text
scripts/olam_reduced_fortran_probe.sh
```

The script compiles a minimal Method-C Fortran harness in `/tmp` using the original OLAM source tree and local stub modules for non-grid dependencies. It does not modify the OLAM source tree.

Default source path:

```text
/Users/zhongwangwei/Desktop/olam-model-code-r1095-trunk
```

Example positive case:

```text
scripts/olam_reduced_fortran_probe.sh nxp6_circle
```

Expected summary:

```text
summary nmd nud nwd         435        1297         865
summary ngr counts           0         864           0           0           0           0
summary mrow min max count          -6          12         864
status 0
```

Example expected-rejection case:

```text
scripts/olam_reduced_fortran_probe.sh nxp6_bad_two_circle
```

Expected behavior:

```text
Current nested grid 3 crosses (or is too close to)
the next coarser grid boundary...
status 2
```

Supported case names are printed by:

```text
scripts/olam_reduced_fortran_probe.sh --help
```

Use environment overrides when needed:

```text
OLAM_SRC=/path/to/olam WORKDIR=/tmp/custom-olam-probe FC=gfortran scripts/olam_reduced_fortran_probe.sh nxp7_two_corridor
```

### Latest full reduced-Fortran probe run

Command:

```text
scripts/olam_reduced_fortran_probe.sh all
```

Latest observed result:
- The script completed with exit status `0`.
- All 10 accepted cases completed with probe `status 0`.
- Both expected-rejection cases completed with probe `status 2` and printed the Fortran next-coarser-grid boundary rejection.

Accepted case summaries from that run:

```text
nxp6_circle: nmd/nud/nwd=435/1297/865; ngr=(0,864,0); mrow=-6..12 count=864
nxp7_circle: nmd/nud/nwd=565/1687/1125; ngr=(57,1067,0); mrow=-6..13 count=1067
nxp6_corridor: nmd/nud/nwd=474/1414/943; ngr=(0,942,0); mrow=-6..12 count=942
nxp7_corridor: nmd/nud/nwd=643/1921/1281; ngr=(25,1255,0); mrow=-8..13 count=1255
nxp6_variable_corridor: nmd/nud/nwd=435/1297/865; ngr=(0,864,0); mrow=-6..12 count=864
nxp6_three_point_corridor: nmd/nud/nwd=552/1648/1099; ngr=(0,1098,0); mrow=-9..12 count=1098
nxp6_two_circle: nmd/nud/nwd=624/1864/1243; ngr=(0,154,1088); mrow=-6..11 count=1242
nxp7_two_circle: nmd/nud/nwd=754/2254/1503; ngr=(3,335,1164); mrow=-6..13 count=1499
nxp6_two_corridor: nmd/nud/nwd=783/2341/1561; ngr=(0,294,1266); mrow=-6..11 count=1560
nxp7_two_corridor: nmd/nud/nwd=715/2137/1425; ngr=(25,287,1112); mrow=-6..13 count=1399
```

Expected-rejection cases from that run:

```text
nxp6_bad_two_circle: status 2; next-coarser-grid boundary rejection
nxp6_bad_two_corridor: status 2; next-coarser-grid boundary rejection
```

### Golden-check mode for reduced Fortran probes

The reduced Fortran probe script also supports golden checking:

```text
scripts/olam_reduced_fortran_probe.sh --check all
```

Latest observed result:
- Script exit status: `0`.
- All 12 cases printed `check ok`.
- The 10 accepted cases matched their expected summary lines.
- The 2 expected-rejection cases matched `status 2` and the Fortran next-coarser-grid boundary rejection text.

Representative commands:

```text
scripts/olam_reduced_fortran_probe.sh --check nxp6_circle
scripts/olam_reduced_fortran_probe.sh --check nxp6_bad_two_circle
```

Note:
- The script uses a per-process scratch `WORKDIR` by default (`/tmp/olam-reduced-probe-run-$$`).
- Default runs use distinct per-process `WORKDIR` values. If you override `WORKDIR` manually, use separate values for parallel runs.

## One-command Fortran/Rust Method-C parity check

Use this wrapper to run the reduced Fortran golden checks and the Rust direct-comparison tests together:

```text
scripts/check_olam_method_c_fortran_parity.sh
```

It runs, in order:

```text
scripts/olam_reduced_fortran_probe.sh --check all
cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib olam_method_c_matches_reduced_fortran -- --nocapture
cargo test --manifest-path rust/earthmesh_mesh/Cargo.toml --lib rejects_reduced_fortran -- --nocapture
```

Latest observed result:
- Reduced Fortran golden checks: 12 cases printed `check ok`.
- Rust accepted reduced-Fortran comparisons: `10 passed; 0 failed`.
- Rust rejected reduced-Fortran comparisons: `2 passed; 0 failed`.
- Wrapper script exit status: `0`.

### Reduced Fortran probe scratch-directory cleanup

The reduced Fortran probe script now uses a per-process default scratch directory and cleans it on exit:

```text
WORKDIR=/tmp/olam-reduced-probe-run-$$
CLEAN_WORKDIR=1
```

Behavior:
- If `WORKDIR` is not explicitly set, the script creates a process-unique scratch directory and removes it on exit.
- If `WORKDIR` is explicitly set, the script preserves it on exit for debugging.
- Set `CLEAN_WORKDIR=0` to preserve an auto-generated scratch directory for inspection.

Example debug run:

```text
CLEAN_WORKDIR=0 scripts/olam_reduced_fortran_probe.sh --check nxp6_circle
```

## Current success criteria for Method-C Fortran/Rust parity

Current behavior-level parity is considered proven when this command exits with status `0`:

```text
scripts/check_olam_method_c_fortran_parity.sh
```

That command proves all of the following:
- The reduced Fortran Method-C harness can be rebuilt from the original OLAM source tree.
- The reduced Fortran harness reproduces the 10 accepted golden topology summaries.
- The reduced Fortran harness reproduces the 2 expected next-coarser-grid boundary rejections.
- Rust matches all 10 accepted Fortran summaries through direct regression tests.
- Rust rejects both invalid/tight two-level cases through direct regression tests.

What this now proves:
- Rust rejects the two reduced invalid/tight cases before the Method-C perimeter-triple invariant, matching the reduced Fortran rejection class more closely.
- Rust preserves the accepted reduced-Fortran summary cases after that earlier check.

What this still does not prove:
- It does not prove full table-by-table equality for every M/U/W field; the tests currently lock the same summary fields that the reduced probe reports.
- It does not prove full OLAM executable parity. The direct Fortran side here is a reduced Method-C harness because the full OLAM build still fails outside this Method-C path.
- It does not prove spring-dynamics parity; the reduced probe keeps spring dynamics as a stub/no-op, while the Rust side has separate spring behavior tests.

Current decision:
- Further parity work should either expand the reduced probe to table-by-table M/U/W dumps or restore a complete OLAM executable build, rather than adding more summary-only cases.

## Expanded parity gates after full-table and spring work

The direct parity gate is now:

```text
scripts/check_olam_method_c_fortran_parity.sh
```

That command now proves more than summary-level topology:
- It still rebuilds and runs the reduced Fortran Method-C harness for the 10 accepted golden cases and 2 rejected boundary cases.
- It still runs the Rust accepted/rejected reduced-Fortran regression tests.
- It now generates canonical reduced-Fortran M/U/W topology-table dumps for all 10 accepted cases and diffs them against Rust dumps generated by `cargo run --example olam_method_c_probe -- tables <case>`.
- The canonical table dump includes active M/U/W ids, M `npoly`, active M `iu/iw`, U `mrlu/im/iu/iw`, W `npoly/mrlw/mrlw_orig/mrow/ngr/im/iu/iw`, and M refinement/grid metadata.
- It normalizes inactive M-tail slots beyond `npoly`, because Fortran leaves some inactive `itab_md%iu/iw` tail entries as ring-walk scratch values while Rust stores them as inactive placeholders.
- It now compiles and runs the real Fortran `spring_dynamics.f90` global spring path for `nxp6_circle` under `MAKEGRID_PLOT` 100-iteration behavior, then compares sampled Rust `spring_global` coordinates at rounded-meter precision.

The full OLAM binary build smoke gate is now:

```text
scripts/check_olam_full_gfortran_build.sh
```

This command builds the complete OLAM `build_olam_test` source list with `gfortran` in a temporary work directory and produces `olam-7.0` on this machine. It does not modify the OLAM source tree. The smoke build applies temporary compatibility shims only inside the scratch directory:
- removes Intel-style module-procedure `import` lines that `gfortran` rejects;
- removes the invalid `sdt/sds` duplicate import from the UMWM source-function use-list;
- removes a stale `map_proj:get_weights_lonlat` use-list entry where the symbol is actually provided by `minterp_lib`;
- links no-op NCAR/GKS plotting stubs so the non-plotting OLAM executable can link without NCAR Graphics libraries.

Remaining boundary:
- The full binary smoke proves the whole OLAM source list can compile and link locally with gfortran plus temporary compatibility shims.
- It does not yet run a full OLAM MAKEGRID namelist end-to-end and compare HDF5 output tables against Rust.
- The spring gate currently proves global `spring_dynamics_globe` sampled numeric parity at rounded-meter precision; nest spring has Rust behavioral tests but does not yet have a full Fortran sampled-coordinate diff gate.

## Method-C automatic mask repair boundary

Rust now treats Fortran `perim_fill3` non-triplet perimeter overruns as a repairable
mask-construction problem rather than reproducing the unchecked Fortran array overrun.
When a selected Method-C mask cannot be consumed in transition triples, Rust first
tries local same-MRL boundary growth, then validates the result through the real
Method-C table emission path. If table emission exposes a local transition-patch or
`npoly > 7` failure, Rust tries targeted boundary fill/shrink repairs and, as a final
single-pass fallback, erodes the selected boundary layer before retrying.

This repair layer is intentionally inactive for already valid golden cases. The
current parity gate still passes all reduced Fortran accepted/rejected cases and all
full M/U/W table diffs for the stable golden set.

Newly covered boundary cases:
- `nxp=7`, single-level circle, radius `4,000,000`: Fortran overruns `perim_fill3`
  on this parameter set; Rust repairs the selected mask and emits a closed topology.
- `nxp=7`, single-level corridor, endpoint radii `4,000,000`: Fortran overruns
  `perim_fill3`; Rust repairs the selected mask and emits a closed topology.

Remaining limitation:
- The same `nxp=7`, radius `4,000,000` parent combined with a second child nest is
  now past the original parent `perim_fill3` overrun, but can still fail at the child
  pass because parent repair and child halo are not planned jointly. Solving that
  requires a Phase 3 parent-child planner that can co-optimize parent expansion or
  child shrinkage before either pass is committed.

### Parent-child retry status

A conservative parent-child retry path now exists for no-spring table generation: if a
child pass fails after a repaired parent pass, Rust can return to the previous parent
checkpoint, erode the parent mask, rebuild the parent, then reselect and retry the
child. Child fallback uses a stricter annealed-mask path so it does not expand itself
back into the repaired parent transition halo.

This improves the architecture for Phase 3 without changing the stable Fortran parity
cases. The full `olam_method_c` regression gate and the reduced Fortran/Rust full-table
parity script still pass after this change.

Current future gates remain ignored:
- `olam_method_c_anneals_nxp7_two_circle_after_repaired_parent`
- `olam_method_c_anneals_nxp7_two_corridor_after_repaired_parent`

Those two cases still require a stronger parent-child halo optimizer. The attempted
local parent erosion / child strict annealing can move the failure boundary but does
not yet prove a legal two-level Method-C layout for the `nxp=7`, parent-radius
`4,000,000` combinations. They are therefore documented as future gates rather than
claimed supported cases.

### Parent-child geometric annealing update

The parent-child retry path now has a geometry-level annealing step for circle and
corridor parent regions. When a child pass fails and the previous parent pass itself
required Method-C repair, Rust retries the parent with scaled parent radii
(`0.95`, `0.90`, ...), rebuilds the parent, then reselects and retries the child.
This allows large parent specifications to retreat to the nearest legal Method-C
layout while keeping the child request active.

The retry is deliberately gated: it is enabled only when the previous parent pass
needed automatic repair. If the parent was already valid under strict Method-C and the
child is too close to the parent boundary, Rust still preserves the reduced Fortran
negative behavior and rejects the case.

New default regression coverage:
- `olam_method_c_anneals_nxp7_two_circle_after_repaired_parent`
- `olam_method_c_anneals_nxp7_two_corridor_after_repaired_parent`

Both previously represented future gates for `nxp=7`, parent radius `4,000,000`; both
now pass with closed topology and `npoly <= 7` after parent geometric annealing.
