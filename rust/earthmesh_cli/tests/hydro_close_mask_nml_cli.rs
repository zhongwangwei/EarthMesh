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
fn binary_can_export_geojson_polygons_to_earthmesh_close_mask_nmls() {
    let root = temp_root("hydro_close_mask_nml_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro");
    let stale = root.join("refine_spc_hydro_R2_100.nml");
    fs::write(&stale, "close_num = 4\nclose_refine = 1\n").expect("write stale file");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R3"},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,0],[3,1],[2,1],[2,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let status = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .args(["R2=1", "R3=2"])
        .status()
        .expect("run earthmesh_cli close mask export");

    assert!(
        status.success(),
        "earthmesh_cli should write close-mask NML files"
    );
    assert!(
        !stale.exists(),
        "stale prefix NMLs should be removed before writing"
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
            "refine_spc_hydro_R2_d1_001.nml",
            "refine_spc_hydro_R3_d1_001.nml",
            "refine_spc_hydro_R3_d2_001.nml",
        ]
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_R2_d1_001.nml"))
            .expect("read R2 close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "1.00000000 0.00000000",
            "1.00000000 1.00000000",
            "0.00000000 1.00000000",
        ]
    );
    assert!(
        fs::read_to_string(root.join("refine_spc_hydro_R3_d2_001.nml"))
            .expect("read R3 degree 2 mask")
            .contains("close_refine = 2")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_can_export_hydro_class_geojson_polygons_to_close_mask_nmls() {
    let root = temp_root("hydro_close_mask_hydro_class_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_class");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"hydro_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,1],[0,1],[0,0]]] }}
          ]
        }"#,
    )
    .expect("write hydro_class input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .output()
        .expect("run earthmesh_cli close mask export with hydro_class");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_class_R2_d1_001.nml"))
            .expect("read hydro_class close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "2.00000000 0.00000000",
            "2.00000000 1.00000000",
            "0.00000000 1.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_geojson_linestring_as_buffered_close_mask() {
    let root = temp_root("hydro_close_mask_linestring_cli");
    let input_geojson = root.join("rivers.geojson");
    let output_prefix = root.join("refine_spc_hydro_line");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"LineString","coordinates":[[0,0],[2,0]]}}
          ]
        }"#,
    )
    .expect("write LineString input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--buffer-deg-by-refine-degree")
        .arg("1=0.1")
        .output()
        .expect("run earthmesh_cli close mask export with LineString");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_line_R2_d1_001.nml"))
            .expect("read LineString close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.10000000",
            "2.00000000 0.10000000",
            "2.00000000 -0.10000000",
            "0.00000000 -0.10000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_geojson_multilinestring_as_cumulative_buffered_close_masks() {
    let root = temp_root("hydro_close_mask_multilinestring_cli");
    let input_geojson = root.join("rivers.geojson");
    let output_prefix = root.join("refine_spc_hydro_multiline");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R3"},"geometry":{"type":"MultiLineString","coordinates":[[[0,0],[1,0],[1,1]],[[2,0],[3,0]]]}}
          ]
        }"#,
    )
    .expect("write MultiLineString input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R3=2")
        .arg("--buffer-deg-by-refine-degree")
        .args(["1=0.1", "2=0.2"])
        .output()
        .expect("run earthmesh_cli close mask export with MultiLineString");

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
        .filter(|name| name.starts_with("refine_spc_hydro_multiline_R3_d"))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "refine_spc_hydro_multiline_R3_d1_001.nml",
            "refine_spc_hydro_multiline_R3_d1_002.nml",
            "refine_spc_hydro_multiline_R3_d2_001.nml",
            "refine_spc_hydro_multiline_R3_d2_002.nml",
        ]
    );
    let bent = fs::read_to_string(root.join("refine_spc_hydro_multiline_R3_d2_001.nml"))
        .expect("read bent MultiLineString close mask");
    assert!(
        bent.contains("0.00000000 0.20000000")
            && bent.contains("0.80000000 0.20000000")
            && bent.contains("1.20000000 -0.20000000"),
        "bent line corridor should preserve the mitered turn instead of becoming a bbox: {bent}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_axis_aligned_polygon_hole_as_separate_close_masks() {
    let root = temp_root("hydro_close_mask_rectangular_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[4,0],[4,4],[0,4],[0,0]],
              [[1,1],[3,1],[3,3],[1,3],[1,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with rectangular hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        4,
        "rectangular interior hole should split the close-mask coverage"
    );
    let first = fs::read_to_string(root.join("refine_spc_hydro_hole_R2_d1_001.nml"))
        .expect("read first rectangular-hole close mask");
    assert!(
        !first.contains("4.00000000 4.00000000\n0.00000000 4.00000000"),
        "hole-aware export must not emit one full outer rectangle mask: {first}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_multiple_axis_aligned_polygon_holes_as_separate_close_masks() {
    let root = temp_root("hydro_close_mask_multiple_rectangular_holes_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_multi_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[6,0],[6,4],[0,4],[0,0]],
              [[1,1],[2,1],[2,3],[1,3],[1,1]],
              [[4,1],[5,1],[5,3],[4,3],[4,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with multiple rectangular holes");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_multi_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        13,
        "two rectangular interior holes should split the covered grid cells without emitting hole cells"
    );
    let concatenated_masks = (1..=13)
        .map(|index| {
            fs::read_to_string(
                root.join(format!("refine_spc_hydro_multi_hole_R2_d1_{index:03}.nml")),
            )
            .expect("read split multi-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !concatenated_masks.contains("1.00000000 1.00000000\n2.00000000 1.00000000\n2.00000000 3.00000000\n1.00000000 3.00000000"),
        "left hole must not be emitted as a close mask: {concatenated_masks}"
    );
    assert!(
        !concatenated_masks.contains("4.00000000 1.00000000\n5.00000000 1.00000000\n5.00000000 3.00000000\n4.00000000 3.00000000"),
        "right hole must not be emitted as a close mask: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_rectilinear_polygon_hole_as_grid_split_close_masks() {
    let root = temp_root("hydro_close_mask_rectilinear_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_rectilinear_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[4,0],[4,4],[0,4],[0,0]],
              [[1,1],[3,1],[3,2],[2,2],[2,3],[1,3],[1,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with rectilinear hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_rectilinear_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        13,
        "L-shaped rectilinear interior holes should be excluded from grid-split close masks"
    );
    let concatenated_masks = (1..=13)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_rectilinear_hole_R2_d1_{index:03}.nml"
            )))
            .expect("read split rectilinear-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !concatenated_masks.contains("1.00000000 1.00000000\n2.00000000 1.00000000\n2.00000000 2.00000000\n1.00000000 2.00000000"),
        "lower-left L-hole cell must not be emitted as a close mask: {concatenated_masks}"
    );
    assert!(
        !concatenated_masks.contains("2.00000000 1.00000000\n3.00000000 1.00000000\n3.00000000 2.00000000\n2.00000000 2.00000000"),
        "lower-right L-hole cell must not be emitted as a close mask: {concatenated_masks}"
    );
    assert!(
        !concatenated_masks.contains("1.00000000 2.00000000\n2.00000000 2.00000000\n2.00000000 3.00000000\n1.00000000 3.00000000"),
        "upper-left L-hole cell must not be emitted as a close mask: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_triangular_polygon_hole_as_separate_close_masks() {
    let root = temp_root("hydro_close_mask_triangular_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_triangular_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[4,0],[4,4],[0,4],[0,0]],
              [[1,1],[3,1],[2,3],[1,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with triangular hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_triangular_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        4,
        "single triangular interior hole should split exterior coverage into separate close masks"
    );
    let first = fs::read_to_string(root.join("refine_spc_hydro_triangular_hole_R2_d1_001.nml"))
        .expect("read first triangular-hole close mask");
    assert!(
        !first.contains("4.00000000 4.00000000\n0.00000000 4.00000000"),
        "triangular-hole export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=4)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_triangular_hole_R2_d1_{index:03}.nml"
            )))
            .expect("read split triangular-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("2.00000000 3.00000000"),
        "split masks should trace the triangular hole apex: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_downward_triangular_polygon_hole_as_separate_close_masks() {
    let root = temp_root("hydro_close_mask_downward_triangular_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_downward_triangular_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[4,0],[4,4],[0,4],[0,0]],
              [[1,3],[2,1],[3,3],[1,3]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with downward triangular hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_downward_triangular_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        4,
        "downward triangular interior hole should split exterior coverage into separate close masks"
    );
    let first =
        fs::read_to_string(root.join("refine_spc_hydro_downward_triangular_hole_R2_d1_001.nml"))
            .expect("read first downward triangular-hole close mask");
    assert!(
        !first.contains("4.00000000 4.00000000\n0.00000000 4.00000000"),
        "downward triangular-hole export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=4)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_downward_triangular_hole_R2_d1_{index:03}.nml"
            )))
            .expect("read split downward triangular-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("2.00000000 1.00000000"),
        "split masks should trace the downward triangular hole apex: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_vertical_base_triangular_polygon_hole_as_separate_close_masks() {
    let root = temp_root("hydro_close_mask_vertical_triangular_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_vertical_triangular_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[4,0],[4,4],[0,4],[0,0]],
              [[1,1],[3,2],[1,3],[1,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with vertical-base triangular hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_vertical_triangular_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        4,
        "vertical-base triangular interior hole should split exterior coverage into separate close masks"
    );
    let first =
        fs::read_to_string(root.join("refine_spc_hydro_vertical_triangular_hole_R2_d1_001.nml"))
            .expect("read first vertical-base triangular-hole close mask");
    assert!(
        !first.contains("4.00000000 4.00000000\n0.00000000 4.00000000"),
        "vertical-base triangular-hole export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=4)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_vertical_triangular_hole_R2_d1_{index:03}.nml"
            )))
            .expect("read split vertical-base triangular-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("3.00000000 2.00000000"),
        "split masks should trace the vertical-base triangular hole apex: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_slanted_triangular_polygon_hole_as_slab_close_masks() {
    let root = temp_root("hydro_close_mask_slanted_triangular_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_slanted_triangular_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[4,0],[4,4],[0,4],[0,0]],
              [[1,1],[3,1.5],[2,3],[1,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with slanted triangular hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_slanted_triangular_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        6,
        "slanted triangular interior hole should split exterior coverage into vertical slab close masks"
    );
    let first =
        fs::read_to_string(root.join("refine_spc_hydro_slanted_triangular_hole_R2_d1_001.nml"))
            .expect("read first slanted triangular-hole close mask");
    assert!(
        !first.contains("4.00000000 4.00000000\n0.00000000 4.00000000"),
        "slanted triangular-hole export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=6)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_slanted_triangular_hole_R2_d1_{index:03}.nml"
            )))
            .expect("read split slanted triangular-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("2.00000000 3.00000000"),
        "slab masks should trace the slanted triangular hole apex: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_multiple_slanted_triangular_polygon_holes_as_slab_close_masks() {
    let root = temp_root("hydro_close_mask_multiple_slanted_triangular_holes_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_multi_slanted_triangular_holes");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[8,0],[8,4],[0,4],[0,0]],
              [[1,1],[3,1.5],[2,3],[1,1]],
              [[5,1],[7,1.5],[6,3],[5,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with multiple slanted triangular holes");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_multi_slanted_triangular_holes_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        11,
        "multiple slanted triangular holes should split exterior coverage into slab close masks"
    );
    let first = fs::read_to_string(
        root.join("refine_spc_hydro_multi_slanted_triangular_holes_R2_d1_001.nml"),
    )
    .expect("read first multi slanted triangular-hole close mask");
    assert!(
        !first.contains("8.00000000 4.00000000\n0.00000000 4.00000000"),
        "multi slanted triangular-hole export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=11)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_multi_slanted_triangular_holes_R2_d1_{index:03}.nml"
            )))
            .expect("read split multi slanted triangular-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("2.00000000 3.00000000")
            && concatenated_masks.contains("6.00000000 3.00000000"),
        "slab masks should trace both triangular hole apices: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_diamond_polygon_hole_as_slab_close_masks() {
    let root = temp_root("hydro_close_mask_diamond_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_diamond_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[4,0],[4,4],[0,4],[0,0]],
              [[2,0.75],[3.25,2],[2,3.25],[0.75,2],[2,0.75]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with diamond polygon hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_diamond_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        6,
        "diamond interior hole should split exterior coverage into vertical slab close masks"
    );
    let first = fs::read_to_string(root.join("refine_spc_hydro_diamond_hole_R2_d1_001.nml"))
        .expect("read first diamond-hole close mask");
    assert!(
        !first.contains("4.00000000 4.00000000\n0.00000000 4.00000000"),
        "diamond-hole export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=6)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_diamond_hole_R2_d1_{index:03}.nml"
            )))
            .expect("read split diamond-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("2.00000000 0.75000000")
            && concatenated_masks.contains("2.00000000 3.25000000"),
        "slab masks should trace the diamond hole vertical apices: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_overlapping_diamond_polygon_holes_as_merged_slab_close_masks() {
    let root = temp_root("hydro_close_mask_overlapping_diamond_holes_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_overlapping_diamond_holes");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[6,0],[6,6],[0,6],[0,0]],
              [[2,1],[3.5,3],[2,5],[0.5,3],[2,1]],
              [[4,1],[5.5,3],[4,5],[2.5,3],[4,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with overlapping diamond polygon holes");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_overlapping_diamond_holes_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        12,
        "overlapping diamond holes should merge active vertical spans instead of falling back to one exterior mask"
    );
    let first =
        fs::read_to_string(root.join("refine_spc_hydro_overlapping_diamond_holes_R2_d1_001.nml"))
            .expect("read first overlapping diamond-hole close mask");
    assert!(
        !first.contains("6.00000000 6.00000000\n0.00000000 6.00000000"),
        "overlapping diamond-hole export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=12)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_overlapping_diamond_holes_R2_d1_{index:03}.nml"
            )))
            .expect("read split overlapping diamond-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("2.00000000 1.00000000")
            && concatenated_masks.contains("4.00000000 5.00000000"),
        "slab masks should trace both overlapping diamond holes: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_concave_multi_span_polygon_hole_as_slab_close_masks() {
    let root = temp_root("hydro_close_mask_concave_multi_span_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_concave_multi_span_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[6,0],[6,6],[0,6],[0,0]],
              [[1,1],[5,1],[5,5],[1,5],[1,4],[3.5,3.2],[3.5,2.8],[1,2],[1,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with concave multi-span polygon hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_concave_multi_span_hole_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        7,
        "concave multi-span hole should preserve the open notch as its own slab close mask"
    );
    let concatenated_masks = (1..=7)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_concave_multi_span_hole_R2_d1_{index:03}.nml"
            )))
            .expect("read split concave multi-span-hole close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("3.50000000 2.80000000")
            && concatenated_masks.contains("3.50000000 3.20000000")
            && concatenated_masks.contains("1.00000000 2.00000000")
            && concatenated_masks.contains("1.00000000 4.00000000"),
        "slab masks should trace the concave notch boundaries: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_crossing_order_non_rectilinear_holes_as_split_slab_close_masks() {
    let root = temp_root("hydro_close_mask_crossing_order_holes_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_crossing_order_holes");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[6,0],[6,6],[0,6],[0,0]],
              [[1,3.6],[5,1.6],[5,2.4],[1,4.4],[1,3.6]],
              [[1,1.6],[5,3.6],[5,4.4],[1,2.4],[1,1.6]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with crossing-order non-rectilinear holes");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_crossing_order_holes_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        10,
        "crossing-order non-rectilinear holes should split slabs at hole-edge crossings instead of falling back to one exterior mask"
    );
    let first =
        fs::read_to_string(root.join("refine_spc_hydro_crossing_order_holes_R2_d1_001.nml"))
            .expect("read first crossing-order close mask");
    assert!(
        !first.contains("6.00000000 6.00000000\n0.00000000 6.00000000"),
        "crossing-order export must not emit one full outer rectangle mask: {first}"
    );
    let concatenated_masks = (1..=10)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_crossing_order_holes_R2_d1_{index:03}.nml"
            )))
            .expect("read split crossing-order close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        concatenated_masks.contains("2.20000000") && concatenated_masks.contains("3.80000000"),
        "slab masks should include the crossing-derived split x coordinates: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_preserves_rectilinear_multi_component_union_hole_as_separate_masks() {
    let root = temp_root("hydro_close_mask_rectilinear_donut_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_rectilinear_donut_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[4,0],[4,1],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,3],[4,3],[4,4],[0,4],[0,3]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,1],[1,1],[1,3],[0,3],[0,1]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[3,1],[4,1],[4,3],[3,3],[3,1]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with rectilinear donut dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_rectilinear_donut_union_R2_d1_"))
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names.len(),
        2,
        "rectilinear multi-component donut union should preserve the interior hole as multiple close masks"
    );
    let concatenated_masks = (1..=2)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_rectilinear_donut_union_R2_d1_{index:03}.nml"
            )))
            .expect("read split rectilinear donut close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !concatenated_masks.contains(
            "close_num = 4\nclose_refine = 1\n0.00000000 0.00000000\n4.00000000 0.00000000\n4.00000000 4.00000000\n0.00000000 4.00000000"
        ),
        "rectilinear donut dissolve must not collapse to one complete outer rectangle mask: {concatenated_masks}"
    );
    assert!(
        concatenated_masks.contains("1.00000000 1.00000000")
            && concatenated_masks.contains("3.00000000 3.00000000"),
        "split masks should retain the interior hole boundaries: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_preserves_non_rectilinear_multi_component_union_hole_without_bbox() {
    let root = temp_root("hydro_close_mask_non_rectilinear_donut_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_non_rectilinear_donut_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,4],[4,4],[3,3],[1,3],[0,4]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[4,4],[4,0],[3,1],[3,3],[4,4]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[4,0],[0,0],[1,1],[3,1],[4,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[0,4],[1,3],[1,1],[0,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with non-rectilinear donut dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_non_rectilinear_donut_union_R2_d1_"))
        .collect::<Vec<_>>();
    names.sort();
    assert!(
        names.len() > 1,
        "non-rectilinear multi-component union with an interior gap should split coverage instead of one coarse mask: {names:?}"
    );
    let concatenated_masks = names
        .iter()
        .map(|name| fs::read_to_string(root.join(name)).expect("read non-rectilinear donut mask"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !concatenated_masks.contains(
            "close_num = 4\nclose_refine = 1\n0.00000000 0.00000000\n4.00000000 0.00000000\n4.00000000 4.00000000\n0.00000000 4.00000000"
        ),
        "non-rectilinear donut dissolve must not collapse to one complete bbox mask: {concatenated_masks}"
    );
    assert!(
        concatenated_masks.contains("1.00000000 1.00000000")
            && concatenated_masks.contains("3.00000000 3.00000000"),
        "split masks should retain the interior gap boundary vertex: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_can_cap_classes_and_skip_nearby_close_mask_rings() {
    let root = temp_root("hydro_close_mask_cap_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_cap");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,2],[0,2],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[1,1],[3,1],[3,3],[1,3],[1,1]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[5,0],[6,0],[6,1],[5,1],[5,0]]] }},
            {"type":"Feature","properties":{"river_class":"R3"},"geometry":{"type":"Polygon","coordinates":[[[10,0],[11,0],[11,1],[10,1],[10,0]]] }},
            {"type":"Feature","properties":{"river_class":"R3"},"geometry":{"type":"Polygon","coordinates":[[[12,0],[13,0],[13,1],[12,1],[12,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let status = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .args(["R2=1", "R3=2"])
        .arg("--max-rings-by-class")
        .args(["R2=3", "R3=1"])
        .arg("--min-ring-separation-deg")
        .arg("0.25")
        .status()
        .expect("run earthmesh_cli close mask export with caps");

    assert!(
        status.success(),
        "earthmesh_cli should support class caps and ring separation"
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
            "refine_spc_hydro_cap_R2_d1_001.nml",
            "refine_spc_hydro_cap_R2_d1_002.nml",
            "refine_spc_hydro_cap_R3_d1_001.nml",
            "refine_spc_hydro_cap_R3_d2_001.nml",
        ]
    );
    assert!(
        fs::read_to_string(root.join("refine_spc_hydro_cap_R2_d1_002.nml"))
            .expect("read second R2 ring")
            .contains("5.00000000 0.00000000")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_can_simplify_geojson_rings_before_close_mask_export() {
    let root = temp_root("hydro_close_mask_simplify_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_simplify");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[0.5,0.001],[1,0],[1.001,0.5],[1,1],[0,1],[0,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--simplify-tolerance-deg")
        .arg("0.05")
        .output()
        .expect("run earthmesh_cli close mask export with simplification");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = fs::read_to_string(root.join("refine_spc_hydro_simplify_R2_d1_001.nml"))
        .expect("read simplified close mask");
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "1.00000000 0.00000000",
            "1.00000000 1.00000000",
            "0.00000000 1.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_can_buffer_geojson_rings_before_close_mask_export() {
    let root = temp_root("hydro_close_mask_buffer_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_buffer");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R3"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R3=2")
        .arg("--buffer-deg-by-refine-degree")
        .args(["1=0.2", "2=0.05"])
        .output()
        .expect("run earthmesh_cli close mask export with buffer");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_buffer_R3_d1_001.nml"))
            .expect("read degree 1 buffered close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "-0.20000000 -0.20000000",
            "1.20000000 -0.20000000",
            "1.20000000 1.20000000",
            "-0.20000000 1.20000000",
        ]
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_buffer_R3_d2_001.nml"))
            .expect("read degree 2 buffered close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 2",
            "-0.05000000 -0.05000000",
            "1.05000000 -0.05000000",
            "1.05000000 1.05000000",
            "-0.05000000 1.05000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_buffers_non_axis_aligned_rings_with_ring_offset_not_bbox_envelope() {
    let root = temp_root("hydro_close_mask_ring_offset_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_ring_offset");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,1],[1,0],[2,1],[1,2],[0,1]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--buffer-deg-by-refine-degree")
        .arg("1=0.1")
        .output()
        .expect("run earthmesh_cli close mask export with ring offset");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_ring_offset_R2_d1_001.nml"))
            .expect("read ring-offset close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "-0.14142136 1.00000000",
            "1.00000000 -0.14142136",
            "2.14142136 1.00000000",
            "1.00000000 2.14142136",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_can_dissolve_overlapping_buffer_envelopes_before_close_mask_export() {
    let root = temp_root("hydro_close_mask_dissolve_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_dissolve");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0.8,0],[2,0],[2,1],[0.8,1],[0.8,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[5,0],[6,0],[6,1],[5,1],[5,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--buffer-deg-by-refine-degree")
        .arg("1=0.1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with envelope dissolve");

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
    assert_eq!(
        names,
        vec![
            "refine_spc_hydro_dissolve_R2_d1_001.nml",
            "refine_spc_hydro_dissolve_R2_d1_002.nml",
        ]
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_dissolve_R2_d1_001.nml"))
            .expect("read dissolved close mask")
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
    assert!(
        fs::read_to_string(root.join("refine_spc_hydro_dissolve_R2_d1_002.nml"))
            .expect("read disjoint close mask")
            .contains("4.90000000 -0.10000000")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_does_not_dissolve_point_touching_polygons_into_bbox() {
    let root = temp_root("hydro_close_mask_point_touch_no_bbox_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_point_touch");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[1,1],[2,1],[2,2],[1,2],[1,1]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with point-touching polygons");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_point_touch_R2_d1_"))
        .collect::<Vec<_>>();
    assert_eq!(
        names.len(),
        2,
        "point-only contact must not collapse to one bbox mask"
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_point_touch_R2_d1_001.nml"))
            .expect("read first point-touch close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "1.00000000 0.00000000",
            "1.00000000 1.00000000",
            "0.00000000 1.00000000",
        ]
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_point_touch_R2_d1_002.nml"))
            .expect("read second point-touch close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "1.00000000 1.00000000",
            "2.00000000 1.00000000",
            "2.00000000 2.00000000",
            "1.00000000 2.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_touching_rectangles_as_l_shape_not_bbox() {
    let root = temp_root("hydro_close_mask_l_shape_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_l_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,1],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,1],[1,1],[1,2],[0,2],[0,1]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with L-shape dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_l_union_R2_d1_001.nml"))
            .expect("read L-shape dissolved close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 6",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "2.00000000 0.00000000",
            "2.00000000 1.00000000",
            "1.00000000 1.00000000",
            "1.00000000 2.00000000",
            "0.00000000 2.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_chained_rectangles_without_collapsing_to_bbox() {
    let root = temp_root("hydro_close_mask_chained_rect_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_chained_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,1],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,1],[1,1],[1,2],[0,2],[0,1]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,2],[3,2],[3,3],[0,3],[0,2]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with chained rectangle dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_chained_union_R2_d1_001.nml"))
            .expect("read chained rectangle dissolved close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 8",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "2.00000000 0.00000000",
            "2.00000000 1.00000000",
            "1.00000000 1.00000000",
            "1.00000000 2.00000000",
            "3.00000000 2.00000000",
            "3.00000000 3.00000000",
            "0.00000000 3.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_shared_edge_non_rectangular_polygons_without_bbox() {
    let root = temp_root("hydro_close_mask_shared_edge_polygon_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_polygon_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[1,0],[2,1],[0,1],[1,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with shared-edge polygon dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_polygon_union_R2_d1_001.nml"))
            .expect("read shared-edge polygon dissolved close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "1.00000000 0.00000000",
            "2.00000000 1.00000000",
            "0.00000000 1.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_partial_shared_edge_polygons_without_bbox() {
    let root = temp_root("hydro_close_mask_partial_shared_edge_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_partial_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[0,2],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,1],[1,1],[2,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with partial shared-edge polygon dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_partial_union_R2_d1_001.nml"))
            .expect("read partial shared-edge polygon dissolved close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 5",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "2.00000000 0.00000000",
            "3.00000000 1.00000000",
            "1.00000000 1.00000000",
            "0.00000000 2.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_chained_non_rectangular_polygons_without_bbox() {
    let root = temp_root("hydro_close_mask_chained_polygon_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_chained_polygon_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[0,2],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[2,0],[3,1],[1,1],[2,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[1,1],[3,1],[4,3],[1,1]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with chained polygon dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_chained_polygon_union_R2_d1_001.nml"))
            .expect("read chained polygon dissolved close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 6",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "2.00000000 0.00000000",
            "3.00000000 1.00000000",
            "4.00000000 3.00000000",
            "1.00000000 1.00000000",
            "0.00000000 2.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_does_not_dissolve_bbox_overlapping_disjoint_non_rectangular_polygons() {
    let root = temp_root("hydro_close_mask_bbox_overlap_disjoint_polygon_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_bbox_overlap_disjoint");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[3,0],[0,3],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[4,1],[4,4],[1,4],[4,1]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with bbox-overlapping disjoint polygons");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_bbox_overlap_disjoint_R2_d1_"))
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        vec![
            "refine_spc_hydro_bbox_overlap_disjoint_R2_d1_001.nml",
            "refine_spc_hydro_bbox_overlap_disjoint_R2_d1_002.nml",
        ],
        "bbox-overlapping but disjoint non-rectilinear polygons must remain separate masks"
    );
    let concatenated_masks = (1..=2)
        .map(|index| {
            fs::read_to_string(root.join(format!(
                "refine_spc_hydro_bbox_overlap_disjoint_R2_d1_{index:03}.nml"
            )))
            .expect("read split bbox-overlap disjoint close mask")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !concatenated_masks.contains(
            "close_num = 4\nclose_refine = 1\n0.00000000 0.00000000\n4.00000000 0.00000000\n4.00000000 4.00000000\n0.00000000 4.00000000"
        ),
        "bbox-overlap gating must not create a coarse envelope over the gap: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_contained_non_rectangular_polygon_without_bbox() {
    let root = temp_root("hydro_close_mask_contained_polygon_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_contained_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[3,0],[0,3],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0.5,0.5],[1.0,0.5],[0.5,1.0],[0.5,0.5]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with contained polygon dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_contained_union_R2_d1_001.nml"))
            .expect("read contained polygon dissolved close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 3",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "3.00000000 0.00000000",
            "0.00000000 3.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_crossing_edge_polygons_without_bbox() {
    let root = temp_root("hydro_close_mask_crossing_polygon_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_crossing_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[0,2],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[1,-1],[3,1],[1,1],[1,-1]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with crossing-edge polygon dissolve");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_crossing_union_R2_d1_001.nml"))
            .expect("read crossing-edge polygon dissolved close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 6",
            "close_refine = 1",
            "1.00000000 -1.00000000",
            "3.00000000 1.00000000",
            "1.00000000 1.00000000",
            "0.00000000 2.00000000",
            "0.00000000 0.00000000",
            "1.00000000 0.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_non_axis_aligned_polygon_hole_as_slab_close_masks() {
    let root = temp_root("hydro_close_mask_non_axis_exterior_hole_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_non_axis_hole");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[
              [[0,0],[5,0],[4,4],[1,4],[0,0]],
              [[2,1],[3,2],[2,3],[1,2],[2,1]]
            ]}}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--no-max-masks-per-refine-degree")
        .output()
        .expect("run earthmesh_cli close mask export with non-axis exterior hole");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut names = fs::read_dir(&root)
        .expect("list output dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("refine_spc_hydro_non_axis_hole_R2_d1_"))
        .collect::<Vec<_>>();
    names.sort();
    assert!(
        names.len() > 1,
        "non-axis-aligned exteriors with holes should be split instead of emitted as one mask"
    );
    let concatenated_masks = names
        .iter()
        .map(|name| fs::read_to_string(root.join(name)).expect("read split non-axis close mask"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !concatenated_masks.contains(
            "close_num = 4\nclose_refine = 1\n0.00000000 0.00000000\n5.00000000 0.00000000\n4.00000000 4.00000000\n1.00000000 4.00000000"
        ),
        "hole-aware export must not emit the full slanted exterior mask: {concatenated_masks}"
    );
    assert!(
        concatenated_masks.contains("3.00000000 2.00000000"),
        "slab masks should trace the non-rectangular hole apex: {concatenated_masks}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_polygons_inside_geojson_geometry_collection() {
    let root = temp_root("hydro_close_mask_geometry_collection_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_geometry_collection");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{
              "type":"GeometryCollection",
              "geometries":[
                {"type":"Point","coordinates":[10,10]},
                {"type":"Polygon","coordinates":[[[0,0],[2,0],[2,1],[0,1],[0,0]]]}
              ]
            }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .output()
        .expect("run earthmesh_cli close mask export with GeometryCollection");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_geometry_collection_R2_d1_001.nml"))
            .expect("read GeometryCollection polygon close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "2.00000000 0.00000000",
            "2.00000000 1.00000000",
            "0.00000000 1.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_exports_single_geojson_feature_without_feature_collection_wrapper() {
    let root = temp_root("hydro_close_mask_single_feature_cli");
    let input_geojson = root.join("corridor.geojson");
    let output_prefix = root.join("refine_spc_hydro_single_feature");
    fs::write(
        &input_geojson,
        r#"{
          "type":"Feature",
          "properties":{"river_class":"R2"},
          "geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[2,1],[0,1],[0,0]]]}
        }"#,
    )
    .expect("write single Feature geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .output()
        .expect("run earthmesh_cli close mask export with single Feature");

    assert!(
        output.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join("refine_spc_hydro_single_feature_R2_d1_001.nml"))
            .expect("read single Feature close mask")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "close_num = 4",
            "close_refine = 1",
            "0.00000000 0.00000000",
            "2.00000000 0.00000000",
            "2.00000000 1.00000000",
            "0.00000000 1.00000000",
        ]
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_overlapping_non_rectilinear_convex_polygons() {
    let root = temp_root("hydro_close_mask_non_rect_convex_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_non_rect_convex_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[2,0],[1.5,1],[0,1],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[1,0],[3,0],[3,1],[0.5,1],[1,0]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with non-rectilinear convex union");

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
        .filter(|name| name.starts_with("refine_spc_hydro_non_rect_convex_union_R2_d1_"))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["refine_spc_hydro_non_rect_convex_union_R2_d1_001.nml"],
        "safe overlapping non-rectilinear convex polygons should dissolve to one mask"
    );
    let text =
        fs::read_to_string(root.join("refine_spc_hydro_non_rect_convex_union_R2_d1_001.nml"))
            .expect("read dissolved non-rectilinear convex mask");
    assert!(
        text.contains("3.00000000 1.00000000") && text.contains("0.00000000 1.00000000"),
        "dissolved mask should trace the outer union boundary instead of preserving separate inputs: {text}"
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn binary_dissolves_overlapping_non_rectilinear_concave_polygon_without_bbox() {
    let root = temp_root("hydro_close_mask_non_rect_concave_union_cli");
    let input_geojson = root.join("corridors.geojson");
    let output_prefix = root.join("refine_spc_hydro_non_rect_concave_union");
    fs::write(
        &input_geojson,
        r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[0,0],[3,0],[3,1],[1.5,1],[1,2.5],[0,2.5],[0,0]]] }},
            {"type":"Feature","properties":{"river_class":"R2"},"geometry":{"type":"Polygon","coordinates":[[[2,0.5],[4,0.5],[4,2],[2,2],[2,0.5]]] }}
          ]
        }"#,
    )
    .expect("write input geojson");

    let exe = std::env::var("CARGO_BIN_EXE_earthmesh_cli").expect("binary path from cargo");
    let output = Command::new(exe)
        .arg("--hydro-close-mask-nmls")
        .arg(&input_geojson)
        .arg(&output_prefix)
        .arg("--class-refine")
        .arg("R2=1")
        .arg("--dissolve-overlapping-envelopes")
        .output()
        .expect("run earthmesh_cli close mask export with non-rectilinear concave union");

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
        .filter(|name| name.starts_with("refine_spc_hydro_non_rect_concave_union_R2_d1_"))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["refine_spc_hydro_non_rect_concave_union_R2_d1_001.nml"],
        "overlapping non-rectilinear concave polygons should dissolve to one real union mask"
    );
    let text =
        fs::read_to_string(root.join("refine_spc_hydro_non_rect_concave_union_R2_d1_001.nml"))
            .expect("read dissolved non-rectilinear concave mask");
    assert!(
        !text.contains(
            "close_num = 4\nclose_refine = 1\n0.00000000 0.00000000\n4.00000000 0.00000000\n4.00000000 2.50000000\n0.00000000 2.50000000"
        ),
        "concave dissolve must not collapse to a coarse bbox mask: {text}"
    );
    assert!(
        text.contains("1.50000000 1.00000000")
            && text.contains("1.00000000 2.50000000")
            && text.contains("4.00000000 2.00000000"),
        "dissolved mask should retain the concave bank and added overlapping branch: {text}"
    );

    let _ = fs::remove_dir_all(root);
}
