use crate::error::{AgError, AgResult};
use crate::math::{Quat, Vec3, QuatExt};
use crate::splat_table::SplatTable;
use image::ImageEncoder;
use serde::Deserialize;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::{ZipArchive, ZipWriter};

type SogShNPayload = (serde_json::Value, Vec<(String, Vec<u8>)>);

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SogMeta {
    V2(SogMetaV2),
    V1(SogMetaV1),
}

#[derive(Debug, Deserialize)]
struct SogMetaV2 {
    version: u32,
    count: usize,
    means: SogMeansV2,
    scales: SogCodebookFiles,
    quats: SogFiles,
    sh0: SogCodebookFiles,
    #[serde(rename = "shN")]
    sh_n: Option<SogShNV2>,
}

#[derive(Debug, Deserialize)]
struct SogMeansV2 {
    mins: [f32; 3],
    maxs: [f32; 3],
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SogCodebookFiles {
    codebook: Vec<f32>,
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SogFiles {
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SogShNV2 {
    count: usize,
    bands: usize,
    codebook: Vec<f32>,
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SogMetaV1 {
    means: SogMeansV1,
    scales: SogRangeFiles,
    quats: SogFiles,
    sh0: SogRangeFiles,
    #[serde(rename = "shN")]
    sh_n: Option<SogShNV1>,
}

#[derive(Debug, Deserialize)]
struct SogMeansV1 {
    shape: [usize; 2],
    mins: [f32; 3],
    maxs: [f32; 3],
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SogRangeFiles {
    mins: Vec<f32>,
    maxs: Vec<f32>,
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SogShNV1 {
    mins: f32,
    maxs: f32,
    files: Vec<String>,
}

struct RgbaTexture {
    rgba: Vec<u8>,
    width: usize,
    height: usize,
}

pub fn read_sog_bundle(path: impl AsRef<Path>) -> AgResult<SplatTable> {
    let bytes = fs::read(path)?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    read_sog_with_loader(|name| {
        let mut file = archive.by_name(name)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

pub fn read_sog_meta(path: impl AsRef<Path>) -> AgResult<SplatTable> {
    let path = path.as_ref();
    let base = path.parent().unwrap_or_else(|| Path::new(""));
    read_sog_with_loader(|name| fs::read(base.join(name)).map_err(AgError::from))
}

fn read_sog_with_loader<F>(mut load: F) -> AgResult<SplatTable>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    let meta_bytes = load("meta.json")?;
    let meta: SogMeta = serde_json::from_slice(&meta_bytes)?;
    match meta {
        SogMeta::V2(meta) => read_sog_v2(meta, load),
        SogMeta::V1(meta) => read_sog_v1(meta, load),
    }
}

fn read_sog_v2<F>(meta: SogMetaV2, mut load: F) -> AgResult<SplatTable>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    if meta.version != 2 {
        return Err(AgError::InvalidInput(format!(
            "unsupported SOG meta version {}",
            meta.version
        )));
    }
    let count = meta.count;
    let (x, y, z) = decode_sog_means(
        count,
        meta.means.mins,
        meta.means.maxs,
        &meta.means.files,
        &mut load,
    )?;
    let quats = decode_sog_quats(count, &meta.quats.files, &mut load)?;
    let scale_tex =
        decode_named_texture(required_file(&meta.scales.files, 0, "scales")?, &mut load)?;
    ensure_texture_count(&scale_tex, count, "SOG scales")?;
    let sh0_tex = decode_named_texture(required_file(&meta.sh0.files, 0, "sh0")?, &mut load)?;
    ensure_texture_count(&sh0_tex, count, "SOG sh0")?;
    let mut scale_0 = Vec::with_capacity(count);
    let mut scale_1 = Vec::with_capacity(count);
    let mut scale_2 = Vec::with_capacity(count);
    let mut f_dc_0 = Vec::with_capacity(count);
    let mut f_dc_1 = Vec::with_capacity(count);
    let mut f_dc_2 = Vec::with_capacity(count);
    let mut opacity = Vec::with_capacity(count);
    for i in 0..count {
        let o = i * 4;
        scale_0.push(codebook_value(
            &meta.scales.codebook,
            scale_tex.rgba[o],
            "scale",
        )?);
        scale_1.push(codebook_value(
            &meta.scales.codebook,
            scale_tex.rgba[o + 1],
            "scale",
        )?);
        scale_2.push(codebook_value(
            &meta.scales.codebook,
            scale_tex.rgba[o + 2],
            "scale",
        )?);
        f_dc_0.push(codebook_value(&meta.sh0.codebook, sh0_tex.rgba[o], "sh0")?);
        f_dc_1.push(codebook_value(
            &meta.sh0.codebook,
            sh0_tex.rgba[o + 1],
            "sh0",
        )?);
        f_dc_2.push(codebook_value(
            &meta.sh0.codebook,
            sh0_tex.rgba[o + 2],
            "sh0",
        )?);
        opacity.push(sigmoid_inv(sh0_tex.rgba[o + 3] as f32 / 255.0));
    }
    let f_rest = if let Some(sh_n) = meta.sh_n {
        decode_sog_shn_v2(count, sh_n, &mut load)?
    } else {
        Vec::new()
    };
    table_from_columns(SogColumns {
        x,
        y,
        z,
        scale_0,
        scale_1,
        scale_2,
        f_dc_0,
        f_dc_1,
        f_dc_2,
        opacity,
        quats,
        f_rest,
    })
}

fn read_sog_v1<F>(meta: SogMetaV1, mut load: F) -> AgResult<SplatTable>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    let count = meta.means.shape[0];
    let (x, y, z) = decode_sog_means(
        count,
        meta.means.mins,
        meta.means.maxs,
        &meta.means.files,
        &mut load,
    )?;
    let quats = decode_sog_quats(count, &meta.quats.files, &mut load)?;
    let scale_tex =
        decode_named_texture(required_file(&meta.scales.files, 0, "scales")?, &mut load)?;
    ensure_texture_count(&scale_tex, count, "SOG scales")?;
    let sh0_tex = decode_named_texture(required_file(&meta.sh0.files, 0, "sh0")?, &mut load)?;
    ensure_texture_count(&sh0_tex, count, "SOG sh0")?;
    let mut scale_0 = Vec::with_capacity(count);
    let mut scale_1 = Vec::with_capacity(count);
    let mut scale_2 = Vec::with_capacity(count);
    let mut f_dc_0 = Vec::with_capacity(count);
    let mut f_dc_1 = Vec::with_capacity(count);
    let mut f_dc_2 = Vec::with_capacity(count);
    let mut opacity = Vec::with_capacity(count);
    for i in 0..count {
        let o = i * 4;
        scale_0.push(lerp_range(
            &meta.scales.mins,
            &meta.scales.maxs,
            0,
            scale_tex.rgba[o],
        )?);
        scale_1.push(lerp_range(
            &meta.scales.mins,
            &meta.scales.maxs,
            1,
            scale_tex.rgba[o + 1],
        )?);
        scale_2.push(lerp_range(
            &meta.scales.mins,
            &meta.scales.maxs,
            2,
            scale_tex.rgba[o + 2],
        )?);
        f_dc_0.push(lerp_range(
            &meta.sh0.mins,
            &meta.sh0.maxs,
            0,
            sh0_tex.rgba[o],
        )?);
        f_dc_1.push(lerp_range(
            &meta.sh0.mins,
            &meta.sh0.maxs,
            1,
            sh0_tex.rgba[o + 1],
        )?);
        f_dc_2.push(lerp_range(
            &meta.sh0.mins,
            &meta.sh0.maxs,
            2,
            sh0_tex.rgba[o + 2],
        )?);
        opacity.push(lerp_range(
            &meta.sh0.mins,
            &meta.sh0.maxs,
            3,
            sh0_tex.rgba[o + 3],
        )?);
    }
    let f_rest = if let Some(sh_n) = meta.sh_n {
        decode_sog_shn_v1(count, sh_n, &mut load)?
    } else {
        Vec::new()
    };
    table_from_columns(SogColumns {
        x,
        y,
        z,
        scale_0,
        scale_1,
        scale_2,
        f_dc_0,
        f_dc_1,
        f_dc_2,
        opacity,
        quats,
        f_rest,
    })
}

fn decode_sog_means<F>(
    count: usize,
    mins: [f32; 3],
    maxs: [f32; 3],
    files: &[String],
    load: &mut F,
) -> AgResult<(Vec<f32>, Vec<f32>, Vec<f32>)>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    let lo = decode_named_texture(required_file(files, 0, "means low")?, load)?;
    let hi = decode_named_texture(required_file(files, 1, "means high")?, load)?;
    ensure_texture_count(&lo, count, "SOG means low")?;
    ensure_texture_count(&hi, count, "SOG means high")?;
    let mut x = Vec::with_capacity(count);
    let mut y = Vec::with_capacity(count);
    let mut z = Vec::with_capacity(count);
    for i in 0..count {
        let o = i * 4;
        let raw_x = lo.rgba[o] as u16 | ((hi.rgba[o] as u16) << 8);
        let raw_y = lo.rgba[o + 1] as u16 | ((hi.rgba[o + 1] as u16) << 8);
        let raw_z = lo.rgba[o + 2] as u16 | ((hi.rgba[o + 2] as u16) << 8);
        x.push(inv_log_transform(lerp_u16(mins[0], maxs[0], raw_x)));
        y.push(inv_log_transform(lerp_u16(mins[1], maxs[1], raw_y)));
        z.push(inv_log_transform(lerp_u16(mins[2], maxs[2], raw_z)));
    }
    Ok((x, y, z))
}

fn decode_sog_quats<F>(count: usize, files: &[String], load: &mut F) -> AgResult<Vec<Quat>>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    let texture = decode_named_texture(required_file(files, 0, "quats")?, load)?;
    ensure_texture_count(&texture, count, "SOG quats")?;
    let mut quats = Vec::with_capacity(count);
    for i in 0..count {
        let o = i * 4;
        quats.push(unpack_sog_quat(
            texture.rgba[o],
            texture.rgba[o + 1],
            texture.rgba[o + 2],
            texture.rgba[o + 3],
        ));
    }
    Ok(quats)
}

fn decode_sog_shn_v2<F>(count: usize, meta: SogShNV2, load: &mut F) -> AgResult<Vec<Vec<f32>>>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    let sh_coeffs = sh_coeff_count(meta.bands)?;
    if sh_coeffs == 0 {
        return Ok(Vec::new());
    }
    let centroids = decode_named_texture(required_file(&meta.files, 0, "shN centroids")?, load)?;
    let labels = decode_named_texture(required_file(&meta.files, 1, "shN labels")?, load)?;
    ensure_texture_count(&labels, count, "SOG shN labels")?;
    if centroids.width != 64 * sh_coeffs {
        return Err(AgError::InvalidInput(format!(
            "SOG shN centroids width {} does not match expected {}",
            centroids.width,
            64 * sh_coeffs
        )));
    }
    let mut rest = vec![vec![0.0; count]; sh_coeffs * 3];
    for (i, rgba) in labels.rgba.chunks_exact(4).take(count).enumerate() {
        let label = rgba[0] as usize | ((rgba[1] as usize) << 8);
        if label >= meta.count {
            continue;
        }
        for coeff in 0..sh_coeffs {
            if let Some([r, g, b]) = centroid_pixel(&centroids, label, coeff, sh_coeffs) {
                rest[coeff][i] = codebook_value(&meta.codebook, r, "shN")?;
                rest[coeff + sh_coeffs][i] = codebook_value(&meta.codebook, g, "shN")?;
                rest[coeff + sh_coeffs * 2][i] = codebook_value(&meta.codebook, b, "shN")?;
            }
        }
    }
    Ok(rest)
}

fn decode_sog_shn_v1<F>(count: usize, meta: SogShNV1, load: &mut F) -> AgResult<Vec<Vec<f32>>>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    let centroids = decode_named_texture(required_file(&meta.files, 0, "shN centroids")?, load)?;
    let labels = decode_named_texture(required_file(&meta.files, 1, "shN labels")?, load)?;
    ensure_texture_count(&labels, count, "SOG shN labels")?;
    let bands = match centroids.width {
        192 => 1,
        512 => 2,
        960 => 3,
        other => {
            return Err(AgError::InvalidInput(format!(
                "SOG V1 shN centroids width {other} is not recognized"
            )));
        }
    };
    let sh_coeffs = sh_coeff_count(bands)?;
    let palette_count = (centroids.width / sh_coeffs) * centroids.height;
    let mut rest = vec![vec![0.0; count]; sh_coeffs * 3];
    for (i, rgba) in labels.rgba.chunks_exact(4).take(count).enumerate() {
        let label = rgba[0] as usize | ((rgba[1] as usize) << 8);
        if label >= palette_count {
            continue;
        }
        for coeff in 0..sh_coeffs {
            if let Some([r, g, b]) = centroid_pixel(&centroids, label, coeff, sh_coeffs) {
                rest[coeff][i] = lerp_byte(meta.mins, meta.maxs, r);
                rest[coeff + sh_coeffs][i] = lerp_byte(meta.mins, meta.maxs, g);
                rest[coeff + sh_coeffs * 2][i] = lerp_byte(meta.mins, meta.maxs, b);
            }
        }
    }
    Ok(rest)
}

struct SogColumns {
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    scale_0: Vec<f32>,
    scale_1: Vec<f32>,
    scale_2: Vec<f32>,
    f_dc_0: Vec<f32>,
    f_dc_1: Vec<f32>,
    f_dc_2: Vec<f32>,
    opacity: Vec<f32>,
    quats: Vec<Quat>,
    f_rest: Vec<Vec<f32>>,
}

fn table_from_columns(cols: SogColumns) -> AgResult<SplatTable> {
    let count = cols.x.len();
    let mut table = SplatTable::default();
    for i in 0..count {
        table.push_standard(
            Vec3::new(cols.x[i], cols.y[i], cols.z[i]),
            Vec3::new(cols.scale_0[i], cols.scale_1[i], cols.scale_2[i]),
            cols.opacity[i],
            Vec3::new(cols.f_dc_0[i], cols.f_dc_1[i], cols.f_dc_2[i]),
            cols.quats[i],
        );
    }
    table.f_rest = cols.f_rest;
    table.validate()?;
    Ok(table)
}

fn decode_named_texture<F>(name: &str, load: &mut F) -> AgResult<RgbaTexture>
where
    F: FnMut(&str) -> AgResult<Vec<u8>>,
{
    let bytes = load(name)?;
    let image = image::load_from_memory(&bytes)?.to_rgba8();
    let (width, height) = image.dimensions();
    Ok(RgbaTexture {
        rgba: image.into_raw(),
        width: width as usize,
        height: height as usize,
    })
}

fn ensure_texture_count(texture: &RgbaTexture, count: usize, label: &str) -> AgResult<()> {
    if texture.width * texture.height < count {
        return Err(AgError::InvalidInput(format!(
            "{label} texture too small for count {count}"
        )));
    }
    Ok(())
}

fn required_file<'a>(files: &'a [String], index: usize, label: &str) -> AgResult<&'a str> {
    files
        .get(index)
        .map(|s| s.as_str())
        .ok_or_else(|| AgError::InvalidInput(format!("missing SOG {label} file")))
}

