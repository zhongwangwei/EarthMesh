mod support;

use std::process::Command;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn a_spawned_child_does_not_keep_a_closed_netcdf_writer_locked() {
    let path = std::env::temp_dir().join(format!("netcdf_child_lock_{}.nc4", std::process::id()));
    let mut file = earthmesh_cli::create_netcdf_quiet(&path).unwrap();
    file.add_dimension("n", 1).unwrap();
    file.add_variable::<i32>("v", &["n"])
        .unwrap()
        .put_values(&[7], ..)
        .unwrap();
    use std::{
        io::{Read, Write},
        os::{
            fd::AsRawFd,
            unix::{net::UnixStream, process::CommandExt},
        },
        time::Duration,
    };
    let (mut parent_signal, child_signal) = UnixStream::pair().unwrap();
    parent_signal
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    child_signal
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let launch = std::thread::spawn(move || {
        let mut command = Command::new("/bin/sleep");
        command.arg("60");
        let fd = child_signal.as_raw_fd();
        unsafe extern "C" {
            fn write(fd: i32, buf: *const u8, len: usize) -> isize;
            fn read(fd: i32, buf: *mut u8, len: usize) -> isize;
        }
        // SAFETY: child uses only async-signal-safe read/write on an owned
        // socket, with bounded reads; all assertions and locks stay in parent.
        unsafe {
            command.pre_exec(move || {
                let mut byte = 1_u8;
                if write(fd, &byte, 1) != 1 || read(fd, &mut byte, 1) != 1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = support::spawn(&mut command);
        drop(child_signal);
        child.unwrap()
    });
    parent_signal.read_exact(&mut [0]).unwrap();
    // Closing/reopening a writer must not race the fork-to-exec interval,
    // even though the inherited descriptors will be closed at exec.
    let fork_is_guarded = hdf5_sys::LOCK.try_lock().is_none();
    parent_signal.write_all(&[1]).unwrap();
    let mut child = launch.join().unwrap();
    let result = (|| -> Result<_, netcdf::Error> {
        file.close()?;
        let reopened = netcdf::open(&path)?;
        let values = reopened.variable("v").unwrap().get_values::<i32, _>(..)?;
        reopened.close()?;
        Ok(values)
    })();
    let child_still_running = child.try_wait().unwrap().is_none();
    let _ = child.kill();
    child.wait().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        fork_is_guarded,
        "NetCDF calls must wait until child exec closes inherited descriptors"
    );
    assert!(child_still_running, "child must outlive the reopen check");
    assert_eq!(
        result.expect("a closed writer must not remain locked in child"),
        [7]
    );
}

#[test]
fn a_missing_executable_reports_its_exec_error() {
    let missing = std::env::temp_dir().join(format!("missing_executable_{}", std::process::id()));
    let error = support::output(&mut Command::new(missing)).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[cfg(unix)]
#[test]
fn child_stdio_environment_directory_and_status_are_preserved() {
    use std::{io::Write, process::Stdio};
    let directory = std::env::temp_dir().canonicalize().unwrap();
    let mut child = support::spawn(Command::new("/bin/sh")
        .arg("-c")
        .arg("test \"$PWD\" = \"$CHECK_DIR\" || exit 8; read -r value; printf '%s' \"$value\"; printf error >&2; exit 7")
        .current_dir(&directory)
        .env("CHECK_DIR", &directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        ).unwrap();
    child.stdin.take().unwrap().write_all(b"payload\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"payload");
    assert_eq!(output.stderr, b"error");
}
