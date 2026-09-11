use std::{
    f64::consts::PI,
    io,
    path::{Path, PathBuf},
};

use earthmesh_geometry::{Point, PreparedSphericalPolygon, SphericalPointLocation};
use serde::Serialize;

use crate::{
    grid_quality_inputs::tri_quality_cells_from_gridfile,
    grid_quality_pipeline::{
        quality_input_from_gridfile_hex_native, read_gridfile_cell_lineages,
        read_gridfile_mesh_points,
    },
    gridfile_m_row_layout, gridfile_w_row_layout, netcdf_to_io_error, GridfileCellKind,
    GridfileMeshPoints,
};

const MAX_PIXELS: usize = 268_435_456;

#[derive(Debug, Serialize)]
pub struct ColmMeshInputReport {
    pub output: PathBuf,
    pub nlon: usize,
    pub nlat: usize,
    pub cells: usize,
    pub assigned_pixels: usize,
    pub outside_pixels: usize,
    pub boundary_tie_pixels: usize,
    pub min_cell_pixels: usize,
    pub max_cell_pixels: usize,
}

struct RasterCell {
    canonical_id: i32,
    lineage: Option<i64>,
    polygon: PreparedSphericalPolygon,
    bounds: PixelBounds,
}

#[derive(Clone, Copy)]
struct PixelBounds {
    j0: usize,
    j1: usize,
    lon0: f64,
    lon1: f64,
    full_lon: bool,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

pub fn write_colm_mesh_from_gridfile(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    pixels_per_degree: usize,
) -> io::Result<ColmMeshInputReport> {
    write_colm_mesh_from_gridfile_with_kind(input, output, pixels_per_degree, GridfileCellKind::Hex)
}

pub fn write_colm_mesh_from_gridfile_with_kind(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    pixels_per_degree: usize,
    kind: GridfileCellKind,
) -> io::Result<ColmMeshInputReport> {
    if pixels_per_degree == 0 {
        return Err(invalid("pixels_per_degree must be positive"));
    }
    let global_nlon = pixels_per_degree
        .checked_mul(360)
        .ok_or_else(|| invalid("longitude pixel count overflows"))?;
    let global_nlat = pixels_per_degree
        .checked_mul(180)
        .ok_or_else(|| invalid("latitude pixel count overflows"))?;

    let input = input.as_ref();
    let output = output.as_ref();
    validate_output_path(input, output)?;

    let mesh = read_gridfile_mesh_points(input)?;
    match kind {
        GridfileCellKind::Hex => {
            // Reuse the existing native-hex validation path before rasterizing its W rings.
            let _ = quality_input_from_gridfile_hex_native(&mesh)?;
        }
        GridfileCellKind::Tri => {}
    }
    let lineages = read_gridfile_cell_lineages(input)?;
    let cells = raster_cells(&mesh, kind, &lineages, pixels_per_degree, global_nlat)?;
    if cells.is_empty() {
        return Err(invalid(format!(
            "gridfile contains no physical {} cells",
            colm_cell_label(kind)
        )));
    }
    let window = raster_window(&cells, global_nlon, pixels_per_degree)?;
    let total = window
        .nlon
        .checked_mul(window.nlat)
        .ok_or_else(|| invalid("CoLM mesh raster size overflows"))?;
    if total > MAX_PIXELS {
        return Err(invalid(format!(
            "CoLM mesh raster window has {total} pixels; maximum is {MAX_PIXELS}"
        )));
    }

    reject_interior_overlaps(&cells, global_nlon, pixels_per_degree)?;

    let lon_w = (0..window.nlon)
        .map(|i| -180.0 + (window.i0 + i) as f64 / pixels_per_degree as f64)
        .collect::<Vec<_>>();
    let lon_e = (0..window.nlon)
        .map(|i| -180.0 + (window.i0 + i + 1) as f64 / pixels_per_degree as f64)
        .collect::<Vec<_>>();
    let lat_n = (0..window.nlat)
        .map(|j| 90.0 - (window.j0 + j) as f64 / pixels_per_degree as f64)
        .collect::<Vec<_>>();
    let lat_s = (0..window.nlat)
        .map(|j| 90.0 - (window.j0 + j + 1) as f64 / pixels_per_degree as f64)
        .collect::<Vec<_>>();

    let mut pixel_counts = vec![0usize; cells.len()];
    let mut boundary_tie_pixels = 0;
    atomic_write(output, |path| {
        let mut file = crate::mask_postproc_writers::create_patchid_file(
            path, &lon_w, &lon_e, &lat_s, &lat_n,
        )?;
        // Latitude sweep keeps only active polygons and one output row in memory.
        let mut starts = (0..cells.len()).collect::<Vec<_>>();
        starts.sort_unstable_by_key(|&i| (cells[i].bounds.j0, i));
        let mut next = 0;
        let mut active = Vec::new();
        let mut owners = vec![usize::MAX; window.nlon];
        let mut inside = vec![false; window.nlon];
        let mut tied = vec![false; window.nlon];
        let mut row = vec![0_i32; window.nlon];
        for j in window.j0..=window.j1 {
            owners.fill(usize::MAX);
            inside.fill(false);
            tied.fill(false);
            row.fill(0);
            while next < starts.len() && cells[starts[next]].bounds.j0 <= j {
                active.push(starts[next]);
                next += 1;
            }
            active.retain(|&c| cells[c].bounds.j1 >= j);
            let lat = pixel_lat_center(j, pixels_per_degree);
            for &c in &active {
                let cell = &cells[c];
                for (i0, i1) in longitude_ranges(&cell.bounds, global_nlon, pixels_per_degree) {
                    let Some((i0, i1)) = intersect_range(i0, i1, window.i0, window.i1) else {
                        continue;
                    };
                    for i in i0..=i1 {
                        let lon = pixel_lon_center(i, pixels_per_degree);
                        let location = cell
                            .polygon
                            .point_location(Point::new(lon, lat))
                            .map_err(|error| invalid(error.to_string()))?;
                        if location == SphericalPointLocation::Outside {
                            continue;
                        }
                        let x = i - window.i0;
                        let new_inside = location == SphericalPointLocation::Inside;
                        if owners[x] == usize::MAX {
                            owners[x] = c;
                            inside[x] = new_inside;
                        } else if inside[x] && new_inside {
                            return Err(invalid(format!(
                                "gridfile native cells overlap in their interiors at lon={lon}, lat={lat}: {} and {}",
                                cells[owners[x]].canonical_id, cell.canonical_id
                            )));
                        } else if new_inside {
                            owners[x] = c;
                            inside[x] = true;
                            tied[x] = false;
                        } else if !inside[x] {
                            tied[x] = true;
                            if cell.canonical_id < cells[owners[x]].canonical_id {
                                owners[x] = c;
                            }
                        }
                    }
                }
            }
            for (x, &owner) in owners.iter().enumerate() {
                if owner != usize::MAX {
                    row[x] = cells[owner].canonical_id;
                    pixel_counts[owner] += 1;
                    boundary_tie_pixels += usize::from(tied[x]);
                }
            }
            file.variable_mut("elmindex")
                .expect("defined elmindex")
                .put_values(&row, (j - window.j0, ..))
                .map_err(netcdf_to_io_error)?;
        }
        let empty = cells
            .iter()
            .zip(&pixel_counts)
            .filter_map(|(cell, &count)| (count == 0).then_some(cell.canonical_id))
            .collect::<Vec<_>>();
        if !empty.is_empty() {
            return Err(invalid(format!(
                "{} CoLM mesh cells receive no raster pixels at {pixels_per_degree} px/degree; increase --pixels-per-degree (first IDs: {:?})",
                empty.len(), &empty[..empty.len().min(8)]
            )));
        }
        write_metadata(&mut file, kind, &cells, &pixel_counts)?;
        file.close().map_err(netcdf_to_io_error)?;
        Ok(())
    })?;
    let assigned_pixels = pixel_counts.iter().sum();
    Ok(ColmMeshInputReport {
        output: output.to_path_buf(),
        nlon: window.nlon,
        nlat: window.nlat,
        cells: cells.len(),
        assigned_pixels,
        outside_pixels: total - assigned_pixels,
        boundary_tie_pixels,
        min_cell_pixels: *pixel_counts.iter().min().unwrap_or(&0),
        max_cell_pixels: *pixel_counts.iter().max().unwrap_or(&0),
    })
}

#[derive(Clone, Copy)]
struct RasterWindow {
    i0: usize,
    i1: usize,
    j0: usize,
    j1: usize,
    nlon: usize,
    nlat: usize,
}

fn raster_window(cells: &[RasterCell], global_nlon: usize, ppd: usize) -> io::Result<RasterWindow> {
    let j0 = cells.iter().map(|cell| cell.bounds.j0).min().unwrap_or(0);
    let j1 = cells.iter().map(|cell| cell.bounds.j1).max().unwrap_or(0);
    let full_lon = cells
        .iter()
        .any(|cell| cell.bounds.full_lon || cell.bounds.lon0 > cell.bounds.lon1);
    let (i0, i1) = if full_lon {
        (0, global_nlon - 1)
    } else {
        let mut lo = global_nlon - 1;
        let mut hi = 0usize;
        for cell in cells {
            for (a, b) in longitude_ranges(&cell.bounds, global_nlon, ppd) {
                lo = lo.min(a);
                hi = hi.max(b);
            }
        }
        (lo, hi)
    };
    if i0 > i1 || j0 > j1 {
        return Err(invalid("CoLM mesh raster window is empty"));
    }
    Ok(RasterWindow {
        i0,
        i1,
        j0,
        j1,
        nlon: i1 - i0 + 1,
        nlat: j1 - j0 + 1,
    })
}

fn intersect_range(a0: usize, a1: usize, b0: usize, b1: usize) -> Option<(usize, usize)> {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    (lo <= hi).then_some((lo, hi))
}

// Sweep cap latitude intervals, then reuse prepared spherical intersections.
// Raster sampling alone would miss subpixel overlap slivers.
fn reject_interior_overlaps(cells: &[RasterCell], nlon: usize, ppd: usize) -> io::Result<()> {
    let ranges = cells
        .iter()
        .map(|c| longitude_ranges(&c.bounds, nlon, ppd))
        .collect::<Vec<_>>();
    let mut starts = (0..cells.len()).collect::<Vec<_>>();
    starts.sort_unstable_by_key(|&i| (cells[i].bounds.j0, i));
    let mut active: Vec<usize> = Vec::new();
    for a in starts {
        active.retain(|&b| cells[b].bounds.j1 >= cells[a].bounds.j0);
        for &b in &active {
            if !ranges[a].iter().any(|&(a0, a1)| {
                ranges[b]
                    .iter()
                    .any(|&(b0, b1)| intersect_range(a0, a1, b0, b1).is_some())
            }) {
                continue;
            }
            let fraction = cells[a]
                .polygon
                .overlap_fraction(&cells[b].polygon)
                .map_err(|error| invalid(format!("W cell overlap check: {error}")))?;
            // Relative area tolerance absorbs roundoff at coincident shared edges.
            if !fraction.is_finite() || fraction > 1.0e-9 {
                return Err(invalid(format!(
                    "gridfile native cells {} and {} have interior overlap (fraction={fraction})",
                    cells[a].canonical_id, cells[b].canonical_id
                )));
            }
        }
        active.push(a);
    }
    Ok(())
}

fn raster_cells(
    mesh: &GridfileMeshPoints,
    kind: GridfileCellKind,
    lineages: &crate::MethodCGridfileLineages,
    pixels_per_degree: usize,
    nlat: usize,
) -> io::Result<Vec<RasterCell>> {
    match kind {
        GridfileCellKind::Hex => raster_w_cells(mesh, &lineages.w, pixels_per_degree, nlat),
        GridfileCellKind::Tri => {
            raster_m_triangle_cells(mesh, &lineages.m, pixels_per_degree, nlat)
        }
    }
}

fn raster_w_cells(
    mesh: &GridfileMeshPoints,
    lineages: &[i64],
    pixels_per_degree: usize,
    nlat: usize,
) -> io::Result<Vec<RasterCell>> {
    if mesh.w_to_m_width == 0 || mesh.w_to_m.is_empty() || mesh.n_w.is_empty() {
        return Err(invalid(
            "CoLM hex mesh export requires itab_w%im and n_ngrwm",
        ));
    }
    let m_layout = gridfile_m_row_layout(mesh);
    let w_layout = gridfile_w_row_layout(mesh);
    let mut cells = Vec::new();
    for row in w_layout.first_physical_row..mesh.w_lon.len() {
        let Some(canonical_id) = w_layout.canonical_id_for_physical_row(row) else {
            continue;
        };
        let count = usize::try_from(*mesh.n_w.get(row).unwrap_or(&0))
            .map_err(|_| invalid(format!("W row {row} has negative n_ngrwm")))?;
        if count < 3 || count > mesh.w_to_m_width {
            return Err(invalid(format!(
                "W row {row} has invalid native ring vertex count {count}"
            )));
        }
        let start = row
            .checked_mul(mesh.w_to_m_width)
            .ok_or_else(|| invalid("W connectivity offset overflows"))?;
        let ring = mesh.w_to_m[start..start + count]
            .iter()
            .map(|&id| {
                let m_row = m_layout
                    .physical_row_for_canonical_id(id, mesh.m_lon.len())
                    .ok_or_else(|| {
                        invalid(format!("W row {row} references invalid M vertex id {id}"))
                    })?;
                Ok(Point::new(mesh.m_lon[m_row], mesh.m_lat[m_row]))
            })
            .collect::<io::Result<Vec<_>>>()?;
        cells.push(raster_cell_from_ring(
            "W",
            canonical_id,
            lineages.get(row).copied(),
            ring,
            pixels_per_degree,
            nlat,
        )?);
    }
    Ok(cells)
}

fn raster_m_triangle_cells(
    mesh: &GridfileMeshPoints,
    lineages: &[i64],
    pixels_per_degree: usize,
    nlat: usize,
) -> io::Result<Vec<RasterCell>> {
    let m_layout = gridfile_m_row_layout(mesh);
    let cells = tri_quality_cells_from_gridfile(mesh)?;
    let mut raster = Vec::new();
    for (row, vertices) in cells {
        let Some(canonical_id) = m_layout.canonical_id_for_physical_row(row) else {
            continue;
        };
        let ring = vertices
            .iter()
            .map(|&w_row| Point::new(mesh.w_lon[w_row], mesh.w_lat[w_row]))
            .collect::<Vec<_>>();
        raster.push(raster_cell_from_ring(
            "M",
            canonical_id,
            lineages.get(row).copied(),
            ring,
            pixels_per_degree,
            nlat,
        )?);
    }
    Ok(raster)
}

fn raster_cell_from_ring(
    label: &str,
    canonical_id: i32,
    lineage: Option<i64>,
    ring: Vec<Point>,
    pixels_per_degree: usize,
    nlat: usize,
) -> io::Result<RasterCell> {
    let polygon = PreparedSphericalPolygon::new(&ring)
        .map_err(|error| invalid(format!("invalid {label} cell {canonical_id}: {error}")))?;
    polygon
        .point_location(ring[0])
        .map_err(|error| invalid(format!("invalid {label} cell {canonical_id}: {error}")))?;
    let bounds = cap_bounds(&ring, pixels_per_degree, nlat)
        .map_err(|message| invalid(format!("{label} cell {canonical_id}: {message}")))?;
    Ok(RasterCell {
        canonical_id,
        lineage,
        polygon,
        bounds,
    })
}

fn cap_bounds(
    ring: &[Point],
    pixels_per_degree: usize,
    nlat: usize,
) -> Result<PixelBounds, String> {
    let units = ring.iter().map(|p| unit(p.x, p.y)).collect::<Vec<_>>();
    let sum = units.iter().fold([0.0; 3], |mut acc, point| {
        acc[0] += point[0];
        acc[1] += point[1];
        acc[2] += point[2];
        acc
    });
    let center = normalize(sum).ok_or_else(|| "cell cap center is degenerate".to_string())?;
    let radius = units
        .iter()
        .map(|point| dot(center, *point).clamp(-1.0, 1.0).acos())
        .fold(0.0_f64, f64::max);
    if radius >= PI / 2.0 {
        return Err("spherical cap bound is at least a hemisphere".to_string());
    }
    let center_lat = center[2].asin();
    let lat_min = (center_lat - radius).max(-PI / 2.0).to_degrees();
    let lat_max = (center_lat + radius).min(PI / 2.0).to_degrees();
    let j0 = latitude_index_for_north_edge(lat_max, pixels_per_degree).min(nlat - 1);
    let j1 = latitude_index_for_south_edge(lat_min, pixels_per_degree).min(nlat - 1);
    if j0 > j1 {
        return Err("cell cap has no candidate latitude pixels".to_string());
    }
    let center_lon = center[1].atan2(center[0]).to_degrees();
    let reaches_pole = center_lat.abs() + radius >= PI / 2.0 - 1.0e-15;
    let cos_lat = center_lat.cos().abs();
    let full_lon = reaches_pole || cos_lat <= 1.0e-15 || radius.sin() >= cos_lat;
    let delta_lon = if full_lon {
        180.0
    } else {
        (radius.sin() / cos_lat).asin().to_degrees()
    };
    Ok(PixelBounds {
        j0,
        j1,
        lon0: wrap_lon(center_lon - delta_lon),
        lon1: wrap_lon(center_lon + delta_lon),
        full_lon,
    })
}

fn longitude_ranges(bounds: &PixelBounds, nlon: usize, ppd: usize) -> Vec<(usize, usize)> {
    if bounds.full_lon {
        return vec![(0, nlon - 1)];
    }
    let start = lon_floor_index(bounds.lon0, ppd, nlon);
    let end = lon_floor_index(bounds.lon1, ppd, nlon);
    if bounds.lon0 <= bounds.lon1 {
        vec![(start, end)]
    } else {
        vec![(0, end), (start, nlon - 1)]
    }
}

fn pixel_lon_center(i: usize, ppd: usize) -> f64 {
    -180.0 + (i as f64 + 0.5) / ppd as f64
}
fn pixel_lat_center(j: usize, ppd: usize) -> f64 {
    90.0 - (j as f64 + 0.5) / ppd as f64
}

fn latitude_index_for_north_edge(lat: f64, ppd: usize) -> usize {
    ((90.0 - lat) * ppd as f64).floor().max(0.0) as usize
}

fn latitude_index_for_south_edge(lat: f64, ppd: usize) -> usize {
    ((90.0 - lat) * ppd as f64).ceil().max(1.0) as usize - 1
}

fn lon_floor_index(lon: f64, ppd: usize, nlon: usize) -> usize {
    (((wrap_lon(lon) + 180.0) * ppd as f64).floor() as usize).min(nlon - 1)
}

fn wrap_lon(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

fn unit(lon: f64, lat: f64) -> [f64; 3] {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot(v, v).sqrt();
    (n > 0.0 && n.is_finite()).then(|| [v[0] / n, v[1] / n, v[2] / n])
}

fn colm_semantics_attribute(kind: GridfileCellKind) -> &'static str {
    match kind {
        GridfileCellKind::Hex => {
            "pixel_center_rasterized_native_w_cell_ids; outside=0; boundary_tie=smallest_id"
        }
        GridfileCellKind::Tri => {
            "pixel_center_rasterized_native_m_cell_ids; outside=0; boundary_tie=smallest_id"
        }
    }
}

fn colm_cell_label(kind: GridfileCellKind) -> &'static str {
    match kind {
        GridfileCellKind::Hex => "W",
        GridfileCellKind::Tri => "M",
    }
}

