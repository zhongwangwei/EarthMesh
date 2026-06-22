//! End-to-end refine integration: a real specified (bbox) refinement with global
//! spring smoothing must produce a refined mesh that (a) actually adds cells and
//! (b) builds into a topologically-consistent closed-sphere MPAS mesh (χ=2).
//!
//! This drives the same entry point the GUI uses
//! (`run_mkgrd_top_level_namelist_with_default_restart_refine_handoff`) via the
//! land-type refine source path, so it needs a real land-type NetCDF. It is
//! ignored by default because the global land-type read dominates runtime
//! (~1-2 min) and depends on machine-local fixtures. Run with `make test-slow`.

use std::fs;
use std::path::PathBuf;

fn landtype_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("EARTHMESH_LANDTYPE") {
        let p = PathBuf::from(p);
        return p.exists().then_some(p);
    }
    let default = PathBuf::from(
        "/Users/zhongwangwei/Desktop/EarthMesh_legacy_archive_20260616_142611/input/landtype_usgs_update.nc",
    );
    default.exists().then_some(default)
}

#[test]
#[ignore = "slow local-fixture refine topology smoke; run with make test-slow"]
fn specified_bbox_refine_produces_consistent_closed_mpas() {
    let Some(landtype) = landtype_path() else {
        eprintln!("skip: no land-type NetCDF (set EARTHMESH_LANDTYPE)");
        return;
    };
    const NXP: usize = 6;
    let root = std::env::temp_dir().join("em_refine_e2e_topology");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let sources = root.join("sources");
    fs::create_dir_all(&sources).unwrap();
    // Specified-refine bbox region (per degree 1 and 2 for max_iter_spc=2).
    // Level 1 intentionally includes a parent halo around the level-2 target;
    // otherwise Fortran Method-C rejects the child transition as too close to
    // the parent boundary in perim_fill3.
    for deg in [1usize, 2] {
        let parent_halo = if deg == 1 { 30.0 } else { 0.0 };
        earthmesh_cli::write_bbox_mask_netcdf(
            sources.join(format!("refine_0{deg}.nc4")),
            &earthmesh_cli::BBoxMask {
                refine_degree: deg,
                points: vec![earthmesh_cli::BBoxPoint {
                    west: 0.0 - parent_halo,
                    east: 40.0 + parent_halo,
                    north: 50.0 + parent_halo,
                    south: 20.0 - parent_halo,
                }],
            },
        )
        .unwrap();
    }
    let base_dir = format!("{}/", root.display());
    let refine_prefix = sources.join("refine_").display().to_string();
    let nml = root.join("refine.nml");
    fs::write(
        &nml,
        format!(
            "&mkgrd\n  NL%EXPNME='rr'\n  NL%base_dir='{base_dir}'\n  NL%NXP={NXP}\n  NL%mesh_type='atmosmesh'\n  NL%mode_grid='hex'\n  NL%mode_file='none'\n  NL%mode_file_description='none'\n  NL%refine=.true.\n  NL%niter=0\n  NL%beta=1.0\n  NL%relax=0.035\n  NL%gridnum_perdegree=120\n  NL%landtype_file='{landtype}'\n  NL%mask_domain_global=.true.\n  NL%mask_domain_type='circle'\n  NL%mask_patch_on=.false.\n  NL%output_format='MPAS'\n/\n&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=1\n  RL%num_rc=1\n  RL%set_dis_type='linear'\n  RL%vertex_pretect_layers=15\n  RL%SpringRegional_type=0\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=2\n  RL%max_iter_cal=0\n  RL%niter_refine=50\n  RL%halo=4,4,3\n  RL%max_transition_row=4,4,3\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_spc_fprefix='{refine_prefix}'\n/\n",
            landtype = landtype.display(),
        ),
    )
    .unwrap();

    earthmesh_cli::run_mkgrd_top_level_namelist_with_default_restart_refine_handoff(
        &nml, &root, 200_000, 0, None, None, None, 1, None,
    )
    .expect("engine refine run");

    let gf = root
        .join("rr/result")
        .join(format!("gridfile_NXP{NXP:04}_hex.nc4"));
    let mesh = earthmesh_cli::read_unstructured_mesh_netcdf(&gf).unwrap();
    // Base NXP6 atmos hex is ~362 cells; refinement must have added cells.
    assert!(
        mesh.w_points.len() > 400,
        "expected refinement to add cells, got {}",
        mesh.w_points.len()
    );

    let cw = vec![100.0f64; mesh.w_points.len()];
    let mpas = earthmesh_cli::build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cw, NXP, 1)
        .expect("build MPAS from refined mesh");
    let r = earthmesh_cli::check_mpas_mesh_topology(&mpas);
    assert!(
        r.is_consistent(),
        "violations: {:?}",
        &r.violations[..r.violations.len().min(8)]
    );
    assert_eq!(
        r.euler_characteristic, 2,
        "refined global mesh must be a closed sphere"
    );
    assert!(r.is_closed);

    let _ = fs::remove_dir_all(&root);
}
