# CoLM Coupling Package Table Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing CoLM coupling utility so a hydro/coast delivery package can produce one all-cell coupling CSV plus a summary JSON.

**Architecture:** Reuse `util/hydro_mesh/colm_coupling.py`. Preserve the existing river-only positional CLI and add a `package` subcommand. Implement pure helpers for loading manifest paths, joining background/river/coast properties by `cell_id`, writing CSV, and writing summary JSON.

**Tech Stack:** Python standard library `csv`, `json`, `argparse`; pytest; existing hydro package manifest format.

---

## File Structure

- Modify: `util/hydro_mesh/colm_coupling.py` — add package-driven all-cell table writer and CLI subcommand.
- Modify: `tests/test_colm_coupling.py` — add package writer/CLI tests while preserving existing river-only tests.
- Modify: `docs/hydro_mesh_data_requirements.md` — document the N112 coupling command.

### Task 1: Package rows and writer

- [ ] **Step 1: Write failing package writer test**

Append a test that builds a synthetic delivery manifest plus background, river, and coast GeoJSON files. Assert `write_colm_package_coupling()` writes three rows for three background cells, joins river/coast fields on `cell_id`, and preserves an all-cell row with no overlaps.

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
python3 -m pytest tests/test_colm_coupling.py::test_write_colm_package_coupling_writes_all_background_cells -q
```

Expected: FAIL because `write_colm_package_coupling` is not defined.

- [ ] **Step 3: Implement package writer**

Add:

- `PACKAGE_COUPLING_FIELDS`
- `write_colm_package_coupling(delivery_manifest, output_dir)`
- `_package_rows_from_collections(case_name, background, river, coast)`
- `_index_feature_properties(collection, fraction_name)`
- `_write_package_csv(rows, output_csv)`
- `_write_package_summary(...)`

- [ ] **Step 4: Verify package writer test passes**

Run the focused test again. Expected: PASS.

### Task 2: Package CLI and docs

- [ ] **Step 1: Add failing CLI test**

Add a test calling:

```python
main(["package", "--delivery-manifest", str(manifest), "--output-dir", str(output_dir)])
```

Assert it returns `0` and writes `colm_coupling_cells.csv` and `colm_coupling_summary.json`.

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
python3 -m pytest tests/test_colm_coupling.py::test_colm_coupling_package_cli_writes_csv_and_summary -q
```

Expected: FAIL until the CLI subcommand exists.

- [ ] **Step 3: Implement CLI subcommand while preserving legacy CLI**

Detect `argv[0] == "package"` and route to a package subparser. Otherwise keep existing positional river-only behavior unchanged.

- [ ] **Step 4: Update docs and run verification**

Document the real N112 command and run:

```bash
python3 -m pytest tests/test_colm_coupling.py tests/test_refinement_package.py -q
python3 -m compileall util/hydro_mesh/colm_coupling.py
```

### Task 3: Real N112 smoke and commit

- [ ] **Step 1: Run real package coupling smoke**

Run:

```bash
python3 -m util.hydro_mesh.colm_coupling package \
  --delivery-manifest /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20/delivery_manifest.json \
  --output-dir /Users/zhongwangwei/Desktop/EarthMesh_cama_scratch/packages/yangtze_delta_N112_r3d3_cst20/colm_coupling
```

Expected summary: `background_cell_count=2574`, `river_cell_count=500`, `coast_cell_count=374`.

- [ ] **Step 2: Final verification and commit**

Run the focused tests and commit with Lore trailers.

## Self-Review

- Spec coverage: package table, all-cell rows, CLI, docs, and real N112 smoke are covered.
- Deferred-work scan: no deferred markers are present.
- Compatibility: existing river-only functions and CLI remain available.
