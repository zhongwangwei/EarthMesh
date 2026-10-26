//! Mesh engine child-process state and control commands.

use std::process::Child;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RunId(u64);

struct RunState {
    id: RunId,
    /// Owned engine child while it is running. Signalling (`kill_active_run`)
    /// and reaping (`capture_mesh_child_with_logger`) are serialized through
    /// this mutex; that is what makes the kill immune to PID reuse.
    child: Option<Arc<Mutex<Child>>>,
    /// Copy of `child.id()` for messages and `running_child_pid()`. `child` and
    /// `child_pid` are `Some`/`None` together.
    child_pid: Option<u32>,
    owner_active: bool,
    cancel_requested: bool,
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
        if state.child.is_none() {
            *slot = None;
        } else {
            // Preserve the child handle if the command future is abandoned while
            // its child is still alive; kill_run can still terminate it safely.
            state.owner_active = false;
        }
    }
}

pub(crate) fn begin_run() -> Result<RunLease, String> {
    let mut slot = RUN_STATE
        .lock()
        .map_err(|_| "run process state lock poisoned".to_string())?;
    if let Some(state) = slot.as_ref() {
        return Err(match state.child_pid {
            Some(pid) => format!("a mesh run is already active (PID {pid})"),
            None => "a mesh run is already starting or finishing".to_string(),
        });
    }
    let id = RunId(NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed));
    *slot = Some(RunState {
        id,
        child: None,
        child_pid: None,
        owner_active: true,
        cancel_requested: false,
    });
    Ok(RunLease { id })
}

/// Register the spawned engine child for a run.
///
/// The owned `Child` (not just its PID) is stored so [`kill_active_run`] can
/// signal exactly the process it started: the wait loop may only reap the child
/// while holding the same mutex the killer takes, and the killer checks the
/// handle's exit state under that mutex before signalling, so a PID recycled
/// after reaping can never be signalled.
pub(crate) fn record_running_child(run_id: RunId, child: Arc<Mutex<Child>>) -> Result<(), String> {
    let pid = child
        .lock()
        .map_err(|_| "mesh child process state lock poisoned".to_string())?
        .id();
    let mut slot = RUN_STATE
        .lock()
        .map_err(|_| "run process state lock poisoned".to_string())?;
    let state = slot
        .as_mut()
        .filter(|state| state.id == run_id && state.owner_active)
        .ok_or_else(|| "mesh run lease is no longer active".to_string())?;
    if state.cancel_requested {
        return Err("mesh run cancelled before child started".to_string());
    }
    if let Some(existing) = state.child_pid {
        return Err(format!(
            "mesh run already has an active child (PID {existing})"
        ));
    }
    state.child = Some(child);
    state.child_pid = Some(pid);
    Ok(())
}

/// Deregister a reaped child. Called by the wait loop while it still holds the
/// child mutex, so reaping and deregistration cannot be interleaved with a kill.
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
    state.child = None;
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

/// Cancel the active run, including one that has not spawned its child yet.
/// Kills the owned child — `Child::kill` on unix and Windows — because a handle
/// is the only identity-safe way to signal the process that was started.
#[tauri::command]
pub(crate) fn kill_run() -> Result<bool, String> {
    kill_active_run()
}

/// Shared implementation of [`kill_run`] and [`stop_running_child_on_exit`].
///
/// `Ok(false)` means there was no run to cancel; `Ok(true)` means the run slot
/// was found and its child (if any) was signalled. A child that had already
/// exited is also `Ok(true)`: there is nothing left to signal.
///
/// Lock order: the state lock is held only to mark the cancellation and clone the
/// child handle; it is released before the child mutex is taken. The wait loop
/// takes them the other way round (child mutex, then the state lock inside
/// [`clear_running_child`]), so neither path waits for the other's lock while
/// holding its own.
fn kill_active_run() -> Result<bool, String> {
    let (child, pid) = {
        let mut slot = RUN_STATE
            .lock()
            .map_err(|_| "run process state lock poisoned".to_string())?;
        let Some(state) = slot.as_mut() else {
            return Ok(false);
        };
        state.cancel_requested = true;
        match (state.child.clone(), state.child_pid) {
            (Some(child), Some(pid)) => (child, pid),
            // The child has not spawned yet; the cancellation flag is enough and
            // `record_running_child` will reject the late child.
            _ => return Ok(true),
        }
    };
    let mut child = child
        .lock()
        .map_err(|_| "mesh child process state lock poisoned".to_string())?;
    // PID-reuse safety:
    // - the wait loop can only reap the child while holding this same mutex, so
    //   no reap can happen between the status check and the signal;
    // - check the exit state first: a child the wait loop already reaped is gone,
    //   and its PID may already belong to another process, so it is never
    //   signalled; the wait loop will observe the cached status and deregister;
    // - an unreaped child that is still running or already a zombie keeps its PID
    //   reserved (zombie PIDs are not reused before reaping), so signalling the
    //   handle is identity-safe.
    match child.try_wait() {
        Ok(Some(_)) => Ok(true),
        Ok(None) => {
            child
                .kill()
                .map_err(|err| format!("kill PID {pid}: {err}"))?;
            Ok(true)
        }
        Err(err) => Err(format!("wait failed while killing PID {pid}: {err}")),
    }
}

/// Best-effort stop of the mesh engine child when Studio is quitting.
///
/// The Tauri run loop calls this from `RunEvent::ExitRequested` (the event loop
/// is still alive there, so the task that waits/reaps the engine can still make
/// progress) and again from `RunEvent::Exit`, the last callback before
/// `std::process::exit`. The second call finds no run and is a no-op.
///
/// Never panics: a poisoned state lock or a failed kill is reported on stderr
/// and otherwise ignored. Returns whether an active run slot was found and a
/// stop was attempted; `false` means there was nothing to stop.
pub(crate) fn stop_running_child_on_exit() -> bool {
    match kill_active_run() {
        Ok(true) => {
            eprintln!("[earthmesh-studio] exit: cancelled the active mesh run");
            true
        }
        Ok(false) => {
            eprintln!("[earthmesh-studio] exit: no active mesh run to stop");
            false
        }
        Err(err) => {
            eprintln!("[earthmesh-studio] exit: failed to cancel the mesh run: {err}");
            // A lock or signal failure does not prove the child is gone; report
            // that a run was present rather than claiming a clean no-op.
            true
        }
    }
}
