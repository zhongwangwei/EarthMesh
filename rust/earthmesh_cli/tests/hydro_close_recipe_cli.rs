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
fn binary_can_write_hydro_close_refinement_recipe_json() {
    let root = temp_root("hydro_close_recipe_cli");
    let input_geojson = root.join("river_corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_r3d3");
    let output_json = root.join("hydro_recipe.json");
    fs::write(
        &input_geojson,
        r#"{"type":"FeatureCollection","features":[]}"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let status = Command::new(exe)
        .arg("--hydro-close-recipe")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg(&output_json)
        .arg("--class-refine")
        .args(["R2=1", "R3=3"])
        .arg("--buffer-deg-by-refine-degree")
        .args(["1=1.5", "2=1.0", "3=0.5"])
        .arg("--simplify-tolerance-deg")
        .arg("0.005")
        .arg("--example-namelist")
        .arg("Atmos_hex_NXP64_hydro_close.nml")
        .status()
        .expect("run earthmesh_cli hydro close recipe export");

    assert!(
        status.success(),
        "earthmesh_cli should write hydro close recipe JSON"
    );
    let text = fs::read_to_string(&output_json).expect("read recipe json");
    assert!(text.contains(r#""kind":"earthmesh_hydro_close_refinement_recipe""#));
    assert!(text.contains(&format!(r#""input_geojson":"{}""#, input_geojson.display())));
    assert!(text.contains(&format!(r#""output_prefix":"{}""#, output_prefix.display())));
    assert!(text.contains(r#""class_refine":{"R2":1,"R3":3}"#));
    assert!(text.contains(r#""buffer_deg_by_refine_degree":{"1":1.5,"2":1,"3":0.5}"#));
    assert!(text.contains(r#""simplify_tolerance_deg":0.005"#));
    assert!(text.contains(r#""RL%refine_spc":".TRUE.""#));
    assert!(text.contains(r#""RL%max_iter_spc":"3""#));
    assert!(text.contains(r#""RL%mask_refine_spc_type":"'close'""#));
    assert!(text.contains(&format!(
        r#""RL%mask_refine_spc_fprefix":"'{}'""#,
        output_prefix.display()
    )));
    assert!(text.contains(r#""close_mask_command":["earthmesh_cli","--hydro-close-mask-nmls""#));
    assert!(text.contains(r#""--class-refine","R2=1","R3=3""#));
    assert!(text.contains(r#""--buffer-deg-by-refine-degree","1=1.5","2=1","3=0.5""#));
    assert!(text.contains(r#""smoke_run_command":["./mkgrd.x","Atmos_hex_NXP64_hydro_close.nml"]"#));

    let _ = fs::remove_dir_all(root);
}
