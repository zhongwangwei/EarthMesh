# EarthMesh v3.0.0-alpha4

EarthMesh generates unstructured meshes for land, ocean, atmosphere, and coupled
Earth-system workflows. The v3 alpha line is the current Rust engine: namelists
remain a supported input format alongside the project schema, quality reports,
hydro/coast tools, and Tauri desktop GUI.

## What is in this alpha

- Rust `mkgrd.x` build path for the active mesh engine.
- Namelist execution for atmosphere, land, ocean, and coupled meshes.
- Project YAML/JSON intent layer through `earthmesh_project`, used by the GUI and
  executable through `./mkgrd.x --project project.yaml`.
- Mesh quality reports through `--mesh-quality`.
- MERIT-Hydro/CaMa export helpers and hydro/coast coupling workflow tools.
- EarthMesh Studio desktop GUI in `gui-tauri/`.

## Requirements

- Rust toolchain with Cargo.
- `make`.
- NetCDF/HDF5 development libraries if building the CLI against system NetCDF.
  The default `make build` path uses the `static-netcdf` feature and can take a
  while on the first build.
- Tauri platform prerequisites only if running the desktop GUI.

## Quick Start

After this alpha is published to PyPI, install the Rust CLI as a Python wheel
(the installed command remains `earthmesh_cli`):

```bash
python -m pip install earthmesh==3.0.0a2
earthmesh_cli --version
earthmesh_cli --help
```

Before the alpha wheel is uploaded to PyPI, build and install the same package
directly from a source checkout:

```bash
python -m pip install .
earthmesh_cli --version
```

Build the compatible executable:

```bash
make build
```

Run the tiny checked-in smoke case:

```bash
./mkgrd.x examples/00_quickstart_n16.nml
```

The run writes outputs under `cases/quickstart_n16/` and records
`run_manifest.json` in the current directory.

For a debug build:

```bash
make build BUILD_PROFILE=debug
```

Clean generated build products:

```bash
make clean
```

## Main CLI Entrypoints

Run a namelist:

```bash
./mkgrd.x examples/default/atmosphere_hex_global.nml
```

Run a v3 project file created by the GUI or another tool:

```bash
./mkgrd.x --project project.yaml
```

Inspect mesh quality from a generated gridfile:

```bash
./mkgrd.x --mesh-quality path/to/gridfile.nc4 quality_out --kind hex
```

Export MERIT-Hydro masks for a regional box:

```bash
./mkgrd.x --merit-hydro-geojson /path/to/MERIT_Hydro out_dir \
    --bbox W S E N --stride 1
```

Physical coast adjacency and production coupling require native `stride 1`;
sparse MERIT samples are rejected rather than treated as neighboring cells.

Convert an MPAS/EarthMesh mesh to cell polygons, then run the hydro/coast
workflow:

```bash
./mkgrd.x --mpas-cell-polygons mesh.nc cells.geojson --bbox W S E N
./mkgrd.x --hydro-workflow cells.geojson corridors.geojson out_dir \
    --classes R2,R3 --max-level 3
```

Use `./mkgrd.x --help` for the top-level usage summary.

## Examples

Runnable examples that do not require external data:

- `examples/00_quickstart_n16.nml` - tiny global smoke case.
- `examples/projects/quickstart.yaml` - minimal Project YAML smoke case.
- `examples/projects/auto_refine.yaml` - regional quality-to-HField AutoRefine loop.
- `examples/default/atmosphere_hex_global.nml` - global hex atmosphere to MPAS.
- `examples/default/land_hex_global.nml` - global hex land to CoLM.
- `examples/default/ocean_hex_global.nml` - global tri ocean to FVCOM.

External-data templates:

- `examples/merit_hydro/gba/` - Greater Bay Area MERIT-Hydro case.
- `examples/merit_hydro/yangtze_delta/` - Yangtze Delta MERIT-Hydro case.

See `examples/README.md` and the case-local READMEs for data requirements.

## EarthMesh Studio

The desktop GUI lives in `gui-tauri/`. It uses a static frontend and a thin Rust
Tauri backend over `earthmesh_project`; the GUI lowers project YAML to a namelist
and runs the discovered `mkgrd.x` executable.

```bash
make build
cd gui-tauri/src-tauri
cargo run
```

See `gui-tauri/README.md` for GUI commands, platform prerequisites, and current
known gaps.

## Repository Layout

```text
EarthMesh/
|-- Cargo.toml                 # Rust workspace
|-- LICENSE                    # GNU General Public License v2.0
|-- Makefile                   # build/test entrypoint
|-- README.md                  # this page
|-- cases/                     # generated and fixture run outputs
|-- docs/                      # architecture and technical references
|-- examples/                  # runnable namelists and external-data templates
|-- gui-tauri/                 # EarthMesh Studio desktop GUI
|-- input/                     # small/default input references
|-- rust/
|   |-- earthmesh_core/        # config, namelist parsing, run manifest support
|   |-- earthmesh_geometry/    # geometry helpers
|   |-- earthmesh_hfield/      # continuous target cell-width field
|   |-- earthmesh_mesh/        # mesh kernels, incl. Method-C nested refinement
|   |-- earthmesh_refine_redgreen/ # red-green refinement: any marked triangles
|   |-- earthmesh_quality/     # mesh quality metrics and reports
|   |-- earthmesh_refine_planner/ # experimental score-based planning crate
|   |-- earthmesh_project/     # v3 project schema and lowering
|   `-- earthmesh_cli/         # mkgrd-compatible CLI and workflows
`-- scripts/                   # local validation helpers
```

