use std::{fs, process::Command};

use earthmesh_cli::{write_unstructured_mesh_netcdf, LonLatPoint, UnstructuredMesh};

fn fixture_mesh() -> UnstructuredMesh {
    UnstructuredMesh {
        m_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
        ],
        w_points: vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 0.0 },
            LonLatPoint { lon: 1.0, lat: 1.0 },
            LonLatPoint { lon: 0.0, lat: 1.0 },
        ],
        m_to_w: vec![[1, 2, 3], [1, 3, 4], [1, 2, 4], [2, 3, 4]],
        w_to_m: vec![
            vec![1, 2, 3, 4],
            vec![1, 2, 3, 4],
            vec![1, 2, 3, 4],
            vec![1, 2, 3, 4],
        ],
        n_w_to_m: vec![4, 4, 4, 4],
    }
}

#[test]
fn mesh_quality_cli_reports_tri_and_hex_views_without_repo_fixture() {
    let root = std::env::temp_dir().join(format!("earthmesh_quality_views_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let gridfile = root.join("gridfile.nc4");
    write_unstructured_mesh_netcdf(&gridfile, &fixture_mesh()).expect("write gridfile");

    for kind in ["tri", "hex"] {
        let out_dir = root.join(kind);
        let output = Command::new(env!("CARGO_BIN_EXE_earthmesh_cli"))
            .arg("--mesh-quality")
            .arg(&gridfile)
            .arg(&out_dir)
            .arg("--kind")
            .arg(kind)
            .output()
            .expect("run earthmesh_cli");
        assert!(
            output.status.success(),
            "{kind} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
        assert!(
            stdout.contains(&format!("mesh_quality_kind={kind}")),
            "{stdout}"
        );
        assert!(stdout.contains("mesh_quality_cell_sides="), "{stdout}");

        let json = fs::read_to_string(out_dir.join("quality_summary.json")).expect("json");
        assert!(
            json.contains(&format!("\"cell_view\": \"{kind}\"")),
            "{json}"
        );
        let csv = fs::read_to_string(out_dir.join("quality_summary.csv")).expect("csv");
        assert!(csv.contains(&format!("summary,cell_view,,{kind}")), "{csv}");
        let md = fs::read_to_string(out_dir.join("quality_report.md")).expect("md");
        assert!(md.contains(&format!("- cell view: `{kind}`")), "{md}");
        assert!(out_dir.join("worst_cells.geojson").is_file());
    }

    let _ = fs::remove_dir_all(&root);
}
