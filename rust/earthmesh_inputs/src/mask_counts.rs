use std::io;
use std::path::{Path, PathBuf};

/// Rust-owned mask counters matching `mask_domain_ndm`, `mask_refine_ndm`, and
/// `mask_patch_ndm` updates in `bbox_mask_make`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaskCountState {
    pub mask_domain_ndm: usize,
    pub mask_refine_ndm: [usize; 10],
    pub mask_patch_ndm: [usize; 10],
}

impl MaskCountState {
    /// Advance counters and return the Canonical bbox output filename.
    pub fn next_bbox_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "bbox", refine_degree, file_dir, 2)
    }

    /// Advance counters and return the Canonical circle output filename.
    pub fn next_circle_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "circle", refine_degree, file_dir, 2)
    }

    /// Advance counters and return the Canonical close output filename.
    pub fn next_close_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "close", refine_degree, file_dir, 3)
    }

    /// Advance counters and return the Canonical lambert output filename.
    pub fn next_lambert_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "lambert", refine_degree, file_dir, 2)
    }

    fn next_mask_output(
        &mut self,
        mask_select: &str,
        mask_type: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
        count_width: usize,
    ) -> io::Result<PathBuf> {
        if refine_degree >= self.mask_refine_ndm.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refine_degree must fit mask counter arrays 0:9",
            ));
        }
        let count = match mask_select {
            "mask_domain" => {
                self.mask_domain_ndm += 1;
                self.mask_domain_ndm
            }
            "mask_refine" => {
                self.mask_refine_ndm[refine_degree] += 1;
                self.mask_refine_ndm[refine_degree]
            }
            "mask_patch" => {
                self.mask_patch_ndm[refine_degree] += 1;
                self.mask_patch_ndm[refine_degree]
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported mask_select {other}"),
                ));
            }
        };
        Ok(file_dir.as_ref().join("tmpfile").join(format!(
            "{mask_select}_{mask_type}_{refine_degree}_{count:0count_width$}.nc4"
        )))
    }
}
