#[path = "src/app_icon.rs"]
mod app_icon;

#[cfg(target_os = "windows")]
fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let icon_path = out_dir.join("fastcloud.ico");
    write_ico(&icon_path).expect("generate Fastcloud icon");

    println!("cargo:rerun-if-changed=src/app_icon.rs");
    println!("cargo:rerun-if-changed=packaging/fastcloud.svg");
    winresource::WindowsResource::new()
        .set_icon(icon_path.to_string_lossy().as_ref())
        .compile()
        .expect("embed Fastcloud icon in the Windows executable");
}

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("cargo:rerun-if-changed=src/app_icon.rs");
    println!("cargo:rerun-if-changed=packaging/fastcloud.svg");
}

#[cfg(target_os = "windows")]
fn write_ico(path: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;

    const SIZES: [u32; 7] = [16, 24, 32, 48, app_icon::ICON_SIZE, 128, 256];
    let images: Vec<Vec<u8>> = SIZES.into_iter().map(icon_dib).collect();
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);

    file.write_all(&0_u16.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&(images.len() as u16).to_le_bytes())?;

    let mut offset = 6 + images.len() as u32 * 16;
    for (size, image) in SIZES.into_iter().zip(&images) {
        file.write_all(&[if size == 256 { 0 } else { size as u8 }])?;
        file.write_all(&[if size == 256 { 0 } else { size as u8 }])?;
        file.write_all(&[0, 0])?;
        file.write_all(&1_u16.to_le_bytes())?;
        file.write_all(&32_u16.to_le_bytes())?;
        file.write_all(&(image.len() as u32).to_le_bytes())?;
        file.write_all(&offset.to_le_bytes())?;
        offset += image.len() as u32;
    }
    for image in images {
        file.write_all(&image)?;
    }
    file.flush()
}

#[cfg(target_os = "windows")]
fn icon_dib(size: u32) -> Vec<u8> {
    let rgba = app_icon::rgba(size);
    let mask_stride = size.div_ceil(32) * 4;
    let bitmap_bytes = size * size * 4;
    let mut dib = Vec::with_capacity((40 + bitmap_bytes + mask_stride * size) as usize);

    dib.extend_from_slice(&40_u32.to_le_bytes());
    dib.extend_from_slice(&(size as i32).to_le_bytes());
    dib.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    dib.extend_from_slice(&1_u16.to_le_bytes());
    dib.extend_from_slice(&32_u16.to_le_bytes());
    dib.extend_from_slice(&0_u32.to_le_bytes());
    dib.extend_from_slice(&bitmap_bytes.to_le_bytes());
    dib.extend_from_slice(&0_i32.to_le_bytes());
    dib.extend_from_slice(&0_i32.to_le_bytes());
    dib.extend_from_slice(&0_u32.to_le_bytes());
    dib.extend_from_slice(&0_u32.to_le_bytes());

    for y in (0..size).rev() {
        for x in 0..size {
            let offset = ((y * size + x) * 4) as usize;
            dib.extend_from_slice(&[
                rgba[offset + 2],
                rgba[offset + 1],
                rgba[offset],
                rgba[offset + 3],
            ]);
        }
    }
    dib.resize(dib.len() + (mask_stride * size) as usize, 0);
    dib
}
