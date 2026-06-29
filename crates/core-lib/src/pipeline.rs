use crate::alignment::bake_alignment;
use crate::config::{EditOperation, ProcessConfig, RecipeBundle, VoxelBackend};
use crate::error::{AgError, AgResult};
use crate::evaluation::mesh_error_against_splat_centers;
use crate::filters::{
    filter_box, filter_cluster, filter_floaters_by_voxel_contribution, filter_nan,
    filter_opacity_min, filter_sphere,
};
use crate::glb::{write_mesh_glb, write_navmesh_bin};
use crate::gpu::voxelize_gpu_blocking;
use crate::manifest::{AlignmentManifest, ArtifactManifest, Manifest, Metrics, SourceStats};
use crate::math::Vec3;
use crate::mesh::{Mesh, extract_mesh};
use crate::navmesh::bake_navmesh;
use crate::readers::{read_source, write_sog_bundle};
use crate::voxel::{VoxelParams, carve_grid, fill_grid, voxelize_cpu};
use crate::webar::write_webar_zip;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessOutput {
    pub manifest: Manifest,
    pub collision_mesh: Mesh,
    pub navmesh: Option<Mesh>,
}

pub fn process_file(
    input_path: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    config: &ProcessConfig,
    recipe: &RecipeBundle,
) -> AgResult<ProcessOutput> {
    process_file_with_cancel(input_path, out_dir, config, recipe, || false)
}

pub fn process_file_with_cancel(
    input_path: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    config: &ProcessConfig,
    recipe: &RecipeBundle,
    should_cancel: impl Fn() -> bool,
) -> AgResult<ProcessOutput> {
    process_file_with_cancel_and_progress(
        input_path,
        out_dir,
        config,
        recipe,
        should_cancel,
        |_| {},
        None,
    )
}

