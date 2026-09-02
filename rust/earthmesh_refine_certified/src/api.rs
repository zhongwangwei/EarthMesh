use crate::{
    certificate::{
        AngleContractId, BalanceCertificate, Certificate, CertificateError, PhysicalCertificate,
    },
    config::CertifiedConfig,
    mother_grid::{mother_cell_count, MotherGrid},
    outcome::{
        CertifiedMeshOutcome, CertifiedPrimalDualMesh, FinalCertificationEvidence,
        GeometryCertifiedMotherGrid,
    },
};

pub fn generate_certified_mother_grid(config: &CertifiedConfig) -> CertifiedMeshOutcome {
    let Some(cells) = mother_cell_count(config.mother_subdivision) else {
        return CertifiedMeshOutcome::InternalCertificationFailure {
            reason: "mother subdivision count overflows usize or is zero".into(),
        };
    };
    if let Some(budget) = config.max_cells {
        if cells > budget {
            return CertifiedMeshOutcome::CellBudgetInsufficient {
                required_cells: cells,
                budget,
            };
        }
    }
    if config.max_level > 0 {
        let Some(max_subdivision) = 1usize.checked_shl(config.max_level as u32) else {
            return CertifiedMeshOutcome::InternalCertificationFailure {
                reason: "max_level is too large for this platform".into(),
            };
        };
        if config.mother_subdivision > max_subdivision {
            return CertifiedMeshOutcome::MaximumLevelReached {
                requested_level: ceil_log2(config.mother_subdivision),
                max_level: config.max_level,
            };
        }
    }
    geometry_certified_mother_grid_with_contract(config.mother_subdivision, config.angle_contract)
}

pub fn safe_mother_only(subdivision: usize, max_cells: usize) -> CertifiedMeshOutcome {
    let mut config = CertifiedConfig::mother_only(subdivision);
    config.max_cells = Some(max_cells);
    generate_certified_mother_grid(&config)
}

pub fn geometry_certified_mother_grid(subdivision: usize) -> CertifiedMeshOutcome {
    geometry_certified_mother_grid_with_contract(subdivision, AngleContractId::default())
}

pub fn geometry_certified_mother_grid_with_contract(
    subdivision: usize,
    angle_contract: AngleContractId,
) -> CertifiedMeshOutcome {
    match MotherGrid::generate(subdivision) {
        Ok(grid) => certify_mother_grid_with_contract(grid, angle_contract),
        Err(error) => CertifiedMeshOutcome::InternalCertificationFailure { reason: error },
    }
}

pub fn certify_mother_grid(grid: MotherGrid) -> CertifiedMeshOutcome {
    certify_mother_grid_with_contract(grid, AngleContractId::default())
}

pub fn certify_mother_grid_with_contract(
    grid: MotherGrid,
    angle_contract: AngleContractId,
) -> CertifiedMeshOutcome {
    match Certificate::final_delivery_for(angle_contract).verify_mother_grid(&grid) {
        Ok(report) => CertifiedMeshOutcome::GeometryCertified(Box::new(
            GeometryCertifiedMotherGrid::new(grid.mesh, report),
        )),
        Err(error)
            if error
                .to_string()
                .contains("not in the certified support table") =>
        {
            CertifiedMeshOutcome::CriterionNotCertifiable {
                reason: error.to_string(),
            }
        }
        Err(error) => CertifiedMeshOutcome::InternalCertificationFailure {
            reason: error.to_string(),
        },
    }
}

pub fn certify_geometry(mesh: earthmesh_mesh::MeshState) -> CertifiedMeshOutcome {
    certify_geometry_with_contract(mesh, AngleContractId::default())
}

pub fn certify_geometry_with_contract(
    mesh: earthmesh_mesh::MeshState,
    angle_contract: AngleContractId,
) -> CertifiedMeshOutcome {
    match Certificate::final_delivery_for(angle_contract).verify_geometry(&mesh) {
        Ok(report) => CertifiedMeshOutcome::GeometryCertified(Box::new(
            GeometryCertifiedMotherGrid::new(mesh, report),
        )),
        Err(error) => CertifiedMeshOutcome::InternalCertificationFailure {
            reason: error.to_string(),
        },
    }
}

pub fn safe_mother_final_evidence(
    required_levels: &[usize],
    delivered_level: usize,
    mesh: &earthmesh_mesh::MeshState,
) -> Result<FinalCertificationEvidence, String> {
    let physical = PhysicalCertificate::certify_uniform_level(required_levels, delivered_level)
        .map_err(|e| e.to_string())?;
    let delivered = vec![delivered_level; required_levels.len()];
    let balance = BalanceCertificate::certify_levels_cover_envelope(&delivered, required_levels)
        .map_err(|e| e.to_string())?;
    let cell_count = mesh.active_vertex_slots().count();
    let remap =
        crate::remap::ConservativeRemap::identity_for_mesh(mesh).certify_identity(cell_count);
    FinalCertificationEvidence::from_certificates(
        physical,
        balance,
        remap,
        crate::fingerprint::mesh_fingerprint(mesh),
    )
}

pub fn finalize_geometry_certified_mother(
    geometry: GeometryCertifiedMotherGrid,
    evidence: FinalCertificationEvidence,
) -> Result<CertifiedPrimalDualMesh, CertificateError> {
    let (primal, geometry) = geometry.into_parts();
    if evidence.remap_rows != geometry.voronoi_cells {
        return Err(CertificateError::RemapRows {
            expected: geometry.voronoi_cells,
            actual: evidence.remap_rows,
        });
    }
    if evidence.target_fingerprint != crate::fingerprint::mesh_fingerprint(&primal) {
        return Err(CertificateError::EvidenceMeshMismatch);
    }
    let report = geometry.into_final(evidence)?;
    Ok(CertifiedPrimalDualMesh::new(primal, report))
}

fn ceil_log2(value: usize) -> usize {
    usize::BITS as usize - (value - 1).leading_zeros() as usize
}
