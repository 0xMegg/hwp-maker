use std::fs;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::error::AppError;

pub struct ImageAsset {
    pub data_url: String,
    pub mime: &'static str,
    pub ext: &'static str,
    pub natural_w_px: u32,
    pub natural_h_px: u32,
}

/// Read an image file from disk and return a `data:` URL plus its natural pixel
/// dimensions. Supports PNG and JPEG.
pub fn load(path: &Path) -> Result<ImageAsset, AppError> {
    let bytes = fs::read(path).map_err(|e| AppError::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let (mime, ext) = match detect(&bytes) {
        Some(m) => m,
        None => match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("png") => ("image/png", "png"),
            Some("jpg") | Some("jpeg") => ("image/jpeg", "jpg"),
            _ => {
                return Err(AppError::Spec(format!(
                    "unsupported image type: {}",
                    path.display()
                )))
            }
        },
    };

    let (w, h) = image::image_dimensions(path).map_err(|e| AppError::Image {
        path: path.display().to_string(),
        source: e,
    })?;

    let b64 = STANDARD.encode(&bytes);
    let data_url = format!("data:{};base64,{}", mime, b64);
    Ok(ImageAsset {
        data_url,
        mime,
        ext,
        natural_w_px: w,
        natural_h_px: h,
    })
}

fn detect(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return Some(("image/png", "png"));
    }
    if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        return Some(("image/jpeg", "jpg"));
    }
    None
}

/// Resolve an image path relative to the spec file's directory.
pub fn resolve(spec_dir: &Path, rel_or_abs: &str) -> PathBuf {
    let p = PathBuf::from(rel_or_abs);
    if p.is_absolute() {
        p
    } else {
        spec_dir.join(p)
    }
}
