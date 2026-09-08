#[path = "../src/app_icon.rs"]
mod app_icon;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("usage: cargo run --example export_icon -- OUTPUT.png"))?;
    let size = app_icon::ICON_SIZE * 16;

    image::save_buffer(
        &output,
        &app_icon::rgba(size),
        size,
        size,
        image::ColorType::Rgba8,
    )?;
    println!("wrote {}", output.display());
    Ok(())
}
