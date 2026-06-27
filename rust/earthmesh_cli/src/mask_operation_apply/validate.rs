use std::io;

use crate::MaskCountState;

/// Preserve the `read_nl` specified-refinement guard.
pub fn validate_mask_refine_reaches_max_iter_spc(
    counts: &MaskCountState,
    max_iter_spc: usize,
) -> io::Result<()> {
    if max_iter_spc >= counts.mask_refine_ndm.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_iter_spc must fit mask_refine_ndm 0:9",
        ));
    }
    if counts.mask_refine_ndm[max_iter_spc] == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mask_refine_ndm(max_iter_spc) must be larger than zero",
        ));
    }
    Ok(())
}
