pub mod bridge;

use serde_json;

#[tauri::command]
async fn run_job(
    input_path: String,
    output_dir: String,
    config: Option<serde_json::Value>,
    edit_recipe: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    // Parse edit recipe operations
    let mut ffi_ops = Vec::new();
    if let Some(recipe_val) = edit_recipe {
        if let Some(ops) = recipe_val.get("operations").and_then(|o| o.as_array()) {
            for op in ops {
                let op_type = op.get("type").and_then(|t| t.as_str()).unwrap_or("").to_string();
                
                let box_mode = op.get("mode").and_then(|m| {
                    match m.as_str() {
                        Some("add") => Some(1),
                        Some("remove") => Some(2),
                        _ => Some(0), // set or default
                    }
                }).unwrap_or(0);
                
                let center = op.get("center").and_then(|c| c.as_array()).map(|c| {
                    [
                        c.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        c.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        c.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    ]
                }).unwrap_or([0.0, 0.0, 0.0]);

                let size = op.get("size").and_then(|s| s.as_array()).map(|s| {
                    [
                        s.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        s.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        s.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    ]
                }).unwrap_or([0.0, 0.0, 0.0]);

                let matrix = op.get("matrix").and_then(|m| m.as_array()).map(|m| {
                    let mut mat = [0.0f32; 16];
                    for i in 0..16 {
                        mat[i] = m.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    }
                    mat
                }).unwrap_or({
                    let mut mat = [0.0f32; 16];
                    mat[0] = 1.0; mat[5] = 1.0; mat[10] = 1.0; mat[15] = 1.0;
                    mat
                });

                let opacity_min = op.get("min").and_then(|m| m.as_f64()).unwrap_or(0.0) as f32;

                ffi_ops.push(bridge::ffi::RustEditOp {
                    op_type,
                    box_mode,
                    center,
                    size,
                    matrix,
                    opacity_min,
                });
            }
        }
    }

    // Default configuration values
    let mut voxel_size = 0.1f32;
    let mut opacity_cutoff = 0.01f32;
    let mut sigma = 1.0f32;
    let mut align_to_blocks = false;

    let mut navmesh_enabled = true;
    let mut navmesh_seed = [0.0f32; 3];
    let mut agent_height = 1.8f32;
    let mut agent_radius = 0.3f32;
    let mut max_slope_degrees = 45.0f32;
    let mut cell_size = 0.05f32;
    let mut cell_height = 0.05f32;

    let mut mesh_mode = "faces".to_string();

    // Parse config if provided
    if let Some(config_val) = config {
        if let Some(vs) = config_val.get("voxelSize").and_then(|v| v.as_f64()) {
            voxel_size = vs as f32;
        }
        if let Some(oc) = config_val.get("opacityCutoff").and_then(|v| v.as_f64()) {
            opacity_cutoff = oc as f32;
        }
        if let Some(s) = config_val.get("sigma").and_then(|v| v.as_f64()) {
            sigma = s as f32;
        }
        if let Some(a) = config_val.get("alignToBlocks").and_then(|v| v.as_bool()) {
            align_to_blocks = a;
        }
        if let Some(mm) = config_val.get("meshMode").and_then(|v| v.as_str()) {
            mesh_mode = mm.to_string();
        }

        if let Some(nav) = config_val.get("navmesh") {
            if let Some(enabled) = nav.get("enabled").and_then(|v| v.as_bool()) {
                navmesh_enabled = enabled;
            }
            if let Some(seed) = nav.get("seed").and_then(|v| v.as_array()) {
                navmesh_seed = [
                    seed.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    seed.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                    seed.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                ];
            }
            if let Some(ah) = nav.get("agentHeight").and_then(|v| v.as_f64()) {
                agent_height = ah as f32;
            }
            if let Some(ar) = nav.get("agentRadius").and_then(|v| v.as_f64()) {
                agent_radius = ar as f32;
            }
            if let Some(ms) = nav.get("maxSlopeDegrees").and_then(|v| v.as_f64()) {
                max_slope_degrees = ms as f32;
            }
            if let Some(cs) = nav.get("cellSize").and_then(|v| v.as_f64()) {
                cell_size = cs as f32;
            }
            if let Some(ch) = nav.get("cellHeight").and_then(|v| v.as_f64()) {
                cell_height = ch as f32;
            }
        }
    }

    let ffi_voxel_opts = bridge::ffi::RustVoxelOptions {
        voxel_size,
        opacity_cutoff,
        sigma,
        align_to_blocks,
    };

    let ffi_nav_cfg = bridge::ffi::RustNavConfig {
        enabled: navmesh_enabled,
        seed: navmesh_seed,
        agent_height,
        agent_radius,
        max_slope_degrees,
        cell_size,
        cell_height,
    };

    // Run the pipeline using tauri's async_runtime::spawn_blocking
    let result_str = tauri::async_runtime::spawn_blocking(move || {
        cxx::let_cxx_string!(c_input_path = input_path);
        cxx::let_cxx_string!(c_output_dir = output_dir);
        cxx::let_cxx_string!(c_mesh_mode = mesh_mode);

        let res_ptr = bridge::ffi::run_pipeline_rust(
            &c_input_path,
            &c_output_dir,
            &ffi_ops,
            &ffi_voxel_opts,
            &ffi_nav_cfg,
            &c_mesh_mode,
        );
        
        if res_ptr.is_null() {
            Err("run_pipeline_rust returned null".to_string())
        } else {
            Ok(res_ptr.to_string())
        }
    })
    .await
    .map_err(|e| format!("Failed to spawn task: {}", e))?
    .map_err(|e| format!("C++ processing error: {}", e))?;

    let parsed_result: serde_json::Value = serde_json::from_str(&result_str)
        .map_err(|e| format!("Failed to parse C++ result JSON: {}", e))?;

    Ok(parsed_result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![run_job])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
