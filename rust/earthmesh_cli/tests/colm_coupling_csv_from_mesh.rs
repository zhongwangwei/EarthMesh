//! CoLM surface-data workflow: a mesh gridfile + a global land-type NetCDF must
//! classify each cell LAND/OCEAN (Area_judge rule) and feed the existing
//! CoLM coupling NetCDF writer. The global land/ocean split is the geographic
//! sanity check (~71% ocean on Earth). Needs the local NXP16 hex gridfile and a
//! land-type NetCDF; skips otherwise (set EARTHMESH_LANDTYPE to override).

use std::path::PathBuf;

fn landtype() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("EARTHMESH_LANDTYPE") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let d = PathBuf::from(
        "/Users/zhongwangwei/Desktop/EarthMesh_legacy_archive_20260616_142611/input/landtype_usgs_update.nc",
    );
    d.exists().then_some(d)
}

#[test]
fn mesh_plus_landtype_classifies_cells_and_writes_colm_netcdf() {
    let gf = PathBuf::from("/tmp/earthmesh_cases/quickstart_n16/gridfile/gridfile_NXP0016_01_hex.nc4");
    let Some(lt) = landtype() else {
        eprintln!("skip: no land-type NetCDF (set EARTHMESH_LANDTYPE)");
        return;
    };
    if !gf.exists() {
        eprintln!("skip: no NXP16 hex gridfile fixture");
        return;
    }
    let tmp = std::env::temp_dir();
    let csv = tmp.join("colm_cells_test.csv");
    let counts = earthmesh_cli::write_colm_coupling_csv_from_mesh(&gf, &lt, 120, "qs16", "hex", &csv)
        .expect("generate CoLM coupling CSV");
    let total = counts.land + counts.ocean;
    assert!(total > 2000, "global NXP16 mesh should have ~2562 cells, got {total}");
    let ocean_frac = counts.ocean as f64 / total as f64;
    // Earth is ~71% ocean; a correct sampling/orientation lands in this band.
    assert!(
        (0.60..=0.80).contains(&ocean_frac),
        "ocean fraction {ocean_frac:.3} outside sane 0.60–0.80 band (sampling orientation?)"
    );

    let nc = tmp.join("colm_cells_test.nc");
    let manifest = tmp.join("colm_manifest_test.json");
    std::fs::write(&manifest, "{}").unwrap();
    earthmesh_cli::write_colm_coupling_netcdf_from_csv(&csv, &nc, "qs16", &manifest)
        .expect("CSV -> CoLM coupling NetCDF");
    assert!(nc.exists() && std::fs::metadata(&nc).unwrap().len() > 0);
}
