use std::{io, path::Path};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

pub(crate) fn validate_output_path(input: &Path, output: &Path) -> io::Result<()> {
    crate::ensure_parent_dir(output)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_meta = std::fs::symlink_metadata(parent)?;
    if !parent_meta.is_dir() || parent_meta.file_type().is_symlink() {
        return Err(invalid("output parent must be a real directory"));
    }
    if let Ok(meta) = std::fs::symlink_metadata(output) {
        if meta.file_type().is_symlink() {
            return Err(invalid("output path must not be a symlink"));
        }
        if meta.is_dir() {
            return Err(invalid("output path must not be a directory"));
        }
    }
    if output.exists() && std::fs::canonicalize(input)? == std::fs::canonicalize(output)? {
        return Err(invalid("input and output must not be the same file"));
    }
    reject_hardlink_alias(input, output)?;
    Ok(())
}

fn reject_hardlink_alias(input: &Path, output: &Path) -> io::Result<()> {
    if !output.exists() {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let input = std::fs::metadata(input)?;
        let output = std::fs::metadata(output)?;
        if input.dev() == output.dev() && input.ino() == output.ino() {
            return Err(invalid("input and output must not be filesystem aliases"));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (input, output);
    }
    Ok(())
}

pub(crate) fn atomic_write(
    output: &Path,
    write: impl FnOnce(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("mesh");
    for attempt in 0..128u32 {
        let tmp = parent.join(format!(".{stem}.tmp-{}-{attempt}", std::process::id()));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => drop(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
        let result = write(&tmp).and_then(|_| std::fs::rename(&tmp, output));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        return result;
    }
    Err(invalid("could not create exclusive temporary output"))
}
