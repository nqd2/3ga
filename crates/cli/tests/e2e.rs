use augmented_gaussian_core::readers::read_sog_bundle as read_exported_sog_bundle;
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

#[test]
fn process_supported_formats_end_to_end() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("config.json");
    fs::write(&config, basic_config()).unwrap();

    let splat = dir.path().join("input.splat");
    fs::write(&splat, splat_bytes()).unwrap();
    assert_processes(&splat, &dir.path().join("out-splat"), &config, "splat");

    let ply = dir.path().join("input.ply");
    fs::write(&ply, ply_bytes()).unwrap();
    assert_processes(&ply, &dir.path().join("out-ply"), &config, "ply");

    let sog_dir = dir.path().join("sog-dir");
    fs::create_dir_all(&sog_dir).unwrap();
    let files = synthetic_sog_files();
    for (name, bytes) in &files {
        fs::write(sog_dir.join(name), bytes).unwrap();
    }
    assert_processes(
        &sog_dir.join("meta.json"),
        &dir.path().join("out-meta"),
        &config,
        "sog",
    );

    let sog_bundle = dir.path().join("input.sog");
    write_sog_bundle(&sog_bundle, &files);
    assert_processes(&sog_bundle, &dir.path().join("out-sog"), &config, "sog");
}

#[test]
fn benchmark_compare_cpu_gpu_emits_parity_metrics() {
    let dir = tempdir().unwrap();
    let config = dir.path().join("config.json");
    fs::write(&config, basic_config()).unwrap();
    let splat = dir.path().join("input.splat");
    fs::write(&splat, splat_bytes()).unwrap();
    let out = dir.path().join("bench");

    let output = Command::new(env!("CARGO_BIN_EXE_augmented-gaussian-cli"))
        .args([
            "benchmark",
            "--input",
            splat.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
            "--compare-cpu-gpu",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(manifest["metrics"]["cpuGpuVoxelMismatches"], 0);
    assert!(manifest["metrics"]["gpuVoxelMs"].as_u64().is_some());
    assert!(manifest["metrics"]["gpuVoxelSpeedup"].as_f64().is_some());
}

#[test]
fn generate_benchmark_scenes_writes_deterministic_splat_files() {
    let dir = tempdir().unwrap();
    let out = dir.path().join("bench-scenes");
    let output = Command::new(env!("CARGO_BIN_EXE_augmented-gaussian-cli"))
        .args([
            "generate-bench-scenes",
            "--out",
            out.to_str().unwrap(),
            "--counts",
            "8,13",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(manifest["scenes"][0]["splatCount"], 8);
    assert_eq!(manifest["scenes"][0]["bytes"], 8 * 32);
    assert_eq!(manifest["scenes"][1]["splatCount"], 13);
    assert_eq!(manifest["scenes"][1]["bytes"], 13 * 32);
    assert_eq!(
        fs::metadata(out.join("bench-8.splat")).unwrap().len(),
        8 * 32
    );
    assert_eq!(
        fs::metadata(out.join("bench-13.splat")).unwrap().len(),
        13 * 32
    );
    assert!(out.join("benchmark-scenes.json").exists());
}

fn assert_processes(input: &Path, out: &Path, config: &Path, format: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_augmented-gaussian-cli"))
        .args([
            "process",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--config",
            config.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(manifest["source"]["format"], format);
    assert_eq!(manifest["source"]["splatCount"], 1);
    assert_eq!(manifest["alignment"]["unitScale"], 1.0);
    assert_eq!(manifest["alignment"]["rotationQuatWxyz"][0], 1.0);
    assert!(manifest["metrics"]["sceneSogBytes"].as_u64().unwrap() > 0);
    assert!(manifest["metrics"]["optimizedGlbBytes"].as_u64().unwrap() > 0);
    assert!(
        manifest["metrics"]["sourceToOptimizedGlbRatio"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        manifest["metrics"]["webarZipToSourceRatio"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    for name in [
        "manifest.json",
        "scene.sog",
        "collision_mesh.json",
        "occlusion.glb",
        "webar.zip",
    ] {
        assert!(out.join(name).exists(), "missing artifact {name}");
    }
    assert_eq!(
        read_exported_sog_bundle(out.join("scene.sog"))
            .unwrap()
            .len(),
        1
    );
    let manifest_file: Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest, manifest_file);
}

fn basic_config() -> &'static str {
    r#"{
  "voxel": { "backend": "cpu", "size": 0.25, "opacityThreshold": 0.05 },
  "voxelFill": { "mode": "none", "dilationSize": 0.0 },
  "voxelCarve": { "enabled": false, "agentHeight": 1.6, "agentRadius": 0.2, "seedPos": [0.0, 0.0, 0.0] },
  "mesh": { "mode": "faces" },
  "navmesh": { "enabled": false, "agentHeight": 1.6, "agentRadius": 0.2, "maxSlopeDegrees": 45.0, "cellSize": 0.1, "cellHeight": 0.05, "walkableClimb": 0.25, "minRegionSize": 4, "mergeRegionSize": 12 }
}"#
}

fn splat_bytes() -> Vec<u8> {
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

fn ply_bytes() -> Vec<u8> {
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
        0.0f32, 0.0, 0.0, -1.2, -1.2, -1.2, 8.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn synthetic_sog_files() -> Vec<(&'static str, Vec<u8>)> {
    let meta = serde_json::json!({
        "version": 2,
        "count": 1,
        "means": {
            "mins": [0.0, 0.0, 0.0],
            "maxs": [0.0, 0.0, 0.0],
            "files": ["means_l.webp", "means_u.webp"]
        },
        "scales": {
            "codebook": [-1.2],
            "files": ["scales.webp"]
        },
        "quats": {
            "files": ["quats.webp"]
        },
        "sh0": {
            "codebook": [0.0],
            "files": ["sh0.webp"]
        }
    });
    vec![
        ("meta.json", serde_json::to_vec(&meta).unwrap()),
        ("means_l.webp", png_rgba([0, 0, 0, 0])),
        ("means_u.webp", png_rgba([0, 0, 0, 0])),
        ("scales.webp", png_rgba([0, 0, 0, 0])),
        ("quats.webp", png_rgba([0, 0, 0, 0])),
        ("sh0.webp", png_rgba([0, 0, 0, 255])),
    ]
}

fn write_sog_bundle(path: &PathBuf, files: &[(&'static str, Vec<u8>)]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    for (name, bytes) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

fn png_rgba(pixel: [u8; 4]) -> Vec<u8> {
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(&pixel, 1, 1, ColorType::Rgba8.into())
        .unwrap();
    out
}
