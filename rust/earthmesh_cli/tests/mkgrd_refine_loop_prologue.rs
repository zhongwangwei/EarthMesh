use std::fs;

use earthmesh_cli::{
    plan_mkgrd_refine_loop_io, run_mkgrd_refine_loop_prologue_snapshot,
    write_unstructured_mesh_netcdf, LonLatPoint, UnstructuredMesh,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};

fn mkgrd_config(base_dir: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_refine_prologue'\n  NL%base_dir='{base_dir}'\n  NL%NXP=4\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.false.\n  RL%refine_cal=.true.\n  RL%max_iter_spc=0\n  RL%max_iter_cal=1\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_cal_type='bbox'\n  RL%refine_num_landtypes=.true.\n/\n",
        "landmesh",
        "tri",
    )
    .expect("parse refine config")
}

#[test]
fn refine_loop_prologue_reads_gridfile_and_copies_original_snapshot() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_refine_loop_prologue_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine io");
    let step = &plan.steps[0];

    write_unstructured_mesh_netcdf(
        &step.refine_loop_input_gridfile,
        &UnstructuredMesh {
            m_points: vec![
                LonLatPoint {
                    lon: -178.5,
                    lat: 88.5,
                },
                LonLatPoint {
                    lon: -177.5,
                    lat: 88.5,
                },
            ],
            w_points: vec![
                LonLatPoint {
                    lon: -179.5,
                    lat: 87.5,
                },
                LonLatPoint {
                    lon: -177.5,
                    lat: 87.5,
                },
                LonLatPoint {
                    lon: -179.5,
                    lat: 89.5,
                },
            ],
            m_to_w: vec![[1, 2, 3], [1, 3, 2]],
            w_to_m: vec![vec![1, 2], vec![1, 2], vec![1, 2]],
            n_w_to_m: vec![2, 2, 2],
        },
    )
    .expect("write current tri gridfile");

    let report = run_mkgrd_refine_loop_prologue_snapshot(step).expect("run prologue snapshot");

    assert_eq!(report.input_gridfile, step.refine_loop_input_gridfile);
    assert_eq!(report.original_tmpfile, step.refine_loop_original_tmpfile);
    assert_eq!(report.sjx_points, 2);
    assert_eq!(report.lbx_points, 3);
    assert!(report.copied_bytes > 0);
    assert_eq!(
        fs::read(&step.refine_loop_original_tmpfile).expect("read original snapshot"),
        fs::read(&step.refine_loop_input_gridfile).expect("read input gridfile")
    );

    let _ = fs::remove_dir_all(&root);
}
