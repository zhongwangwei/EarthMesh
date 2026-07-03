use std::path::PathBuf;

use super::usage;

/// `--mpas-cell-polygons <mesh.nc> <out.geojson> [--bbox W S E N] [--max-cells N]`:
/// read an MPAS/EarthMesh mesh NetCDF into cell-polygon GeoJSON (the cells input for
/// --hydro-cell-intersections / --hydro-complete-cell-mask). Port of read_mpas_cell_polygons.
pub(crate) fn run_mpas_cell_polygons(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut bbox: Option<[f64; 4]> = None;
    let mut max_cells: Option<usize> = None;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--bbox" => {
                let mut v = [0.0; 4];
                for slot in v.iter_mut() {
                    i += 1;
                    *slot = rest
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--bbox needs W S E N"))?;
                }
                bbox = Some(v);
            }
            "--max-cells" => {
                i += 1;
                max_cells = Some(
                    rest.get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--max-cells requires an integer"))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --mpas-cell-polygons option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 2 {
        return Err(usage("--mpas-cell-polygons needs <mesh.nc> <out.geojson>"));
    }
    let count = earthmesh_cli::write_mpas_cell_polygons_geojson(
        &positional[0],
        &positional[1],
        bbox,
        max_cells,
    )
    .map_err(|err| format!("mpas cell polygons: {err}"))?;
    println!("mpas_cell_features={count}");
    println!("mpas_cell_output={}", positional[1].display());
    Ok(())
}

/// `--gridfile-cell-polygons <gridfile.nc4> <out.geojson> [--kind hex|tri] [--bbox W S E N]
/// [--max-cells N]`: read an EarthMesh gridfile (GLONM/GLONW + itab connectivity) and write
/// cell-polygon GeoJSON in degrees. `hex` (default) draws W cells from their M corners;
/// `tri` draws one triangle per M cell. For map overlay of either mesh type.
pub(crate) fn run_gridfile_cell_polygons(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut bbox: Option<[f64; 4]> = None;
    let mut max_cells: Option<usize> = None;
    let mut kind = earthmesh_cli::GridfileCellKind::Hex;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--kind" => {
                i += 1;
                kind = match rest.get(i).map(String::as_str) {
                    Some("hex") => earthmesh_cli::GridfileCellKind::Hex,
                    Some("tri") => earthmesh_cli::GridfileCellKind::Tri,
                    _ => return Err(usage("--kind needs hex|tri")),
                };
            }
            "--bbox" => {
                let mut v = [0.0; 4];
                for slot in v.iter_mut() {
                    i += 1;
                    *slot = rest
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--bbox needs W S E N"))?;
                }
                bbox = Some(v);
            }
            "--max-cells" => {
                i += 1;
                max_cells = Some(
                    rest.get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--max-cells requires an integer"))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --gridfile-cell-polygons option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 2 {
        return Err(usage(
            "--gridfile-cell-polygons needs <gridfile.nc4> <out.geojson> [--kind hex|tri]",
        ));
    }
    let count = earthmesh_cli::write_gridfile_cell_polygons_geojson(
        &positional[0],
        &positional[1],
        kind,
        bbox,
        max_cells,
    )
    .map_err(|err| format!("gridfile cell polygons: {err}"))?;
    println!("gridfile_cell_features={count}");
    println!("gridfile_cell_output={}", positional[1].display());
    Ok(())
}

/// `--landtype-cell-mask <cells.geojson> <landtype.nc> <out.geojson>
/// [--gridnum-perdegree N]`: annotate final EarthMesh cells with land/ocean fractions
/// sampled from the landtype grid. Mixed land/ocean cells become COAST.
pub(crate) fn run_landtype_cell_mask(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut gridnum_perdegree: Option<usize> = None;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--gridnum-perdegree" => {
                i += 1;
                gridnum_perdegree = Some(
                    rest.get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--gridnum-perdegree requires an integer"))?,
                );
            }
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --landtype-cell-mask option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 3 {
        return Err(usage(
            "--landtype-cell-mask needs <cells.geojson> <landtype.nc> <out.geojson>",
        ));
    }
    let gridnum_perdegree = match gridnum_perdegree {
        Some(value) => value,
        None => earthmesh_cli::landtype_gridnum_perdegree(&positional[1])
            .map_err(|err| format!("landtype cell mask: {err}"))?,
    };
    let count = earthmesh_cli::write_landtype_cell_mask_geojson(
        &positional[0],
        &positional[1],
        gridnum_perdegree,
        &positional[2],
    )
    .map_err(|err| format!("landtype cell mask: {err}"))?;
    println!("landtype_cell_mask_features={count}");
    println!("landtype_cell_mask_output={}", positional[2].display());
    Ok(())
}

