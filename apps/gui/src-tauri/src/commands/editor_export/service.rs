use super::{EditorSourceExport, EditorSourceExportRequest, SceneTransformRequest};
use crate::paths::expand_user_path;
use augmented_gaussian_core::readers::{read_source, write_ply};
use glam::{EulerRot, Quat, Vec3};

pub async fn export_edited_source(
    request: EditorSourceExportRequest,
) -> Result<EditorSourceExport, String> {
    if request.deleted || !request.visible {
        return Err("editor scene is not visible".to_string());
    }
    validate_scene_transform(&request.transform)?;
    let input_path = expand_user_path(request.input_path)?;
    let absolute_input = std::fs::canonicalize(&input_path).map_err(|err| {
        format!(
            "failed to resolve absolute path for '{}': {}",
            input_path.to_string_lossy(),
            err
        )
    })?;
    let output_path = super::temp::next_editor_export_path("ply")?;
    let transform = request.transform.clone();
    let original_path = absolute_input.to_string_lossy().to_string();
    let path_for_job = output_path.clone();

    tauri::async_runtime::spawn_blocking(move || {
        let (mut table, _) = read_source(&absolute_input).map_err(|err| err.to_string())?;
        apply_scene_transform_to_table(&mut table, &transform);
        let bounds = table.scene_bounds();
        let splat_count = table.len();
        write_ply(&path_for_job, &table).map_err(|err| err.to_string())?;
        let bytes = std::fs::metadata(&path_for_job)
            .map_err(|err| err.to_string())?
            .len();
        Ok::<_, String>(EditorSourceExport {
            path: path_for_job.to_string_lossy().to_string(),
            original_path,
            bytes,
            splat_count,
            bounds,
        })
    })
    .await
    .map_err(|err| err.to_string())?
}

fn validate_scene_transform(transform: &SceneTransformRequest) -> Result<(), String> {
    if transform
        .position
        .iter()
        .chain(transform.rotation_euler_deg.iter())
        .any(|value| !value.is_finite())
    {
        return Err("scene transform contains a non-finite value".to_string());
    }
    Ok(())
}

pub(crate) fn apply_scene_transform_to_table(
    table: &mut augmented_gaussian_core::SplatTable,
    transform: &SceneTransformRequest,
) {
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        transform.rotation_euler_deg[0].to_radians(),
        transform.rotation_euler_deg[1].to_radians(),
        transform.rotation_euler_deg[2].to_radians(),
    )
    .normalize();
    let translation = Vec3::from_array(transform.position);
    if rotation == Quat::IDENTITY && translation == Vec3::ZERO {
        return;
    }
    for index in 0..table.len() {
        table.set_position(index, rotation * table.position(index) + translation);
        table.set_rotation(index, rotation * table.rotation(index));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_transform_updates_positions_and_rotation() {
        let mut table = augmented_gaussian_core::SplatTable::default();
        table.push_standard(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
            1.0,
            Vec3::ZERO,
            Quat::IDENTITY,
        );

        apply_scene_transform_to_table(
            &mut table,
            &SceneTransformRequest {
                position: [0.0, 2.0, 0.0],
                rotation_euler_deg: [0.0, 0.0, 90.0],
            },
        );

        let point = table.position(0);
        assert!(point.x.abs() < 1e-5);
        assert!((point.y - 3.0).abs() < 1e-5);
        assert!(point.z.abs() < 1e-5);
    }
}
