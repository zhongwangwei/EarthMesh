//! Mesh quality and map-overlay command handlers.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::engine::resolve_mkgrd;
use crate::mesh_paths::{existing_file_path, gridfile_dir};
use crate::quality::{parse_quality_summary, MeshQuality};

const MERIT_SURFACE_PREVIEW_STRIDE: u32 = 50;
const MERIT_RIVER_CELL_MIN_FRACTION: f64 = 0.001;

pub(crate) fn checked_mesh_kind(kind: Option<&str>) -> Result<&'static str, String> {
    match kind.map(str::trim) {
        None | Some("hex") => Ok("hex"),
        Some("tri") => Ok("tri"),
        Some(other) => Err(format!("mesh kind must be tri or hex, got {other:?}")),
    }
}

fn resolve_layer_file(path: Option<&str>, gridfile_dir: &Path) -> Option<PathBuf> {
    let path = path?.trim();
    if path.is_empty() || path.eq_ignore_ascii_case("none") {
        return None;
    }
    let direct = Path::new(path);
    if direct.is_absolute() {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    for base in gridfile_dir.ancestors() {
        if let Some(found) = existing_file_path(path, base) {
            return Some(found);
        }
    }
    None
}

/// Run `mkgrd.x --mesh-quality <gridfile> <dir> --kind <tri|hex>` and parse the
/// resulting `quality_summary.json` for the Quality dashboard.
#[tauri::command]
pub(crate) fn mesh_quality(gridfile: String, kind: Option<String>) -> Result<MeshQuality, String> {
    let kind = checked_mesh_kind(kind.as_deref())?;
    let dir = gridfile_dir(&gridfile)?;
    // Measure hexagon cells for hex/atmos (MPAS) meshes, triangles for FVCOM —
    // matching the cell view the map renders, so the reported angles are the real
    // cell angles (≈120° for hexagons), not the dual triangles (≈60°).
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
    let kind = checked_mesh_kind(Some(&kind))?;
    let dir = gridfile_dir(&gridfile)?;
    let out_geojson = dir.join("mesh_cells.geojson");
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

/// Classify final mesh cells against real MERIT-Hydro river/coast/surface masks.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn mesh_merit_cells(
    gridfile: String,
    kind: String,
    merit_root: String,
    w: f64,
    e: f64,
    s: f64,
    n: f64,
    stride: Option<u32>,
    landtype_file: Option<String>,
) -> Result<String, String> {
    if !([w, e, s, n].iter().all(|v| v.is_finite()) && e > w && n > s) {
        return Err("invalid MERIT-Hydro mesh bbox".to_string());
    }
    if !Path::new(&merit_root).is_dir() {
        return Err(format!("MERIT-Hydro directory not found: {merit_root}"));
    }
    let dir = gridfile_dir(&gridfile)?;
    let out_dir = dir.join("mesh_merit_cells");
    let _ = fs::remove_dir_all(&out_dir);
    fs::create_dir_all(&out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;
    let cells = out_dir.join("mesh_cells.geojson");
    let merit_hydro_dir = out_dir.join("merit_hydro");
    let merit_surface_dir = out_dir.join("merit_surface");
    let landtype_cells = out_dir.join("landtype_cell_mask.geojson");
    let river_cells = out_dir.join("river_cell_intersections.geojson");
    let coast_cells = out_dir.join("coast_cell_intersections.geojson");
    let classified = out_dir.join("mesh_cells_merit.geojson");
    let kind = if kind == "tri" { "tri" } else { "hex" };
    let bin = resolve_mkgrd();
    let landtype_file = resolve_layer_file(landtype_file.as_deref(), &dir);

    let res = Command::new(&bin)
        .arg("--gridfile-cell-polygons")
        .arg(&gridfile)
        .arg(&cells)
        .arg("--kind")
        .arg(kind)
        .output()
        .map_err(|e| format!("run --gridfile-cell-polygons ({bin}): {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "cell-polygons failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }

    let res = Command::new(&bin)
        .arg("--merit-hydro-geojson")
        .arg(&merit_root)
        .arg(&merit_hydro_dir)
        .arg("--bbox")
        .arg(w.to_string())
        .arg(s.to_string())
        .arg(e.to_string())
        .arg(n.to_string())
        .arg("--stride")
        .arg("1")
        .arg("--skip-surface-mask")
        .output()
        .map_err(|e| format!("run --merit-hydro-geojson ({bin}): {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "MERIT-Hydro river/coast masks failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }

    let res = Command::new(&bin)
        .arg("--hydro-cell-intersections")
        .arg(&cells)
        .arg(merit_hydro_dir.join("merit_river_masks.geojson"))
        .arg(&river_cells)
        .arg("--classes")
        .arg("R2,R3")
        .arg("--min-fraction")
        .arg(MERIT_RIVER_CELL_MIN_FRACTION.to_string())
        .arg("--domain-bbox")
        .arg(w.to_string())
        .arg(s.to_string())
        .arg(e.to_string())
        .arg(n.to_string())
        .output()
        .map_err(|e| format!("run --hydro-cell-intersections ({bin}): {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "river intersections failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }

    let background = if let Some(landtype) = &landtype_file {
        let res = Command::new(&bin)
            .arg("--landtype-cell-mask")
            .arg(&cells)
            .arg(landtype)
            .arg(&landtype_cells)
            .output()
            .map_err(|e| format!("run --landtype-cell-mask ({bin}): {e}"))?;
        if !res.status.success() {
            return Err(format!(
                "landtype cell mask failed: {}",
                String::from_utf8_lossy(&res.stderr)
            ));
        }
        landtype_cells.as_path()
    } else {
        let res = Command::new(&bin)
            .arg("--merit-hydro-geojson")
            .arg(&merit_root)
            .arg(&merit_surface_dir)
            .arg("--bbox")
            .arg(w.to_string())
            .arg(s.to_string())
            .arg(e.to_string())
            .arg(n.to_string())
            .arg("--stride")
            .arg(
                stride
                    .unwrap_or(MERIT_SURFACE_PREVIEW_STRIDE)
                    .max(1)
                    .to_string(),
            )
            .output()
            .map_err(|e| format!("run --merit-hydro-geojson ({bin}): {e}"))?;
        if !res.status.success() {
            return Err(format!(
                "MERIT-Hydro surface masks failed: {}",
                String::from_utf8_lossy(&res.stderr)
            ));
        }

        let res = Command::new(&bin)
            .arg("--hydro-cell-intersections")
            .arg(&cells)
            .arg(merit_hydro_dir.join("merit_coast_masks.geojson"))
            .arg(&coast_cells)
            .arg("--classes")
            .arg("COAST_LAND,COAST_OCEAN")
            .arg("--min-fraction")
            .arg("0")
            .arg("--domain-bbox")
            .arg(w.to_string())
            .arg(s.to_string())
            .arg(e.to_string())
            .arg(n.to_string())
            .output()
            .map_err(|e| format!("run --hydro-cell-intersections ({bin}): {e}"))?;
        if !res.status.success() {
            return Err(format!(
                "coast intersections failed: {}",
                String::from_utf8_lossy(&res.stderr)
            ));
        }
        cells.as_path()
    };

    let mut cmd = Command::new(&bin);
    cmd.arg("--hydro-complete-cell-mask")
        .arg(background)
        .arg(&classified)
        .arg("--river-geojson")
        .arg(&river_cells);
    if landtype_file.is_none() {
        cmd.arg("--coast-geojson")
            .arg(&coast_cells)
            .arg("--surface-geojson")
            .arg(merit_surface_dir.join("merit_surface_masks.geojson"));
    }
    let res = cmd
        .output()
        .map_err(|e| format!("run --hydro-complete-cell-mask ({bin}): {e}"))?;
    if !res.status.success() {
        return Err(format!(
            "complete cell mask failed: {}",
            String::from_utf8_lossy(&res.stderr)
        ));
    }

    fs::read_to_string(&classified).map_err(|e| format!("read {}: {e}", classified.display()))
}
