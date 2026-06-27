//! Thread-local progress + cooperative-cancellation channel between engine runs
//! and callers such as the GUI.

use std::cell::RefCell;

type Callback = Box<dyn Fn(&str, usize, usize) -> bool>;

thread_local! {
    static CALLBACK: RefCell<Option<Callback>> = const { RefCell::new(None) };
}

/// Install a progress callback for the current thread. It receives
/// `(phase, done, total)` and returns `false` to request cancellation.
pub fn set<F: Fn(&str, usize, usize) -> bool + 'static>(callback: F) {
    CALLBACK.with(|cell| *cell.borrow_mut() = Some(Box::new(callback)));
}

/// Remove the current thread's progress callback.
pub fn clear() {
    CALLBACK.with(|cell| *cell.borrow_mut() = None);
}

/// Report progress from an engine hook point. Returns `false` when the caller
/// requested cancellation; engine loops should stop and unwind when so.
pub fn report(phase: &str, done: usize, total: usize) -> bool {
    CALLBACK.with(|cell| {
        cell.borrow()
            .as_ref()
            .is_none_or(|cb| cb(phase, done, total))
    })
}
