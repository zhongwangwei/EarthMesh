# Experimental CMRC local-quality updates

This is an **off-by-default experiment**, not a new default coarsening policy or
an automatic optimizer. It consumes a precomputed coordinate proposal and sends
the accepted mesh through the existing source, remap, certification and atomic
MPAS publication pipeline. Solver accuracy has not yet been demonstrated.

## Admission and use

Run the same certified namelist once without the environment variable to obtain
the control gridfile and MPAS mesh. A subsequent run can explicitly request:

```sh
EARTHMESH_CMRC_LOCAL_UPDATE=/absolute/path/proposal.json \
  earthmesh_cli /absolute/path/project.nml --max-tris 20000000
```

Use a separate output directory. The control and proposal must describe the same
mesh configuration. Only refined global `atmos`/`atmosmesh`, `hex`, MPAS output,
`reverse_coarsening`, `coupled` delivery, the `domain_quality_38_to_82_v1` angle
contract, and mixed requirements with maximum level 1 or 2 are admitted. Unset
the variable for ordinary behavior; there is no GUI toggle or optimizer
dependency.

The JSON request contains:

| Field | Meaning |
|---|---|
| `source_mpas` | Absolute path to the frozen control MPAS NetCDF |
| `source_gridfile` | Absolute path to its gridfile, including delivered levels |
| `target_cell` | Zero-based physical MPAS cell row, not an internal vertex slot |
| `updates` | 1–10 objects with physical `id` and unit-sphere `xyz: [x,y,z]` |
| `flips` | Optional empty array; topology changes are forbidden |
| `case` | Optional diagnostic string |

All control cell positions, face topology and physical refinement levels are
bound to the current construction before updates. Repeated/out-of-range IDs,
non-finite/non-unit coordinates, original pentagon anchor moves, mixed-level
interface moves, and displacements above 3% of each site's local spacing fail.
The request schema rejects unknown fields (a misspelled optional key such as
`"Flips"` is an `InvalidInput` error, not a silent no-op), matching the strict
project-YAML style.

The delivered manifest's `experimental_local_update.proposal_validation` embeds
the apply report, which records `provenance` for the accepted update: the
absolute request path and SHA-256 of the request bytes that were parsed, plus
each baseline's absolute path and the streaming SHA-256 of its file bytes
(`request_sha256`, `source_mpas_sha256`, `source_gridfile_sha256`,
`hash_algorithm: "sha256"`). The hashes bind the delivered coordinates to the
exact read inputs; they are not a re-hash after publication. The baseline files
are hashed and then opened by the NetCDF reader as a separate step, so a file
replaced inside that window is a residual TOCTOU caveat — freeze the inputs
until the run finishes when reproducer-grade provenance matters.

## Acceptance and failure semantics

- No cells or connectivity are added or removed. A clone is committed only
  after native geometry and quality checks pass.
- Global maximum dual-cell edge CV and target CV/aspect must improve by more
  than `1e-8`. Global dual aspect/adjacent resolution ratio and triangle/dual
  angle extrema must not worsen beyond `1e-9`.
- Counts of triangle angles and triangles outside the preferred 40–80 degree
  interval, and triangle-angle RMSE to 60 degrees, must not increase. The hard
  delivery angle contract remains 38–82 degrees.
- **There is no per-element angle non-degradation guarantee.** This policy
  deliberately tests a different tradeoff, not a stricter triangle optimizer.
- Candidate geometry/quality rejection retains the original control. Malformed
  requests and baseline mismatch fail, rather than silently becoming a quality
  fallback. **An accepted proposal is only adopted if it also passes the final
  source projection, the freshly computed conservative remap, the geometry
  certificate and the final delivery certification. If any of those fails the
  run keeps the fully certified control, records the reason
  (`experimental_local_update.decision = "control"` in the manifest plus one
  stderr line) and still exits 0 — exit 0 means the run completed, not that the
  proposal was delivered.** Only a callback error other than `InvalidData`
  aborts the run.
- Accepted coordinates precede the final source projection and freshly computed
  conservative remap. Old certificate metadata is not reused as new evidence.
- The control's own final gates (source projection, fresh remap, delivery check)
  are materialized lazily: they run only when the candidate is rejected and the
  control is retained. A successful candidate always carries its own freshly
  computed projection, conservative remap, geometry certificate and delivery
  evidence, so the delivered mesh is certified exactly as before; what changes
  is that the control is not separately re-certified past its geometry gate
  when it will not be delivered.
  The successful manifest records `experimental_local_update.decision` and
  `full_delivery_recertified`; its nested `geometry_only` report describes only
  the earlier proposal-validation stage.

## Measured delivery results (2026-09-10)

NXP64 global atmosphere cases, compared with their frozen CMRC controls, **not
v2**. CV is the maximum of the per-cell edge-length coefficients of variation,
not `cell_area_cv` or mean mesh quality.

| Case | Cells, unchanged | Control CV | Delivered CV | Decision |
|---|---:|---:|---:|---|
| equatorial | 63,472 | 0.484828 | 0.471029 | candidate |
| small | 47,238 | 0.497682 | 0.489630 | candidate |
| large | 97,805 | 0.498002 | 0.482430 | candidate |
| nested | 58,162 | 0.490705 | 0.490705 | control |
| single_level | 46,777 | 0.457888 | 0.455030 | candidate |
| dateline_highlat | 51,028 | 0.487827 | 0.487827 | control |

Four cases improved this worst-cell metric by 0.62–3.13%. Two failed the
improvement guard and retained byte-identical control payloads. Global angle
extrema did not worsen, but `large` still has a per-triangle maximum-angle
increase of **1.342934 degrees** and a minimum-angle decrease of 1.140982 degrees.
Do not describe this as every angle improving or as proven simulation benefit.

Evidence: 13 complete CLI deliveries, seven independent artifact audits, six
cases replaying six payload classes byte-for-byte (gridfile, MPAS, graph, remap,
certificate, ready), and three malformed/conflicting-input probes preserving all
eight files of an existing completed bundle. MPAS geometry/orientation and source
coverage passed. Raw legacy gridfile ring-order diagnostics are not the same as
published MPAS orientation and are not claimed to be all zero.

Candidate determinism required setting BLAS/OpenMP thread counts **before**
NumPy/SciPy import; six pairs of independent runs matched on the same numerical
runtime. This is not a cross-platform bitwise guarantee. Path/timing-bearing
manifest and resources files are excluded from payload identity claims.

The local evidence archive is
`.omx/artifacts/cmrc-local-delivery-20260909/` (not committed: about 1.5 GB), with
`verification.json`, `report.md`, raw logs, audits and SHA-256 fingerprints. The
frozen engine SHA-256 is
`e37db0783a9b7a0a5afaac1d2af17b2a6302ae6c2ed3f4d86e56932a75c5a1a2`.

## Default-promotion gate

Keep this option disabled while running matched control/candidate transport
tests with the same solver, initial field, flow and timestep. Assess error,
conservation, boundedness and timestep stability; geometric certificates alone
do not establish solver compatibility or physical accuracy. Any model adapter
must preserve geometry and explicitly handle this export's unit-sphere units.
