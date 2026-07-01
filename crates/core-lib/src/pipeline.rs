use crate::alignment::{AlignmentTransform, bake_alignment};
use crate::config::{EditOperation, ProcessConfig, RecipeBundle, VoxelBackend, VoxelCarveConfig};
use crate::error::{AgError, AgResult};
use crate::evaluation::mesh_error_against_splat_centers;
use crate::filters::{
    filter_box, filter_cluster_with_stats, filter_floaters_by_voxel_contribution_with_stats,
    filter_nan, filter_opacity_min, filter_sphere,
};
use crate::glb::{write_mesh_glb, write_navmesh_bin};
use crate::gpu::voxelize_gpu_blocking;
use crate::manifest::{AlignmentManifest, ArtifactManifest, Manifest, Metrics, SourceStats};
use crate::math::{Bounds, Vec3};
use crate::mesh::{Mesh, extract_mesh};
use crate::navmesh::bake_navmesh;
use crate::readers::{read_source, write_sog_bundle};
use crate::voxel::{VoxelParams, carve_grid_with_status, fill_grid_with_status, voxelize_cpu};
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
    let aligned_carve_config = aligned_voxel_carve_config(&config.voxel_carve, alignment_transform);
    progress("filters");
    table = filter_nan(&table)?;
    let mut collision_warnings = Vec::new();
    let mut filter_cluster_input_count = 0usize;
    let mut filter_cluster_output_count = 0usize;
    let mut filter_cluster_removed_count = 0usize;
    let mut filter_cluster_requested_seed = None;
    let mut filter_cluster_resolved_seed = None;
    let mut filter_cluster_seed_resolved = false;
    let mut filter_cluster_occupied_cells = 0usize;
    let mut filter_cluster_cells = 0usize;
    let mut floater_filter_input_count = 0usize;
    let mut floater_filter_output_count = 0usize;
    let mut floater_filter_removed_count = 0usize;
    if let Some(edit_recipe) = &recipe.edit_recipe {
        for op in &edit_recipe.operations {
            match op {
                EditOperation::SelectAll
                | EditOperation::SelectNone
                | EditOperation::FilterFloatersByVoxelContribution { .. } => {}
                EditOperation::FilterOpacity { min } => {
                    table = filter_opacity_min(&table, *min)?;
                }
                EditOperation::FilterBox { min, max } => {
                    let (min, max) = aligned_filter_box(alignment_transform, *min, *max);
                    table = filter_box(&table, min, max)?;
                }
                EditOperation::FilterSphere { center, radius } => {
                    let center = alignment_transform.apply_point(Vec3::from_array(*center));
                    table = filter_sphere(&table, center, *radius * alignment_transform.scale)?;
                }
                EditOperation::FilterCluster {
                    coarse_voxel_size,
                    opacity_threshold,
                    seed_pos,
                    min_contribution,
                } => {
                    let outcome = filter_cluster_with_stats(
                        &table,
                        *coarse_voxel_size,
                        *opacity_threshold,
                        alignment_transform.apply_point(Vec3::from_array(*seed_pos)),
                        *min_contribution,
                    )?;
                    filter_cluster_input_count += outcome.input_count;
                    filter_cluster_output_count = outcome.output_count;
                    filter_cluster_removed_count += outcome.removed_count;
                    filter_cluster_requested_seed = Some(outcome.requested_seed);
                    filter_cluster_resolved_seed = Some(outcome.resolved_seed);
                    filter_cluster_seed_resolved |= outcome.seed_was_resolved;
                    filter_cluster_occupied_cells = outcome.occupied_cells;
                    filter_cluster_cells = outcome.cluster_cells;
                    if outcome.seed_was_resolved {
                        collision_warnings.push(format!(
                            "filterCluster seed resolved from {:?} to nearest occupied voxel {:?}",
                            outcome.requested_seed, outcome.resolved_seed
                        ));
                    }
                    table = outcome.table;
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
    if let Some(min_contribution) = recipe.edit_recipe.as_ref().and_then(|recipe| {
        recipe.operations.iter().find_map(|op| {
            if let EditOperation::FilterFloatersByVoxelContribution { min_contribution } = op {
                Some(*min_contribution)
            } else {
                None
            }
        })
    }) {
        let outcome = filter_floaters_by_voxel_contribution_with_stats(
            &table,
            &cpu_grid,
            voxel_params,
            min_contribution,
        )?;
        floater_filter_input_count += outcome.input_count;
        floater_filter_output_count = outcome.output_count;
        floater_filter_removed_count += outcome.removed_count;
        table = outcome.table;
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
    let fill_outcome = fill_grid_with_status(
        &grid,
        &config.voxel_fill,
        Vec3::from_array(aligned_carve_config.seed_pos),
    );
    let fill_ms = start.elapsed().as_millis();
    if let Some(warning) = &fill_outcome.warning {
        collision_warnings.push(warning.clone());
    }
    let filled_solid_cells = fill_outcome.after_solid;
    let filled = fill_outcome.grid;
    check_cancelled(&should_cancel)?;

    progress("carve");
    let start = Instant::now();
    let carve_outcome = carve_grid_with_status(&filled, &aligned_carve_config);
    let carve_ms = start.elapsed().as_millis();
    if let Some(warning) = &carve_outcome.warning {
        collision_warnings.push(warning.clone());
    }
    let carve_reachable_cells = carve_outcome.reachable_cells;
    let carve_requested_seed = carve_outcome.requested_seed;
    let carve_resolved_seed = carve_outcome.resolved_seed;
    let carved_solid_cells = carve_outcome.after_solid;
    let carved = carve_outcome.grid;
    check_cancelled(&should_cancel)?;

    progress("mesh");
    let start = Instant::now();
    let (collision_grid, crop_stats) = carved.crop_to_occupied();
    let collision_mesh = extract_mesh(&collision_grid, config.mesh.mode)?;
    let mesh_ms = start.elapsed().as_millis();
    let triangle_count = collision_mesh.triangle_count();
    if triangle_count == 0 {
        collision_warnings.push(
            "collision mesh generated 0 triangles; check voxel size, opacity threshold, bake profile, or carve/fill seed"
                .to_string(),
        );
    }
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
    if config.navmesh.enabled && navmesh_triangle_count == 0 {
        collision_warnings.push(
            "navmesh generated 0 triangles; likely causes are a blocked seed, empty carve result, wrong floor plane, or wrong up axis"
                .to_string(),
        );
    }
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
            ..SourceStats::default()
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
            filter_cluster_input_count,
            filter_cluster_output_count,
            filter_cluster_removed_count,
            filter_cluster_requested_seed,
            filter_cluster_resolved_seed,
            filter_cluster_seed_resolved,
            filter_cluster_occupied_cells,
            filter_cluster_cells,
            floater_filter_input_count,
            floater_filter_output_count,
            floater_filter_removed_count,
            fill_ms,
            carve_ms,
            mesh_ms,
            navmesh_ms,
            export_ms: 0,
            voxel_solid_cells: grid.solid_count(),
            filled_solid_cells,
            carved_solid_cells,
            cropped_solid_cells: collision_grid.solid_count(),
            voxel_grid_dims: grid.dims,
            filled_grid_dims: filled.dims,
            carved_grid_dims: carved.dims,
            cropped_grid_dims: collision_grid.dims,
            crop_min_cell: crop_stats.min_cell,
            crop_max_cell: crop_stats.max_cell,
            carve_reachable_cells,
            carve_requested_seed,
            carve_resolved_seed,
            collision_warnings,
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

fn aligned_voxel_carve_config(
    config: &VoxelCarveConfig,
    transform: AlignmentTransform,
) -> VoxelCarveConfig {
    let mut aligned = config.clone();
    aligned.seed_pos = transform
        .apply_point(Vec3::from_array(config.seed_pos))
        .to_array();
    aligned
}

fn aligned_filter_box(transform: AlignmentTransform, min: [f32; 3], max: [f32; 3]) -> (Vec3, Vec3) {
    let min = Vec3::from_array(min);
    let max = Vec3::from_array(max);
    let mut bounds = Bounds::empty();
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                bounds.include(transform.apply_point(Vec3::new(x, y, z)));
            }
        }
    }
    (bounds.min, bounds.max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AlignmentRecipe, EditRecipe, FillMode, MeshMode};

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

    #[test]
    fn alignment_is_applied_to_downstream_seed_coordinates() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("scene.splat");
        fs::write(&input, splat_bytes_at(&[[2.0, 0.0, 0.0], [4.0, 0.0, 0.0]])).unwrap();
        let out = dir.path().join("out");

        let mut config = ProcessConfig::default();
        config.voxel.size = 0.25;
        config.voxel.opacity_threshold = 0.05;
        config.voxel_fill.mode = FillMode::None;
        config.voxel_carve.enabled = false;
        config.mesh.mode = MeshMode::Faces;
        config.navmesh.enabled = false;
        config.export.write_webar_zip = false;

        let recipe = RecipeBundle {
            alignment_recipe: Some(AlignmentRecipe {
                up_axis: None,
                floor_normal: None,
                scale_points: Some([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
                scale_distance_meters: Some(1.0),
                origin: None,
            }),
            edit_recipe: Some(EditRecipe {
                operations: vec![EditOperation::FilterCluster {
                    coarse_voxel_size: 0.25,
                    opacity_threshold: 0.05,
                    seed_pos: [2.0, 0.0, 0.0],
                    min_contribution: 0.05,
                }],
            }),
        };

        let output = process_file(&input, &out, &config, &recipe).unwrap();

        assert_eq!(output.manifest.source.kept_count, 1);
        let requested_seed = output
            .manifest
            .metrics
            .filter_cluster_requested_seed
            .unwrap();
        assert!((requested_seed[0] - 1.0).abs() < 1e-6);
        let bounds = output.manifest.bounds.unwrap();
        let center_x = (bounds.min.x + bounds.max.x) * 0.5;
        assert!(
            (center_x - 1.0).abs() < 0.05,
            "expected the source-space seed to select the first aligned cluster, got {bounds:?}"
        );
    }

    #[test]
    fn object_style_config_keeps_all_splats_without_cluster_filter() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("object.splat");
        fs::write(
            &input,
            splat_bytes_at(&[[0.0, 0.0, 0.0], [0.2, 0.0, 0.0], [0.0, 0.2, 0.0]]),
        )
        .unwrap();
        let out = dir.path().join("out");

        let mut config = ProcessConfig::default();
        config.voxel.size = 0.05;
        config.voxel.opacity_threshold = 0.1;
        config.voxel_fill.mode = FillMode::None;
        config.voxel_fill.dilation_size = 0.0;
        config.voxel_carve.enabled = false;
        config.mesh.mode = MeshMode::Smooth;
        config.navmesh.enabled = false;
        config.export.write_webar_zip = false;

        let output = process_file(&input, &out, &config, &RecipeBundle::default()).unwrap();

        assert_eq!(output.manifest.source.splat_count, 3);
        assert_eq!(output.manifest.source.kept_count, 3);
        assert_eq!(output.manifest.metrics.filter_cluster_input_count, 0);
        assert_eq!(output.manifest.metrics.filter_cluster_removed_count, 0);
        assert!(output.manifest.metrics.collision_triangles_after_merge > 0);
        assert!(
            output
                .manifest
                .metrics
                .collision_warnings
                .iter()
                .all(|warning| !warning.contains("navmesh generated 0 triangles"))
        );
    }

    fn single_splat_bytes() -> Vec<u8> {
        splat_bytes_at(&[[0.0, 0.0, 0.0]])
    }

    fn splat_bytes_at(points: &[[f32; 3]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for point in points {
            bytes.extend_from_slice(&point[0].to_le_bytes());
            bytes.extend_from_slice(&point[1].to_le_bytes());
            bytes.extend_from_slice(&point[2].to_le_bytes());
            bytes.extend_from_slice(&0.3f32.to_le_bytes());
            bytes.extend_from_slice(&0.3f32.to_le_bytes());
            bytes.extend_from_slice(&0.3f32.to_le_bytes());
            bytes.extend_from_slice(&[128, 128, 128, 255, 255, 128, 128, 128]);
        }
        bytes
    }
}