/// `--coastal-band-geojson <map_dir> <out.geojson> --bbox W S E N
/// [--radius-cells N] [--no-dissolve] [--no-yrev] [--undef U]`:
/// CaMa elevtn -> land mask -> coastal band -> GeoJSON (port of coastal_band.py end-to-end).
pub(crate) fn run_coastal_band_geojson(args: impl Iterator<Item = String>) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut bbox: Option<[f64; 4]> = None;
    let mut radius_cells: i64 = 3;
    let mut dissolve = true;
    let mut y_reversed = true;
    let mut undef = -9999.0f64;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--bbox" => {
                let mut v = [0.0; 4];
                for slot in v.iter_mut() {
                    i += 1;
                    *slot = rest
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--bbox needs W S E N"))?;
                }
                bbox = Some(v);
            }
            "--radius-cells" => {
                i += 1;
                radius_cells = rest
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| usage("--radius-cells requires an integer"))?;
            }
            "--undef" => {
                i += 1;
                undef = rest
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| usage("--undef requires a number"))?;
            }
            "--no-dissolve" => dissolve = false,
            "--no-yrev" => y_reversed = false,
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --coastal-band-geojson option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 2 {
        return Err(usage(
            "--coastal-band-geojson needs <map_dir> <out.geojson>",
        ));
    }
    let bbox = bbox.ok_or_else(|| usage("--coastal-band-geojson requires --bbox W S E N"))?;
    let count = earthmesh_cli::write_coastal_band_geojson_from_cama(
        &positional[0],
        &positional[1],
        bbox[0],
        bbox[1],
        bbox[2],
        bbox[3],
        radius_cells,
        y_reversed,
        dissolve,
        undef,
    )
    .map_err(|err| format!("coastal band geojson: {err}"))?;
    println!("coastal_band_features={count}");
    println!("coastal_band_output={}", positional[1].display());
    Ok(())
}

