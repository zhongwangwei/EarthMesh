# EarthMesh — working notes

## There are two Rust workspaces, not one

The root workspace holds thirteen crates, all under `rust/` -- including all
three refinement backends (`earthmesh_refine_method_c`,
`earthmesh_refine_redgreen`, `earthmesh_refine_harp_dv`).
**`gui-tauri/src-tauri` is its own workspace and is not a member of it.**

That means a root-level command silently covers only part of the repository:

| command | covers | misses |
|---|---|---|
| `cargo test --workspace` | the thirteen engine crates | the Tauri crate |
| `cargo fmt --all` | the thirteen engine crates | the Tauri crate |
| `cargo clippy --workspace` | the thirteen engine crates | the Tauri crate |

Nothing fails when the GUI is skipped — the command reports success for what it
did run, which reads as "everything passed". After touching anything under
`gui-tauri/`, run the GUI gates explicitly:

```
make check-gui-js     # frontend drift + syntax + i18n key coverage
make fmt-gui          # cargo fmt --check on the Tauri crate
make clippy-gui       # -D warnings
make test-gui         # Tauri command tests (also runs check-gui-js)
```

These four are exactly CI's `gui` job. `make test-full` runs the engine tests and
the GUI tests together; a root-level `cargo test` does not.

## Which gate matches which CI job

CI (`.github/workflows/ci.yml`) has three jobs. Reproduce them locally with:

- `fast` → `make fmt && make clippy && make test-fast` (no NetCDF, no GUI)
- `gui` → the four commands above
- `heavy` → clippy and tests on `earthmesh_cli` alone, against **dynamic system
  NetCDF**:
  ```
  cargo clippy --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets -- -D warnings
  cargo test   --manifest-path rust/earthmesh_cli/Cargo.toml --all-targets
  ```
  `make test` is the nearest local equivalent, but it links `static-netcdf` and
  covers every crate — so it is a superset that exercises a different NetCDF
  path. When a `heavy` failure will not reproduce, that difference is the first
  thing to check.

`make fmt` and `make clippy` list crates one by one rather than using
`--workspace`, because `earthmesh_cli` needs NetCDF and the fast job has none.
A crate added to the workspace is **not** automatically covered — add it to the
Makefile lists too. This has already gone wrong once: `earthmesh_boundary`,
`earthmesh_refine` and `earthmesh_refine_harp_dv` were skipped by the fast job
for several commits while it reported success. The counts to check against are
13 in `fmt` (every crate), 12 in `clippy` and `test-fast` (all but
`earthmesh_cli`, which needs NetCDF), and 13 workspace members.

## End-to-end regressions

```
make regression          # both of the below
make regression-boundary # polar + dateline refinement boundaries
make regression-basin-hole  # regional domain with an interior hole
```

Both build a release CLI first; set `EARTHMESH_BOUNDARY_CLI` /
`EARTHMESH_BASIN_HOLE_CLI` to an existing binary to skip that. They generate
their own fixtures — the repository carries no shapefiles or rasters.

## Cutting a release

The convention is **a branch per version**, not just a tag: `v1.0.0`, `v2.0.0`,
`v3.0.0-alpha1`, `v3.0.0-alpha2`, … Both workflows trigger on `branches: v*`, so
pushing the branch is what builds the wheels. `master` is a separate lineage and
is not where the v3 line lives.

1. Bump the version everywhere. It appears in fourteen `Cargo.toml` files plus
   both `Cargo.lock` files (`cargo update -w` in the root and in
   `gui-tauri/src-tauri`), `gui-tauri/src-tauri/tauri.conf.json`, the README
   title and changelog, and **four `--version` assertions in
   `.github/workflows/python-release.yml`** that compare the string verbatim.
   `git grep -l '<old version>'` finds them; check `earthmesh_cli --version`
   afterwards.
2. Commit, create branch `v<version>`, push it, then push the tag as a separate
   refspec — branch and tag share a name, so `git push origin v<version>` is
   ambiguous and fails:
   ```
   git push origin refs/heads/v3.0.0-alpha4:refs/heads/v3.0.0-alpha4
   git push origin refs/tags/v3.0.0-alpha4:refs/tags/v3.0.0-alpha4
   ```
3. `gh release create v<version> --prerelease --notes-file …` for an alpha.