pub fn process_file_with_cancel_and_progress(
    input_path: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
    config: &ProcessConfig,
    recipe: &RecipeBundle,
    should_cancel: impl Fn() -> bool,
    progress: impl Fn(&str),
    wgpu_ctx: Option<&crate::gpu::WgpuContext>,
) -> AgResult<ProcessOutput> {
    let input_path = input_path.as_ref();
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    progress("decode");
    let source_bytes = fs::metadata(input_path)
        .map(|m| m.len())
        .unwrap_or_default();
    let start = Instant::now();
    let (mut table, format) = read_source(input_path)?;
    let decode_ms = start.elapsed().as_millis();
    let source_count = table.len();
    check_cancelled(&should_cancel)?;

    progress("alignment");
    let start = Instant::now();
    let alignment_transform = bake_alignment(&mut table, recipe.alignment_recipe.as_ref())?;
    let alignment_ms = start.elapsed().as_millis();
    progress("filters");
    table = filter_nan(&table)?;
    if let Some(edit_recipe) = &recipe.edit_recipe {
        for op in &edit_recipe.operations {
            match op {
                EditOperation::SelectAll
                | EditOperation::SelectNone
                | EditOperation::FilterFloatersByVoxelContribution => {}
                EditOperation::FilterOpacity { min } => {
                    table = filter_opacity_min(&table, *min)?;
                }
                EditOperation::FilterBox { min, max } => {
                    table = filter_box(&table, Vec3::from_array(*min), Vec3::from_array(*max))?;
                }
                EditOperation::FilterSphere { center, radius } => {
                    table = filter_sphere(&table, Vec3::from_array(*center), *radius)?;
                }
                EditOperation::FilterCluster {
                    coarse_voxel_size,
                    opacity_threshold,
                    seed_pos,
                } => {
                    table = filter_cluster(
                        &table,
                        *coarse_voxel_size,
                        *opacity_threshold,
                        Vec3::from_array(*seed_pos),
                    )?;
                }
            }
        }
    }
    check_cancelled(&should_cancel)?;

    progress("voxelize");
    let voxel_params = VoxelParams {
        size: config.voxel.size,
        opacity_threshold: config.voxel.opacity_threshold,
    };
    let start = Instant::now();
    let mut cpu_grid = voxelize_cpu(&table, voxel_params)?;
    if recipe
        .edit_recipe
        .as_ref()
        .map(|recipe| {
            recipe
                .operations
                .iter()
                .any(|op| matches!(op, EditOperation::FilterFloatersByVoxelContribution))
        })
        .unwrap_or(false)
    {
        table = filter_floaters_by_voxel_contribution(&table, &cpu_grid, voxel_params)?;
        cpu_grid = voxelize_cpu(&table, voxel_params)?;
    }
    let cpu_voxel_ms = start.elapsed().as_millis();
    check_cancelled(&should_cancel)?;
    let mut gpu_voxel_ms = None;
    let mut cpu_gpu_voxel_mismatches = None;
    let (grid, voxel_ms, voxel_backend) = match config.voxel.backend {
        VoxelBackend::Cpu => {
            if config.voxel.compare_cpu_gpu {
                let start = Instant::now();
                let gpu_grid = voxelize_gpu_blocking(&table, voxel_params, wgpu_ctx)?;
                gpu_voxel_ms = Some(start.elapsed().as_millis());
                cpu_gpu_voxel_mismatches = Some(cpu_grid.mismatch_count(&gpu_grid));
                check_cancelled(&should_cancel)?;
            }
            (cpu_grid, cpu_voxel_ms, "cpu".to_string())
        }
        VoxelBackend::Gpu => {
            let start = Instant::now();
            let gpu_grid = voxelize_gpu_blocking(&table, voxel_params, wgpu_ctx)?;
            let elapsed = start.elapsed().as_millis();
            gpu_voxel_ms = Some(elapsed);
            let mismatches = cpu_grid.mismatch_count(&gpu_grid);
            if config.voxel.compare_cpu_gpu {
                cpu_gpu_voxel_mismatches = Some(mismatches);
            }
            (gpu_grid, elapsed, "gpu".to_string())
        }
    };
    check_cancelled(&should_cancel)?;

    progress("fill");
    let start = Instant::now();
    let filled = fill_grid(
        &grid,
        &config.voxel_fill,
        Vec3::from_array(config.voxel_carve.seed_pos),
    );
    let fill_ms = start.elapsed().as_millis();
    check_cancelled(&should_cancel)?;

    progress("carve");
    let start = Instant::now();
    let carved = carve_grid(&filled, &config.voxel_carve);
    let carve_ms = start.elapsed().as_millis();
    check_cancelled(&should_cancel)?;

    progress("mesh");
    let start = Instant::now();
    let collision_mesh = extract_mesh(&carved, config.mesh.mode)?;
    let mesh_ms = start.elapsed().as_millis();
    let triangle_count = collision_mesh.triangle_count();
    let geometric_error = mesh_error_against_splat_centers(&table, &collision_mesh);
    check_cancelled(&should_cancel)?;

    progress("navmesh");
    let start = Instant::now();
    let navmesh = bake_navmesh(&collision_mesh, &config.navmesh)?;
    let navmesh_ms = start.elapsed().as_millis();
    check_cancelled(&should_cancel)?;
    let mesh_path = out_dir.join("collision_mesh.json");
    let occlusion_glb_path = out_dir.join("occlusion.glb");
    let scene_name = "scene.sog".to_string();
    let scene_path = out_dir.join(&scene_name);
    let mut navmesh_glb_name = None;
    let mut navmesh_bin_name = None;
    let navmesh_triangle_count = navmesh.as_ref().map(|m| m.triangle_count()).unwrap_or(0);
    let navmesh_glb_path = out_dir.join("navmesh.glb");
    let navmesh_bin_path = out_dir.join("navmesh.bin");
    if navmesh.is_some() {
        navmesh_glb_name = Some(file_name(&navmesh_glb_path));
        navmesh_bin_name = Some(file_name(&navmesh_bin_path));
    } else {
        remove_if_exists(&navmesh_glb_path)?;
        remove_if_exists(&navmesh_bin_path)?;
    }

    progress("export");
    let start = Instant::now();
    fs::write(&mesh_path, serde_json::to_vec_pretty(&collision_mesh)?)?;
    write_mesh_glb(&occlusion_glb_path, &collision_mesh, "GA3D_OCCLUSION")?;
    write_sog_bundle(&scene_path, &table)?;
    if let Some(navmesh) = &navmesh {
        write_mesh_glb(&navmesh_glb_path, navmesh, "GA3D_NAVMESH")?;
        write_navmesh_bin(&navmesh_bin_path, navmesh)?;
    }
    let scene_sog_bytes = file_size(&scene_path);
    let optimized_glb_bytes = file_size(&occlusion_glb_path);

    let mut manifest = Manifest {
        version: 1,
        source: SourceStats {
            format,
            splat_count: source_count,
            kept_count: table.len(),
        },
        alignment: AlignmentManifest {
            unit_scale: alignment_transform.scale,
            origin: alignment_transform.origin.to_array(),
            rotation_quat_wxyz: [
                alignment_transform.rotation.w,
                alignment_transform.rotation.x,
                alignment_transform.rotation.y,
                alignment_transform.rotation.z,
            ],
        },
        bounds: Some(table.scene_bounds()),
        artifacts: ArtifactManifest {
            manifest: "manifest.json".to_string(),
            index_html: "index.html".to_string(),
            scene: scene_name.clone(),
            collision_mesh_json: file_name(&mesh_path),
            occlusion_glb: file_name(&occlusion_glb_path),
            navmesh_glb: navmesh_glb_name.clone(),
            navmesh_bin: navmesh_bin_name.clone(),
            webar_zip: config
                .export
                .write_webar_zip
                .then(|| "webar.zip".to_string()),
        },
        metrics: Metrics {
            decode_ms,
            alignment_ms,
            voxel_ms,
            cpu_voxel_ms,
            gpu_voxel_ms,
            gpu_voxel_speedup: gpu_voxel_ms.and_then(|gpu| {
                if gpu == 0 {
                    None
                } else {
                    Some(cpu_voxel_ms as f64 / gpu as f64)
                }
            }),
            voxel_backend,
            cpu_gpu_voxel_mismatches,
            fill_ms,
            carve_ms,
            mesh_ms,
            navmesh_ms,
            export_ms: 0,
            source_bytes,
            scene_sog_bytes,
            optimized_glb_bytes,
            source_to_optimized_glb_ratio: size_ratio(source_bytes, optimized_glb_bytes),
            optimized_glb_to_source_ratio: size_ratio(optimized_glb_bytes, source_bytes),
            source_to_webar_zip_ratio: None,
            webar_zip_to_source_ratio: None,
            collision_triangles_before_merge: collision_mesh.triangles_before_merge,
            collision_triangles_after_merge: triangle_count,
            navmesh_triangles: navmesh_triangle_count,
            webar_zip_bytes: 0,
            geometric_error_sample_count: geometric_error.sample_count,
            geometric_error_mean: geometric_error.mean,
            geometric_error_rms: geometric_error.rms,
            geometric_error_p95: geometric_error.p95,
        },
    };

    let manifest_path = out_dir.join("manifest.json");
    let index_html_path = out_dir.join("index.html");
    let asset_js_dir = out_dir.join("assets").join("js");
    let playcanvas_path = asset_js_dir.join("playcanvas.min.js");
    let playcanvas_license_path = asset_js_dir.join("playcanvas.LICENSE.txt");
    fs::create_dir_all(&asset_js_dir)?;
    fs::write(&playcanvas_path, crate::webar::playcanvas_runtime())?;
    fs::write(&playcanvas_license_path, crate::webar::playcanvas_license())?;
    fs::write(&index_html_path, crate::webar::viewer_html())?;
    let mut files = vec![
        ("index.html", index_html_path.clone()),
        ("assets/js/playcanvas.min.js", playcanvas_path.clone()),
        (
            "assets/js/playcanvas.LICENSE.txt",
            playcanvas_license_path.clone(),
        ),
        (scene_name.as_str(), scene_path.clone()),
        ("collision_mesh.json", mesh_path.clone()),
        ("occlusion.glb", occlusion_glb_path.clone()),
    ];
    if let Some(name) = &navmesh_glb_name {
        files.push((name.as_str(), out_dir.join(name)));
    }
    if let Some(name) = &navmesh_bin_name {
        files.push((name.as_str(), out_dir.join(name)));
    }
    let webar_zip_path = out_dir.join("webar.zip");
    if config.export.write_webar_zip {
        for _ in 0..10 {
            fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
            write_webar_zip(&webar_zip_path, &manifest, &files)?;
            let size = fs::metadata(&webar_zip_path).map(|m| m.len()).unwrap_or(0);
            let elapsed = start.elapsed().as_millis();
            if size == manifest.metrics.webar_zip_bytes {
                manifest.metrics.export_ms = elapsed;
                break;
            }
            manifest.metrics.webar_zip_bytes = size;
            manifest.metrics.source_to_webar_zip_ratio = size_ratio(source_bytes, size);
            manifest.metrics.webar_zip_to_source_ratio = size_ratio(size, source_bytes);
            manifest.metrics.export_ms = elapsed;
        }
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
        write_webar_zip(&webar_zip_path, &manifest, &files)?;
    } else {
        remove_if_exists(&webar_zip_path)?;
        manifest.metrics.export_ms = start.elapsed().as_millis();
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    }

    progress("done");
    Ok(ProcessOutput {
        manifest,
        collision_mesh,
        navmesh,
    })
}

