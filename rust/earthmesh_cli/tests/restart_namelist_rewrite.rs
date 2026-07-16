use std::path::Path;

use earthmesh_cli::mkgrd_default_restart_handoff::rewrite_restart_refine_namelist_contents;
use earthmesh_core::{namelist_assignments, EarthmeshConfig};

#[test]
fn restart_rewrite_handles_inline_group_and_preserves_following_group() {
    let input = "prefix\n&mkgrd NL%EXPNME='rewrite', NL%NXP=4, NL%mesh_type='earthmesh', NL%mode_grid='tri', NL%output_format='CoLM', NL%refine=.false., NL%mask_restart=.true., NL%mode_file='old/path', NL%mode_file_description='old' / &quality QL%enabled=.true. /\nsuffix\n";
    let rewritten =
        rewrite_restart_refine_namelist_contents(input, Path::new("/tmp/new/gridfile.nc4"))
            .expect("rewrite inline restart namelist");

    let config = EarthmeshConfig::from_mkgrd_namelist(&rewritten).expect("parse rewritten mkgrd");
    assert!(!config.mask_restart);
    assert_eq!(config.mode_file, "/tmp/new/gridfile.nc4");
    assert_eq!(config.mode_file_description, "EarthMesh");
    assert_eq!(rewritten.matches("&mkgrd").count(), 1);
    assert!(rewritten.contains(" &quality QL%enabled=.true. /"));
    assert!(rewritten.starts_with("prefix\n"));
    assert!(rewritten.ends_with("\nsuffix\n"));
}

#[test]
fn restart_rewrite_escapes_quote_in_gridfile_and_adds_missing_fields_once() {
    let input = "&mkgrd\n  NL%EXPNME='rewrite'\n  NL%NXP=4\n  NL%mesh_type='earthmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%refine=.false.\n  NL%mask_restart=.true.\n/\n";
    let rewritten =
        rewrite_restart_refine_namelist_contents(input, Path::new("/tmp/case's/gridfile.nc4"))
            .expect("rewrite quoted restart path");

    let assignments = namelist_assignments(&rewritten, "mkgrd").expect("parse rewritten fields");
    assert_eq!(
        assignments
            .iter()
            .filter(|assignment| assignment.field.eq_ignore_ascii_case("mode_file"))
            .count(),
        1
    );
    let config = EarthmeshConfig::from_mkgrd_namelist(&rewritten).expect("parse rewritten mkgrd");
    assert_eq!(config.mode_file, "/tmp/case's/gridfile.nc4");
    assert_eq!(config.mode_file_description, "EarthMesh");
    assert!(!config.mask_restart);
}
