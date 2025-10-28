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

All source files are located in the root directory for easy access:

- `mkgrd.F90` - Main program for mesh generation
- `MOD_*.F90` - Module files for various preprocessing and refinement operations
- `*.nml` - Namelist configuration files for different mesh generation scenarios
- `Makefile` - Build configuration
- `Makeoptions*` - Compiler-specific options
- `make.sh`, `make_gnu.sh` - Build scripts
- `switch_compiler.sh` - Utility to switch between compilers

## Compilation

### Using Intel Compiler

```bash
make
```

### Using GNU Fortran

```bash
./make_gnu.sh
```

Or manually:

```bash
make -f Makefile MAKEOPTIONS=Makeoptions.gnu
```

### Switch Compiler

Use the `switch_compiler.sh` script to switch between Intel and GNU compilers:

```bash
./switch_compiler.sh
```

## Usage

### Basic Usage

Run the executable with a namelist file:

```bash
./mkgrd.x <namelist_file.nml>
```

Example:

```bash
./mkgrd.x Atmos_hex_NXP64_refine2_Global_251027.nml
```

### Configuration

Edit the namelist file (e.g., `Atmos_hex_NXP64_refine2_Global_251027.nml`) to configure:
- Mesh resolution
- Refinement criteria and thresholds
- Input/output file paths
- Domain specifications

### Example Namelist Files

- `Atmos_hex_NXP64_refine2_Global_251027.nml` - Global atmospheric mesh with refinement
- `Atmos_hex_NXP64_refine2_Global_Simple_251027.nml` - Simplified global atmospheric mesh

## Output

Output directories are created based on the configuration in the namelist file. Typical output includes:

- **meshfile/** - Main mesh definition files (cell vertices, connectivity)
- **result/** - Final mesh files from the last refinement iteration
- **contain/** - Containment relationship files
- **threshold/** - Threshold files for adaptive refinement
- **tmpfile/** - Intermediate output files during refinement

## Module Description

- `MOD_grid_preprocess.F90` - Grid preprocessing and initialization
- `MOD_refine.F90` - Mesh refinement algorithms
- `MOD_Area_judge.F90` - Area and region determination
- `MOD_GetContain.F90` - Cell containment calculations
- `MOD_GetRef.F90` - Reference data processing
- `MOD_data_preprocess.F90` - Input data preprocessing
- `MOD_file_preprocess.F90` - File I/O operations
- `MOD_mask_postproc.F90` - Post-processing of mask data

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

- 2025.01.09 - V2 initial version developed by Rui Zhang
- 2024.07.19 - Updates by Zhongwang Wei
- 2023.10.28 - Development by Hanwen Fan and Zhongwang Wei @ SYSU
- 2023.02.21 - Updates by Zhongwang Wei @ SYSU
- 2021.12.02 - Initial development by Zhongwang Wei @ SYSU
