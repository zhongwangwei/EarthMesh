use std::fs;
use std::io;
use std::path::Path;

use crate::*;

/// Single-entry MERIT-Hydro regional close-mask workflow (the "Greater Bay Area"
/// LOC recipe): select the MERIT-Hydro tiles overlapping `bbox`, read+classify
/// each into river/coast masks, write the GeoJSON layers, then emit the
/// EarthMesh close-mask `.nml` files (one set for rivers, one for coasts) that
/// drive specified (`mask_refine_spc_type='close'`) refinement. `stride`
/// subsamples the 90 m MERIT grid. Everything runs in pure Rust over the local
/// MERIT-Hydro tiles; `[需数据]`: needs the MERIT-Hydro tile directory.
pub fn write_merit_hydro_region_close_masks(
    merit_root: impl AsRef<Path>,
    bbox: MeritLonLatBbox,
    stride: usize,
    thresholds: MeritMaskThresholds,
    output_dir: impl AsRef<Path>,
    nml_options: HydroCloseMaskNmlOptions,
) -> io::Result<MeritHydroRegionWorkflowReport> {
    let output_dir = output_dir.as_ref().to_path_buf();
    fs::create_dir_all(&output_dir)?;
    let tiles = select_merit_hydro_tiles(&merit_root, bbox)?;
    if tiles.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "no MERIT-Hydro tiles under {} overlap the region",
                merit_root.as_ref().display()
            ),
        ));
    }
    let mut windows = Vec::new();
    for tile in &tiles {
        // A selected tile may still not overlap the exact bbox window; skip it.
        match read_merit_hydro_window(tile, bbox, stride) {
            Ok(window) => windows.push(window),
            Err(err) if err.kind() == io::ErrorKind::InvalidInput => continue,
            Err(err) => return Err(err),
        }
    }
    if windows.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no MERIT-Hydro window overlapped the region",
        ));
    }
    let geojson = write_merit_hydro_mask_geojson_layers(&windows, thresholds, &output_dir, true)?;
    let river_nml = write_hydro_close_mask_nmls(
        &geojson.river_geojson,
        output_dir.join("refine_spc_river"),
        nml_options.clone(),
    )?;
    let coast_nml = write_hydro_close_mask_nmls(
        &geojson.coast_geojson,
        output_dir.join("refine_spc_coast"),
        nml_options,
    )?;
    Ok(MeritHydroRegionWorkflowReport {
        tile_count: tiles.len(),
        window_count: windows.len(),
        geojson,
        river_nml,
        coast_nml,
    })
}
