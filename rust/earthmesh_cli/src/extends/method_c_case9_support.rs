//! Test-only support retained for the archived native Case 9 experiments.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use earthmesh_mesh::{
    method_c_parent_support_request, MethodCDelaunayMesh, MethodCHfieldLegalizationPreflight,
    MethodCHfieldSelectionCheckpoint,
};
use serde::{Deserialize, Serialize};

pub(crate) fn add_method_c_face_lineage_demands(
    mesh: &MethodCDelaunayMesh,
    face_demand: &mut [bool],
    requested: &BTreeSet<i64>,
) -> io::Result<usize> {
    if requested.is_empty() {
        return Ok(0);
    }
    if face_demand.len() != mesh.nwd + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Method-C support face-demand length does not match the current mesh",
        ));
    }
    let lineages = mesh.gridfile_m_cell_lineages()?;
    let mut matched = 0usize;
    for (row, lineage) in lineages.into_iter().enumerate() {
        if requested.contains(&lineage) {
            face_demand[row + 1] = true;
            matched += 1;
        }
    }
    if matched != requested.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Method-C cross-level support mapped {matched}/{} stable parent-face lineages",
                requested.len()
            ),
        ));
    }
    Ok(matched)
}

pub(crate) fn record_method_c_parent_support_request(
    error: &io::Error,
    requested: &mut BTreeSet<i64>,
) -> io::Result<Option<usize>> {
    let Some(request) = method_c_parent_support_request(error) else {
        return Ok(None);
    };
    let mut added = 0usize;
    for &lineage in &request.lineages {
        let lineage = i64::try_from(lineage).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Method-C parent-support lineage {lineage} exceeds i64"),
            )
        })?;
        added += usize::from(requested.insert(lineage));
    }
    Ok(Some(added))
}

pub(crate) const M0_LEGALIZATION_CHECKPOINT_SCHEMA: &str =
    "earthmesh-method-c-legalization-checkpoint-v2";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct M0LegalizationCheckpointProvenance {
    pub(crate) build_profile: String,
    pub(crate) executable_sha256: String,
    pub(crate) namelist_sha256: String,
    pub(crate) landcover_file_name: String,
    pub(crate) landcover_sha256: String,
    pub(crate) source_nlon: usize,
    pub(crate) source_nlat: usize,
    pub(crate) source_samples_per_degree: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct M0MethodCLegalizationCheckpoint {
    pub(crate) schema: String,
    pub(crate) pass: usize,
    pub(crate) child_grid_number: usize,
    pub(crate) field_max_level: usize,
    pub(crate) max_mrows: usize,
    pub(crate) support_lineages: Vec<Vec<i64>>,
    pub(crate) selection: MethodCHfieldSelectionCheckpoint,
    pub(crate) preflight: MethodCHfieldLegalizationPreflight,
    pub(crate) mesh: MethodCDelaunayMesh,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct M0MethodCLegalizationCheckpointReceipt {
    pub(crate) checkpoint_sha256: String,
    pub(crate) provenance: M0LegalizationCheckpointProvenance,
}

pub(crate) fn m0_legalization_checkpoint_bytes(
    checkpoint: &M0MethodCLegalizationCheckpoint,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(checkpoint).map_err(io::Error::other)
}

pub(crate) fn m0_sidecar_path(path: &Path, suffix: &str) -> io::Result<PathBuf> {
    let mut file_name = path.file_name().map(OsString::from).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "checkpoint has no file name")
    })?;
    file_name.push(suffix);
    Ok(path.with_file_name(file_name))
}

pub(crate) fn write_m0_method_c_legalization_checkpoint(
    path: &Path,
    checkpoint: &M0MethodCLegalizationCheckpoint,
    provenance: &M0LegalizationCheckpointProvenance,
) -> io::Result<String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = m0_legalization_checkpoint_bytes(checkpoint)?;
    let mut temp_name = path.file_name().map(OsString::from).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "checkpoint has no file name")
    })?;
    temp_name.push(format!(".{}.tmp", std::process::id()));
    let temp = path.with_file_name(temp_name);
    fs::write(&temp, bytes)?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    let sha256 = earthmesh_project::file_content_hash(path)?;
    fs::write(m0_sidecar_path(path, ".sha256")?, format!("{sha256}\n"))?;
    fs::write(
        m0_sidecar_path(path, ".provenance.json")?,
        serde_json::to_vec(&M0MethodCLegalizationCheckpointReceipt {
            checkpoint_sha256: sha256.clone(),
            provenance: provenance.clone(),
        })
        .map_err(io::Error::other)?,
    )?;
    Ok(sha256)
}
