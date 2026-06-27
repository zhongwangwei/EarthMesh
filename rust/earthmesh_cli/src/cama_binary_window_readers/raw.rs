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
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| {
            if little_endian {
                f32::from_le_bytes(chunk.try_into().expect("f32 chunk size"))
            } else {
                f32::from_be_bytes(chunk.try_into().expect("f32 chunk size"))
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
        .chunks_exact(std::mem::size_of::<i32>())
        .map(|chunk| {
            if little_endian {
                i32::from_le_bytes(chunk.try_into().expect("i32 chunk size"))
            } else {
                i32::from_be_bytes(chunk.try_into().expect("i32 chunk size"))
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
