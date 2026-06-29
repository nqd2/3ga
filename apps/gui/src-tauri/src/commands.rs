use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use augmented_gaussian_core::pipeline::process_file_with_cancel_and_progress;
use augmented_gaussian_core::readers::read_source;
use augmented_gaussian_core::{Manifest, ProcessConfig, RecipeBundle};

pub static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRequest {
    pub input_path: PathBuf,
    pub out_dir: PathBuf,
    pub config_json: String,
    pub recipe_json: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMetadata {
    pub path: String,
    pub bytes: u64,
    pub format: String,
    pub splat_count: usize,
    pub bounds: Option<augmented_gaussian_core::math::Bounds>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBundleRequest {
    pub out_dir: PathBuf,
    pub destination_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub stage: String,
}

pub struct AppState {
    pub wgpu_ctx: Option<augmented_gaussian_core::gpu::WgpuContext>,
}

pub fn load_source(path: String) -> Result<SourceMetadata, String> {
    let metadata = std::fs::metadata(&path).map_err(|err| err.to_string())?;
    let (table, format) = read_source(&path).map_err(|err| err.to_string())?;
    Ok(SourceMetadata {
        path,
        bytes: metadata.len(),
        format,
        splat_count: table.len(),
        bounds: Some(table.scene_bounds()),
    })
}

pub async fn process_job(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: ProcessRequest,
) -> Result<Manifest, String> {
    CANCEL_REQUESTED.store(false, Ordering::SeqCst);
    let config: ProcessConfig = serde_json::from_str(&request.config_json)
        .map_err(|err| format!("invalid config json: {err}"))?;
    let recipe: RecipeBundle = serde_json::from_str(&request.recipe_json)
        .map_err(|err| format!("invalid recipe json: {err}"))?;
    let app_for_job = app.clone();
    let wgpu_ctx = state.wgpu_ctx.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        process_file_with_cancel_and_progress(
            request.input_path,
            request.out_dir,
            &config,
            &recipe,
            || CANCEL_REQUESTED.load(Ordering::SeqCst),
            |stage| {
                let _ = app_for_job.emit(
                    "job-progress",
                    JobProgress {
                        stage: stage.to_string(),
                    },
                );
            },
            wgpu_ctx.as_ref(),
        )
    })
    .await
    .map_err(|err| err.to_string())?
    .map_err(|err| err.to_string())?;
    Ok(output.manifest)
}

pub fn cancel_job() -> Result<bool, String> {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    Ok(true)
}

pub fn save_bundle(request: SaveBundleRequest) -> Result<String, String> {
    let source = request.out_dir.join("webar.zip");
    std::fs::copy(&source, &request.destination_path).map_err(|err| err.to_string())?;
    Ok(request.destination_path.to_string_lossy().to_string())
}

pub fn preview_metadata(path: String) -> Result<serde_json::Value, String> {
    let metadata = std::fs::metadata(&path).map_err(|err| err.to_string())?;
    Ok(serde_json::json!({
        "path": path,
        "bytes": metadata.len()
    }))
}

pub fn open_webar_viewer(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let absolute_path = std::fs::canonicalize(&path)
        .map_err(|err| format!("failed to resolve absolute path for '{}': {}", path, err))?;
    let path_str = absolute_path.to_string_lossy();
    let asset_url = format!("asset://localhost{}", path_str);
    let parsed_url = tauri::Url::parse(&asset_url).map_err(|err| err.to_string())?;
    
    let _window = tauri::WebviewWindowBuilder::new(
        &app,
        "webar-viewer",
        tauri::WebviewUrl::External(parsed_url),
    )
    .title("WebAR Viewer Preview")
    .inner_size(1024.0, 768.0)
    .build()
    .map_err(|e| e.to_string())?;
    
    Ok(())
}
