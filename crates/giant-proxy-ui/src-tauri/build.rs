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

    // PROFILE env var is set by cargo for build scripts
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());
    let giantd_src = workspace_root.join("target").join(&profile).join("giantd");

    let binaries_dir = manifest_dir.join("binaries");
    std::fs::create_dir_all(&binaries_dir).ok();

    let giantd_dest = binaries_dir.join(format!("giantd-{}", target_triple));

    if giantd_src.exists() {
        std::fs::copy(&giantd_src, &giantd_dest).expect("failed to copy giantd to binaries/");
        println!("cargo:warning=copied giantd to {}", giantd_dest.display());
    } else {
        println!(
            "cargo:warning=giantd not found at {}. build giantd first: cargo build --release -p giantd",
            giantd_src.display()
        );
    }

    tauri_build::build()
}
