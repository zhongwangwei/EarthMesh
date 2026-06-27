//! Mesh engine child-process state and control commands.

use std::process::Command;
use std::sync::Mutex;

/// PID of the mesh-generator child currently running (if any). `run_project` sets
/// it on spawn and clears it on exit, so `kill_run` can terminate the run.
static RUNNING_CHILD_PID: Mutex<Option<u32>> = Mutex::new(None);

pub(crate) fn record_running_child(pid: u32) {
    *RUNNING_CHILD_PID.lock().unwrap() = Some(pid);
}

pub(crate) fn clear_running_child() {
    *RUNNING_CHILD_PID.lock().unwrap() = None;
}

/// Terminate the running mesh-generator process, if any. Returns whether
/// a process was signalled. Kills by PID — SIGKILL on unix, `taskkill /F /T` on
/// Windows (which also reaps any child threads/processes).
#[tauri::command]
pub(crate) fn kill_run() -> Result<bool, String> {
    let pid = *RUNNING_CHILD_PID.lock().unwrap();
    let Some(pid) = pid else {
        return Ok(false);
    };
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F", "/T"])
            .status();
    }
    clear_running_child();
    Ok(true)
}