fn inv_log_transform(v: f32) -> f32 {
    let e = v.abs().exp() - 1.0;
    if v < 0.0 { -e } else { e }
}

fn lerp_u16(min: f32, max: f32, value: u16) -> f32 {
    let span = max - min;
    min + if span == 0.0 { 1.0 } else { span } * (value as f32 / 65535.0)
}

fn lerp_byte(min: f32, max: f32, value: u8) -> f32 {
    min + (max - min) * (value as f32 / 255.0)
}

fn sigmoid_inv(y: f32) -> f32 {
    let y = y.clamp(1e-6, 1.0 - 1e-6);
    (y / (1.0 - y)).ln()
}

fn unpack_sog_quat(px: u8, py: u8, pz: u8, tag: u8) -> Quat {
    if !(252..=255).contains(&tag) {
        return Quat::IDENTITY;
    }
    let max_comp = (tag - 252) as usize;
    let a = px as f32 / 255.0 * 2.0 - 1.0;
    let b = py as f32 / 255.0 * 2.0 - 1.0;
    let c = pz as f32 / 255.0 * 2.0 - 1.0;
    let sqrt2 = 2.0f32.sqrt();
    let mut comps = [0.0f32; 4];
    let idx = match max_comp {
        0 => [1, 2, 3],
        1 => [0, 2, 3],
        2 => [0, 1, 3],
        _ => [0, 1, 2],
    };
    comps[idx[0]] = a / sqrt2;
    comps[idx[1]] = b / sqrt2;
    comps[idx[2]] = c / sqrt2;
    let used = comps.iter().map(|v| v * v).sum::<f32>();
    comps[max_comp] = (1.0 - used).max(0.0).sqrt();
    Quat::from_wxyz(comps[0], comps[1], comps[2], comps[3])
        .normalized()
}

