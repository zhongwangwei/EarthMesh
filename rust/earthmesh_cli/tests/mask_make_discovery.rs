use std::fs;

#[test]
fn discover_mask_sources_matches_canonical_prefix_glob_and_path_split() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_discovery_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("domain_02.txt"), "2").expect("domain 02");
    fs::write(root.join("domain_01.txt"), "1").expect("domain 01");
    fs::write(root.join("other_01.txt"), "other").expect("other");

    let prefix = root.join("domain");
    let discovery = earthmesh_cli::mask_source_discovery::discover_mask_sources(&prefix)
        .expect("discover prefix files");

    assert_eq!(discovery.directory, root);
    assert_eq!(discovery.file_prefix, "domain");
    assert_eq!(
        discovery.files,
        vec![
            discovery.directory.join("domain_01.txt"),
            discovery.directory.join("domain_02.txt"),
        ]
    );
}

#[test]
fn discover_mask_sources_requires_parent_directory_like_mask_make() {
    let err = earthmesh_cli::mask_source_discovery::discover_mask_sources("domain_prefix")
        .expect_err("Canonical Mask_make requires a path separator in mask_fprefix");

    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("mask_fprefix"));
}
