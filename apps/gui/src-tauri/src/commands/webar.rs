use crate::paths::expand_user_path;
use serde::Deserialize;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBundleRequest {
    pub out_dir: PathBuf,
    pub destination_path: PathBuf,
}

pub fn save_bundle(request: SaveBundleRequest) -> Result<String, String> {
    let out_dir = expand_user_path(request.out_dir)?;
    let destination_path = expand_user_path(request.destination_path)?;
    let source = out_dir.join("webar.zip");
    if let Some(parent) = destination_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create destination directory '{}': {}",
                parent.to_string_lossy(),
                err
            )
        })?;
    }
    std::fs::copy(&source, &destination_path).map_err(|err| {
        format!(
            "failed to copy '{}' to '{}': {}",
            source.to_string_lossy(),
            destination_path.to_string_lossy(),
            err
        )
    })?;
    Ok(destination_path.to_string_lossy().to_string())
}

pub fn open_webar_viewer(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let resolved_path = expand_user_path(PathBuf::from(&path))?;
    let absolute_path = std::fs::canonicalize(&resolved_path)
        .map_err(|err| format!("failed to resolve absolute path for '{}': {}", path, err))?;
    let path_str = absolute_path.to_string_lossy();
    let asset_url = format!("asset://localhost{}", path_str);
    let parsed_url = tauri::Url::parse(&asset_url).map_err(|err| err.to_string())?;

    if let Some(window) = app.get_webview_window("webar-viewer") {
        window.navigate(parsed_url).map_err(|e| e.to_string())?;
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(
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
