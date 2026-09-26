use std::fs;
use std::io;
use std::path::Path;

/// Build one composite close-mask recipe JSON for an (r2_cap, coast_cap) sweep case
/// (faithful to `refinement_sweep.py::build_river_coast_sweep`).
fn build_sweep_recipe_json(
    river_geojson: &str,
    coast_geojson: &str,
    r2_cap: i64,
    coast_cap: i64,
    r3_cap: i64,
) -> String {
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "{{\n  \"max_masks_per_refine_degree\": 999,\n  \"components\": [\n    \
         {{\"name\": \"coastline_support\", \"input_geojson\": \"{coast}\", \
         \"class_refine\": {{\"COAST\": 1}}, \"max_rings_by_class\": {{\"COAST\": {coast_cap}}}, \
         \"simplify_tolerance_deg\": 0.005}},\n    \
         {{\"name\": \"ranked_river_corridors\", \"input_geojson\": \"{river}\", \
         \"class_refine\": {{\"R2\": 1, \"R3\": 3}}, \
         \"max_rings_by_class\": {{\"R2\": {r2_cap}, \"R3\": {r3_cap}}}, \
         \"buffer_deg_by_refine_degree\": {{\"1\": 1.5, \"2\": 1.0, \"3\": 0.5}}, \
         \"simplify_tolerance_deg\": 0.005}}\n  ]\n}}\n",
        coast = esc(coast_geojson),
        river = esc(river_geojson),
    )
}

/// Faithful port of `refinement_sweep.py::write_sweep_recipes`: write one recipe JSON
/// per (r2_cap, coast_cap) case + a `sweep_manifest.json`.
pub fn write_sweep_recipes(
    output_dir: impl AsRef<Path>,
    river_geojson: &str,
    coast_geojson: &str,
    mut r2_caps: Vec<i64>,
    mut coast_caps: Vec<i64>,
    r3_cap: i64,
) -> io::Result<usize> {
    let dir = output_dir.as_ref();
    fs::create_dir_all(dir)?;
    r2_caps.sort_unstable();
    coast_caps.sort_unstable();
    let esc = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
    let mut manifest_cases = Vec::new();
    for &r2_cap in &r2_caps {
        for &coast_cap in &coast_caps {
            let case_name = format!("r2cap{r2_cap}_coast{coast_cap}");
            let recipe_path = dir.join(format!("{case_name}_recipe.json"));
            fs::write(
                &recipe_path,
                build_sweep_recipe_json(river_geojson, coast_geojson, r2_cap, coast_cap, r3_cap),
            )?;
            manifest_cases.push(format!(
                "    {{\"case_name\": \"{case_name}\", \"r2_cap\": {r2_cap}, \"coast_cap\": {coast_cap}, \"recipe_json\": \"{}\"}}",
                esc(&recipe_path.display().to_string())
            ));
        }
    }
    let case_count = manifest_cases.len();
    let ints = |v: &[i64]| {
        v.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let manifest = format!(
        "{{\n  \"kind\": \"earthmesh_refinement_sweep_manifest\",\n  \"case_count\": {},\n  \
         \"river_geojson\": \"{}\",\n  \"coast_geojson\": \"{}\",\n  \"r2_caps\": [{}],\n  \
         \"coast_caps\": [{}],\n  \"r3_cap\": {},\n  \"cases\": [\n{}\n  ]\n}}\n",
        case_count,
        esc(river_geojson),
        esc(coast_geojson),
        ints(&r2_caps),
        ints(&coast_caps),
        r3_cap,
        manifest_cases.join(",\n"),
    );
    fs::write(dir.join("sweep_manifest.json"), manifest)?;
    Ok(case_count)
}
