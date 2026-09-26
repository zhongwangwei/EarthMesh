use std::fs;
use std::io::{self, Read, Seek, SeekFrom};

pub(super) fn read_cama_f32_row_window(
    handle: &mut fs::File,
    offset: usize,
    byte_len: usize,
    little_endian: bool,
) -> io::Result<Vec<f32>> {
    handle.seek(SeekFrom::Start(offset as u64))?;
    let mut bytes = vec![0_u8; byte_len];
    handle.read_exact(&mut bytes).map_err(binary_window_eof)?;
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| {
            if little_endian {
                f32::from_le_bytes(*chunk)
            } else {
                f32::from_be_bytes(*chunk)
            }
        })
        .collect())
}

pub(super) fn read_cama_i32_row_window(
    handle: &mut fs::File,
    offset: usize,
    byte_len: usize,
    little_endian: bool,
) -> io::Result<Vec<i32>> {
    handle.seek(SeekFrom::Start(offset as u64))?;
    let mut bytes = vec![0_u8; byte_len];
    handle.read_exact(&mut bytes).map_err(binary_window_eof)?;
    Ok(bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| {
            if little_endian {
                i32::from_le_bytes(*chunk)
            } else {
                i32::from_be_bytes(*chunk)
            }
        })
        .collect())
}

fn binary_window_eof(err: io::Error) -> io::Error {
    if err.kind() == io::ErrorKind::UnexpectedEof {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "binary file ended before requested window was read",
        )
    } else {
        err
    }
}
