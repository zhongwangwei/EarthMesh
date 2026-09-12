use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

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

/// Publish staged files, restoring the previous generation on an I/O error.
/// The last file is the readiness marker: withdraw it first and restore it last.
/// ponytail: rollback is not crash-atomic or concurrent-reader isolation; use a
/// versioned directory with an atomic pointer if those guarantees are required.
pub(crate) fn publish_artifacts(
    publications: &[(&Path, &Path)],
    obsolete_paths: &[&Path],
) -> io::Result<()> {
    let ready = publications
        .len()
        .checked_sub(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no artifacts to publish"))?;
    let publication_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let backup_path = |path: &Path, index: usize| {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        path.with_file_name(format!(".{name}.earthmesh-backup-{publication_id}-{index}"))
    };
    let backup_targets = std::iter::once((publications[ready].1, true))
        .chain(
            publications[..ready]
                .iter()
                .map(|(_, final_path)| (*final_path, false)),
        )
        .chain(obsolete_paths.iter().map(|path| (*path, false)));
    let mut backups = Vec::new();
    for (index, (final_path, is_ready)) in backup_targets.enumerate() {
        if final_path.exists() {
            if !final_path.is_file() {
                restore_backups(&backups);
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "artifact target is not a regular file: {}",
                        final_path.display()
                    ),
                ));
            }
            let backup = backup_path(final_path, index);
            if let Err(error) = fs::rename(final_path, &backup) {
                restore_backups(&backups);
                return Err(error);
            }
            backups.push((final_path.to_path_buf(), backup, is_ready));
        }
    }

    let mut published: Vec<usize> = Vec::new();
    for (index, &(temporary, final_path)) in publications.iter().enumerate() {
        if let Err(error) = fs::rename(temporary, final_path) {
            for &published_index in published.iter().rev() {
                let _ = fs::remove_file(publications[published_index].1);
            }
            restore_backups(&backups);
            return Err(error);
        }
        published.push(index);
    }
    for (_, backup, _) in backups {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn restore_backups(backups: &[(PathBuf, PathBuf, bool)]) {
    for (original, backup, _) in backups.iter().filter(|(_, _, is_ready)| !is_ready) {
        let _ = fs::rename(backup, original);
    }
    if let Some((original, backup, _)) = backups.iter().find(|(_, _, is_ready)| *is_ready) {
        let _ = fs::rename(backup, original);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_first_mesh_last_failure_restores_both_outputs() {
        for previous in [false, true] {
            let dir = std::env::temp_dir().join(format!(
                "earthmesh-mpas-publish-{}-{previous}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            let graph = dir.join("graph.info");
            let mesh = dir.join("mesh.nc4");
            if previous {
                fs::write(&graph, "old graph").unwrap();
                fs::write(&mesh, "old mesh").unwrap();
            }
            let staged_graph = dir.join("new.graph");
            fs::write(&staged_graph, "new graph").unwrap();
            assert!(publish_artifacts(
                &[(&staged_graph, &graph), (&dir.join("missing.mesh"), &mesh)],
                &[]
            )
            .is_err());
            if previous {
                assert_eq!(fs::read(&graph).unwrap(), b"old graph");
                assert_eq!(fs::read(&mesh).unwrap(), b"old mesh");
            } else {
                assert!(!mesh.exists() && !graph.exists());
            }
            assert!(fs::read_dir(&dir).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("backup")));
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
