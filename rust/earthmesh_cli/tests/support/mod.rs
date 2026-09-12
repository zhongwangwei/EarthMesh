use std::{
    io,
    process::{Child, Command, Output, Stdio},
};

/// Do not pass native NetCDF/HDF5 descriptors to unrelated test children.
/// HDF5 sec2 does not set O_CLOEXEC; inherited writers keep file locks alive.
pub fn spawn(command: &mut Command) -> io::Result<Child> {
    // The parent must not close/reopen HDF files while fork temporarily shares
    // their locks with the child. Release this guard at exec, not at child exit.
    let _guard = hdf5_sys::LOCK.lock();
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::os::unix::process::CommandExt;
        unsafe extern "C" {
            fn getdtablesize() -> i32;
            fn fcntl(fd: i32, command: i32, ...) -> i32;
        }
        // POSIX descriptor commands on the supported macOS/Linux test hosts.
        // Obtain the limit before fork; the child hook must not allocate, acquire locks, or use stdio.
        const F_GETFD: i32 = 1;
        const F_SETFD: i32 = 2;
        const FD_CLOEXEC: i32 = 1;
        const EBADF: i32 = 9;
        let limit = unsafe { getdtablesize() };
        assert!(limit >= 3, "cannot obtain subprocess descriptor limit");
        // SAFETY: fcntl is async-signal-safe. Stdio is already installed by
        // Command. Marking, not closing, preserves its exec-error pipe on failure.
        unsafe {
            command.pre_exec(move || {
                // ponytail: scan the fd limit per test spawn; use native CLOEXEC
                // range marking only if this cost becomes measurable in CI.
                for fd in 3..limit {
                    let flags = fcntl(fd, F_GETFD);
                    if flags < 0 {
                        let error = io::Error::last_os_error();
                        if error.raw_os_error() == Some(EBADF) {
                            continue;
                        }
                        return Err(error);
                    }
                    if flags & FD_CLOEXEC == 0 && fcntl(fd, F_SETFD, flags | FD_CLOEXEC) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
    }
    command.spawn()
}

/// Capture a fresh test command with Command::output's default stdio policy.
/// Waiting outside the HDF lock lets other tests keep doing native I/O.
pub fn output(command: &mut Command) -> io::Result<Output> {
    spawn(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?
    .wait_with_output()
}
