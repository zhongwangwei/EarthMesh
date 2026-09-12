# Independent threshold evaluation region

Project configuration separates three different spatial requests:

1. `refinement.threshold_region`: where statistical support centers are eligible
   for threshold evaluation. Selected supports retain their full sample footprint.
2. `refinement.specified_*`: hard, named refinement requirements.
3. `domain`: which mesh is delivered after construction and applicable masking.

These regions need not coincide. A threshold window is not a crop, does not
request refinement by itself, and does not constrain separately requested hard
regions, geometric coastline refinement or hydro demands. Gradation/transition
cells can extend outside the threshold window. The statistical support scale
is independent of both input raster sampling and CoLM delivery sampling.

The configuration-to-engine sequence is:

**data and thresholds + evaluation scope → resolution requirements → selected
algorithm → applicable domain/surface extraction → checks for actual topology
and cell shape → requested model delivery.**

An unmasked global Earth/atmosphere mesh is a closed sphere. Regional and
land/ocean-masked meshes may instead have boundaries, holes and multiple
components; those checks are not interchangeable with closed-sphere checks.
This feature does not change validation contracts or algorithm kernels.

## Project final admission

The default `--project` workflow now sends the **selected final gridfile** from
CMRC, canonical Method-C/HField, RedGreen or LEPP through the same final admission
entry point after AutoRefine and hydro, before explicit CoLM mesh delivery or
configured TRI/FVCOM delivery.
`final_quality/quality_summary.json` records this check separately from candidate
and hydro diagnostics, including a `final_mesh_admission` gate.

Admission uses physical M triangles for TRI and the stored W rings for HEX:
TRI cells require 3 edges; HEX cells require 5–7 edges, independent of backend.
It does not sort corners or repair individual rings. Native producers use both
winding conventions; one whole-mesh reversal can translate clockwise into the
quality library's counter-clockwise convention. The report records this choice;
mixed winding, misoriented shared edges and self-intersections remain detectable.
The source gridfile is unchanged.

Invalid connectivity, malformed polygons and the physical cell contract cannot
be waived with `Warn`. Unmasked global Earth/atmosphere additionally requires
Euler 2, no boundary edges and exactly one connected component. Regional and
surface-masked meshes may have manifold boundaries and multiple components.
Disconnected components and isolated complete cells remain visible as warnings,
not false connectivity failures; malformed boundary junctions still fail. Numerical
geometry gates still follow `Warn` / `Block` / `AutoRefine`; final admission does
not start another repair loop.

This unifies **final Project admission**, not all model-export lifecycles.
Low-level NML paths, internal CMRC certificates and legacy model artifacts are
unchanged. Some adapters still emit artifacts inside the engine pipeline; their
existence does not mean the Project passed final admission. Refined MPAS still
requires aligned cellwidth/global-parent context; RedGreen/LEPP do not yet persist
per-W nominal widths, so uniform widths are not substituted to pretend migration
is complete.

### FVCOM selected-final delivery

For `target.cell: Tri` + `target.model_format: Fvcom`, Project writes
`standard/FVCOM_<selected_gridfile_stem>.2dm` beside the selected gridfile, after
final admission. The backend and land/ocean/atmosphere intent do not select a
separate export algorithm. HEX/FVCOM retains the existing grid-only contract.

New ocean TRI mask-postproc/clean-ocean gridfiles embed the exact same-run
`obc_order` as the integer global attribute `earthmesh_fvcom_obc_order` (canonical
vertex IDs, with `1` as the sequence placeholder and segment separator). This
survives metadata rewrites, byte copies, and CMRC temporary-directory cleanup;
Project never searches neighboring directories for a possibly stale OBC sidecar.
An absent attribute differs from an explicitly present empty order. A mesh with
boundary edges and missing context is rejected before writing; a closed mesh
needs no OBC metadata. Older regional files must be regenerated through the
boundary-producing path rather than silently exported with an empty boundary.

