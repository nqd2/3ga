use crate::paths::expand_user_path;
use augmented_gaussian_core::readers::read_ply_metadata;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMetadata {
    pub path: String,
    pub bytes: u64,
    pub format: String,
    pub splat_count: usize,
    pub bounds: Option<augmented_gaussian_core::math::Bounds>,
    pub preview_path: Option<String>,
}

pub fn load_source(path: String) -> Result<SourceMetadata, String> {
    let resolved_path = expand_user_path(PathBuf::from(&path))?;
    let absolute_path = std::fs::canonicalize(&resolved_path)
        .map_err(|err| format!("failed to resolve absolute path for '{}': {}", path, err))?;
    let metadata = std::fs::metadata(&absolute_path).map_err(|err| err.to_string())?;
    let ext = absolute_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let (format, splat_count) = match ext.as_str() {
        "ply" => {
            let metadata = read_ply_metadata(&absolute_path).map_err(|err| err.to_string())?;
            ("ply".to_string(), metadata.vertex_count)
        }
        "splat" => {
            if metadata.len() == 0 || metadata.len() % 32 != 0 {
                return Err(format!(
                    "invalid .splat byte length {}; expected a non-empty multiple of 32",
                    metadata.len()
                ));
            }
            ("splat".to_string(), (metadata.len() / 32) as usize)
        }
        _ => return Err(format!("unsupported input format: {ext}")),
    };

    Ok(SourceMetadata {
        path: absolute_path.to_string_lossy().to_string(),
        bytes: metadata.len(),
        format,
        splat_count,
        bounds: None,
        preview_path: None,
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn minimal_ply_bytes() -> Vec<u8> {
        let header = concat!(
            "ply\n",
            "format binary_little_endian 1.0\n",
            "element vertex 1\n",
            "property float x\n",
            "property float y\n",
            "property float z\n",
            "property float scale_0\n",
            "property float scale_1\n",
            "property float scale_2\n",
            "property float opacity\n",
            "property float f_dc_0\n",
            "property float f_dc_1\n",
            "property float f_dc_2\n",
            "property float rot_0\n",
            "property float rot_1\n",
            "property float rot_2\n",
            "property float rot_3\n",
            "end_header\n",
        );
        let mut bytes = header.as_bytes().to_vec();
        for value in [
            1.0f32, 2.0, 3.0, 0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 1.0, 0.0, 0.0, 0.0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn ply_load_source_uses_fast_metadata_without_preview_file() {
        let mut input = tempfile::Builder::new().suffix(".ply").tempfile().unwrap();
        input.write_all(&minimal_ply_bytes()).unwrap();

        let metadata = load_source(input.path().to_string_lossy().to_string()).unwrap();

        assert!(std::path::Path::new(&metadata.path).is_absolute());
        assert_eq!(metadata.format, "ply");
        assert_eq!(metadata.splat_count, 1);
        assert!(metadata.bounds.is_none());
        assert!(metadata.preview_path.is_none());
    }

    #[test]
    fn splat_load_source_derives_count_from_byte_length() {
        let mut input = tempfile::Builder::new()
            .suffix(".splat")
            .tempfile()
            .unwrap();
        input.write_all(&[0; 64]).unwrap();

        let metadata = load_source(input.path().to_string_lossy().to_string()).unwrap();

        assert!(std::path::Path::new(&metadata.path).is_absolute());
        assert_eq!(metadata.format, "splat");
        assert_eq!(metadata.splat_count, 2);
        assert!(metadata.bounds.is_none());
        assert!(metadata.preview_path.is_none());
    }
}