fn codebook_value(codebook: &[f32], index: u8, label: &str) -> AgResult<f32> {
    codebook.get(index as usize).copied().ok_or_else(|| {
        AgError::InvalidInput(format!(
            "SOG {label} codebook index {index} is out of range"
        ))
    })
}

fn lerp_range(mins: &[f32], maxs: &[f32], index: usize, value: u8) -> AgResult<f32> {
    let min = mins
        .get(index)
        .copied()
        .ok_or_else(|| AgError::InvalidInput(format!("missing SOG range min {index}")))?;
    let max = maxs
        .get(index)
        .copied()
        .ok_or_else(|| AgError::InvalidInput(format!("missing SOG range max {index}")))?;
    Ok(lerp_byte(min, max, value))
}

fn sh_coeff_count(bands: usize) -> AgResult<usize> {
    match bands {
        0 => Ok(0),
        1 => Ok(3),
        2 => Ok(8),
        3 => Ok(15),
        _ => Err(AgError::InvalidInput(format!(
            "unsupported SOG SH band count {bands}"
        ))),
    }
}

fn centroid_pixel(
    texture: &RgbaTexture,
    label: usize,
    coeff: usize,
    sh_coeffs: usize,
) -> Option<[u8; 3]> {
    let cx = (label % 64) * sh_coeffs + coeff;
    let cy = label / 64;
    if cx >= texture.width || cy >= texture.height {
        return None;
    }
    let index = (cy * texture.width + cx) * 4;
    Some([
        texture.rgba[index],
        texture.rgba[index + 1],
        texture.rgba[index + 2],
    ])
}