fn write_metadata(
    file: &mut netcdf::FileMut,
    kind: GridfileCellKind,
    cells: &[RasterCell],
    pixel_counts: &[usize],
) -> io::Result<()> {
    file.add_attribute("earthmesh_semantics", colm_semantics_attribute(kind))
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("cell", cells.len())
        .map_err(netcdf_to_io_error)?;
    let ids = cells
        .iter()
        .map(|cell| cell.canonical_id)
        .collect::<Vec<_>>();
    file.add_variable::<i32>("cell_id", &["cell"])
        .map_err(netcdf_to_io_error)?
        .put_values(&ids, ..)
        .map_err(netcdf_to_io_error)?;
    let counts = pixel_counts.iter().map(|&n| n as i64).collect::<Vec<_>>();
    file.add_variable::<i64>("pixel_count", &["cell"])
        .map_err(netcdf_to_io_error)?
        .put_values(&counts, ..)
        .map_err(netcdf_to_io_error)?;
    if cells.iter().any(|cell| cell.lineage.is_some()) {
        let lineage = cells
            .iter()
            .map(|cell| {
                cell.lineage
                    .ok_or_else(|| invalid("incomplete source lineage"))
            })
            .collect::<io::Result<Vec<_>>>()?;
        file.add_variable::<i64>("source_lineage", &["cell"])
            .map_err(netcdf_to_io_error)?
            .put_values(&lineage, ..)
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

fn validate_output_path(input: &Path, output: &Path) -> io::Result<()> {
    crate::ensure_parent_dir(output)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_meta = std::fs::symlink_metadata(parent)?;
    if !parent_meta.is_dir() || parent_meta.file_type().is_symlink() {
        return Err(invalid("output parent must be a real directory"));
    }
    if let Ok(meta) = std::fs::symlink_metadata(output) {
        if meta.file_type().is_symlink() {
            return Err(invalid("output path must not be a symlink"));
        }
        if meta.is_dir() {
            return Err(invalid("output path must not be a directory"));
        }
    }
    if output.exists() && std::fs::canonicalize(input)? == std::fs::canonicalize(output)? {
        return Err(invalid("input and output must not be the same file"));
    }
    reject_hardlink_alias(input, output)?;
    Ok(())
}

fn reject_hardlink_alias(input: &Path, output: &Path) -> io::Result<()> {
    if !output.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let input = std::fs::metadata(input)?;
        let output = std::fs::metadata(output)?;
        if input.dev() == output.dev() && input.ino() == output.ino() {
            return Err(invalid("input and output must not be filesystem aliases"));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (input, output);
    }
    Ok(())
}

fn atomic_write(output: &Path, write: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("colm_mesh.nc");
    for attempt in 0..128u32 {
        let tmp = parent.join(format!(".{stem}.tmp-{}-{attempt}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => drop(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
        let result = write(&tmp).and_then(|_| std::fs::rename(&tmp, output));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        return result;
    }
    Err(invalid("could not create exclusive temporary output"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_bounds_handles_dateline_wrap() {
        let ring = vec![
            Point::new(179.5, -0.5),
            Point::new(-179.5, -0.5),
            Point::new(-179.5, 0.5),
            Point::new(179.5, 0.5),
        ];
        let bounds = cap_bounds(&ring, 2, 360).unwrap();
        assert!(bounds.lon0 > bounds.lon1 || bounds.full_lon);
        assert!(bounds.j0 <= bounds.j1);
    }

    #[test]
    fn atomic_write_preserves_old_output_on_failure() {
        let path =
            std::env::temp_dir().join(format!("earthmesh_colm_atomic_{}.nc", std::process::id()));
        std::fs::write(&path, b"old").unwrap();
        let error = atomic_write(&path, |_tmp| Err(invalid("boom"))).unwrap_err();
        assert!(error.to_string().contains("boom"));
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
        let _ = std::fs::remove_file(path);
    }
}
