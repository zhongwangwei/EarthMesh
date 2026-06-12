use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=NETCDF_DIR");
    println!("cargo:rerun-if-env-changed=PATH");

    let Ok(output) = Command::new("nc-config").arg("--libs").output() else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let libs = String::from_utf8_lossy(&output.stdout);
    for token in libs.split_whitespace() {
        let Some(path) = token.strip_prefix("-L") else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
        println!("cargo:rustc-link-arg-tests=-Wl,-rpath,{path}");
    }
}
