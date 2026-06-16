use std::fs;

use earthmesh_cli::{
    plan_mkgrd_refine_loop_io, read_contain_netcdf, write_bbox_mask_netcdf,
    write_unstructured_mesh_netcdf, BBoxMask, BBoxPoint, LonLatPoint,
    MkgrdSpecifiedRefineSourceExecutor, MkgrdSpecifiedRefineSourceExecutorOptions,
    UnstructuredMesh,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};

fn mkgrd_config(base_dir: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='case_spc_source'\n  NL%base_dir='{base_dir}'\n  NL%NXP=4\n  NL%mesh_type='landmesh'\n  NL%mode_grid='tri'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n/\n",
        "landmesh",
        "tri",
    )
    .expect("parse refine config")
}

#[test]
fn specified_refine_source_executor_runs_area_contain_and_getref_files() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mkgrd_specified_refine_source_executor_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gridfile")).expect("create gridfile dir");
    fs::create_dir_all(root.join("tmpfile")).expect("create tmpfile dir");
    let base_dir = format!("{}/", root.display());
    let mkgrd = mkgrd_config(&base_dir);
    let refine = refine_config();
    let plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine io");
    let step = &plan.steps[0];
    let source = &step.sources[0];

    write_bbox_mask_netcdf(
        root.join("case_spc_source/tmpfile/mask_refine_bbox_1_01.nc4"),
        &BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write specified bbox");
    write_unstructured_mesh_netcdf(
        &step.refine_loop_input_gridfile,
        &UnstructuredMesh {
            m_points: vec![LonLatPoint {
                lon: -178.5,
                lat: 88.5,
            }],
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
            m_to_w: vec![[1, 2, 3]],
            w_to_m: vec![vec![1], vec![1], vec![1]],
            n_w_to_m: vec![1, 1, 1],
        },
    )
    .expect("write current tri gridfile");

    let lon_i = vec![f64::NAN, -179.5, -178.5, -177.5, -176.5, -175.5, -174.5];
    let lat_i = vec![f64::NAN, 89.5, 88.5, 87.5, 86.5, 85.5, 84.5];
    let lon_vertex = vec![
        f64::NAN,
        -180.0,
        -179.0,
        -178.0,
        -177.0,
        -176.0,
        -175.0,
        -174.0,
    ];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 84.0];
    let is_in_domain = vec![vec![0; lat_i.len()]; lon_i.len()]
        .into_iter()
        .enumerate()
        .map(|(lon, mut row)| {
            if lon > 0 {
                for value in row.iter_mut().skip(1) {
                    *value = 1;
                }
            }
            row
        })
        .collect::<Vec<_>>();
    let mut seaorland = vec![vec![0; lat_i.len()]; lon_i.len()];
    seaorland[1][1] = 1;
    seaorland[1][2] = 1;
    seaorland[2][1] = 0;
    seaorland[2][2] = 1;

    let runner =
        MkgrdSpecifiedRefineSourceExecutor::new(MkgrdSpecifiedRefineSourceExecutorOptions {
            file_dir: &plan.file_dir,
            mesh_type: &plan.mesh_type,
            mask_refine_spc_type: "bbox",
            mask_refine_ndm: 1,
            mask_refine_ndm_by_iter: &[0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            is_in_domain: &is_in_domain,
            seaorland: &seaorland,
            lon_vertex: &lon_vertex,
            lat_vertex: &lat_vertex,
            lon_i: &lon_i,
            lat_i: &lat_i,
            gridnum_perdegree: 1,
            nlons_source: 6,
            nlats_source: 6,
            num_vertex: 0,
        });

    let report = runner
        .run_source_branch_report(step, source)
        .expect("run specified source branch");

    assert_eq!(report.area.refine_write.output, source.area_judge_output);
    assert!(source.area_judge_output.exists());
    assert_eq!(report.contain.output, source.contain_output);
    assert_eq!(report.contain.contained_source_pixels, 1);
    assert_eq!(
        report.specified_threshold.output,
        source.specified_threshold_output.as_ref().unwrap().clone()
    );

    let contain = read_contain_netcdf(&source.contain_output).expect("read contain");
    assert_eq!(contain.is_in_area_ustr, vec![0, 1]);
    let threshold = netcdf::open(source.specified_threshold_output.as_ref().unwrap())
        .expect("open specified threshold");
    let values = threshold
        .variable("IsInRfArea_sjx_specified")
        .expect("threshold var")
        .get_values::<i32, _>(..)
        .expect("read specified threshold");
    assert_eq!(values, vec![1]);

    let _ = fs::remove_dir_all(&root);
}
