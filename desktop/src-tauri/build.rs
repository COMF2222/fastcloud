fn main() {
    // Windows embeds this resource in the executable. Regenerate it when the
    // artwork changes, even if Cargo can reuse the rest of the build.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
