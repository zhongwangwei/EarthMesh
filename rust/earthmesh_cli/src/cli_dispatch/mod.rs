use std::env;

use super::cli_args::usage;
use super::cli_colm_netcdf::run_colm_coupling_csv_to_netcdf;
use super::cli_hydro_close::{
    run_hydro_close_mask_nmls, run_hydro_close_recipe, run_hydro_composite_close_mask_nmls,
};
use super::cli_hydro_export::{run_cama_reach_export, run_merit_hydro_geojson};
use super::cli_hydro_workflow::{
    run_coastal_band_geojson, run_colm_coupling_from_intersections, run_coupling_quality_from_mesh,
    run_gridfile_cell_polygons, run_hydro_cell_intersections, run_hydro_complete_cell_mask,
    run_hydro_delivery_manifest, run_hydro_mesh_qa, run_hydro_refinement_eval,
    run_hydro_sweep_rank, run_hydro_sweep_recipes, run_hydro_workflow, run_landtype_cell_mask,
    run_mpas_cell_polygons, run_plan_refinement_from_hydro,
};
use super::cli_mkgrd_run::run_mkgrd_or_project;
use super::cli_quality::run_mesh_quality;

pub(crate) fn run_cli_command() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let first = args
        .next()
        .ok_or_else(|| usage("missing command or mkgrd namelist path"))?;
    if first == "-h" || first == "--help" {
        println!("{}", usage(""));
        return Ok(());
    }
    if first == "--cama-reach-jsonl" || first == "--cama-reach-geojson" {
        return run_cama_reach_export(&first, args);
    }
    if first == "--merit-hydro-geojson" {
        return run_merit_hydro_geojson(args);
    }
    if first == "--hydro-close-recipe" {
        return run_hydro_close_recipe(args);
    }
    if first == "--hydro-close-mask-nmls" {
        return run_hydro_close_mask_nmls(args);
    }
    if first == "--hydro-composite-close-mask-nmls" {
        return run_hydro_composite_close_mask_nmls(args);
    }
    if first == "--colm-coupling-csv-to-netcdf" {
        return run_colm_coupling_csv_to_netcdf(args);
    }
    if first == "--colm-coupling-from-intersections" {
        return run_colm_coupling_from_intersections(args);
    }
    if first == "--hydro-mesh-qa" {
        return run_hydro_mesh_qa(args);
    }
    if first == "--hydro-refinement-eval" {
        return run_hydro_refinement_eval(args);
    }
    if first == "--hydro-sweep-recipes" {
        return run_hydro_sweep_recipes(args);
    }
    if first == "--hydro-sweep-rank" {
        return run_hydro_sweep_rank(args);
    }
    if first == "--hydro-delivery-manifest" {
        return run_hydro_delivery_manifest(args);
    }
    if first == "--hydro-cell-intersections" {
        return run_hydro_cell_intersections(args);
    }
    if first == "--hydro-complete-cell-mask" {
        return run_hydro_complete_cell_mask(args);
    }
    if first == "--coastal-band-geojson" {
        return run_coastal_band_geojson(args);
    }
    if first == "--mpas-cell-polygons" {
        return run_mpas_cell_polygons(args);
    }
    if first == "--gridfile-cell-polygons" {
        return run_gridfile_cell_polygons(args);
    }
    if first == "--landtype-cell-mask" {
        return run_landtype_cell_mask(args);
    }
    if first == "--coupling-quality-from-mesh" {
        return run_coupling_quality_from_mesh(args);
    }
    if first == "--plan-refinement-from-hydro" {
        return run_plan_refinement_from_hydro(args);
    }
    if first == "--hydro-workflow" {
        return run_hydro_workflow(args);
    }
    if first == "--mesh-quality" {
        return run_mesh_quality(args);
    }
    run_mkgrd_or_project(first, args)
}
