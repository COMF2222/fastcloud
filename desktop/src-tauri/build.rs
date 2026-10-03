fn main() -> Result<(), Box<dyn std::error::Error>> {
    // CI embeds this list in both signed installers. Downloads can then be
    // checked without trusting a mutable remote resource manifest.
    println!("cargo:rerun-if-env-changed=FASTCLOUD_COMPONENT_MANIFEST");
    let manifest = if let Some(path) = std::env::var_os("FASTCLOUD_COMPONENT_MANIFEST") {
        println!(
            "cargo:rerun-if-changed={}",
            std::path::Path::new(&path).display()
        );
        std::fs::read_to_string(path)?
    } else {
        "null".to_owned()
    };
    let destination =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("Cargo OUT_DIR missing")?);
    std::fs::write(destination.join("component-manifest.json"), manifest)?;
    // Windows embeds this resource in the executable. Regenerate it when the
    // artwork changes, even if Cargo can reuse the rest of the build.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();
    Ok(())
}
