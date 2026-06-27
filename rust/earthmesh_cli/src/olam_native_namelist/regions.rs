use std::collections::BTreeMap;
use std::io;

use earthmesh_mesh::{LonLatDegrees, OlamRefinementRegion};

use crate::olam_native_parser::{olam_namelist_assignments, olam_native_index};

use super::query::olam_native_atmosphere_grid_count_spawns;
use super::scalars::read_olam_native_mdomain;
use super::validation::{max_grid_points, max_grids};
use super::{
    parse_olam_native_f64, parse_olam_native_usize, validate_olam_native_assignment_grid_index,
    validate_olam_native_assignment_grid_point_index, validate_olam_native_lat_lon_radius,
    validate_olam_native_optional_usize_bounds,
};

pub(crate) fn read_olam_native_refinement_regions(
    contents: &str,
    is_atmosmesh: bool,
    validate_geographic_bounds: bool,
) -> io::Result<Vec<OlamRefinementRegion>> {
    validate_olam_native_optional_usize_bounds(contents, "gridplot_base", 2, max_grids())?;
    validate_olam_native_optional_usize_bounds(contents, "sfcgridplot_base", 1, max_grids())?;
    if is_atmosmesh {
        return read_olam_native_refinement_regions_for_grid(
            contents,
            true,
            validate_geographic_bounds,
        );
    }
    let mut regions =
        read_olam_native_refinement_regions_for_grid(contents, true, validate_geographic_bounds)?;
    regions.extend(read_olam_native_refinement_regions_for_grid(
        contents,
        false,
        validate_geographic_bounds,
    )?);
    Ok(regions)
}

pub(crate) fn read_olam_native_refinement_regions_for_grid(
    contents: &str,
    is_atmosgrid: bool,
    validate_geographic_bounds: bool,
) -> io::Result<Vec<OlamRefinementRegion>> {
    let grid_count_field = if is_atmosgrid { "ngrids" } else { "nsfcgrids" };
    let point_count_field = if is_atmosgrid { "ngrdll" } else { "nsfcgrdll" };
    let radius_field = if is_atmosgrid { "grdrad" } else { "sfcgrdrad" };
    let lat_field = if is_atmosgrid { "grdlat" } else { "sfcgrdlat" };
    let lon_field = if is_atmosgrid { "grdlon" } else { "sfcgrdlon" };

    let mut grid_count = None;
    let mut point_counts = BTreeMap::<usize, usize>::new();
    let mut radii = BTreeMap::<(usize, usize), f64>::new();
    let mut lats = BTreeMap::<(usize, usize), f64>::new();
    let mut lons = BTreeMap::<(usize, usize), f64>::new();

    for assignment in olam_namelist_assignments(contents, "mkgrd")? {
        match assignment.field.as_str() {
            field if field == grid_count_field => {
                grid_count = Some(parse_olam_native_usize(
                    &assignment.field,
                    &assignment.value,
                )?);
            }
            field if field == point_count_field => {
                let grid_index = olam_native_index(&assignment, 0)?;
                validate_olam_native_assignment_grid_index(
                    point_count_field,
                    grid_index,
                    max_grids(),
                )?;
                point_counts.insert(
                    grid_index,
                    parse_olam_native_usize(&assignment.field, &assignment.value)?,
                );
            }
            field if field == radius_field => {
                let grid_index = olam_native_index(&assignment, 0)?;
                let point_index = olam_native_index(&assignment, 1)?;
                validate_olam_native_assignment_grid_point_index(
                    radius_field,
                    grid_index,
                    point_index,
                    max_grids(),
                    max_grid_points(),
                )?;
                radii.insert(
                    (grid_index, point_index),
                    parse_olam_native_f64(&assignment.field, &assignment.value)?,
                );
            }
            field if field == lat_field => {
                let grid_index = olam_native_index(&assignment, 0)?;
                let point_index = olam_native_index(&assignment, 1)?;
                validate_olam_native_assignment_grid_point_index(
                    lat_field,
                    grid_index,
                    point_index,
                    max_grids(),
                    max_grid_points(),
                )?;
                lats.insert(
                    (grid_index, point_index),
                    parse_olam_native_f64(&assignment.field, &assignment.value)?,
                );
            }
            field if field == lon_field => {
                let grid_index = olam_native_index(&assignment, 0)?;
                let point_index = olam_native_index(&assignment, 1)?;
                validate_olam_native_assignment_grid_point_index(
                    lon_field,
                    grid_index,
                    point_index,
                    max_grids(),
                    max_grid_points(),
                )?;
                lons.insert(
                    (grid_index, point_index),
                    parse_olam_native_f64(&assignment.field, &assignment.value)?,
                );
            }
            _ => {}
        }
    }

    let Some(grid_count) = grid_count else {
        return Ok(Vec::new());
    };
    if is_atmosgrid && grid_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native OLAM ngrids must be at least 1",
        ));
    }
    if grid_count > max_grids() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native OLAM {grid_count_field} must be no greater than {}, got {grid_count}",
                max_grids()
            ),
        ));
    }
    if is_atmosgrid
        && !olam_native_atmosphere_grid_count_spawns(
            read_olam_native_mdomain(contents)?,
            grid_count,
        )
    {
        return Ok(Vec::new());
    }
    let first_grid = if is_atmosgrid { 2 } else { 1 };
    if grid_count < first_grid {
        return Ok(Vec::new());
    }

    let mut regions = Vec::new();
    for grid_index in first_grid..=grid_count {
        let point_count = *point_counts.get(&grid_index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("native OLAM {point_count_field}({grid_index}) is required"),
            )
        })?;
        if point_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("native OLAM {point_count_field}({grid_index}) must be positive"),
            ));
        }
        if point_count > max_grid_points() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "native OLAM {point_count_field}({grid_index}) must be no greater than {}, got {point_count}",
                    max_grid_points()
                ),
            ));
        }
        let level = if is_atmosgrid {
            grid_index - 1
        } else {
            grid_index
        };
        let mut points = Vec::with_capacity(point_count);
        let mut radius_meters = Vec::with_capacity(point_count);
        for point_index in 1..=point_count {
            let key = (grid_index, point_index);
            let lat = *lats.get(&key).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("native OLAM {lat_field}({grid_index},{point_index}) is required"),
                )
            })?;
            let lon = *lons.get(&key).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("native OLAM {lon_field}({grid_index},{point_index}) is required"),
                )
            })?;
            let radius = *radii.get(&key).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("native OLAM {radius_field}({grid_index},{point_index}) is required"),
                )
            })?;
            if validate_geographic_bounds {
                validate_olam_native_lat_lon_radius(
                    lat_field,
                    lon_field,
                    radius_field,
                    grid_index,
                    point_index,
                    lat,
                    lon,
                    radius,
                )?;
            }
            points.push(LonLatDegrees::new(lon, lat));
            radius_meters.push(radius);
        }
        if points.len() == 1 {
            regions.push(OlamRefinementRegion::Circle {
                center: points[0],
                radius_meters: radius_meters[0],
                level,
            });
        } else {
            regions.push(OlamRefinementRegion::Corridor {
                points,
                radius_meters,
                level,
            });
        }
    }
    Ok(regions)
}