The adapter validates boundary vertices and consecutive boundary edges, retains
NS segmentation, checks physical M-triangle/W-node counts, and atomically
publishes without replacing an old output on failure. `fvcom_mesh_input=` points
to the new artifact. `fvcom_boundary_status=` distinguishes `closed_mesh`,
`open_chains_preserved` and `no_open_chains_classified`. The last state keeps a
warning: a recorded empty/all-separator order does **not** prove that the model's
open-boundary conditions are complete. Existing synthetic CMRC ocean fixtures
produce this state; the exporter preserves it rather than inventing NS records.
A `.2dm` mesh and its available OBC are not bathymetry, forcing, or solver-run
certification.

## Configuration

Add this to an otherwise valid CMRC project (same region syntax as `domain`):

```yaml
refinement:
  enabled: true
  threshold_enabled: true
  max_passes: 2
  backend: Certified
  threshold_region: !Close
    path: input/evaluation_window.nml
    format: Nml
    boundary:
      mode: polyline
```

`!Bbox {w: 170, e: -170, s: -10, n: 10}`, `!Circle {lon: 110, lat: 20,
radius_km: 500}`, `!Shapefile {path: input/window.shp}` and `!Close` imports
(`Nml`, `Netcdf`, `LonLatText`, `PolygonShp`) are supported. Bboxes preserve
directed longitude spans, including antimeridian crossings. Polygon shapefile
imports retain the existing polygon-reader acceptance rules.

- Absent/null: existing threshold behavior is unchanged. A regional delivery
  domain is **not** implicitly copied into the threshold region. Existing
  adapter/domain constraints still apply; this field adds an independent mask.
- With either refinement or thresholds switched off: the configuration is
  retained but inactive. Stored geometry is still validated.
- Active: requires at least one enabled statistical criterion and an explicitly
  supported route. Unsupported combinations fail rather than broaden the window
  or interpret it as hard demand.
- Relative input paths are resolved against the Project file, including after
  GUI open/edit/save/run. GUI preserves this field; it currently has no separate
  visual editor. Set it in YAML/JSON.

| Route | Independent active threshold region |
|---|---|
| Certified / CMRC | Supported |
| Method-C canonical with enabled HField | Supported |
| Method-C canonical without HField | Rejected |
| Method-C LEPP-Delaunay with adaptive enabled | Supported |
| RedGreen with adaptive enabled | Supported |
| RedGreen / LEPP-Delaunay with adaptive disabled | Rejected |

For RedGreen and LEPP-Delaunay, an omitted `adaptive` recipe defaults to enabled;
`adaptive.enabled: false` explicitly disables the statistical demand consumer.
Canonical Method-C without HField still has no supported criteria-driven loop.
Land, atmosphere and ocean use the same field and criteria pipeline; no
case-specific region is hard-coded.

Both adaptive routes reuse the shared statistical-support mask. In raw NML,
when statistical criteria and an adaptive/HField consumer are active, calculated
regions with degree zero are evaluation windows, not additional hard demands.
Positive-degree calculated regions remain hard demands. Without statistical
sources or without either consumer, the legacy reader still interprets degree
zero as the maximum configured hard-refinement level, where that route is valid.
Callers that relied on the former adaptive double consumption must use a
positive degree or `specified_*` for an independent hard requirement.

## Staging and failure behavior

`earthmesh_cli --project` stages each active shape as an evaluation-only
mask with refinement degree **zero**, then sets `RL%mask_refine_cal_type` and
`RL%mask_refine_cal_fprefix`. Bbox/circle primitives also become files because
the calculated-region reader does not consume inline geometry. Raw
`ProjectConfig::lower().to_namelist()` is an intermediate representation for
these shapes; use the Project CLI to resolve and stage all inputs.

NML/NetCDF imports must be concrete files with `close_refine=0` / zero stored
refinement degree. Positive-degree files are rejected: use `specified_close`
for hard refinement. Files are read exactly, not expanded as caller-provided
prefixes, then copied byte-for-byte under a run-owned prefix, preserving their
coordinate precision and representation. Text/shapefile conversion uses the
existing NML writer (10 decimal places). The original inputs are never rewritten.
Missing/malformed imports fail and clean the failed run directory;
they never fall back to global evaluation.

Only `polyline` close boundaries are accepted here. Spherical Chaikin and
enclosing-cap transforms are rejected because the calculated reader has no
boundary-transform contract. Final delivery-domain transformations remain
independently configured and unchanged.
