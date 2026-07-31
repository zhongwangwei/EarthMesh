use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, RefineConfig};
use earthmesh_quality::{HfieldConfigDiagnostics, MeshQualityReport, QualityMeshInput};

use crate::hfield_refine::read_hfield_refine_options;

use super::gridfile::read_gridfile_mesh_points;
use super::hfield_support_coverage::target_levels_with_hard_coverage;
use crate::{
    namelist_has_section, native_grid_refinement_requested, native_spawn_uses_cartesian_xy,
    read_native_grid_mdomain, read_native_grid_refine_controls, GridfileMeshPoints,
};

/// Attach h-field diagnostics to a mesh-quality report when `namelist_contents`
/// is a full mkgrd/mkrefine/hfield namelist. Plain `&quality` files return
/// `Ok(false)` and keep the compatibility report shape.
pub fn attach_hfield_diagnostics_from_namelist(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    mesh: &GridfileMeshPoints,
    kind: &str,
    namelist_contents: &str,
) -> io::Result<bool> {
    attach_hfield_diagnostics(report, input, mesh, None, kind, namelist_contents)
}

pub fn attach_hfield_diagnostics_from_namelist_for_gridfile(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    mesh: &GridfileMeshPoints,
    gridfile: impl AsRef<Path>,
    kind: &str,
    namelist_contents: &str,
) -> io::Result<bool> {
    attach_hfield_diagnostics(
        report,
        input,
        mesh,
        Some(gridfile.as_ref()),
        kind,
        namelist_contents,
    )
}

fn attach_hfield_diagnostics(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    _mesh: &GridfileMeshPoints,
    gridfile: Option<&Path>,
    kind: &str,
    namelist_contents: &str,
) -> io::Result<bool> {
    if !namelist_has_section(namelist_contents, "mkgrd") {
        return Ok(false);
    }

    let config = EarthmeshConfig::from_mkgrd_namelist(namelist_contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NL%NXP must fit usize"))?;
    if nxp == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NL%NXP must be positive for h-field diagnostics",
        ));
    }
    let Some(hfield) = read_hfield_refine_options(namelist_contents)? else {
        return Ok(false);
    };
    let native_mdomain = read_native_grid_mdomain(namelist_contents)?;
    let native_requested =
        native_grid_refinement_requested(namelist_contents, config.mesh_type.trim())?;
    let refine = RefineConfig::from_mkrefine_namelist_with_external_field(
        namelist_contents,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
        hfield.hydro_target_paths().is_some(),
    )
    .or_else(|_| {
        read_native_grid_refine_controls(namelist_contents).map_err(|error| error.to_string())
    })
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let native_only = native_requested && !refine.refine_spc && !refine.refine_cal;
    if native_spawn_uses_cartesian_xy(native_mdomain, config.mask_domain_global, native_only)
        || native_mdomain == Some(5)
    {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Cartesian-XY HField source-demand snapshots are not implemented; refusing to silently skip or rebuild quality targets",
        ));
    }

    let inferred_gridfile = gridfile.is_none().then(|| {
        PathBuf::from(config.file_dir())
            .join("result")
            .join(format!(
                "gridfile_NXP{nxp:04}_{}.nc4",
                config.mode_grid.trim()
            ))
    });
    let gridfile = gridfile
        .or(inferred_gridfile.as_deref())
        .expect("an explicit or inferred gridfile always exists");
    let demand =
        crate::source_demand_artifact::load_hfield_source_demand(gridfile, namelist_contents)?;
    let _demand_chain_identity = (demand.snapshot_hash, demand.chain_tip_hash);
    match kind.trim() {
        "tri" | "hex" => {}
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("h-field quality diagnostics support tri or hex view, got {other}"),
            ));
        }
    }
    let (target_levels, coverage) = target_levels_with_hard_coverage(
        input,
        demand.nlon,
        demand.nlat,
        &demand.hard_levels,
        &demand.hard_levels,
        &demand.intended_output_support,
    )?;

    earthmesh_quality::attach_hfield_diagnostics(
        report,
        input,
        &target_levels,
        HfieldConfigDiagnostics {
            enabled: true,
            g: Some(demand.g),
            max_level: Some(u32::from(demand.max_level)),
            base_m: Some(demand.base_m),
        },
    );
    earthmesh_quality::attach_hfield_support_coverage(
        report,
        coverage.active_bin_count,
        coverage.adequately_covered_bin_count,
    );
    Ok(true)
}

