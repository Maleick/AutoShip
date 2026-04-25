use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-changed=csrc/detour_query_shim.cpp");
    println!("cargo:rerun-if-changed=schemas/replay.fbs");
    println!("cargo:rerun-if-env-changed=CARGO_HOME");
    println!("cargo:rerun-if-env-changed=USERPROFILE");
    println!("cargo:rerun-if-env-changed=HOME");
    println!(
        "cargo:rustc-env=TEXTQUEST_POLICY_SHA={}",
        policy_sha().unwrap_or_else(|| String::from("unknown"))
    );

    flatbuffers_build::BuilderOptions::new_with_files(["schemas/replay.fbs"])
        .compile()
        .expect("flatbuffer compilation failed");

    // Find the recastnavigation-sys source directory in the cargo registry
    // to get the Detour include headers.
    let home = std::env::var("CARGO_HOME")
        .or_else(|_| std::env::var("USERPROFILE").map(|h| format!("{h}/.cargo")))
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("Cannot find HOME or CARGO_HOME");
            format!("{home}/.cargo")
        });

    // Search for any version of recastnavigation-sys, not just a hardcoded one.
    let registry_src = std::path::Path::new(&home).join("registry/src");
    let mut include_dir = None;

    if let Ok(registries) = std::fs::read_dir(&registry_src) {
        for registry in registries.flatten() {
            // Each registry index has its own subdirectory
            let registry_path = registry.path();
            if let Ok(crates) = std::fs::read_dir(&registry_path) {
                for entry in crates.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("recastnavigation-sys-") {
                        let candidate = entry.path().join("recastnavigation/Detour/Include");
                        if candidate.exists() {
                            include_dir = Some(candidate);
                            break;
                        }
                    }
                }
            }
            if include_dir.is_some() {
                break;
            }
        }
    }

    let include_dir = match include_dir {
        Some(dir) => dir,
        None => {
            eprintln!(
                "cargo:warning=Could not find recastnavigation-sys Detour headers in {}. The C++ \
                 shim will not be compiled. Ensure recastnavigation-sys is downloaded (cargo \
                 fetch).",
                registry_src.display()
            );
            return;
        }
    };

    cc::Build::new()
        .cpp(true)
        .file("csrc/detour_query_shim.cpp")
        .include(&include_dir)
        .define("DT_POLYREF64", "1")
        .compile("detour_query_shim");
}

fn policy_sha() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let sha = String::from_utf8(output.stdout).ok()?;
    let sha = sha.trim();
    (!sha.is_empty()).then(|| sha.to_owned())
}
