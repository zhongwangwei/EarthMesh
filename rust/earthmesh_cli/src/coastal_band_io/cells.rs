use std::io;

/// Faithful port of `coastal_band.py::land_mask_from_elevation`: True where the CaMa
/// elevation value is finite and not the undef sentinel.
pub fn coastal_land_mask_from_elevation(elevation: &[Vec<f64>], undef: f64) -> Vec<Vec<bool>> {
    elevation
        .iter()
        .map(|row| row.iter().map(|&v| v.is_finite() && v != undef).collect())
        .collect()
}

/// Faithful port of `coastal_band.py::coastal_band_cells`: select cells within a
/// Chebyshev `radius_cells` of a land/ocean transition (a neighbor of the opposite
/// type). `include_land_side` / `include_ocean_side` straddle the coastline.
pub fn coastal_band_cells(
    land_mask: &[Vec<bool>],
    radius_cells: i64,
    include_land_side: bool,
    include_ocean_side: bool,
) -> io::Result<Vec<Vec<bool>>> {
    coastal_band_cells_with_x_wrap(
        land_mask,
        radius_cells,
        include_land_side,
        include_ocean_side,
        false,
    )
}

pub(super) fn coastal_band_cells_periodic_x(
    land_mask: &[Vec<bool>],
    radius_cells: i64,
    include_land_side: bool,
    include_ocean_side: bool,
) -> io::Result<Vec<Vec<bool>>> {
    coastal_band_cells_with_x_wrap(
        land_mask,
        radius_cells,
        include_land_side,
        include_ocean_side,
        true,
    )
}

fn coastal_band_cells_with_x_wrap(
    land_mask: &[Vec<bool>],
    radius_cells: i64,
    include_land_side: bool,
    include_ocean_side: bool,
    wrap_x: bool,
) -> io::Result<Vec<Vec<bool>>> {
    if radius_cells < 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "radius_cells must be at least 1",
        ));
    }
    let height = land_mask.len();
    if height == 0 || land_mask.iter().any(|r| r.len() != land_mask[0].len()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "land_mask must be a non-empty rectangular grid",
        ));
    }
    let width = land_mask[0].len();
    let r = radius_cells as usize;
    let mut band = vec![vec![false; width]; height];
    for y in 0..height {
        for x in 0..width {
            let is_land = land_mask[y][x];
            if is_land && !include_land_side {
                continue;
            }
            if !is_land && !include_ocean_side {
                continue;
            }
            let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(height));
            let mut found_opposite = false;
            'scan: for yy in y0..y1 {
                for xx in x_range(width, x, r, wrap_x) {
                    if xx == x && yy == y {
                        continue;
                    }
                    if land_mask[yy][xx] != is_land {
                        found_opposite = true;
                        break 'scan;
                    }
                }
            }
            band[y][x] = found_opposite;
        }
    }
    Ok(band)
}

fn x_range(width: usize, x: usize, radius: usize, wrap_x: bool) -> Vec<usize> {
    if wrap_x {
        return (-(radius as isize)..=(radius as isize))
            .map(|dx| (x as isize + dx).rem_euclid(width as isize) as usize)
            .collect();
    }
    let x0 = x.saturating_sub(radius);
    let x1 = (x + radius + 1).min(width);
    (x0..x1).collect()
}
