use std::{fs, path::PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

#[test]
#[ignore = "real IGBP NXP80 acceptance takes several minutes"]
fn real_igbp_nxp80_coupled_safe_mother_passes_every_hard_gate() {
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
             NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  \
             NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  \
             NL%refine_backend='certified'\n  NL%mask_domain_global=.true.\n  \
             NL%landtype_file='{}'\n/\n\
             &mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  \
             RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  \
             RL%max_iter_cal=1\n  RL%mask_refine_cal_type='bbox'\n  \
             RL%mask_refine_cal_fprefix='none'\n  RL%refine_num_landtypes=.true.\n  \
             RL%th_num_landtypes=1\n/\n\
             &certified\n  NL%mode='safe_mother_only'\n  NL%delivery='coupled'\n  \
             NL%maximum_level=1\n  NL%maximum_cells=1000000\n  \
             NL%gradation_rings_per_level=3\n  NL%search_budget=100\n/\n",
            root.display(),
            landtype.display()
        ),
    )
    .unwrap();

    let run =
        earthmesh_cli::run_refine_pipeline_namelist(&namelist, &root, 1_000_000, None).unwrap();
    let certified = run.certified_run.unwrap();
    assert_eq!(certified.chosen_level, 1);
    assert_eq!(certified.mother_subdivision, 160);
    assert_eq!(certified.mother_cells, 512_000);
    assert_eq!(certified.physical_residuals, 0);
    assert_eq!(certified.balance_residuals, 0);
    assert_eq!(certified.topology_errors, 0);
    assert_eq!(certified.dual_errors, 0);
    assert_eq!(certified.remap_closure_errors, 0);
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
