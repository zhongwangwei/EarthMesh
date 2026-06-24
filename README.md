# EarthMesh

EarthMesh is a mesh generation tool for land surface, ocean, and atmospheric models. It generates unstructured meshes with adaptive refinement based on various surface characteristics.

## Key Features

- Generates well-centered Delaunay triangle and dual hexagonal meshes
- Performs adaptive mesh refinement based on configurable thresholds
- Supports refinement criteria like land type heterogeneity, topography, LAI, soil properties, etc.
- Supports generation of land surface, ocean and atmospheric meshes for global/limited-area modeling
- Outputs mesh files compatible with CoLM2024, FVCOM, MPAS, OLAM and other models

## Dependencies

- Rust toolchain with Cargo
- NetCDF library

## Directory Structure

```
EarthMesh/
├── rust/                   # Rust implementation crates
│   ├── earthmesh_core      # Constants, configuration, runtime state
│   ├── earthmesh_mesh      # Mesh and refinement kernels
│   ├── earthmesh_geometry  # Geometry backend
│   └── earthmesh_cli       # mkgrd-compatible command-line adapter
├── examples/               # Curated runnable/reference cases
│   ├── default/            # Default atmosphere/land/ocean hex global namelists
│   └── merit_hydro/        # MERIT-Hydro regional hydro/coast cases
├── Makefile                # Rust/Cargo build entrypoint
├── make.sh                 # Compatibility wrapper around make
├── make_gnu.sh             # Compatibility wrapper around make
├── switch_compiler.sh      # Compatibility no-op; Rust/Cargo is the build path
├── .gitignore              # Git ignore rules
└── README.md               # This file
```

## Compilation

EarthMesh now builds through the Rust implementation. The root `Makefile` compiles `rust/earthmesh_cli` and copies the resulting binary to `./mkgrd.x` for compatibility with existing workflows.

```bash
make
```

For a debug build:

```bash
make BUILD_PROFILE=debug
```

The compatibility scripts delegate to the same Rust build path:

```bash
./make.sh
./make_gnu.sh
```

After compilation, the executable `mkgrd.x` will be created in the root directory.

### Clean Build

To clean compiled files:

```bash
make clean
```

## Usage

### Basic Usage

Run the executable with a namelist file from the examples directory:

```bash
./mkgrd.x examples/default/atmosphere_hex_global.nml
```

### Configuration

Edit a namelist file under `examples/default/` or a regional case under `examples/merit_hydro/` to configure:
- Mesh resolution
- Refinement criteria and thresholds
- Input/output file paths
- Domain specifications

### Example Namelist Files

- `examples/default/atmosphere_hex_global.nml` - Default global atmospheric hex mesh
- `examples/default/land_hex_global.nml` - Default global land hex mesh
- `examples/default/ocean_hex_global.nml` - Default global ocean hex mesh
- `examples/merit_hydro/gba/` - MERIT-Hydro Greater Bay Area hydro/coast case package
- `examples/merit_hydro/yangtze_delta/` - MERIT-Hydro Yangtze Delta hydro/coast case package

### Hydro coupling/refinement workflow

Beyond mesh generation, the same `./mkgrd.x` binary drives an end-to-end hydro pipeline:
a mesh plus MERIT-Hydro river/coast corridors become a CoLM coupling table, an R8
refinement plan, and (optionally) an R7 land/ocean coupling-quality report.

**1. Mesh → cell polygons.** Read an MPAS/EarthMesh mesh NetCDF into per-cell GeoJSON
(the overlay input the workflow consumes):

```bash
./mkgrd.x --mpas-cell-polygons mesh.nc cells.geojson [--bbox W S E N] [--max-cells N]
```

**2. One-shot workflow.** Overlay `cells.geojson` × `corridors.geojson` (the river/coast
corridors, e.g. from a `examples/merit_hydro/*` case) and write, into `out_dir/`,
`intersections.geojson`, `colm_coupling.csv`, `refinement_plan.json`, and
`workflow_manifest.json`:

```bash
./mkgrd.x --hydro-workflow cells.geojson corridors.geojson out_dir \
    [--classes R2,R3] [--min-fraction 0.0] [--max-level 3] [--max-refined-cells N] \
    [--domain-bbox W S E N | --domain-geojson region.geojson]
```

Add an EarthMesh gridfile + a land-type NetCDF to also write `coupling_quality.json`
(R7 land/ocean coupling QA — `--mesh` and `--landtype` must be given together):

```bash
./mkgrd.x --hydro-workflow cells.geojson corridors.geojson out_dir \
    --mesh gridfile.nc --landtype landtype.nc [--gridnum-perdegree 120]
```

**Individual steps** — the workflow chains these; run them standalone if preferred:

```bash
./mkgrd.x --hydro-cell-intersections cells.geojson corridors.geojson intersections.geojson [--classes R2,R3]
./mkgrd.x --colm-coupling-from-intersections intersections.geojson colm_coupling.csv [min_fraction]
./mkgrd.x --plan-refinement-from-hydro intersections.geojson refinement_plan.json [--max-level 3]
./mkgrd.x --coupling-quality-from-mesh gridfile.nc landtype.nc coupling_quality.json [--gridnum-perdegree 120]
```

## Output

Output directories are created based on the configuration in the namelist file. Typical output includes:

- **meshfile/** - Main mesh definition files (cell vertices, connectivity)
- **result/** - Final mesh files from the last refinement iteration
- **contain/** - Containment relationship files
- **threshold/** - Threshold files for adaptive refinement
- **tmpfile/** - Intermediate output files during refinement

## Implementation Layout

The active implementation is in Rust:

- `rust/earthmesh_core` - constants, namelist configuration, and runtime state
- `rust/earthmesh_mesh` - mesh generation, geometry, refinement, and post-processing kernels
- `rust/earthmesh_geometry` - geometry backend
- `rust/earthmesh_cli` - mkgrd-compatible command-line adapter and model-output writers

The legacy Fortran sources are no longer part of the active source tree. Their
completed migration status is tracked in `docs/fortran_to_rust_migration_manifest.json`.

## Authors

- Rui Zhang (V2)
- Hanwen Fan (V1)
- Zhongwang Wei @ SYSU

## Citation

If you use this tool in your research, please cite:

Fan, H., Xu, Q., Bai, F., Wei, Z., Zhang, Y., Lu, X., Wei, N., Zhang, S., Yuan, H., Liu, S. and Li, X., 2024. An unstructured mesh generation tool for efficient high-resolution representation of spatial heterogeneity in land surface models. *Geophysical Research Letters*, 51(6), p.e2023GL107059.

## License

This project is licensed under the GNU General Public License v2.0.

## Contact

For questions or support, please contact:
- Zhongwang Wei (zhongwang007@gmail.com)

## Revision History

- 2025.10.28 - Reorganized code structure with src/ and examples/ directories
- 2025.01.09 - V2 initial version developed by Rui Zhang
- 2024.07.19 - Updates by Zhongwang Wei
- 2023.10.28 - Development by Hanwen Fan and Zhongwang Wei @ SYSU
- 2023.02.21 - Updates by Zhongwang Wei @ SYSU
- 2021.12.02 - Initial development by Zhongwang Wei @ SYSU