The crate boundaries and the canonical Project/CLI/GUI execution paths are
documented in [`docs/architecture.md`](docs/architecture.md).

## Verification

Fast local gate, no NetCDF-linked CLI:

```bash
make release-check
```

Full CLI tests with the default bundled NetCDF build:

```bash
make test
```

GUI checks:

```bash
make test-gui
```

Slow external-data and end-to-end gates are intentionally separated:

```bash
make test-slow
make test-full
```

## Outputs

Typical mesh runs write under the configured case directory:

- `gridfile/` - gridfile NetCDF outputs.
- `result/` - final model-facing mesh outputs.
- `contain/` - containment relationship files.
- `threshold/` - staged threshold/refinement inputs.
- `tmpfile/` - intermediate files.
- `run_manifest.json` - command, status, timestamps, cwd, warnings, and optional
  git SHA for the latest CLI run in the current directory.

Quality runs write `quality_summary.json`, `quality_summary.csv`,
`worst_cells.geojson`, and `quality_report.md` to the requested output directory.
Polygon side-count rows in those reports are observational; quality decisions come
from the reported gates and topology issues.
Use `--mesh-quality ... --kind tri|hex` to select the cell view; the report records
the selected view in `cell_view`. If omitted, the CLI uses the default `tri` view.
The CLI also prints `mesh_quality_cell_sides=...` as the same observational
summary.
Run `make check-mesh-quality-views` to smoke-test both `tri` and `hex` report
views against a generated temporary gridfile.

## Authors

- Zhongwang Wei (V3)
- Rui Zhang (V2)
- Hanwen Fan (V1)

## Citation

If you use this tool in your research, please cite:

Zhang, R., Z. Wei, Y. Luo, H. Fan, Q. Xu, S. Zhang, N. Wei, X. Lu, L. Li,
H. Yuan, X.-X. Li, S. Liu, W. Shangguan, and Y. Dai (submitted), EarthMesh: A
multi-scale unstructured mesh generation tool for Earth system models,
*Journal of Advances in Modeling Earth Systems*.

Fan, H., Xu, Q., Bai, F., Wei, Z., Zhang, Y., Lu, X., Wei, N., Zhang, S., Yuan,
H., Liu, S. and Li, X., 2024. An unstructured mesh generation tool for efficient
high-resolution representation of spatial heterogeneity in land surface models.
*Geophysical Research Letters*, 51(6), p.e2023GL107059.

## License

This project is licensed under the GNU General Public License v2.0 only. See
[`LICENSE`](LICENSE) for the complete terms.

## Contact

For questions or support, please contact:

- Zhongwang Wei (zhongwang007@gmail.com)

## Revision History

- 2026.07.16 - The v3 alpha Rust API removed unused `core::paths`, Project
  reproducibility-manifest/prelude, `quality::hydro_coast`, and planner `io`
  modules. These experimental facades had no production callers; alpha API
  consumers must use the active owner APIs instead of relying on compatibility
  shims. There is no general replacement for Project's removed fingerprint
  manifest: integrations should use the command-specific manifests or own their
  input hashing. Planner integrations should consume `RefinementReport` directly
  and serialize it at their CLI/application boundary instead of calling the
  removed generic `io` writers. The diagnostic `run_manifest.json` contract is
  now explicitly versioned as schema 1; its current minimal fields replace the
  earlier experimental manifest shape.
- 2026.08.08 - v3.0.0-alpha4 makes all three backends serve a closed curve, wires the
  boundary subsystem into the carve, and fixes a spring the default refinement route
  had been dropping in silence.
- 2026.08.05 - v3.0.0-alpha3 makes point+radius the default refinement route,
  decouples the demand criteria from how they are covered, carries cell lineage
  through refinement, adds the ICON and MPAS-Ocean writers, and closes five
  silent-failure defects found by a systematic audit (see the technical guide,
  section 11.1).
- 2026.07.14 - v3.0.0-alpha2 closes the AutoRefine/Hydro quality loops, adds
  auditable GUI decisions, and introduces the maturin/PyPI binary package.
- 2026.06.29 - v3.0.0-alpha1 README refresh for Rust CLI, project schema,
  quality, hydro/coast workflows, and Tauri GUI.
- 2025.10.28 - Reorganized code structure with src/ and examples/ directories.
- 2025.01.09 - V2 initial version developed by Rui Zhang.
- 2024.07.19 - Updates by Zhongwang Wei.
- 2023.10.28 - Development by Hanwen Fan and Zhongwang Wei @ SYSU.
- 2023.02.21 - Updates by Zhongwang Wei @ SYSU.
- 2021.12.02 - Initial development by Zhongwang Wei @ SYSU.