/// `--hydro-complete-cell-mask <background.geojson> <out.geojson>
/// [--river-geojson R] [--coast-geojson C] [--surface-geojson S]`:
/// annotate every background cell with surface_class + dominant mask_class
/// (port of cell_mask_merge.py). The output is the file --hydro-mesh-qa consumes.
pub(crate) fn run_hydro_complete_cell_mask(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut river: Option<PathBuf> = None;
    let mut coast: Option<PathBuf> = None;
    let mut surface: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < rest.len() {
        let need = |i: &mut usize, flag: &str| -> Result<PathBuf, String> {
            *i += 1;
            rest.get(*i)
                .map(PathBuf::from)
                .ok_or_else(|| usage(&format!("{flag} requires a value")))
        };
        match rest[i].as_str() {
            "--river-geojson" => river = Some(need(&mut i, "--river-geojson")?),
            "--coast-geojson" => coast = Some(need(&mut i, "--coast-geojson")?),
            "--surface-geojson" => surface = Some(need(&mut i, "--surface-geojson")?),
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --hydro-complete-cell-mask option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 2 {
        return Err(usage(
            "--hydro-complete-cell-mask needs <background.geojson> <out.geojson>",
        ));
    }
    let count = earthmesh_cli::write_complete_cell_mask_geojson(
        &positional[0],
        &positional[1],
        river.as_deref(),
        coast.as_deref(),
        surface.as_deref(),
    )
    .map_err(|err| format!("complete cell mask: {err}"))?;
    println!("hydro_complete_cell_mask_features={count}");
    println!(
        "hydro_complete_cell_mask_output={}",
        positional[1].display()
    );
    Ok(())
}

/// `--hydro-cell-intersections <cells.geojson> <corridors.geojson> <out.geojson>
/// [--classes R2,R3] [--min-fraction F] [--unit-sphere-area]`:
/// overlay EarthMesh cells x river/coast corridors -> per-cell intersection GeoJSON
/// (the input that --colm-coupling-from-intersections consumes). Port of
/// earthmesh_intersection.py.
pub(crate) fn run_hydro_cell_intersections(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let rest = args.collect::<Vec<_>>();
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut classes: Vec<String> = vec!["R2".into(), "R3".into()];
    let mut min_fraction = 0.0f64;
    let mut unit_sphere = false;
    let mut domain: Option<Vec<Vec<(f64, f64)>>> = None;
    let mut i = 0usize;
    while i < rest.len() {
        match rest[i].as_str() {
            "--domain-bbox" => {
                let mut v = [0.0; 4];
                for slot in v.iter_mut() {
                    i += 1;
                    *slot = rest
                        .get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| usage("--domain-bbox needs W S E N"))?;
                }
                domain = Some(vec![vec![
                    (v[0], v[1]),
                    (v[2], v[1]),
                    (v[2], v[3]),
                    (v[0], v[3]),
                ]]);
            }
            "--domain-geojson" => {
                i += 1;
                let path = rest
                    .get(i)
                    .ok_or_else(|| usage("--domain-geojson requires a value"))?;
                domain = Some(
                    earthmesh_cli::read_polygon_outer_rings(path)
                        .map_err(|err| format!("read domain geojson: {err}"))?,
                );
            }
            "--classes" => {
                i += 1;
                classes = rest
                    .get(i)
                    .ok_or_else(|| usage("--classes requires a value"))?
                    .split(',')
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string())
                    .collect();
            }
            "--min-fraction" => {
                i += 1;
                min_fraction = rest
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| usage("--min-fraction requires a number"))?;
            }
            "--unit-sphere-area" => unit_sphere = true,
            other if other.starts_with("--") => {
                return Err(usage(&format!(
                    "unknown --hydro-cell-intersections option: {other}"
                )));
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }
    if positional.len() != 3 {
        return Err(usage(
            "--hydro-cell-intersections needs <cells.geojson> <corridors.geojson> <out.geojson>",
        ));
    }
    let count = earthmesh_cli::write_earthmesh_intersection_geojson(
        &positional[0],
        &positional[1],
        &positional[2],
        &classes,
        min_fraction,
        unit_sphere,
        domain.as_deref(),
    )
    .map_err(|err| format!("cell intersections: {err}"))?;
    println!("hydro_cell_intersection_features={count}");
    println!("hydro_cell_intersection_output={}", positional[2].display());
    Ok(())
}

/// `--colm-coupling-from-intersections <intersection.geojson> <out.csv> [min_fraction]`:
/// assemble a CoLM coupling CSV from an EarthMesh cell×river intersection GeoJSON
/// (Rust port of util/hydro_mesh/colm_coupling.py).
pub(crate) fn run_colm_coupling_from_intersections(
    args: impl Iterator<Item = String>,
) -> Result<(), String> {
    let mut args = args.collect::<Vec<_>>().into_iter();
    let input_geojson = PathBuf::from(args.next().ok_or_else(|| {
        usage("--colm-coupling-from-intersections requires an input intersection GeoJSON")
    })?);
    let output_csv = PathBuf::from(
        args.next()
            .ok_or_else(|| usage("--colm-coupling-from-intersections requires an output CSV"))?,
    );
    let min_fraction = match args.next() {
        Some(value) => value
            .parse::<f64>()
            .map_err(|_| usage("min_fraction must be a number in [0,1]"))?,
        None => 0.0,
    };
    let rows = earthmesh_cli::write_colm_coupling_csv_from_intersections(
        &input_geojson,
        &output_csv,
        min_fraction,
    )
    .map_err(|err| format!("write coupling csv {}: {err}", output_csv.display()))?;
    println!("colm_coupling_rows={rows}");
    println!("colm_coupling_output={}", output_csv.display());
    Ok(())
}