**The same name collision silently misreads on the way back in.** `git fetch
origin v3.0.0-alpha4` and `git checkout v3.0.0-alpha4` do not fail the way the
push does — they resolve, to the *tag*, which stays where it was cut while the
branch moves on. After merging into the branch, that reads back as "the merge
did not happen": a real merge into `refs/heads/v3.0.0-alpha4` still showed the
month-old tag commit as the tip. Use the full ref, or ask the API, when the
answer matters:

```
git fetch origin refs/heads/v3.0.0-alpha4:refs/remotes/origin/v3.0.0-alpha4
gh api repos/zhongwangwei/EarthMesh/git/ref/heads/v3.0.0-alpha4 --jq .object.sha
```

**Pushing over HTTPS fails when the commit touches `.github/workflows/`**: the
`gh` OAuth token carries `gist, read:org, repo` but not `workflow`. SSH
(`git@github.com:zhongwangwei/EarthMesh.git`) works and is what `gh` is
configured for anyway. `gh auth refresh -s workflow` fixes the HTTPS path.

## When a run seems stuck

Sample it before reading code: `sample <pid> 4 -f /tmp/out.txt`, then look at the
main thread's frames. A global run that appeared hung was spending every sample
in one neighbourhood loop whose cost scaled with the *mesh* generation, not the
raster — 1.7e14 reads for one criterion, days rather than minutes, and perfectly
fast on every small test. Guide section 11.3 has the numbers and the fix.

Two traps that follow from that case:

- **Radius-dependent costs hide in small tests.** The demand producers take
  `radius_cells` from the cell size being refined; at production raster
  resolution that is ~107 cells, and a fixture at 1 cell per degree never shows
  it. Estimate the production number before assuming a loop is affordable.
- **Passing tests are not equivalence.** When rewriting an algorithm for speed,
  keep the old implementation as a test oracle and compare cell for cell. The
  existing land-type tests all stayed green across a rewrite that would have
  changed results; the oracle test caught the one real discrepancy — which
  turned out to be the oracle's own bug, and only a direct comparison could
  have told the difference.
- **A runaway guard that scans to get its bound is the runaway.** `MeshState`
  has cheap-looking helpers that walk every slot — `triangle_count()` is
  `active_triangle_slots().count()`. Three loops called it just to size a
  "stop if this spins" limit, so a six-triangle fan paid a sweep of the whole
  mesh on every objective evaluation: 90.9% of samples, and CI timing out for
  two weeks. Such a limit needs an upper bound, never an exact count, and
  `triangles().len()` is one in O(1). Guide 11.68; 11.3 is the same defect
  class in a different loop.
- **A job that times out reports nothing about what came after it.** The `fast`
  job hid a genuine assertion failure behind its timeout for two weeks. Before
  reading a green-ish CI history as evidence, check the jobs actually finished.
  A **cancelled** job says just as little, and the usual cause is self-inflicted:
  pushing again while a run is in flight cancels it through the concurrency
  group, and `heavy` is the only job that runs `refine_pipeline` in full. Hold
  doc-only commits until the run finishes rather than throwing away the one
  result that covers the CLI.
- **Before concluding from silence, check the instrument can speak.** Three times
  in one session an absent signal was read as an absent event, and every time the
  tool was suppressing it: `eprintln!` probes print nothing without
  `--nocapture`, so a live branch measured as "fires zero times" and was nearly
  deleted as dead; `cmd | grep | tail` emits nothing until the stream ends, so a
  running test suite looked hung; a cancelled CI job looked like a failing one.
  When a measurement returns nothing, prove the path works — probe something that
  definitely happens — before believing the nothing. Related: an analysis can lie
  the same way a tool can. A per-invocation counter restarting at 1 was used to
  split runs apart, which paired one pass's opening value with another's closing
  value and produced a 65x growth that does not exist. Print both ends on one
  line instead of reconstructing pairs afterwards.

## Reading the code

`docs/mesh_construction_technical_guide.md` is the algorithm reference —
Method-C nesting, the h-field, canonical 1-based indexing, the mask post-process
chain. Section 11 records known limits, and 11.1 the silent-failure defect
classes found in the 2026-08 audit: every one produced a mesh that was valid,
passed its quality checks, and was not what the project asked for. Worth reading
before adding a refinement criterion or touching the carve.
