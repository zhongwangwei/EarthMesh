use std::{fs, path::Path};

use earthmesh_project::ProjectConfig;

#[test]
fn committed_project_yaml_examples_parse_and_lower() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/projects");
    let mut checked = 0usize;
    for entry in fs::read_dir(&root).expect("read examples/projects") {
        let path = entry.expect("example dir entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read project yaml example");
        let project = ProjectConfig::from_yaml(&text).expect("project yaml example validates");
        let lowered = project.try_lower().expect("project yaml example lowers");
        assert!(
            lowered.to_namelist().contains("&mkgrd"),
            "{}",
            path.display()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "expected at least one examples/projects/*.yaml"
    );
}
