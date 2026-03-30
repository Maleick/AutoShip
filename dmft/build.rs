fn main() {
    // Find the recastnavigation-sys source directory in the cargo registry
    // to get the Detour include headers.
    let home = std::env::var("CARGO_HOME")
        .or_else(|_| std::env::var("USERPROFILE").map(|h| format!("{h}/.cargo")))
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("Cannot find HOME or CARGO_HOME");
            format!("{home}/.cargo")
        });

    // Search for the recastnavigation-sys crate source
    let registry_src = std::path::Path::new(&home).join("registry/src");
    let mut include_dir = None;

    if let Ok(entries) = std::fs::read_dir(&registry_src) {
        for entry in entries.flatten() {
            let candidate = entry
                .path()
                .join("recastnavigation-sys-1.0.3/recastnavigation/Detour/Include");
            if candidate.exists() {
                include_dir = Some(candidate);
                break;
            }
        }
    }

    let include_dir = include_dir.unwrap_or_else(|| {
        panic!(
            "Could not find recastnavigation-sys headers in {}. \
             Ensure recastnavigation-sys is downloaded.",
            registry_src.display()
        )
    });

    cc::Build::new()
        .cpp(true)
        .file("csrc/detour_query_shim.cpp")
        .include(&include_dir)
        .define("DT_POLYREF64", "1")
        .compile("detour_query_shim");
}
