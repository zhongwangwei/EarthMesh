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
    println!("refine_realized_max_level={}", report.realized_max_level);
    // Measured off the mesh, so it means the same thing whichever backend made
    // it -- unlike the line above, which means face generations, completed
    // passes, site generations, or nothing measured, depending on the backend.
    if report.finest_cell_km > 0.0 {
        println!("refine_finest_cell_km={:.3}", report.finest_cell_km);
        println!("refine_coarsest_cell_km={:.3}", report.coarsest_cell_km);
        println!(
            "refine_realized_halvings={:.2}",
            (report.coarsest_cell_km / report.finest_cell_km).log2()
        );
        // What the requested level actually delivered, in the region that
        // asked for it. The line above spans the globe and is not a level.
        println!(
            "refine_realized_region_halvings={:.2}",
            report.realized_region_halvings
        );
    }
    let hfield = report.hfield_diagnostics;
    if hfield.requested_anchor_count > 0 {
        println!(
            "refine_hfield_requested_anchors={}",
            hfield.requested_anchor_count
        );
        println!(
            "refine_hfield_covered_anchors={}",
            hfield.covered_anchor_count
        );
        println!(
            "refine_hfield_boundary_clipped_anchors={}",
            hfield.boundary_clipped_anchor_count
        );
        println!(
            "refine_hfield_demanded_faces={}",
            hfield.demanded_face_count
        );
        println!("refine_hfield_unmet_faces={}", hfield.unmet_face_count);
    }
    // An empty demand legitimately produces no refinement, so a shortfall is
    // only worth reporting when the field actually asked for something that
    // Method-C then clipped away.
    if report.realized_max_level < report.max_level && hfield.boundary_clipped_anchor_count > 0 {
        eprintln!(
            "earthmesh_cli: warning: refinement reached level {} of the {} requested; \
             {} of {} h-field demand anchors were clipped for lacking a legal rad3 footprint",
            report.realized_max_level,
            report.max_level,
            hfield.boundary_clipped_anchor_count,
            hfield.requested_anchor_count
        );
    }
    println!("refine_transition_faces={}", report.transition_faces);
    println!("refine_spring_passes={}", report.spring_nest_passes);
    // Printed for HARP-DV only, because the other two backends have no cycles,
    // no refusals and no stop reason to report. A line of `None` for them would
    // be noise that trains people to skip the line that matters.
    if let Some(harp) = &report.harp_dv_run {
        println!("harp_dv_stop_reason={}", harp.stop_reason);
        println!("harp_dv_cycles={}", harp.cycles_completed);
        println!(
            "harp_dv_transactions_committed={}",
            harp.transactions_committed
        );
        println!(
            "harp_dv_fallback_transactions_committed={}",
            harp.fallback_transactions_committed
        );
        println!("harp_dv_r_adaptation_moves={}", harp.r_adaptation_moves);
        println!(
            "harp_dv_paired_r_adaptation_moves={}",
            harp.paired_r_adaptation_moves
        );
        println!("harp_dv_unresolved_cells={}", harp.unresolved_cells);
        println!(
            "harp_dv_physical_demands_remaining={}",
            harp.physical_demands_remaining
        );
        println!(
            "harp_dv_balance_demands_remaining={}",
            harp.balance_demands_remaining
        );
        println!(
            "harp_dv_quality_constrained_cells={}",
            harp.quality_constrained_cells
        );
        println!(
            "harp_dv_unbalanced_pairs={}",
            harp.unbalanced_pairs_remaining
        );
    }
    if let Some(lepp) = &report.lepp_adaptive_hybrid {
        println!("lepp_adaptive_stop_reason={}", lepp.stop_reason);
        println!("lepp_adaptive_cycles={}", lepp.cycles);
        println!(
            "lepp_adaptive_physical_insertions={}",
            lepp.physical_insertions
        );
        println!(
            "lepp_adaptive_balance_insertions={}",
            lepp.balance_insertions
        );
        println!(
            "lepp_adaptive_quality_insertions={}",
            lepp.quality_insertions
        );
        println!(
            "lepp_adaptive_boundary_insertions={}",
            lepp.boundary_insertions
        );
        println!(
            "lepp_adaptive_unresolved_demands={}",
            lepp.unresolved_demands
        );
        println!("lepp_adaptive_report={}", lepp.report.display());
        println!(
            "lepp_adaptive_unresolved_report={}",
            lepp.unresolved_report.display()
        );
    }
    if let Some(lepp) = &report.lepp_post_quality {
        println!("lepp_post_quality_stop_reason={}", lepp.stop_reason);
        println!("lepp_post_quality_attempted={}", lepp.attempted);
        println!("lepp_post_quality_committed={}", lepp.committed);
        println!("lepp_post_quality_rejected={}", lepp.rejected);
        println!(
            "lepp_post_quality_violations_before={}",
            lepp.violations_before
        );
        println!(
            "lepp_post_quality_violations_after={}",
            lepp.violations_after
        );
        println!(
            "lepp_post_quality_worst_violation_before={}",
            lepp.worst_violation_before
        );
        println!(
            "lepp_post_quality_worst_violation_after={}",
            lepp.worst_violation_after
        );
        println!(
            "lepp_post_quality_gridfile={}",
            lepp.output.output.display()
        );
        println!("lepp_post_quality_report={}", lepp.report.display());
        if let Some(raw_output) = &lepp.raw_output {
            println!(
                "lepp_post_quality_raw_gridfile={}",
                raw_output.output.display()
            );
        }
        if let Some(masked_cells) = lepp.landtype_masked_cells {
            println!("lepp_post_quality_landtype_masked_cells={masked_cells}");
        }
        if let Some(coupled) = &lepp.coupled_outputs {
            print_coupled_outputs("lepp_post_quality", coupled);
        }
    }
    println!("refine_spring_iterations={}", report.spring_nest_iterations);
    if let Some(raw_output) = &report.raw_output {
        println!("refine_raw_gridfile={}", raw_output.output.display());
    }
    if let Some(landtype_masked_cells) = report.landtype_masked_cells {
        println!("refine_landtype_masked_cells={landtype_masked_cells}");
    }
    if let Some(coupled) = &report.coupled_outputs {
        print_coupled_outputs("refine", coupled);
    }
}

fn print_coupled_outputs(
    prefix: &str,
    coupled: &earthmesh_cli::mkgrd_run_types::RefineCoupledOutputReport,
) {
    println!(
        "{prefix}_land_gridfile={}",
        coupled.land_output.output.display()
    );
    println!(
        "{prefix}_ocean_gridfile={}",
        coupled.ocean_output.output.display()
    );
    println!("{prefix}_coupling_csv={}", coupled.coupling_csv.display());
    println!(
        "{prefix}_coupling_netcdf={}",
        coupled.coupling_netcdf.output.display()
    );
    println!(
        "{prefix}_coupling_quality={}",
        coupled.coupling_quality.display()
    );
    println!("{prefix}_coupling_manifest={}", coupled.manifest.display());
    println!("{prefix}_coupling_rows={}", coupled.coupling_netcdf.rows);
}
