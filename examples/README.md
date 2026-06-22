# EarthMesh v3 examples

Examples fall into two classes. The CI / regression suite only exercises **runnable
templates**; **external-data cases** are documented but require data you provide.

## Runnable templates (no external data)

Work out-of-the-box — no large dataset, no `${EARTHMESH_DATA}`. Used by the smoke /
regression tests.

| Path | Target | Notes |
|------|--------|-------|
| `00_quickstart_n16.nml` | Quickstart global mesh (NXP=16) | Tiny synthetic; default smoke case loaded by the GUI. |
| `default/atmosphere_hex_global.nml` | Global hex atmosphere → MPAS | Base-mesh only. |
| `default/land_hex_global.nml` | Global hex land → CoLM | Base-mesh only. |
| `default/ocean_hex_global.nml` | Global hex ocean → FVCOM | Base-mesh only. |

Run one:

```sh
make build
./mkgrd.x examples/00_quickstart_n16.nml
```

## External-data cases (require `${EARTHMESH_DATA}`)

Need real input datasets (MERIT-Hydro tiles, landtype, etc.). Paths use the
`${EARTHMESH_DATA}` placeholder; set it before running. Each case has its own README.

| Path | Needs | Notes |
|------|-------|-------|
| `merit_hydro/gba/` | MERIT-Hydro tiles | Greater Bay Area river/coast refined coupled mesh. See `gba/README.md`. |
| `merit_hydro/yangtze_delta/` | MERIT-Hydro tiles | Yangtze delta estuary mesh. See `yangtze_delta/README.md`. |

```sh
export EARTHMESH_DATA=/path/to/your/datasets
./mkgrd.x examples/merit_hydro/gba/case.nml
```

> The `examples_paths` regression test (in `earthmesh_core`) guards that committed
> examples carry no personal absolute paths, that runnable templates contain no
> `${EARTHMESH_DATA}` placeholder, and that every external-data case ships a README.
