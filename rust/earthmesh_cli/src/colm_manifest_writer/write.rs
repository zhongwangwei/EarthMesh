use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::json_escape_string;

/// Write the CoLM package delivery manifest tying together Rust-written
/// coupling, restart-template, and forcing-template handoff products.
pub fn write_colm_package_delivery_manifest(
    output_manifest: impl AsRef<Path>,
    case_name: &str,
    rows: usize,
    coupling_netcdf: impl AsRef<Path>,
    restart_template_netcdf: Option<&Path>,
    forcing_template_netcdf: Option<&Path>,
) -> io::Result<PathBuf> {
    let output_manifest = output_manifest.as_ref().to_path_buf();
    if let Some(parent) = output_manifest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = String::from("{\"kind\":\"earthmesh_colm_package_manifest\"");
    text.push_str(",\"case_name\":\"");
    text.push_str(&json_escape_string(case_name));
    text.push('"');
    text.push_str(",\"rows\":");
    text.push_str(&rows.to_string());
    text.push_str(",\"products\":{");
    text.push_str("\"coupling_netcdf\":\"");
    text.push_str(&json_escape_string(
        &coupling_netcdf.as_ref().display().to_string(),
    ));
    text.push('"');
    if let Some(path) = restart_template_netcdf {
        text.push_str(",\"restart_template_netcdf\":\"");
        text.push_str(&json_escape_string(&path.display().to_string()));
        text.push('"');
    }
    if let Some(path) = forcing_template_netcdf {
        text.push_str(",\"forcing_template_netcdf\":\"");
        text.push_str(&json_escape_string(&path.display().to_string()));
        text.push('"');
    }
    text.push_str("}}\n");
    fs::write(&output_manifest, text)?;
    Ok(output_manifest)
}
