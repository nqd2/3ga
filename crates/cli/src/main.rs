use anyhow::Context;
use augmented_gaussian_core::{ProcessConfig, RecipeBundle, process_file};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(name = "augmented-gaussian-cli")]
#[command(about = "Process 3D Gaussian Splatting data into AR geometry artifacts")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Process {
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        recipe: Option<PathBuf>,
    },
    Benchmark {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = false)]
        compare_cpu_gpu: bool,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        recipe: Option<PathBuf>,
    },
    GenerateBenchScenes {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, value_delimiter = ',', default_value = "500000,5000000,10000000")]
        counts: Vec<usize>,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Process {
            input,
            out,
            config,
            recipe,
        } => run_pipeline(input, out, config, recipe),
        Command::Benchmark {
            input,
            out,
            compare_cpu_gpu,
            config,
            recipe,
        } => run_pipeline_with_benchmark(input, out, config, recipe, compare_cpu_gpu),
        Command::GenerateBenchScenes { out, counts } => generate_bench_scenes(out, counts),
    }
}

fn run_pipeline(
    input: PathBuf,
    out: PathBuf,
    config_path: Option<PathBuf>,
    recipe_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    run_pipeline_with_config(
        input,
        out,
        read_json_or_default::<ProcessConfig>(config_path)?,
        recipe_path,
    )
}

fn run_pipeline_with_benchmark(
    input: PathBuf,
    out: PathBuf,
    config_path: Option<PathBuf>,
    recipe_path: Option<PathBuf>,
    compare_cpu_gpu: bool,
) -> anyhow::Result<()> {
    let mut config = read_json_or_default::<ProcessConfig>(config_path)?;
    config.voxel.compare_cpu_gpu = compare_cpu_gpu;
    config.export.write_webar_zip = false;
    run_pipeline_with_config(input, out, config, recipe_path)
}

fn run_pipeline_with_config(
    input: PathBuf,
    out: PathBuf,
    config: ProcessConfig,
    recipe_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    let recipe = read_json_or_default::<RecipeBundle>(recipe_path)?;
    let output = process_file(&input, &out, &config, &recipe)
        .with_context(|| format!("failed to process {}", input.display()))?;
    println!("{}", serde_json::to_string_pretty(&output.manifest)?);
    Ok(())
}

fn read_json_or_default<T>(path: Option<PathBuf>) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    let Some(path) = path else {
        return Ok(T::default());
    };
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchSceneManifest {
    scenes: Vec<BenchSceneEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchSceneEntry {
    name: String,
    splat_count: usize,
    bytes: u64,
}

fn generate_bench_scenes(out: PathBuf, counts: Vec<usize>) -> anyhow::Result<()> {
    fs::create_dir_all(&out).with_context(|| format!("failed to create {}", out.display()))?;
    let mut scenes = Vec::new();
    for count in counts {
        let name = format!("bench-{count}.splat");
        let path = out.join(&name);
        write_synthetic_splat_scene(&path, count)
            .with_context(|| format!("failed to write {}", path.display()))?;
        let bytes = fs::metadata(&path)?.len();
        scenes.push(BenchSceneEntry {
            name,
            splat_count: count,
            bytes,
        });
    }
    let manifest = BenchSceneManifest { scenes };
    fs::write(
        out.join("benchmark-scenes.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}

fn write_synthetic_splat_scene(path: &Path, count: usize) -> anyhow::Result<()> {
    let file = fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    let side = (count as f64).cbrt().ceil().max(1.0) as usize;
    for i in 0..count {
        let ix = i % side;
        let iy = (i / side) % side;
        let iz = i / (side * side);
        let x = (ix as f32 - side as f32 * 0.5) * 0.08;
        let y = iy as f32 * 0.04;
        let z = (iz as f32 - side as f32 * 0.5) * 0.08;
        let scale = 0.035f32 + ((i % 11) as f32 * 0.001);
        writer.write_all(&x.to_le_bytes())?;
        writer.write_all(&y.to_le_bytes())?;
        writer.write_all(&z.to_le_bytes())?;
        writer.write_all(&scale.to_le_bytes())?;
        writer.write_all(&scale.to_le_bytes())?;
        writer.write_all(&(scale * 0.8).to_le_bytes())?;
        writer.write_all(&[
            (80 + (i % 100) as u8),
            (120 + (i % 80) as u8),
            (160 + (i % 60) as u8),
            220,
            255,
            128,
            128,
            128,
        ])?;
    }
    writer.flush()?;
    Ok(())
}
