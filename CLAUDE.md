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

## Reading the code

`docs/mesh_construction_technical_guide.md` is the algorithm reference —
Method-C nesting, the h-field, canonical 1-based indexing, the mask post-process
chain. Section 11 records known limits, and 11.1 the silent-failure defect
classes found in the 2026-08 audit: every one produced a mesh that was valid,
passed its quality checks, and was not what the project asked for. Worth reading
before adding a refinement criterion or touching the carve.
