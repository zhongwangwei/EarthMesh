//! The macOS allocator's large-block cache, turned off for a run.
//!
//! macOS keeps freed large blocks to hand out again, and counts them against
//! the process until it does. A refinement run frees each pass's tables and
//! then allocates the next pass's, a little larger -- so the cached blocks
//! never fit and are never reused. Near the peak of a Heihe 1 km run, 3.4 GB
//! of 6.6 GB resident was live and 2.9 GB was "Malloc Large (empty)";
//! `malloc_zone_pressure_relief` released none of it. With the cache off:
//!
//! | run | peak with the cache | without | time |
//! |---|---|---|---|
//! | Heihe 1 km (five passes) | 7.09 GB | 4.39 GB | about +10% |
//! | global 30 km slope, Method-C | 14.30 GB | 5.75 GB | +5% |
//!
//! The same meshes, cell for cell. The allocator reads `MallocLargeCache`
//! only as the process starts, so the CLI starts itself again with it set.
//! A caller who sets the variable, to either value, is left alone.

/// Restart the process with `MallocLargeCache=0` unless the variable is
/// already set. Returns only when no restart happened (another platform, the
/// variable set, or the restart failed -- the run then goes on with the
/// cache, as it always did).
pub(crate) fn turn_off_large_block_cache() {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;

        const KEY: &str = "MallocLargeCache";
        if std::env::var_os(KEY).is_some() {
            return;
        }
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let mut args = std::env::args_os();
        let mut command = std::process::Command::new(exe);
        if let Some(arg0) = args.next() {
            command.arg0(arg0);
        }
        // `exec` returns only if it failed.
        let _ = command.args(args).env(KEY, "0").exec();
    }
}