pub fn attach_hfield_diagnostics_from_gridfile_namelist(
    report: &mut MeshQualityReport,
    input: &QualityMeshInput,
    gridfile: impl AsRef<std::path::Path>,
    kind: &str,
    namelist_contents: &str,
) -> io::Result<bool> {
    let gridfile = gridfile.as_ref();
    let mesh = read_gridfile_mesh_points(gridfile)?;
    attach_hfield_diagnostics_from_namelist_for_gridfile(
        report,
        input,
        &mesh,
        gridfile,
        kind,
        namelist_contents,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn diagnostics_require_and_use_the_persisted_hydro_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_hydro_hfield_diagnostics_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cells = root.join("cells.geojson");
        let plan = root.join("plan.json");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"cell_id":"1","center_lon":0.3,"center_lat":0.3},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[0,1],[0,0]]]}}]}"#,
        )
        .unwrap();
        fs::write(
            &plan,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":0,"cell_id":"1","target_level":1}]}"#,
        )
        .unwrap();
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.3],
            m_lat: vec![0.3],
            w_lon: vec![0.0, 1.0, 0.0],
            w_lat: vec![0.0, 0.0, 1.0],
            m_to_w: vec![1, 2, 3],
            m_refine_level: vec![1],
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };
        let input = super::super::gridfile::quality_input_from_gridfile(&mesh).unwrap();
        let mut report =
            earthmesh_quality::compute(&input, &earthmesh_quality::QualityThresholds::default());
        let namelist = format!(
            "&mkgrd\n NL%EXPNME='case'\n NL%base_dir='{}/'\n NL%NXP=4\n NL%mesh_type='landmesh'\n NL%mode_grid='tri'\n NL%output_format='CoLM'\n NL%refine=.true.\n NL%mask_domain_global=.true.\n/\n&mkrefine\n RL%SpringGlobal_type=1\n RL%refine_spc=.false.\n RL%refine_cal=.false.\n/\n&hfield\n NL%hfield_on=.true.\n NL%hfield_g=0.2\n NL%hfield_max_level=1\n NL%hfield_base_m=100.0\n NL%hfield_nlon=36\n NL%hfield_nlat=18\n NL%hfield_target_cells_geojson='{}'\n NL%hfield_target_levels_json='{}'\n/\n",
            root.display(),
            cells.display(),
            plan.display()
        );

        let missing =
            attach_hfield_diagnostics_from_namelist(&mut report, &input, &mesh, "tri", &namelist)
                .unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);

        let expected_gridfile = root.join("case/result/gridfile_NXP0004_tri.nc4");
        fs::create_dir_all(expected_gridfile.parent().unwrap()).unwrap();
        fs::write(&expected_gridfile, b"final-gridfile").unwrap();
        let mut field = earthmesh_hfield::HField::uniform(36, 18, 100.0).unwrap();
        field.set(0, 0, 50.0);
        crate::source_demand_artifact::PreparedHfieldDemand::capture(
            &field, 100.0, 1, 0.2, &namelist,
        )
        .unwrap()
        .persist_for_gridfile(&expected_gridfile)
        .unwrap();
        assert!(attach_hfield_diagnostics_from_namelist(
            &mut report,
            &input,
            &mesh,
            "tri",
            &namelist,
        )
        .unwrap());
        assert_eq!(report.hfield.as_ref().unwrap().config.g, Some(0.2));
        assert!(report.gates.iter().any(|gate| {
            gate.metric == "hfield_uncovered_hard_support_bin_count"
                && gate.value == 1.0
                && gate.level == earthmesh_quality::QualityLevel::Fail
        }));
        let _ = fs::remove_dir_all(root);
    }
}
