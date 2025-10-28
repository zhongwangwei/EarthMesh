# EarthMesh

EarthMesh is a mesh generation tool for land surface, ocean, and atmospheric models. It generates unstructured meshes with adaptive refinement based on various surface characteristics.

## Key Features

- Generates well-centered Delaunay triangle and dual hexagonal meshes
- Performs adaptive mesh refinement based on configurable thresholds
- Supports refinement criteria like land type heterogeneity, topography, LAI, soil properties, etc.
- Supports generation of land surface, ocean and atmospheric meshes for global/limited-area modeling
- Outputs mesh files compatible with CoLM2024, FVCOM, MPAS, OLAM and other models

## Dependencies

- NetCDF library
- Fortran compiler (Intel Fortran or gfortran)

## Directory Structure

```
EarthMesh/
├── src/                    # Source code directory
│   ├── mkgrd.F90          # Main program for mesh generation
│   ├── MOD_*.F90          # Module files for preprocessing and refinement
│   ├── blas.F90           # BLAS routines
│   ├── lapack.F90         # LAPACK routines
│   ├── consts_coms.F90    # Constants and common variables
│   └── icosahedron.F90    # Icosahedral mesh generation
├── examples/               # Example configuration files
│   ├── *.nml              # Namelist configuration files
│   └── mask_refine_filelist.txt
├── Makefile                # Build configuration
├── Makeoptions*            # Compiler-specific options
├── make.sh                 # Intel compiler build script
├── make_gnu.sh             # GNU compiler build script
├── switch_compiler.sh      # Compiler switching utility
├── .gitignore              # Git ignore rules
└── README.md               # This file
```

## Compilation

### Using Intel Compiler

```bash
make
```

Or use the build script:

```bash
./make.sh
```

### Using GNU Fortran

```bash
./make_gnu.sh
```

Or manually:

```bash
cp Makeoptions.gnu Makeoptions
make
```

### Switch Compiler

Use the `switch_compiler.sh` script to switch between Intel and GNU compilers:

```bash
./switch_compiler.sh
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
./mkgrd.x examples/Atmos_hex_NXP64_refine2_Global_251027.nml
```

### Configuration

Edit the namelist file in the `examples/` directory (e.g., `Atmos_hex_NXP64_refine2_Global_251027.nml`) to configure:
- Mesh resolution
- Refinement criteria and thresholds
- Input/output file paths
- Domain specifications

### Example Namelist Files

- `examples/Atmos_hex_NXP64_refine2_Global_251027.nml` - Global atmospheric mesh with refinement
- `examples/Atmos_hex_NXP64_refine2_Global_Simple_251027.nml` - Simplified global atmospheric mesh

## Output

Output directories are created based on the configuration in the namelist file. Typical output includes:

- **meshfile/** - Main mesh definition files (cell vertices, connectivity)
- **result/** - Final mesh files from the last refinement iteration
- **contain/** - Containment relationship files
- **threshold/** - Threshold files for adaptive refinement
- **tmpfile/** - Intermediate output files during refinement

## Module Description

All source modules are located in the `src/` directory:

- `mkgrd.F90` - Main program
- `MOD_grid_preprocess.F90` - Grid preprocessing and initialization
- `MOD_refine.F90` - Mesh refinement algorithms
- `MOD_Area_judge.F90` - Area and region determination
- `MOD_GetContain.F90` - Cell containment calculations
- `MOD_GetRef.F90` - Reference data processing
- `MOD_data_preprocess.F90` - Input data preprocessing
- `MOD_file_preprocess.F90` - File I/O operations
- `MOD_mask_postproc.F90` - Post-processing of mask data
- `consts_coms.F90` - Constants and common variables
- `icosahedron.F90` - Icosahedral mesh generation
- `blas.F90`, `lapack.F90` - Linear algebra routines

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
