//! Mesh engine child-process state and control commands.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RunId(u64);

#[derive(Clone, Copy, Debug)]
struct RunState {
    id: RunId,
    child_pid: Option<u32>,
    owner_active: bool,
}

/// One GUI command owns the run slot from validation through any follow-up hydro
/// stage. Child PIDs may change during that command, but a second command cannot
/// replace the active state in between stages.
#[derive(Debug)]
pub(crate) struct RunLease {
    id: RunId,
}

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);
static RUN_STATE: Mutex<Option<RunState>> = Mutex::new(None);

impl RunLease {
    pub(crate) fn id(&self) -> RunId {
        self.id
    }
}

impl Drop for RunLease {
    fn drop(&mut self) {
        let Ok(mut slot) = RUN_STATE.lock() else {
            return;
        };
        let Some(state) = slot.as_mut().filter(|state| state.id == self.id) else {
            return;
        };
        if state.child_pid.is_none() {
            *slot = None;
        } else {
            // Preserve the PID if the command future is abandoned while its child
            // is still alive; kill_run can still terminate it safely.
            state.owner_active = false;
        }
    }
}

pub(crate) fn begin_run() -> Result<RunLease, String> {
    let mut slot = RUN_STATE
        .lock()
        .map_err(|_| "run process state lock poisoned".to_string())?;
    if let Some(state) = *slot {
        return Err(match state.child_pid {
            Some(pid) => format!("a mesh run is already active (PID {pid})"),
            None => "a mesh run is already starting or finishing".to_string(),
        });
    }
    let id = RunId(NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed));
    *slot = Some(RunState {
        id,
        child_pid: None,
        owner_active: true,
    });
    Ok(RunLease { id })
}

pub(crate) fn record_running_child(run_id: RunId, pid: u32) -> Result<(), String> {
    let mut slot = RUN_STATE
        .lock()
        .map_err(|_| "run process state lock poisoned".to_string())?;
    let state = slot
        .as_mut()
        .filter(|state| state.id == run_id && state.owner_active)
        .ok_or_else(|| "mesh run lease is no longer active".to_string())?;
    if let Some(existing) = state.child_pid {
        return Err(format!(
            "mesh run already has an active child (PID {existing})"
        ));
    }
    state.child_pid = Some(pid);
    Ok(())
}

pub(crate) fn clear_running_child(run_id: RunId, pid: u32) {
    let Ok(mut slot) = RUN_STATE.lock() else {
        return;
    };
    let Some(state) = slot
        .as_mut()
        .filter(|state| state.id == run_id && state.child_pid == Some(pid))
    else {
        return;
    };
    if state.owner_active {
        state.child_pid = None;
    } else {
        *slot = None;
    }
}

#[cfg(test)]
pub(crate) fn running_child_pid() -> Option<u32> {
    RUN_STATE
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().and_then(|state| state.child_pid))
}

/// Terminate the running mesh-generator process, if any. Returns whether
/// a process was signalled. Kills by PID — SIGKILL on unix, `taskkill /F /T` on
/// Windows (which also reaps any child threads/processes).
#[tauri::command]
pub(crate) fn kill_run() -> Result<bool, String> {
    let mut slot = RUN_STATE
        .lock()
        .map_err(|_| "run process state lock poisoned".to_string())?;
    let Some(state) = slot.as_mut() else {
        return Ok(false);
    };
    let Some(pid) = state.child_pid else {
        return Ok(false);
    };
    #[cfg(unix)]
    let output = Command::new("kill")
        .arg("-KILL")
        .arg(pid.to_string())
        .output()
        .map_err(|err| format!("kill PID {pid}: {err}"))?;
    #[cfg(windows)]
    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F", "/T"])
        .output()
        .map_err(|err| format!("kill PID {pid}: {err}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "kill PID {pid} exited with {}: {}",
            output.status,
            detail.trim()
        ));
    }
    if state.owner_active {
        state.child_pid = None;
    } else {
        *slot = None;
    }
    Ok(true)
}
