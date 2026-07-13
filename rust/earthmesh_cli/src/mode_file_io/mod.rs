mod existing;
mod fvcom;
mod iap;
mod mpas;
mod write;

use std::io;

pub use existing::copy_existing_earthmesh_mode_file;
pub use fvcom::convert_fvcom_mode_file_to_earthmesh;
pub use iap::{convert_iap_ocean_mode_file_to_earthmesh, read_iap_mesh_netcdf};
pub use mpas::convert_mpas_mode_file_to_earthmesh;
pub use write::{write_gridfile_from_one_based_state, write_gridfile_from_state};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectivityBase {
    /// EarthMesh's historical MPAS/FVCOM dialect: external ids are 0..n-1.
    Zero,
    /// Standard MPAS/FVCOM: external ids are 1..n and 0 is a missing-neighbor sentinel.
    One,
}

fn detect_connectivity_base(
    label: &str,
    values: &[i32],
    upper_exclusive: usize,
) -> io::Result<ConnectivityBase> {
    let mut saw_zero = false;
    let mut saw_upper = false;
    for (idx, &value) in values.iter().enumerate() {
        saw_zero |= value == 0;
        if value < 0 || value as usize > upper_exclusive {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{label} id {value} at flat index {idx} is outside accepted 0..={upper_exclusive} connectivity range"
                ),
            ));
        }
        saw_upper |= value as usize == upper_exclusive;
    }
    if saw_zero && saw_upper {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} mixes zero-based id 0 with one-based maximum id {upper_exclusive}"),
        ));
    }
    if saw_upper || (!values.is_empty() && !saw_zero) {
        return Ok(ConnectivityBase::One);
    }
    Ok(ConnectivityBase::Zero)
}

fn validate_connectivity_base(
    label: &str,
    values: &[i32],
    upper_exclusive: usize,
    base: ConnectivityBase,
    zero_is_padding: bool,
) -> io::Result<()> {
    for (idx, &value) in values.iter().enumerate() {
        let valid = match base {
            ConnectivityBase::Zero => value >= 0 && (value as usize) < upper_exclusive,
            ConnectivityBase::One => {
                (zero_is_padding && value == 0)
                    || (value > 0 && (value as usize) <= upper_exclusive)
            }
        };
        if !valid {
            let expected = match base {
                ConnectivityBase::Zero => format!("0..{}", upper_exclusive.saturating_sub(1)),
                ConnectivityBase::One if zero_is_padding => {
                    format!("1..={upper_exclusive} (or 0 padding)")
                }
                ConnectivityBase::One => format!("1..={upper_exclusive}"),
            };
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{label} id {value} at flat index {idx} must be in {expected}"),
            ));
        }
    }
    Ok(())
}

fn earthmesh_canonical_connectivity_id(value: i32, base: ConnectivityBase) -> i32 {
    match base {
        ConnectivityBase::Zero => value + 1,
        ConnectivityBase::One => {
            if value == 0 {
                1
            } else {
                value
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        detect_connectivity_base, earthmesh_canonical_connectivity_id, validate_connectivity_base,
        ConnectivityBase,
    };

    #[test]
    fn connectivity_base_detects_earthmesh_zero_based_dialect() {
        let base = detect_connectivity_base("cellsOnVertex", &[0, 1, 2], 3).unwrap();
        assert_eq!(base, ConnectivityBase::Zero);
        assert_eq!(earthmesh_canonical_connectivity_id(0, base), 1);
        assert_eq!(earthmesh_canonical_connectivity_id(2, base), 3);
    }

    #[test]
    fn connectivity_base_accepts_standard_one_based_max_id() {
        let base = detect_connectivity_base("cellsOnVertex", &[1, 2, 3], 3).unwrap();
        assert_eq!(base, ConnectivityBase::One);
        assert_eq!(earthmesh_canonical_connectivity_id(1, base), 1);
        assert_eq!(earthmesh_canonical_connectivity_id(3, base), 3);
    }

    #[test]
    fn connectivity_base_accepts_positive_only_one_based_subset() {
        let base = detect_connectivity_base("cellsOnVertex", &[1, 2], 3).unwrap();
        assert_eq!(base, ConnectivityBase::One);
    }

    #[test]
    fn connectivity_base_maps_standard_zero_sentinel_to_placeholder() {
        let base = ConnectivityBase::One;
        validate_connectivity_base("cellsOnVertex", &[0, 1, 3], 3, base, true).unwrap();
        assert_eq!(earthmesh_canonical_connectivity_id(0, base), 1);
        assert_eq!(earthmesh_canonical_connectivity_id(3, base), 3);
    }

    #[test]
    fn connectivity_base_rejects_out_of_range_ids() {
        let err = detect_connectivity_base("cellsOnVertex", &[4], 3).unwrap_err();
        assert!(err.to_string().contains("outside accepted"));
    }

    #[test]
    fn connectivity_base_rejects_mixed_zero_and_one_based_maximum() {
        let err = detect_connectivity_base("verticesOnCell", &[0, 1, 3], 3).unwrap_err();
        assert!(err.to_string().contains("mixes zero-based"));
    }

    #[test]
    fn one_based_padding_is_validated_separately_from_active_ids() {
        validate_connectivity_base(
            "verticesOnCell",
            &[1, 2, 3, 0],
            3,
            ConnectivityBase::One,
            true,
        )
        .unwrap();
        assert!(validate_connectivity_base(
            "verticesOnCell active",
            &[1, 0, 3],
            3,
            ConnectivityBase::One,
            false,
        )
        .is_err());
    }
}
