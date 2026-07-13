use crate::ProjectConfig;
use serde::{Deserialize, Serialize};

/// A content fingerprint of one input file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputFingerprint {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Auto-generated record for reproducing a run: tool /
/// schema versions, input file hashes, and the lowered namelist snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReproducibilityManifest {
    pub tool_version: String,
    pub schema_version: String,
    pub project_name: String,
    pub inputs: Vec<InputFingerprint>,
    pub lowered_namelist: String,
}

impl ReproducibilityManifest {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
}

fn sha256_file(path: &str) -> Result<InputFingerprint, String> {
    use sha2::{Digest, Sha256};
    let bytes =
        std::fs::read(path).map_err(|err| format!("fingerprint enabled input {path}: {err}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(InputFingerprint {
        path: path.to_string(),
        sha256,
        bytes: bytes.len() as u64,
    })
}

impl ProjectConfig {
    /// Build a reproducibility manifest: hash the enabled data-layer inputs and
    /// snapshot the lowered namelist. Every enabled input must be readable so a
    /// manifest can never claim reproducibility while silently omitting data.
    pub fn try_reproducibility_manifest(&self) -> Result<ReproducibilityManifest, String> {
        let inputs = self
            .data_layers
            .iter()
            .filter(|l| l.enabled && !l.path.trim().is_empty())
            .map(|l| sha256_file(&l.path))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ReproducibilityManifest {
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: self.schema_version.clone(),
            project_name: self.metadata.name.clone(),
            inputs,
            lowered_namelist: self.try_lower()?.to_namelist(),
        })
    }

    /// Build a reproducibility manifest for an already-validated project.
    pub fn reproducibility_manifest(&self) -> ReproducibilityManifest {
        self.try_reproducibility_manifest()
            .expect("ProjectConfig::reproducibility_manifest requires a valid project")
    }
}