pub fn write_sog_bundle(path: impl AsRef<Path>, table: &SplatTable) -> AgResult<()> {
    table.validate()?;
    ensure_finite_table(table)?;
    let count = table.len();
    let (width, height) = texture_dims(count);
    let texels = width * height;

    let mut mean_values = Vec::with_capacity(count * 3);
    for i in 0..count {
        mean_values.push(log_transform(table.x[i]));
        mean_values.push(log_transform(table.y[i]));
        mean_values.push(log_transform(table.z[i]));
    }
    let means_minmax = component_minmax(&mean_values, 3);
    let mut means_l = vec![0u8; texels * 4];
    let mut means_u = vec![0u8; texels * 4];
    for i in 0..count {
        for axis in 0..3 {
            let value = mean_values[i * 3 + axis];
            let raw = quantize_u16(value, means_minmax[axis].0, means_minmax[axis].1);
            means_l[i * 4 + axis] = (raw & 0xff) as u8;
            means_u[i * 4 + axis] = (raw >> 8) as u8;
        }
    }

    let (scale_min, scale_max) = finite_minmax(
        table
            .scale_0
            .iter()
            .chain(table.scale_1.iter())
            .chain(table.scale_2.iter())
            .copied(),
        "SOG scale",
    )?;
    let scale_codebook = uniform_codebook(scale_min, scale_max);
    let mut scales = vec![0u8; texels * 4];
    for i in 0..count {
        scales[i * 4] = quantize_codebook(table.scale_0[i], scale_min, scale_max);
        scales[i * 4 + 1] = quantize_codebook(table.scale_1[i], scale_min, scale_max);
        scales[i * 4 + 2] = quantize_codebook(table.scale_2[i], scale_min, scale_max);
    }

    let (sh0_min, sh0_max) = finite_minmax(
        table
            .f_dc_0
            .iter()
            .chain(table.f_dc_1.iter())
            .chain(table.f_dc_2.iter())
            .copied(),
        "SOG sh0",
    )?;
    let sh0_codebook = uniform_codebook(sh0_min, sh0_max);
    let mut sh0 = vec![0u8; texels * 4];
    for i in 0..count {
        sh0[i * 4] = quantize_codebook(table.f_dc_0[i], sh0_min, sh0_max);
        sh0[i * 4 + 1] = quantize_codebook(table.f_dc_1[i], sh0_min, sh0_max);
        sh0[i * 4 + 2] = quantize_codebook(table.f_dc_2[i], sh0_min, sh0_max);
        sh0[i * 4 + 3] = (table.linear_alpha(i).clamp(0.0, 1.0) * 255.0).round() as u8;
    }

    let mut quats = vec![0u8; texels * 4];
    for i in 0..count {
        quats[i * 4..i * 4 + 4].copy_from_slice(&pack_sog_quat(table.rotation(i)));
    }

    let mut meta = serde_json::json!({
        "version": 2,
        "count": count,
        "means": {
            "mins": [means_minmax[0].0, means_minmax[1].0, means_minmax[2].0],
            "maxs": [means_minmax[0].1, means_minmax[1].1, means_minmax[2].1],
            "files": ["means_l.webp", "means_u.webp"]
        },
        "scales": {
            "codebook": scale_codebook,
            "files": ["scales.webp"]
        },
        "quats": {
            "files": ["quats.webp"]
        },
        "sh0": {
            "codebook": sh0_codebook,
            "files": ["sh0.webp"]
        }
    });

    let mut files = vec![
        (
            "means_l.webp".to_string(),
            encode_webp_rgba(&means_l, width, height)?,
        ),
        (
            "means_u.webp".to_string(),
            encode_webp_rgba(&means_u, width, height)?,
        ),
        (
            "scales.webp".to_string(),
            encode_webp_rgba(&scales, width, height)?,
        ),
        (
            "quats.webp".to_string(),
            encode_webp_rgba(&quats, width, height)?,
        ),
        (
            "sh0.webp".to_string(),
            encode_webp_rgba(&sh0, width, height)?,
        ),
    ];

    if let Some((shn_meta, shn_files)) = encode_sog_shn(table)? {
        meta.as_object_mut()
            .expect("SOG meta is an object")
            .insert("shN".to_string(), shn_meta);
        files.extend(shn_files);
    }

    let file = fs::File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("meta.json", options)?;
    zip.write_all(&serde_json::to_vec(&meta)?)?;
    for (name, bytes) in files {
        zip.start_file(name, options)?;
        zip.write_all(&bytes)?;
    }
    zip.finish()?;
    Ok(())
}

