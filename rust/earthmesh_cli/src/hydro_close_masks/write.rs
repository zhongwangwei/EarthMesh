use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::{HydroCloseMaskNmlOptions, HydroCloseMaskNmlWriteReport, HydroCloseMaskSpec};

use super::read::read_hydro_close_mask_specs;

/// Write EarthMesh close-mask `.nml` files from hydro/coast GeoJSON polygons.
///
/// This is the Rust-native core for polygon and multipolygon exterior rings.
pub fn write_hydro_close_mask_nmls(
    input_geojson: impl AsRef<Path>,
    output_prefix: impl AsRef<Path>,
    options: HydroCloseMaskNmlOptions,
) -> io::Result<HydroCloseMaskNmlWriteReport> {
    let specs = read_hydro_close_mask_specs(input_geojson, options)?;
    write_hydro_close_mask_specs(output_prefix, &specs)
}

pub fn write_hydro_close_mask_specs(
    output_prefix: impl AsRef<Path>,
    specs: &[HydroCloseMaskSpec],
) -> io::Result<HydroCloseMaskNmlWriteReport> {
    let output_prefix = output_prefix.as_ref().to_path_buf();
    crate::ensure_parent_dir(&output_prefix)?;
    remove_stale_close_mask_nmls(&output_prefix)?;

    let mut counts_by_class_degree = BTreeMap::<(String, usize), usize>::new();
    let mut files = Vec::with_capacity(specs.len());
    for spec in specs {
        let count_key = (spec.river_class.clone(), spec.refine_degree);
        let count = counts_by_class_degree.entry(count_key).or_insert(0);
        *count += 1;
        let file_name = format!(
            "{}_{}_d{}_{:03}.nml",
            output_prefix
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("refine_spc_hydro"),
            spec.river_class,
            spec.refine_degree,
            *count
        );
        let path = output_prefix.with_file_name(file_name);
        fs::write(&path, hydro_close_mask_text(spec))?;
        files.push(path);
    }

    Ok(HydroCloseMaskNmlWriteReport {
        output_prefix,
        files,
        spec_count: specs.len(),
    })
}

fn remove_stale_close_mask_nmls(output_prefix: &Path) -> io::Result<()> {
    let Some(parent) = output_prefix.parent() else {
        return Ok(());
    };
    let Some(prefix_name) = output_prefix.file_name().and_then(|value| value.to_str()) else {
        return Ok(());
    };
    let stale_prefix = format!("{prefix_name}_");
    for entry in fs::read_dir(parent)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(&stale_prefix) && name.ends_with(".nml") {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn hydro_close_mask_text(spec: &HydroCloseMaskSpec) -> String {
    let mut text = format!(
        "close_num = {}\nclose_refine = {}\n",
        spec.coordinates.len(),
        spec.refine_degree
    );
    for (lon, lat) in &spec.coordinates {
        text.push_str(&format!("{lon:.8} {lat:.8}\n"));
    }
    text
}
