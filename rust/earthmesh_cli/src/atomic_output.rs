use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

pub(crate) fn validate_output_destination(output: &Path) -> io::Result<()> {
    crate::ensure_parent_dir(output)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_meta = std::fs::symlink_metadata(parent)?;
    if !parent_meta.is_dir() || parent_meta.file_type().is_symlink() {
        return Err(invalid("output parent must be a real directory"));
    }
    match fs::symlink_metadata(output) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(invalid("output path must not be a symlink"));
        }
        Ok(meta) if meta.is_dir() => {
            return Err(invalid("output path must not be a directory"));
        }
        Ok(meta) if !meta.is_file() => {
            return Err(invalid("output path must be a regular file"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

pub(crate) fn validate_output_path(input: &Path, output: &Path) -> io::Result<()> {
    validate_output_destination(output)?;
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

fn publish_with_restore_context(
    error: io::Error,
    backups: &[(PathBuf, PathBuf, bool)],
) -> io::Error {
    let publication_error = error.to_string();
    match restore_backups(backups) {
        Ok(()) => error,
        Err(restore_error) => io::Error::new(
            restore_error.kind(),
            format!(
                "publication failed ({publication_error}); rollback also failed ({restore_error})"
            ),
        ),
    }
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
                let error = io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "artifact target is not a regular file: {}",
                        final_path.display()
                    ),
                );
                return Err(publish_with_restore_context(error, &backups));
            }
            let backup = backup_path(final_path, index);
            if let Err(error) = fs::rename(final_path, &backup) {
                return Err(publish_with_restore_context(error, &backups));
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
            return Err(publish_with_restore_context(error, &backups));
        }
        published.push(index);
    }
    for (_, backup, _) in backups {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn restore_backups(backups: &[(PathBuf, PathBuf, bool)]) -> io::Result<()> {
    for (original, backup, _) in backups.iter().filter(|(_, _, is_ready)| !is_ready) {
        fs::rename(backup, original).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to restore artifact backup {} -> {}: {error}",
                    backup.display(),
                    original.display()
                ),
            )
        })?;
    }
    if let Some((original, backup, _)) = backups.iter().find(|(_, _, is_ready)| *is_ready) {
        fs::rename(backup, original).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to restore readiness backup {} -> {}: {error}",
                    backup.display(),
                    original.display()
                ),
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn output_destination_rejects_special_files() {
        let dir =
            std::env::temp_dir().join(format!("earthmesh-special-output-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let output = dir.join("socket");
        let socket = std::os::unix::net::UnixListener::bind(&output).unwrap();
        assert!(validate_output_destination(&output)
            .unwrap_err()
            .to_string()
            .contains("regular file"));
        drop(socket);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn restore_backups_keeps_ready_withdrawn_when_nonready_restore_fails() {
        let dir =
            std::env::temp_dir().join(format!("earthmesh-restore-failure-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let live = dir.join("live");
        fs::create_dir_all(&live).unwrap();
        let data = live.join("data.nc");
        let ready = live.join("ready.marker");
        let data_backup = dir.join("data.backup");
        let ready_backup = dir.join("ready.backup");
        fs::write(&data_backup, "old data").unwrap();
        fs::write(&ready_backup, "old ready").unwrap();
        fs::remove_dir_all(&live).unwrap();

        let error = restore_backups(&[
            (data.clone(), data_backup.clone(), false),
            (ready.clone(), ready_backup.clone(), true),
        ])
        .expect_err("non-ready restore must fail when destination parent is missing");

        assert!(
            error
                .to_string()
                .contains("failed to restore artifact backup"),
            "unexpected error: {error}"
        );
        assert!(
            data_backup.exists(),
            "failed non-ready backup must be retained"
        );
        assert!(
            ready_backup.exists(),
            "ready backup must not be restored early"
        );
        assert!(!ready.exists(), "readiness marker must remain withdrawn");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn publication_error_reports_failed_restore_context() {
        let dir =
            std::env::temp_dir().join(format!("earthmesh-restore-context-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let missing_parent = dir.join("missing-parent");
        let original = missing_parent.join("data.nc");
        let backup = dir.join("data.backup");
        fs::write(&backup, "old data").unwrap();

        let error = publish_with_restore_context(
            io::Error::new(io::ErrorKind::NotFound, "missing staged artifact"),
            &[(original, backup.clone(), false)],
        );

        let message = error.to_string();
        assert!(message.contains("publication failed (missing staged artifact)"));
        assert!(message.contains("rollback also failed"));
        assert!(
            backup.exists(),
            "failed backup must be retained for manual recovery"
        );
        let _ = fs::remove_dir_all(&dir);
    }

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
