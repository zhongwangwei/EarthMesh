pub(crate) fn spring_global_debug(message: &str) {
    if std::env::var_os("EARTHMESH_SPRING_DEBUG").is_some() {
        eprintln!("EARTHMESH_SPRING_DEBUG: {message}");
    }
}
