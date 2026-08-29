# Real IGBP NXP80 safe-mother acceptance

Run on 2026-08-29 from the CMRC working tree based on commit
`ae9082a190a0878ab7744afc0d0132ee6bbb7791`
with `input/landtype_igbp_update.nc` (SHA-256
`89bde86be2436f8762bd9d2b9bcfa727193e74299941e9d1545222b54e41be2a`).
The calculated landtype criterion used `max_iter_cal=1`,
`refine_num_landtypes=true`, and `th_num_landtypes=1` so the real raster had
to produce a nonzero requirement before certification.

```text
NXP                              80
chosen level                      1
mother subdivision              160
vertices                     256002
edges                        768000
faces                        512000
minimum angle              54.00042866418126 degrees
maximum angle              72.00000000000148 degrees
open edges                       0
topology errors                  0
degree outside [5,7]             0
Euler                            2
charge                          12
Delaunay violations              0
Voronoi invalid cells            0
primal-dual errors               0
physical residuals               0
balance residuals                0
remap closure errors             0
Voronoi remap rows           256002
Voronoi remap entries        256002
requirement raster cells     259200
certification elapsed       295.557 s
test wall time (debug)       295.58 s
sampled peak RSS             914592 KiB
```

Peak RSS was sampled externally every 2 seconds because the portable runtime
record intentionally leaves `peak_memory_bytes` unset. Staged artifact sizes
were 26,635,697 bytes (gridfile), 8,481,869 bytes (remap), 1,146 bytes
(certificate), and 824 bytes (manifest).

Artifact SHA-256:

| Artifact | SHA-256 |
|---|---|
| certificate | `4bc4b143c511b6f51d854980d3ce2d8c89d5ec089009c724c7dfbcbf2721f5e9` |
| manifest | `7f901e29e34fbc61c4251f98c4fcbb36b2c6958d27155f977ef2fc1cce2686cd` |
| ready marker | `098044b3818d5eee8d25dd11fc92fb956d6a79a9c29aafd6a84cc3c4c16aaca4` |
| Voronoi identity remap | `783e1e69226f687ea91fd991d54085aaa5faef8f3b39f019f9caf80408a7581b` |
| gridfile | `a17714ef60e8e5df67bdae04cdd49da308e6779db6081686313509d9a59a7f01` |
| resource report | `086e4d0442d40a71eb743a625db3463cb7bbb424030add8637bcf01e1c24a7cc` |

Reproduce the six-artifact acceptance with:

```sh
cargo test -p earthmesh_cli --test certified_production_acceptance \
  real_igbp_nxp80_coupled_safe_mother_passes_every_hard_gate -- \
  --ignored --nocapture
```

This acceptance closes the safe-mother G4 gate, including exact validation of
the published primal/dual tables and one conservative identity row per Voronoi
cell. It is the production-scale safe-mother acceptance; mixed finite-cavity
delivery is covered separately by the nonuniform reverse-mode CLI regression.
