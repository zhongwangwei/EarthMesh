use crate::{mkgrd_config::EarthmeshConfig, RefineConfig};

/// Non-destructive representation of a Canonical `Mask_make(...)` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskOperation {
    pub mask_select: String,
    pub type_select: String,
    pub mask_fprefix: String,
}

impl MaskOperation {
    pub fn new(mask_select: &str, type_select: &str, mask_fprefix: &str) -> Self {
        Self {
            mask_select: mask_select.to_string(),
            type_select: type_select.to_string(),
            mask_fprefix: mask_fprefix.to_string(),
        }
    }
}

/// Non-destructive execution plan for the filesystem and mask-preprocess side
/// effects triggered by `mkgrd.F90:read_nl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdWorkspacePlan {
    pub file_dir: String,
    pub remove_existing_file_dir: bool,
    pub remove_filelists: bool,
    pub directories_to_create: Vec<String>,
    pub namelist_save_path: String,
    pub mask_operations: Vec<MaskOperation>,
}

impl EarthmeshConfig {
    /// Build the side-effect plan implied by `read_nl` without executing shell
    /// commands or touching the filesystem.
    pub fn read_nl_workspace_plan(
        &self,
        refine_config: Option<&RefineConfig>,
    ) -> MkgrdWorkspacePlan {
        let file_dir = self.file_dir();
        let mut plan = MkgrdWorkspacePlan {
            namelist_save_path: format!("{file_dir}result/namelist.save"),
            file_dir: file_dir.clone(),
            remove_existing_file_dir: false,
            remove_filelists: false,
            directories_to_create: Vec::new(),
            mask_operations: Vec::new(),
        };

        if self.mask_restart {
            if self.mask_patch_on {
                plan.mask_operations.push(MaskOperation::new(
                    "mask_patch",
                    &self.mask_patch_type,
                    &self.mask_patch_fprefix,
                ));
            }
            return plan;
        }

        plan.remove_existing_file_dir = true;
        plan.remove_filelists = true;
        for subdir in ["contain", "gridfile", "patchtype", "result", "tmpfile"] {
            plan.directories_to_create
                .push(format!("{file_dir}{subdir}/"));
        }

        if !self.mask_domain_global {
            plan.mask_operations.push(MaskOperation::new(
                "mask_domain",
                &self.mask_domain_type,
                &self.mask_domain_fprefix,
            ));
        }
        if self.mask_patch_on {
            plan.mask_operations.push(MaskOperation::new(
                "mask_patch",
                &self.mask_patch_type,
                &self.mask_patch_fprefix,
            ));
        }

        if self.refine {
            plan.directories_to_create
                .push(format!("{file_dir}threshold/"));
            if let Some(refine) = refine_config {
                if refine.refine_setting == "specified" || refine.refine_setting == "mixed" {
                    plan.mask_operations.push(MaskOperation::new(
                        "mask_refine",
                        &refine.mask_refine_spc_type,
                        &refine.mask_refine_spc_fprefix,
                    ));
                }
                if refine.refine_setting == "calculate" || refine.refine_setting == "mixed" {
                    plan.mask_operations.push(MaskOperation::new(
                        "mask_refine",
                        &refine.mask_refine_cal_type,
                        &refine.mask_refine_cal_fprefix,
                    ));
                }
            }
        }

        plan
    }
}
