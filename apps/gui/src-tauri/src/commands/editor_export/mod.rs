mod service;
mod temp;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneTransformRequest {
    pub position: [f32; 3],
    pub rotation_euler_deg: [f32; 3],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSourceExportRequest {
    pub input_path: PathBuf,
    pub transform: SceneTransformRequest,
    pub visible: bool,
    pub deleted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSourceExport {
    pub path: String,
    pub original_path: String,
    pub bytes: u64,
    pub splat_count: usize,
    pub bounds: augmented_gaussian_core::math::Bounds,
}

pub async fn export_edited_source(
    request: EditorSourceExportRequest,
) -> Result<EditorSourceExport, String> {
    service::export_edited_source(request).await
}
