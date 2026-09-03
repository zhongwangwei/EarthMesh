use std::{fs, path::PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

#[test]
#[ignore = "real IGBP NXP80 ocean acceptance is the production gate"]
fn real_igbp_nxp80_ocean_tri_reverse_coarsening_passes_every_hard_gate() {
    let repository = repository_root();
    let landtype = repository.join("input/landtype_igbp_update.nc");
    assert!(landtype.exists(), "missing {}", landtype.display());
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cmrc_real_igbp_nxp80_{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    let namelist = root.join("cmrc.nml");
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='real_igbp_nxp80'\n  NL%base_dir='{}/'\n  NL%NXP=80\n  \
             NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  \
             NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  \
             NL%refine_backend='certified'\n  NL%mask_domain_global=.true.\n  \
             NL%landtype_file='{}'\n/\n\
             &mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  \
             RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  \
             RL%max_iter_cal=3\n  RL%mask_refine_cal_type='bbox'\n  \
             RL%mask_refine_cal_fprefix='none'\n  RL%refine_num_landtypes=.true.\n  \
             RL%th_num_landtypes=12\n/\n\
             &certified\n  NL%mode='reverse_coarsening'\n  NL%delivery='tri'\n  \
             NL%maximum_level=8\n  NL%maximum_cells=10000000\n  \
             NL%gradation_rings_per_level=3\n  NL%search_budget=100000\n/\n",
            root.display(),
            landtype.display()
        ),
    )
    .unwrap();

    let run =
        earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 8_192_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    assert_eq!(certified.chosen_level, 3);
    assert!(certified.mother_cells < 8_192_000);
    assert!(certified.fulfillment.delivered_level_min < 3);
    assert_eq!(certified.fulfillment.delivered_level_max, 3);
    assert_eq!(certified.physical_residuals, 0);
    assert_eq!(certified.balance_residuals, 0);
    assert_eq!(certified.topology_errors, 0);
    assert_eq!(certified.dual_errors, 0);
    assert_eq!(certified.remap_closure_errors, 0);
    assert!(run.landtype_masked_cells.is_some());
    assert_eq!(
        run.output.output.file_name().unwrap().to_str().unwrap(),
        "gridfile_NXP0080_tri_oceanmesh.nc4"
    );
    let certificate: serde_json::Value =
        serde_json::from_slice(&fs::read(&certified.certificate).unwrap()).unwrap();
    assert_eq!(
        certificate["coarsening_strategy"],
        "elastic_component_epochs"
    );
    assert_eq!(certificate["physical_residuals"], 0);
    assert_eq!(certificate["balance_residuals"], 0);
    assert_eq!(certificate["remap_closure_errors"], 0);
    assert!(
        certificate["elastic_component_epochs"]["aggregate"]["components_committed"]
            .as_u64()
            .unwrap()
            > 0
    );
    for path in [
        run.output.output,
        certified.remap,
        certified.certificate,
        certified.manifest,
        certified.resources,
        certified.ready_marker,
    ] {
        assert!(path.exists(), "missing {}", path.display());
    }
    println!(
        "CMRC_ACCEPTANCE_DIR={}",
        root.join("real_igbp_nxp80/result").display()
    );
}
