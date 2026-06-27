use std::io;

use earthmesh_core::GridMemory;

use crate::coordinates::{require_grid_coordinate_len, xyz_to_lonlat_degrees, CartesianPoint};

/// State-level port of `mkgrd.F90:grid_xyz2lonlat`.
///
/// The legacy routine allocates `GLONM/GLATM/GLONW/GLATW` for the full
/// one-based grid footprint and fills entries up to `nma` and `nwa`. The Rust
/// state keeps the same placeholder-inclusive layout using zero-based vectors.
pub fn grid_xyz2lonlat_state(grid: &mut GridMemory) -> io::Result<()> {
    require_grid_coordinate_len("xem", grid.xem.len(), grid.nma)?;
    require_grid_coordinate_len("yem", grid.yem.len(), grid.nma)?;
    require_grid_coordinate_len("zem", grid.zem.len(), grid.nma)?;
    require_grid_coordinate_len("xew", grid.xew.len(), grid.nwa)?;
    require_grid_coordinate_len("yew", grid.yew.len(), grid.nwa)?;
    require_grid_coordinate_len("zew", grid.zew.len(), grid.nwa)?;

    grid.allocate_grid_lonlatmw(grid.nma, grid.nva, grid.nwa);
    for im in 0..grid.nma {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xem[im]),
            f64::from(grid.yem[im]),
            f64::from(grid.zem[im]),
        ));
        grid.glonm[im] = lonlat.lon_degrees as f32;
        grid.glatm[im] = lonlat.lat_degrees as f32;
    }
    for iw in 0..grid.nwa {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xew[iw]),
            f64::from(grid.yew[iw]),
            f64::from(grid.zew[iw]),
        ));
        grid.glonw[iw] = lonlat.lon_degrees as f32;
        grid.glatw[iw] = lonlat.lat_degrees as f32;
    }
    Ok(())
}

/// State-level `grid_xyz2lonlat` for direct Fortran one-based arrays.
///
/// Index `0` is kept unused and records `1..=nma` / `1..=nwa` are filled.
pub fn grid_xyz2lonlat_fortran_indexed_state(grid: &mut GridMemory) -> io::Result<()> {
    require_grid_coordinate_len("xem", grid.xem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("yem", grid.yem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("zem", grid.zem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("xew", grid.xew.len(), grid.nwa + 1)?;
    require_grid_coordinate_len("yew", grid.yew.len(), grid.nwa + 1)?;
    require_grid_coordinate_len("zew", grid.zew.len(), grid.nwa + 1)?;

    grid.allocate_grid_lonlatmw(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for im in 1..=grid.nma {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xem[im]),
            f64::from(grid.yem[im]),
            f64::from(grid.zem[im]),
        ));
        grid.glonm[im] = lonlat.lon_degrees as f32;
        grid.glatm[im] = lonlat.lat_degrees as f32;
    }
    for iw in 1..=grid.nwa {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xew[iw]),
            f64::from(grid.yew[iw]),
            f64::from(grid.zew[iw]),
        ));
        grid.glonw[iw] = lonlat.lon_degrees as f32;
        grid.glatw[iw] = lonlat.lat_degrees as f32;
    }
    Ok(())
}

pub fn grid_cartesian_xy_to_lonlat_placeholders_fortran_indexed_state(
    grid: &mut GridMemory,
) -> io::Result<()> {
    require_grid_coordinate_len("xem", grid.xem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("yem", grid.yem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("xew", grid.xew.len(), grid.nwa + 1)?;
    require_grid_coordinate_len("yew", grid.yew.len(), grid.nwa + 1)?;

    grid.allocate_grid_lonlatmw(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for im in 1..=grid.nma {
        grid.glonm[im] = grid.xem[im];
        grid.glatm[im] = grid.yem[im];
    }
    for iw in 1..=grid.nwa {
        grid.glonw[iw] = grid.xew[iw];
        grid.glatw[iw] = grid.yew[iw];
    }
    Ok(())
}