fn ensure_finite_table(table: &SplatTable) -> AgResult<()> {
    let columns: &[(&str, &[f32])] = &[
        ("x", table.x.as_slice()),
        ("y", table.y.as_slice()),
        ("z", table.z.as_slice()),
        ("scale_0", table.scale_0.as_slice()),
        ("scale_1", table.scale_1.as_slice()),
        ("scale_2", table.scale_2.as_slice()),
        ("opacity", table.opacity.as_slice()),
        ("f_dc_0", table.f_dc_0.as_slice()),
        ("f_dc_1", table.f_dc_1.as_slice()),
        ("f_dc_2", table.f_dc_2.as_slice()),
        ("rot_0", table.rot_0.as_slice()),
        ("rot_1", table.rot_1.as_slice()),
        ("rot_2", table.rot_2.as_slice()),
        ("rot_3", table.rot_3.as_slice()),
    ];
    for &(name, values) in columns {
        for (index, &value) in values.iter().enumerate() {
            if !value.is_finite() {
                return Err(AgError::InvalidInput(format!(
                    "SOG export column {name}[{index}] is not finite: {value}"
                )));
            }
        }
    }
    for (column, values) in table.f_rest.iter().enumerate() {
        for (index, &value) in values.iter().enumerate() {
            if !value.is_finite() {
                return Err(AgError::InvalidInput(format!(
                    "SOG export column f_rest_{column}[{index}] is not finite: {value}"
                )));
            }
        }
    }
    Ok(())
}

