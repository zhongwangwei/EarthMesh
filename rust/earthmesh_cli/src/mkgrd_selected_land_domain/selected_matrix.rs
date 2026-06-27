use std::io;

use crate::{validate_area_judge_grid_payload, AreaJudgeGridPayload, SelectedLandDomainMatrix};

pub fn selected_land_domain_from_area_judge_grid_payload(
    payload: &AreaJudgeGridPayload,
) -> io::Result<SelectedLandDomainMatrix> {
    validate_area_judge_grid_payload(payload)?;
    let seaorland = payload.seaorland_select.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "restart-refine land postproc requires seaorland_select in IsInDmArea_grid.nc4",
        )
    })?;
    let nlons = payload
        .bounds
        .maxlon_source
        .checked_sub(payload.bounds.minlon_source)
        .map(|value| value + 1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "restart-refine land postproc restart payload has invalid longitude bounds",
            )
        })?;
    let nlats = payload
        .bounds
        .minlat_source
        .checked_sub(payload.bounds.maxlat_source)
        .map(|value| value + 1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "restart-refine land postproc restart payload has invalid latitude bounds",
            )
        })?;

    Ok(SelectedLandDomainMatrix {
        minlon_source: payload.bounds.minlon_source,
        maxlat_source: payload.bounds.maxlat_source,
        nlons,
        nlats,
        seaorland,
    })
}

/// Derive the minimal selected land-domain sea/land matrix from a full
/// Fortran-indexed source `seaorland` raster. This is used when land final
/// `mask_postproc` is driven from an owned data_preprocess source-state rather
/// than from a saved Area_judge restart selected-grid payload.
pub fn selected_land_domain_from_full_source_seaorland_fortran_order(
    matrix: &[Vec<i32>],
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<SelectedLandDomainMatrix> {
    if matrix.len() < nlons_source + 1 || matrix.iter().any(|row| row.len() < nlats_source + 1) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source-state selected matrix is smaller than source dimensions",
        ));
    }

    let mut minlon = nlons_source + 1;
    let mut maxlon = 0usize;
    let mut north_lat = nlats_source + 1;
    let mut south_lat = 0usize;
    for (lon, row) in matrix.iter().enumerate().take(nlons_source + 1).skip(1) {
        for (lat, &value) in row.iter().enumerate().take(nlats_source + 1).skip(1) {
            if value != 0 {
                minlon = minlon.min(lon);
                maxlon = maxlon.max(lon);
                north_lat = north_lat.min(lat);
                south_lat = south_lat.max(lat);
            }
        }
    }
    if maxlon == 0 {
        return Ok(SelectedLandDomainMatrix {
            minlon_source: 1,
            maxlat_source: 1,
            nlons: 1,
            nlats: 1,
            seaorland: vec![vec![0]],
        });
    }

    let nlons = maxlon - minlon + 1;
    let nlats = south_lat - north_lat + 1;
    let seaorland = (minlon..=maxlon)
        .map(|lon| {
            (north_lat..=south_lat)
                .map(|lat| matrix[lon][lat])
                .collect()
        })
        .collect();
    Ok(SelectedLandDomainMatrix {
        minlon_source: minlon,
        maxlat_source: north_lat,
        nlons,
        nlats,
        seaorland,
    })
}