fn check_cancelled(should_cancel: &impl Fn() -> bool) -> AgResult<()> {
    if should_cancel() {
        Err(AgError::Cancelled)
    } else {
        Ok(())
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string()
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn size_ratio(numerator: u64, denominator: u64) -> Option<f64> {
    if numerator == 0 || denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

fn remove_if_exists(path: &Path) -> AgResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MeshMode;

    #[test]
    fn optional_artifacts_are_absent_when_manifest_says_absent() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("scene.splat");
        fs::write(&input, single_splat_bytes()).unwrap();
        let out = dir.path().join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("navmesh.glb"), b"stale").unwrap();
        fs::write(out.join("navmesh.bin"), b"stale").unwrap();
        fs::write(out.join("webar.zip"), b"stale").unwrap();

        let mut config = ProcessConfig::default();
        config.voxel.size = 0.25;
        config.voxel.opacity_threshold = 0.05;
        config.mesh.mode = MeshMode::Faces;
        config.navmesh.enabled = false;
        config.export.write_webar_zip = false;

        let output = process_file(&input, &out, &config, &RecipeBundle::default()).unwrap();

        assert!(output.manifest.artifacts.navmesh_glb.is_none());
        assert!(output.manifest.artifacts.navmesh_bin.is_none());
        assert!(output.manifest.artifacts.webar_zip.is_none());
        assert!(!out.join("navmesh.glb").exists());
        assert!(!out.join("navmesh.bin").exists());
        assert!(!out.join("webar.zip").exists());
    }

    fn single_splat_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0.0f32.to_le_bytes());
        bytes.extend_from_slice(&0.0f32.to_le_bytes());
        bytes.extend_from_slice(&0.0f32.to_le_bytes());
        bytes.extend_from_slice(&0.3f32.to_le_bytes());
        bytes.extend_from_slice(&0.3f32.to_le_bytes());
        bytes.extend_from_slice(&0.3f32.to_le_bytes());
        bytes.extend_from_slice(&[128, 128, 128, 255, 128, 128, 128, 128]);
        bytes
    }
}