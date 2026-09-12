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
configured TRI/FVCOM or HEX/MPAS-family delivery.
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
Low-level NML paths retain their own dispatch; internal CMRC certificates and
legacy model artifacts remain separate from final admission. Some adapters still
emit artifacts inside the engine pipeline; their
existence does not mean the Project passed final admission. Refined MPAS still
requires aligned cellwidth/global-parent context; RedGreen/LEPP do not yet persist
per-W nominal widths, so uniform widths are not substituted to pretend migration
is complete.

### MPAS native width context

Native gridfiles can carry `earthmesh_w_cellwidth_km` (f64, `lbx_points`, units
`km`) together with `earthmesh_mpas_base_nxp`, `earthmesh_mpas_step`,
`earthmesh_mpas_density_reference_width_km` and `earthmesh_mpas_cellwidth_source`.
The vector follows every native W row, including placeholders. The reference
width is the producer-global physical minimum, not the minimum of a later crop;
removing the finest cells must not renormalize `meshDensity`.

Newly generated, unrefined base grids record their known uniform `7680/NXP` km
width with step 1; imported grids do not acquire invented widths. This W-row
metadata is retained for both TRI and HEX native files: the native file stores
both views, and the standalone MPAS adapter always consumes W polygons. It does
not override the Project TRI/MPAS grid-only contract.
CMRC records its existing delivered-W-level width formula for all native output
formats. Its legacy MPAS export consumes those same values. CMRC native W rings
are written in the certified primal's rotational face-fan order, rather than the
generic conversion's unordered incident-face list; this preserves the certified
corners and fixes inconsistent native shared-edge winding before admission. The explicit
`write_springjustment_global_gridfile` adapter saves the older Spring core's exact
widths, including transition interpolation, when that core supplies them. It does
not infer widths from Method-C levels or mutate the input gridfile. This library
adapter is not an automatic modern Project Method-C integration.

Regional, landtype, mask-postproc and clean-ocean metadata paths compact widths
with W-row identities, preserve the global reference/NXP/step/source, and copy
widths when W vertices are split. Width-only clean-ocean rewrites also retain OBC.
Absent context stays absent; partial headers, wrong dimensions/types/units or
invalid values fail rather than select a uniform fallback.

Modern Method-C/HField, RedGreen and LEPP do not yet produce this context: their
actual nominal-width provenance must be retained before they can supply it.
Regional full MPAS still needs global-parent metric/weight context; compacted
native widths alone are not sufficient. Existing low-level builders still
normalize by their input minimum, whereas the final adapter below explicitly
uses the preserved global reference.

### MPAS selected-final delivery

For `target.cell: Hex` and `Mpas`, `MpasOcean` or `MpasSimple`, Project runs the
shared native-context adapter **after** selected-file final admission, independent
of backend or intent. Authoritative outputs are
`standard/MPAS_<selected_gridfile_stem>/mesh.nc4` and (full/Ocean) `graph.info`;
stdout identifies them as `mpas_mesh_input` and `mpas_graph_info`.
TRI/MPAS remains explicitly grid-only, matching the capability registry.

Global delivery checks actual W polygons (5–7 sides), manifold/reciprocal
topology, no boundary, Euler 2 and one component. Regional delivery requires an
explicit closed global parent from the selected run's `raw_output`; CMRC retains
that native parent in the same artifact publication set and records its path as
`global_parent_gridfile`. No filename discovery or nearest-centre matching is used.

The shared verifier binds each final row by positive ancestry plus exact finite
coordinates (signed zero is equivalent), not by assuming ancestor IDs are unique.
It rejects ambiguous or repeated parent-row matches, changed optional levels,
cyclic corners or W widths/reference/NXP/step/source. Regional polygons separately
pass 5–7, manifold-boundary and component-aware Euler checks; legitimate islands
and isolated whole cells are allowed. The ordered subset preserves final W order,
parent metrics and density. Dropped stencil edges retain the existing zero-weight
sentinel behavior; this is not a physical boundary-condition prescription.

The connected regional producer is CMRC whole-land dual-cell selection, now
independent of model format. Other producers may use the same adapter only when
they supply its complete parent/geometry/width contract. Missing or malformed
context fails, and modern Method-C/HField, RedGreen and LEPP still do not acquire
invented widths. No new ocean-HEX selection algorithm is included. Retaining a
parent costs additional disk space. Internal/legacy MPAS artifacts alone are not
proof that Project final delivery passed.

Density is `(producer_global_reference_width / final_W_width)^4`, aligned to
physical rows with placeholders excluded. It is not renormalized to the cropped
minimum. All other full/Simple/Ocean builder formulas and format conventions are
retained, including the legacy integer `nominalMinDc` calculation; final full
export rejects a nonpositive result. MPAS uses unit-sphere metrics; MPAS-Ocean
uses the existing physical-radius writer. Simple remains an incomplete mesh
schema, not a solver-ready full mesh, and has no graph.

Outputs are staged and closed before publication. The existing CMRC artifact-set
rollback helper is shared: graph is published first and mesh last; ordinary
publication errors restore the previous files. This is not crash-atomic or
concurrent-reader transaction isolation. Changing to Simple removes a stale
graph in the same publication transaction. Solver execution, boundary forcing
and model-specific initial conditions are outside mesh export validation.

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
