use std::io;
use std::path::PathBuf;

use crate::rows_from_flat_i32;

use super::validation::validate_flat_contain_mesh;

/// Rust data shape written by `MOD_file_preprocess.F90:Contain_Save`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainMesh {
    pub ustr_id: Vec<Vec<i32>>,
    pub ustr_ii: Vec<Vec<i32>>,
    pub is_in_area_ustr: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatContainMesh {
    pub ustr_id_values: Vec<i32>,
    pub ustr_id_width: usize,
    pub ustr_ii_values: Vec<i32>,
    pub ustr_ii_width: usize,
    pub is_in_area_ustr: Vec<i32>,
}

impl FlatContainMesh {
    pub fn num_ustr(&self) -> usize {
        self.ustr_id_values
            .len()
            .checked_div(self.ustr_id_width)
            .unwrap_or(0)
    }

    pub fn num_ii(&self) -> usize {
        self.ustr_ii_values
            .len()
            .checked_div(self.ustr_ii_width)
            .unwrap_or(0)
    }

    pub fn to_contain_mesh(&self) -> io::Result<ContainMesh> {
        validate_flat_contain_mesh(self)?;
        Ok(ContainMesh {
            ustr_id: rows_from_flat_i32(&self.ustr_id_values, self.ustr_id_width),
            ustr_ii: rows_from_flat_i32(&self.ustr_ii_values, self.ustr_ii_width),
            is_in_area_ustr: self.is_in_area_ustr.clone(),
        })
    }
}

/// Evidence report from reading/writing a contain-domain file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainWriteReport {
    pub output: PathBuf,
    pub num_ustr: usize,
    pub num_ii: usize,
    pub dim_a: usize,
    pub dim_b: usize,
}