fn texture_dims(count: usize) -> (usize, usize) {
    if count == 0 {
        return (1, 1);
    }
    let width = count.min(4096);
    let height = count.div_ceil(width);
    (width, height)
}

fn log_transform(v: f32) -> f32 {
    let transformed = (v.abs() + 1.0).ln();
    if v < 0.0 { -transformed } else { transformed }
}

fn component_minmax(values: &[f32], stride: usize) -> [(f32, f32); 3] {
    let mut out = [(0.0, 0.0); 3];
    for axis in 0..3 {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for chunk in values.chunks_exact(stride) {
            min = min.min(chunk[axis]);
            max = max.max(chunk[axis]);
        }
        if min.is_finite() {
            out[axis] = (min, max);
        }
    }
    out
}

fn finite_minmax<I>(values: I, label: &str) -> AgResult<(f32, f32)>
where
    I: IntoIterator<Item = f32>,
{
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    let mut seen = false;
    for value in values {
        if !value.is_finite() {
            return Err(AgError::InvalidInput(format!(
                "{label} value is not finite: {value}"
            )));
        }
        min = min.min(value);
        max = max.max(value);
        seen = true;
    }
    if seen { Ok((min, max)) } else { Ok((0.0, 0.0)) }
}

fn uniform_codebook(min: f32, max: f32) -> Vec<f32> {
    (0..=255)
        .map(|i| {
            if (max - min).abs() <= f32::EPSILON {
                min
            } else {
                min + (max - min) * (i as f32 / 255.0)
            }
        })
        .collect()
}

