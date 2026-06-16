use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp root");
    path
}

#[test]
fn binary_can_export_composite_close_masks_and_summary_json() {
    let root = temp_root("hydro_composite_close_mask_cli");
    let river_geojson = root.join("river.geojson");
    let coast_geojson = root.join("coast.geojson");
    let recipe_json = root.join("recipe.json");
    let output_prefix = root.join("refine_spc_hydro_mix");
    let summary_json = root.join("summary.json");
    fs::write(
        &river_geojson,
        r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]] }},
          {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,0],[3,1],[2,1],[2,0]]] }},
          {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[4,0],[5,0],[5,1],[4,1],[4,0]]] }},
          {"type":"Feature","properties":{"river_class":"R3"},"geometry":{"type":"Polygon","coordinates":[[[10,0],[11,0],[11,1],[10,1],[10,0]]] }}
        ]}"#,
    )
    .expect("write river geojson");
    fs::write(
        &coast_geojson,
        r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"mask_class":"COAST"},"geometry":{"type":"Polygon","coordinates":[[[20,0],[21,0],[21,1],[20,1],[20,0]]] }},
          {"type":"Feature","properties":{"mask_class":"COAST"},"geometry":{"type":"Polygon","coordinates":[[[22,0],[23,0],[23,1],[22,1],[22,0]]] }}
        ]}"#,
    )
    .expect("write coast geojson");
    fs::write(
        &recipe_json,
        format!(
            r#"{{"components":[
              {{"name":"river","input_geojson":"{}","class_refine":{{"R2":1,"R3":3}},"max_rings_by_class":{{"R2":2,"R3":1}}}},
              {{"name":"coast","input_geojson":"{}","class_refine":{{"COAST":1}},"max_rings_by_class":{{"COAST":1}}}}
            ]}}"#,
            river_geojson.display(),
            coast_geojson.display()
        ),
    )
    .expect("write recipe json");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let status = Command::new(exe)
        .arg("--hydro-composite-close-mask-nmls")
        .arg(&recipe_json)
        .arg(&output_prefix)
        .arg("--summary-json")
        .arg(&summary_json)
        .status()
        .expect("run earthmesh_cli composite close mask export");

    assert!(
        status.success(),
        "earthmesh_cli should write composite close-mask outputs"
    );
    let mut names: Vec<_> = fs::read_dir(&root)
        .expect("read output dir")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .filter(|name| name.ends_with(".nml"))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "refine_spc_hydro_mix_COAST_d1_001.nml",
            "refine_spc_hydro_mix_R2_d1_001.nml",
            "refine_spc_hydro_mix_R2_d1_002.nml",
            "refine_spc_hydro_mix_R3_d1_001.nml",
            "refine_spc_hydro_mix_R3_d2_001.nml",
            "refine_spc_hydro_mix_R3_d3_001.nml",
        ]
    );
    let summary = fs::read_to_string(&summary_json).expect("read summary json");
    assert!(summary.contains(r#""kind":"earthmesh_composite_close_mask_summary""#));
    assert!(summary.contains(r#""files_written":6"#));
    assert!(summary.contains(r#""counts_by_component":{"coast":1,"river":5}"#));
    assert!(summary.contains(
        r#""counts_by_class_degree":{"COAST_d1":1,"R2_d1":2,"R3_d1":1,"R3_d2":1,"R3_d3":1}"#
    ));
    assert!(summary.contains(r#""name":"river""#));
    assert!(summary.contains(r#""files_selected":5"#));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn composite_recipe_can_dissolve_component_close_mask_envelopes() {
    let root = temp_root("hydro_composite_close_mask_dissolve_cli");
    let river_geojson = root.join("river.geojson");
    let recipe_json = root.join("recipe.json");
    let output_prefix = root.join("refine_spc_hydro_dissolve_mix");
    let summary_json = root.join("summary.json");
    fs::write(
        &river_geojson,
        r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]] }},
          {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0.8,0],[2,0],[2,1],[0.8,1],[0.8,0]]] }}
        ]}"#,
    )
    .expect("write river geojson");
    fs::write(
        &recipe_json,
        format!(
            r#"{{"components":[{{"name":"river","input_geojson":"{}","class_refine":{{"R2":1}},"buffer_deg_by_refine_degree":{{"1":0.1}},"dissolve_overlapping_envelopes":true}}]}}"#,
            river_geojson.display()
        ),
    )
    .expect("write recipe json");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-composite-close-mask-nmls")
        .arg(&recipe_json)
        .arg(&output_prefix)
        .arg("--summary-json")
        .arg(&summary_json)
        .output()
        .expect("run earthmesh_cli composite close mask export");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut names: Vec<_> = fs::read_dir(&root)
        .expect("read output dir")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .filter(|name| name.ends_with(".nml"))
        .collect();
    names.sort();
    assert_eq!(names, vec!["refine_spc_hydro_dissolve_mix_R2_d1_001.nml"]);
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_dissolve_mix_R2_d1_001.nml"))
            .expect("read dissolved composite close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "-0.10000000 -0.10000000",
            "2.10000000 -0.10000000",
            "2.10000000 1.10000000",
            "-0.10000000 1.10000000",
        ]
    );
    let summary = fs::read_to_string(&summary_json).expect("read summary json");
    assert!(summary.contains(r#""files_written":1"#));
    assert!(summary.contains(r#""counts_by_component":{"river":1}"#));

    let _ = fs::remove_dir_all(root);
}
