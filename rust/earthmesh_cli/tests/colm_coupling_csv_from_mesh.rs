//! CoLM surface-data workflow: a mesh gridfile + a global land-type NetCDF must
//! classify each cell LAND/OCEAN (Area_judge rule) and feed the existing
//! CoLM coupling NetCDF writer. The global land/ocean split is the geographic
//! sanity check (~71% ocean on Earth). Needs the local NXP16 hex gridfile and a
//! land-type NetCDF; ignored by default because the global land-type read is
//! slow and depends on machine-local fixtures. Run with `make test-slow`.

use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create temp root");
    path
}

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
fn colm_coupling_netcdf_exposes_surface_class_points_for_preview_coloring() {
    let root = temp_root("colm_surface_preview");
    let csv = root.join("cells.csv");
    let nc = root.join("coupling.nc4");
    let manifest = root.join("manifest.json");
    std::fs::write(&manifest, "{}").expect("write manifest");
    std::fs::write(
        &csv,
        "cell_id,cell_index,center_lon,center_lat,surface_class,has_river,river_class,river_fraction,estimated_river_area_m2,has_coast,coast_class,coastal_fraction,normalized_cell_area_m2,source_areaCell\n\
case_1,1,110.000000,20.000000,LAND,false,none,0.0,0.0,false,none,0.0,0.0,0.0\n\
case_2,2,111.000000,21.000000,OCEAN,false,none,0.0,0.0,false,none,0.0,0.0,0.0\n\
case_3,3,112.000000,22.000000,COAST,false,none,0.0,0.0,true,COAST,0.5,0.0,0.0\n",
    )
    .expect("write csv");

    earthmesh_cli::write_colm_coupling_netcdf_from_csv(&csv, &nc, "case", &manifest)
        .expect("write CoLM coupling NetCDF");
    let points =
        earthmesh_cli::read_colm_surface_class_points_netcdf(&nc).expect("read class points");

    assert_eq!(points.len(), 3);
    assert_eq!(
        points.iter().map(|point| point.code).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(points[0].lon, 110.0);
    assert_eq!(points[2].lat, 22.0);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[ignore = "slow local-fixture CoLM smoke; run with make test-slow"]
fn mesh_plus_landtype_classifies_cells_and_writes_colm_netcdf() {
    let gf =
        PathBuf::from("/tmp/earthmesh_cases/quickstart_n16/gridfile/gridfile_NXP0016_01_hex.nc4");
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
    let counts =
        earthmesh_cli::write_colm_coupling_csv_from_mesh(&gf, &lt, 120, "qs16", "hex", &csv)
            .expect("generate CoLM coupling CSV");
    let total = counts.land + counts.ocean;
    assert!(
        total > 2000,
        "global NXP16 mesh should have ~2562 cells, got {total}"
    );
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

// R7 coupling-quality validator end-to-end on the same mesh+land-type fixtures: each
// cell's land/ocean fraction (centre + corners) + adjacency feed
// earthmesh_quality::coupling, producing coupling_quality.json. Ignored like the sibling
// (needs the NXP16 gridfile + a land-type NetCDF); run with `make test-slow`.
#[test]
#[ignore = "slow local-fixture coupling-quality smoke; run with make test-slow"]
fn mesh_plus_landtype_coupling_quality_report() {
    let gf =
        PathBuf::from("/tmp/earthmesh_cases/quickstart_n16/gridfile/gridfile_NXP0016_01_hex.nc4");
    let Some(lt) = landtype() else {
        eprintln!("skip: no land-type NetCDF (set EARTHMESH_LANDTYPE)");
        return;
    };
    if !gf.exists() {
        eprintln!("skip: no NXP16 hex gridfile fixture");
        return;
    }
    let out = std::env::temp_dir().join("coupling_quality_test.json");
    let report = earthmesh_cli::write_coupling_quality_from_gridfile(&gf, &lt, 120, &out)
        .expect("coupling quality from gridfile");
    let total = report.total_land_cells + report.total_ocean_cells;
    assert!(total > 2000, "global NXP16 mesh ~2562 cells, got {total}");
    // a real global coastline must produce some fractional mixed-coast cells
    assert!(
        report.mixed_coastline_cells > 0,
        "expected coastline cells on a global mesh"
    );
    assert!(matches!(report.verdict.as_str(), "pass" | "warn" | "fail"));
    let json = std::fs::read_to_string(&out).unwrap();
    assert!(
        json.contains("\"kind\": \"earthmesh_coupling_quality\""),
        "{json}"
    );
}
