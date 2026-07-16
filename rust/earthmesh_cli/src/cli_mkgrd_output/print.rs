use earthmesh_cli::mkgrd_run_types::MkgrdTopLevelDispatchRunReport;

pub(crate) fn print_top_level_dispatch_report(report: &MkgrdTopLevelDispatchRunReport) {
    match report {
        MkgrdTopLevelDispatchRunReport::Gridinit(report) => {
            println!("gridfile={}", report.gridfile.output.display());
            if let Some(fvcom_2dm) = &report.fvcom_2dm {
                println!("fvcom_2dm={}", fvcom_2dm.output.display());
            }
            println!("sjx_points={}", report.gridfile.sjx_points);
            println!("lbx_points={}", report.gridfile.lbx_points);
        }
        MkgrdTopLevelDispatchRunReport::RefinePipeline(report) => {
            print_refine_pipeline_report(report);
        }
        MkgrdTopLevelDispatchRunReport::MaskRestartPatch(report) => {
            print_mask_restart_patch_report(report);
        }
        MkgrdTopLevelDispatchRunReport::MaskRestartOcean(report) => {
            print_mask_restart_ocean_report(report);
        }
        MkgrdTopLevelDispatchRunReport::MaskRestartAreaJudge(report) => {
            print_mask_restart_area_judge_report(report);
        }
        MkgrdTopLevelDispatchRunReport::MaskRestartPlan(report) => {
            println!("mask_restart_action={:?}", report.remask.action);
            println!("mask_restart_step={}", report.remask.step);
            println!("mask_restart_file_dir={}", report.remask.file_dir.display());
        }
    }
}

pub(crate) fn print_mask_restart_patch_report(
    report: &earthmesh_cli::mkgrd_restart_types::MkgrdMaskRestartPatchRunReport,
) {
    println!("mask_restart_action={:?}", report.plan.remask.action);
    println!(
        "mask_patch_reports={}",
        report.workspace_mask.mask_reports.len()
    );
    println!(
        "mask_patch_ndm={}",
        report.workspace_mask.mask_counts.mask_patch_ndm[0]
    );
}

pub(crate) fn print_mask_restart_ocean_report(
    report: &earthmesh_cli::mkgrd_restart_types::MkgrdMaskRestartOceanRunReport,
) {
    println!("mask_restart_action={:?}", report.plan.remask.action);
    println!(
        "mask_postproc_result_gridfile={}",
        report.postproc.final_gridfile.output.display()
    );
    if let Some(obc) = &report.postproc.obc {
        println!("mask_postproc_obc={}", obc.output.display());
    }
    if let Some(obcv2) = &report.postproc.obcv2 {
        println!("mask_postproc_obcv2={}", obcv2.output.display());
    }
}

pub(crate) fn print_mask_restart_area_judge_report(
    report: &earthmesh_cli::mkgrd_restart_types::MkgrdRestartAreaJudgeGlobalSourceRunReport,
) {
    let restart = &report.restart;
    println!("mask_restart_action={:?}", restart.plan.remask.action);
    println!(
        "mask_patch_reports={}",
        restart.workspace_mask.mask_reports.len()
    );
    println!(
        "mask_restart_area_selected_cells={}",
        restart.area_write.selected_cells
    );
    println!(
        "mask_restart_area_grid={}",
        restart.area_write.output.display()
    );
    if let Some(postproc_report) = &report.postproc {
        println!(
            "mask_restart_contain={}",
            postproc_report.contain.output.display()
        );
        match &postproc_report.postproc {
            earthmesh_cli::mkgrd_restart_types::MkgrdFinalDomainPostprocReport::Earth(postproc) => {
                println!(
                    "mask_restart_postproc_gridfile={}",
                    postproc.final_gridfile.output.display()
                );
                println!(
                    "mask_restart_postproc_patchtype={}",
                    postproc.patchtype.output.display()
                );
                println!(
                    "mask_restart_postproc_earthmesh_info={}",
                    postproc.earthmesh_info.output.display()
                );
            }
            earthmesh_cli::mkgrd_restart_types::MkgrdFinalDomainPostprocReport::Land(postproc) => {
                println!(
                    "mask_restart_postproc_gridfile={}",
                    postproc.final_gridfile.output.display()
                );
                println!(
                    "mask_restart_postproc_patchtype={}",
                    postproc.patchtype.output.display()
                );
            }
            earthmesh_cli::mkgrd_restart_types::MkgrdFinalDomainPostprocReport::Ocean(postproc) => {
                println!(
                    "mask_restart_postproc_gridfile={}",
                    postproc.final_gridfile.output.display()
                );
                if let Some(obc) = &postproc.obc {
                    println!("mask_restart_postproc_obc={}", obc.output.display());
                }
                if let Some(obcv2) = &postproc.obcv2 {
                    println!("mask_restart_postproc_obcv2={}", obcv2.output.display());
                }
            }
            earthmesh_cli::mkgrd_restart_types::MkgrdFinalDomainPostprocReport::Atmos(postproc) => {
                println!(
                    "mask_restart_postproc_mpas_simple={}",
                    postproc.output.display()
                );
            }
            earthmesh_cli::mkgrd_restart_types::MkgrdFinalDomainPostprocReport::AtmosFull(
                postproc,
            ) => {
                println!(
                    "mask_restart_postproc_mpas={}",
                    postproc.mesh.output.display()
                );
                println!(
                    "mask_restart_postproc_mpas_graph={}",
                    postproc.graph_info.output.display()
                );
            }
        }
    }
}

pub(crate) fn print_refine_pipeline_report(
    report: &earthmesh_cli::mkgrd_run_types::RefinePipelineRunReport,
) {
    println!("refine_source=refine_pipeline");
    println!("refine_stack={}", report.refine_stack());
    println!("gridfile={}", report.output.output.display());
    println!("sjx_points={}", report.output.sjx_points);
    println!("lbx_points={}", report.output.lbx_points);
    println!("refine_regions={}", report.regions.len());
    println!("refine_max_level={}", report.max_level);
    println!("refine_transition_faces={}", report.transition_faces);
    println!("refine_spring_passes={}", report.spring_nest_passes);
    println!("refine_spring_iterations={}", report.spring_nest_iterations);
    if let Some(raw_output) = &report.raw_output {
        println!("refine_raw_gridfile={}", raw_output.output.display());
    }
    if let Some(landtype_masked_cells) = report.landtype_masked_cells {
        println!("refine_landtype_masked_cells={landtype_masked_cells}");
    }
    if let Some(coupled) = &report.coupled_outputs {
        println!(
            "refine_land_gridfile={}",
            coupled.land_output.output.display()
        );
        println!(
            "refine_ocean_gridfile={}",
            coupled.ocean_output.output.display()
        );
        println!("refine_coupling_csv={}", coupled.coupling_csv.display());
        println!(
            "refine_coupling_netcdf={}",
            coupled.coupling_netcdf.output.display()
        );
        println!(
            "refine_coupling_quality={}",
            coupled.coupling_quality.display()
        );
        println!("refine_coupling_manifest={}", coupled.manifest.display());
        println!("refine_coupling_rows={}", coupled.coupling_netcdf.rows);
    }
}
