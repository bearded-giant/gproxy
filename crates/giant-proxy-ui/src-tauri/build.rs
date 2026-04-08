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
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let binaries_dir = manifest_dir.join("binaries");
    std::fs::create_dir_all(&binaries_dir).ok();

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());

    for bin_name in &["giantd", "gproxy"] {
        let dest = binaries_dir.join(format!("{}-{}", bin_name, target_triple));
        let src = workspace_root.join("target").join(&profile).join(bin_name);
        if src.exists() {
            // always copy to pick up changes from workspace builds
            std::fs::copy(&src, &dest)
                .unwrap_or_else(|_| panic!("failed to copy {} to binaries/", bin_name));
            println!("cargo:warning=copied {} to {}", bin_name, dest.display());
        } else if !dest.exists() {
            std::fs::write(&dest, "").ok();
            println!("cargo:warning={} placeholder created", bin_name);
        }
    }

    tauri_build::build()
}
