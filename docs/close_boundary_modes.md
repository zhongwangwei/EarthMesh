# Close boundary modes

EarthMesh keeps the existing close polygon behavior by default. Project files
can opt into one of two spherical preprocessing modes; EarthMesh applies the
selected mode once, after reading the close mask and before domain clipping,
specified refinement, or h-field construction.

Studio accepts `.shp`, `.nml`, `.nc`, `.nc4`, `.txt`, and `.csv` close sources.
SHP and lon/lat text sources are staged as temporary NML masks before the
engine applies the selected boundary mode.

For SHP input, EarthMesh reads the adjacent `.prj` file and converts WKT1/WKT2
projected or geographic coordinates to WGS84 through `proj4wkt`/`proj4rs`.
Native WGS84, Web Mercator, and WGS84 UTM retain their direct fast paths.
Multipart polygons are classified by containment; holes are retained with an
even/odd bridge and nested islands remain independent polygon parts. A SHP
without `.prj` is accepted only when every coordinate is already plausible
longitude/latitude.

## Default: original polyline

```yaml
domain: !Regional
  shape: !Close
    path: ./masks/domain_close.nml
    format: Nml
    boundary:
      mode: polyline
  sea_ratio: 0.5
```

Omitting `boundary` is identical to `mode: polyline`. Points and existing
membership behavior are preserved.

## Spherical Chaikin smoothing

```yaml
boundary:
  mode: spherical_chaikin
  iterations: 2
  max_segment_angle_deg: 0.25
```

EarthMesh performs Chaikin corner cutting with spherical linear interpolation
(SLERP), then densifies the result so no output edge exceeds
`max_segment_angle_deg`. This avoids interpolation discontinuities at the
antimeridian, but it cuts corners and usually shrinks a convex footprint.

## Conservative enclosing cap

```yaml
boundary:
  mode: enclosing_cap
  margin_km: 20.0
  max_radius_deg: 80.0
  max_segment_angle_deg: 0.25
```

EarthMesh samples the input lon/lat segments, estimates a spherical mean
direction, and chooses the farthest sampled boundary distance plus a
conservative sampling-gap bound and `margin_km`. The result reuses the existing
circle region path. This is a deterministic over-covering cap, not a claim of
the mathematically smallest spherical cap.

## Specified close refinement

The same `boundary` block is available under `specified_close`:

```yaml
refinement:
  enabled: true
  max_passes: 2
  specified_close:
    path: ./masks/refine_close.nml
    boundary:
      mode: enclosing_cap
      margin_km: 20.0
      max_radius_deg: 80.0
      max_segment_angle_deg: 0.25
```

## Safety limits

Non-default modes reject:

- fewer than three unique points, duplicate consecutive points, zero-area or
  self-intersecting rings;
- non-finite coordinates, invalid latitudes, and antipodal/near-antipodal
  edges;
- rings that do not fit in one open hemisphere;
- caps above the configured `max_radius_deg` (which must be below 90°);
- more than 20,000 input or generated boundary points.

Each transformed source writes a concise runtime report with input/output point
counts, spherical areas, area delta, and either maximum corner displacement or
cap radius. Invalid transformations fail explicitly; EarthMesh does not
silently fall back to the original polygon.
