fn main() {
    let target_triple = std::env::var("TARGET").unwrap_or_else(|_| {
        let arch = std::env::consts::ARCH;
        let os = std::env::consts::OS;
        match (arch, os) {
            ("aarch64", "macos") => "aarch64-apple-darwin".to_string(),
            ("x86_64", "macos") => "x86_64-apple-darwin".to_string(),
            ("x86_64", "linux") => "x86_64-unknown-linux-gnu".to_string(),
            ("aarch64", "linux") => "aarch64-unknown-linux-gnu".to_string(),
            _ => format!("{}-unknown-{}", arch, os),
        }
    });

    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // src-tauri -> giant-proxy-ui -> crates -> gproxy (workspace root)
    let workspace_root = manifest_dir
        .parent().unwrap()
        .parent().unwrap()
        .parent().unwrap();

    let binaries_dir = manifest_dir.join("binaries");
    std::fs::create_dir_all(&binaries_dir).ok();

    let giantd_dest = binaries_dir.join(format!("giantd-{}", target_triple));

    // check for pre-placed sidecar (CI builds giantd separately, e.g. lipo universal)
    if !giantd_dest.exists() {
        // try copying from workspace target dir
        let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
        let giantd_src = workspace_root.join("target").join(&profile).join("giantd");

        if giantd_src.exists() {
            std::fs::copy(&giantd_src, &giantd_dest).expect("failed to copy giantd to binaries/");
            println!("cargo:warning=copied giantd to {}", giantd_dest.display());
        } else {
            // create placeholder so tauri_build doesn't fail during check/clippy/test
            std::fs::write(&giantd_dest, "").ok();
            println!("cargo:warning=giantd placeholder created (build giantd first for a real bundle)");
        }
    }

    tauri_build::build()
}