fn quantize_u16(value: f32, min: f32, max: f32) -> u16 {
    if (max - min).abs() <= f32::EPSILON {
        0
    } else {
        (((value - min) / (max - min)).clamp(0.0, 1.0) * 65535.0).round() as u16
    }
}

fn quantize_codebook(value: f32, min: f32, max: f32) -> u8 {
    if (max - min).abs() <= f32::EPSILON {
        0
    } else {
        (((value - min) / (max - min)).clamp(0.0, 1.0) * 255.0).round() as u8
    }
}

fn encode_webp_rgba(rgba: &[u8], width: usize, height: usize) -> AgResult<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut out).write_image(
        rgba,
        width as u32,
        height as u32,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(out)
}

fn pack_sog_quat(quat: Quat) -> [u8; 4] {
    let mut comps = [quat.w, quat.x, quat.y, quat.z];
    let max_comp = comps
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .map(|(index, _)| index)
        .unwrap_or(0);
    if comps[max_comp] < 0.0 {
        for value in &mut comps {
            *value = -*value;
        }
    }
    let idx = match max_comp {
        0 => [1, 2, 3],
        1 => [0, 2, 3],
        2 => [0, 1, 3],
        _ => [0, 1, 2],
    };
    let sqrt2 = 2.0f32.sqrt();
    [
        quantize_quat_component(comps[idx[0]], sqrt2),
        quantize_quat_component(comps[idx[1]], sqrt2),
        quantize_quat_component(comps[idx[2]], sqrt2),
        252 + max_comp as u8,
    ]
}

fn quantize_quat_component(value: f32, sqrt2: f32) -> u8 {
    (((value * sqrt2).clamp(-1.0, 1.0) + 1.0) * 0.5 * 255.0).round() as u8
}

fn encode_sog_shn(table: &SplatTable) -> AgResult<Option<SogShNPayload>> {
    if table.f_rest.is_empty() {
        return Ok(None);
    }
    let count = table.len();
    let bands = match table.f_rest.len() / 3 {
        3 if table.f_rest.len() == 9 => 1,
        8 if table.f_rest.len() == 24 => 2,
        15 if table.f_rest.len() == 45 => 3,
        other => {
            return Err(AgError::InvalidInput(format!(
                "SOG export supports 9, 24, or 45 f_rest columns, got {other}"
            )));
        }
    };
    let sh_coeffs = table.f_rest.len() / 3;
    let palette_count = count.clamp(1, 65_536);
    let mut sums = vec![0.0f32; palette_count * sh_coeffs * 3];
    let mut counts = vec![0u32; palette_count];
    for i in 0..count {
        let label = sh_label(i, count, palette_count);
        counts[label] += 1;
        for channel in 0..3 {
            for coeff in 0..sh_coeffs {
                sums[(label * sh_coeffs + coeff) * 3 + channel] +=
                    table.f_rest[channel * sh_coeffs + coeff][i];
            }
        }
    }
    for label in 0..palette_count {
        let denom = counts[label].max(1) as f32;
        for coeff in 0..sh_coeffs {
            for channel in 0..3 {
                sums[(label * sh_coeffs + coeff) * 3 + channel] /= denom;
            }
        }
    }
    let (min, max) = finite_minmax(sums.iter().copied(), "SOG shN")?;
    let codebook = uniform_codebook(min, max);
    let centroid_width = 64 * sh_coeffs;
    let centroid_height = palette_count.div_ceil(64);
    let mut centroids = vec![0u8; centroid_width * centroid_height * 4];
    for label in 0..palette_count {
        let y = label / 64;
        let x0 = (label % 64) * sh_coeffs;
        for coeff in 0..sh_coeffs {
            let dst = (y * centroid_width + x0 + coeff) * 4;
            centroids[dst] = quantize_codebook(sums[(label * sh_coeffs + coeff) * 3], min, max);
            centroids[dst + 1] =
                quantize_codebook(sums[(label * sh_coeffs + coeff) * 3 + 1], min, max);
            centroids[dst + 2] =
                quantize_codebook(sums[(label * sh_coeffs + coeff) * 3 + 2], min, max);
            centroids[dst + 3] = 255;
        }
    }

    let (label_width, label_height) = texture_dims(count);
    let mut labels = vec![0u8; label_width * label_height * 4];
    for i in 0..count {
        let label = sh_label(i, count, palette_count) as u16;
        labels[i * 4] = (label & 0xff) as u8;
        labels[i * 4 + 1] = (label >> 8) as u8;
    }

    let meta = serde_json::json!({
        "count": palette_count,
        "bands": bands,
        "codebook": codebook,
        "files": ["shN_centroids.webp", "shN_labels.webp"]
    });
    Ok(Some((
        meta,
        vec![
            (
                "shN_centroids.webp".to_string(),
                encode_webp_rgba(&centroids, centroid_width, centroid_height)?,
            ),
            (
                "shN_labels.webp".to_string(),
                encode_webp_rgba(&labels, label_width, label_height)?,
            ),
        ],
    )))
}

