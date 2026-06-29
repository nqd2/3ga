mod commands;

use commands::AppState;

#[tauri::command]
fn load_source(path: String) -> Result<commands::SourceMetadata, String> {
    commands::load_source(path)
}

#[tauri::command]
async fn process_job(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: commands::ProcessRequest,
) -> Result<augmented_gaussian_core::Manifest, String> {
    commands::process_job(app, state, request).await
}

#[tauri::command]
fn cancel_job() -> Result<bool, String> {
    commands::cancel_job()
}

#[tauri::command]
fn save_bundle(request: commands::SaveBundleRequest) -> Result<String, String> {
    commands::save_bundle(request)
}

#[tauri::command]
fn preview_metadata(path: String) -> Result<serde_json::Value, String> {
    commands::preview_metadata(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let wgpu_ctx = augmented_gaussian_core::gpu::init_wgpu();
    let state = AppState { wgpu_ctx };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            load_source,
            preview_metadata,
            process_job,
            cancel_job,
            save_bundle
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
