use std::path::{Path, PathBuf};

/// Keep the source bytes, resolution and animation; wallpaper is not artwork
/// and must not pass through thumbnail generation or single-frame decoding.
pub fn save(source: &Path, directory: &Path) -> Result<PathBuf, String> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|value| {
            matches!(
                value.as_str(),
                "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp"
            )
        })
        .ok_or("Choose a PNG, JPEG, WebP, GIF or BMP image")?;
    let metadata = std::fs::metadata(source).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > 25 * 1024 * 1024 {
        return Err("Choose an image smaller than 25 MiB".into());
    }
    let destination = directory.join(format!("background.{extension}"));
    if source != destination {
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        std::fs::copy(source, &destination).map_err(|error| error.to_string())?;
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animated_gif_keeps_original_canvas_frames_and_bytes() {
        // Two frames on a 1920x1080 canvas, with a looping animation extension.
        let mut gif = b"GIF89a\x80\x07\x38\x04\x80\0\0\0\0\0\xff\xff\xff\x21\xff\x0bNETSCAPE2.0\x03\x01\0\0\0".to_vec();
        for delay in [10, 20] {
            gif.extend_from_slice(&[0x21, 0xf9, 4, 0, delay, 0, 0, 0]);
            gif.extend_from_slice(b"\x2c\0\0\0\0\x01\0\x01\0\0\x02\x02\x44\x01\0");
        }
        gif.push(0x3b);
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("original.GIF");
        std::fs::write(&source, &gif).unwrap();
        let saved = save(&source, &temp.path().join("settings")).unwrap();
        assert_eq!(std::fs::read(&saved).unwrap(), gif);
        // Selecting the existing saved wallpaper must not truncate itself.
        assert_eq!(save(&saved, saved.parent().unwrap()).unwrap(), saved);
        assert_eq!(std::fs::read(saved).unwrap(), gif);
    }

    #[test]
    fn oversized_wallpaper_does_not_replace_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("large.gif");
        std::fs::File::create(&source)
            .unwrap()
            .set_len(25 * 1024 * 1024 + 1)
            .unwrap();
        let destination = temp.path().join("settings");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("background.gif"), b"old wallpaper").unwrap();
        assert!(save(&source, &destination).is_err());
        assert_eq!(
            std::fs::read(destination.join("background.gif")).unwrap(),
            b"old wallpaper"
        );
    }
}