fn sh_label(index: usize, count: usize, palette_count: usize) -> usize {
    if count <= palette_count {
        index
    } else {
        index % palette_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::png::PngEncoder;
    use image::ColorType;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    #[test]
    fn sog_writer_round_trips_v2_bundle() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("scene.sog");
        let mut table = SplatTable::default();
        table.push_standard(
            Vec3::new(-1.25, 0.5, 3.0),
            Vec3::new(-0.2, 0.1, 0.4),
            1.5,
            Vec3::new(0.1, -0.2, 0.3),
            Quat::from_wxyz(0.9238795, 0.0, 0.3826834, 0.0),
        );
        table.push_standard(
            Vec3::new(2.0, -0.75, 0.25),
            Vec3::new(0.3, -0.4, 0.0),
            -0.75,
            Vec3::new(-0.4, 0.2, 0.0),
            Quat::from_wxyz(0.8660254, 0.5, 0.0, 0.0),
        );

        write_sog_bundle(&path, &table).unwrap();
        let decoded = read_sog_bundle(&path).unwrap();

        assert_eq!(decoded.len(), 2);
        for i in 0..2 {
            let p0 = table.position(i);
            let p1 = decoded.position(i);
            assert!((p0 - p1).length() < 0.001);
            assert!((table.scale_0[i] - decoded.scale_0[i]).abs() < 0.01);
            assert!((table.scale_1[i] - decoded.scale_1[i]).abs() < 0.01);
            assert!((table.scale_2[i] - decoded.scale_2[i]).abs() < 0.01);
            assert!((table.linear_alpha(i) - decoded.linear_alpha(i)).abs() < 0.005);
            let a = table.rotation(i);
            let b = decoded.rotation(i);
            let dot = a.w * b.w + a.x * b.x + a.y * b.y + a.z * b.z;
            assert!(dot.abs() > 0.999);
        }
    }

    #[test]
    fn sog_bundle_and_meta_directory_match() {
        let dir = tempdir().unwrap();
        let files = synthetic_sog_files();
        for (name, bytes) in &files {
            fs::write(dir.path().join(name), bytes).unwrap();
        }

        let from_meta = read_sog_meta(dir.path().join("meta.json")).unwrap();

        let bundle_path = dir.path().join("scene.sog");
        {
            let file = fs::File::create(&bundle_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            for (name, bytes) in &files {
                zip.start_file(*name, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        let from_bundle = read_sog_bundle(bundle_path).unwrap();

        assert_eq!(from_meta.len(), 1);
        assert_eq!(from_bundle.len(), 1);
        assert_eq!(from_meta.position(0), from_bundle.position(0));
        assert_eq!(from_meta.scale_0[0], from_bundle.scale_0[0]);
        assert_eq!(from_meta.rotation(0), Quat::IDENTITY);
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
                "codebook": [0.0],
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
            ("sh0.webp", png_rgba([0, 0, 0, 128])),
        ]
    }

    fn png_rgba(pixel: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        PngEncoder::new(&mut out)
            .write_image(&pixel, 1, 1, ColorType::Rgba8.into())
            .unwrap();
        out
    }
}
