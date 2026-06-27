//! Mesh quality and map-overlay command handlers.

use std::{fs, process::Command};

use crate::engine::resolve_mkgrd;
use crate::mesh_paths::gridfile_dir;
use crate::quality::{parse_quality_summary, MeshQuality};

/// Run `mkgrd.x --mesh-quality <gridfile> <dir>` and parse the resulting
/// `quality_summary.json` for the Quality dashboard.
#[tauri::command]
pub(crate) fn mesh_quality(gridfile: String, kind: Option<String>) -> Result<MeshQuality, String> {
    let dir = gridfile_dir(&gridfile)?;
    // Measure hexagon cells for hex/atmos (MPAS) meshes, triangles for FVCOM —
    // matching the cell view the map renders, so the reported angles are the real
    // cell angles (≈120° for hexagons), not the dual triangles (≈60°).
    let kind = if kind.as_deref() == Some("tri") {
        "tri"
    } else {
        "hex"
    };
    let bin = resolve_mkgrd();
    let out = Command::new(&bin)
        .arg("--mesh-quality")
        .arg(&gridfile)
        .arg(&dir)
        .arg("--kind")
        .arg(kind)
        .output()
        .map_err(|e| format!("run --mesh-quality ({bin}): {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "mesh-quality failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let json_path = dir.join("quality_summary.json");
    let text =
        fs::read_to_string(&json_path).map_err(|e| format!("read {}: {e}", json_path.display()))?;
    parse_quality_summary(&text, &dir)
}

/// Run `mkgrd.x --gridfile-cell-polygons <gridfile> <out.geojson> --kind <hex|tri>`
/// and return the GeoJSON text for the frontend to overlay on the map.
#[tauri::command]
pub(crate) fn mesh_cell_polygons(
    gridfile: String,
    kind: String,
    max_cells: Option<u32>,
) -> Result<String, String> {
    let dir = gridfile_dir(&gridfile)?;
    let out_geojson = dir.join("mesh_cells.geojson");
    let kind = if kind == "tri" { "tri" } else { "hex" };
    let bin = resolve_mkgrd();
    let mut cmd = Command::new(&bin);
    cmd.arg("--gridfile-cell-polygons")
        .arg(&gridfile)
        .arg(&out_geojson)
        .arg("--kind")
        .arg(kind);
    if let Some(mc) = max_cells {
        cmd.arg("--max-cells").arg(mc.to_string());
    }
    let res = cmd
        .output()
        .map_err(|e| format!("run --gridfile-cell-polygons ({bin}): {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "cell-polygons failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }
    fs::read_to_string(&out_geojson).map_err(|e| format!("read {}: {e}", out_geojson.display()))
}
