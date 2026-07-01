use crate::commands::AppState;
use crate::paths::expand_user_path;
use augmented_gaussian_core::pipeline::process_file_with_cancel_and_progress;
use augmented_gaussian_core::{Manifest, ProcessConfig, RecipeBundle};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Emitter;

pub static CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRequest {
    pub input_path: PathBuf,
    pub out_dir: PathBuf,
    pub config_json: String,
    pub recipe_json: String,
    #[serde(default)]
    pub source_context: Option<ProcessSourceContext>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSourceContext {
    pub original_path: String,
    pub edited_path: String,
    pub edited_splat_count: usize,
    pub edited_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub stage: String,
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
    let source_context = request.source_context;
    let input_path = expand_user_path(request.input_path)?;
    let out_dir = expand_user_path(request.out_dir)?;
    let app_for_job = app.clone();
    let wgpu_ctx = state.wgpu_ctx.clone();
    let output = tauri::async_runtime::spawn_blocking(move || {
        process_file_with_cancel_and_progress(
            input_path,
            out_dir,
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
    let mut manifest = output.manifest;
    if let Some(context) = source_context {
        manifest.source.original_path = Some(context.original_path);
        manifest.source.edited_path = Some(context.edited_path);
        manifest.source.edited_splat_count = Some(context.edited_splat_count);
        manifest.source.edited_bytes = Some(context.edited_bytes);
    }
    Ok(manifest)
}

pub fn cancel_job() -> Result<bool, String> {
    CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    Ok(true)
}
