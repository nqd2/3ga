use crate::error::{AgError, AgResult};
use crate::math::{Quat, Vec3, QuatExt};
use crate::splat_table::SplatTable;
use std::fs;
use std::path::Path;

pub const SH_C0: f32 = 0.282_094_8;

pub fn read_splat(path: impl AsRef<Path>) -> AgResult<SplatTable> {
    let bytes = fs::read(path)?;
    read_splat_bytes(&bytes)
}

pub fn read_splat_bytes(bytes: &[u8]) -> AgResult<SplatTable> {
    if bytes.is_empty() {
        return Err(AgError::InvalidInput("empty splat file".to_string()));
    }
    if !bytes.len().is_multiple_of(32) {
        return Err(AgError::InvalidInput(format!(
            "splat byte length {} is not divisible by 32",
            bytes.len()
        )));
    }
    let mut table = SplatTable::default();
    for chunk in bytes.chunks_exact(32) {
        let position = Vec3::new(
            f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
            f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
        );
        let sx = f32::from_le_bytes(chunk[12..16].try_into().unwrap());
        let sy = f32::from_le_bytes(chunk[16..20].try_into().unwrap());
        let sz = f32::from_le_bytes(chunk[20..24].try_into().unwrap());
        if sx <= 0.0 || sy <= 0.0 || sz <= 0.0 {
            return Err(AgError::InvalidInput(
                "splat linear scale must be positive".to_string(),
            ));
        }
        let scale = Vec3::new(sx.ln(), sy.ln(), sz.ln());
        let color_dc = Vec3::new(
            dc_from_u8(chunk[24]),
            dc_from_u8(chunk[25]),
            dc_from_u8(chunk[26]),
        );
        let alpha = (chunk[27] as f32 / 255.0).clamp(1e-6, 1.0 - 1e-6);
        let opacity = (alpha / (1.0 - alpha)).ln();
        let q = Quat::from_wxyz(
            byte_to_quat(chunk[28]),
            byte_to_quat(chunk[29]),
            byte_to_quat(chunk[30]),
            byte_to_quat(chunk[31]),
        )
        .normalized();
        table.push_standard(position, scale, opacity, color_dc, q);
    }
    table.validate()?;
    Ok(table)
}

pub fn write_splat_bytes(table: &SplatTable) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(table.len() * 32);
    for i in 0..table.len() {
        bytes.extend_from_slice(&table.x[i].to_le_bytes());
        bytes.extend_from_slice(&table.y[i].to_le_bytes());
        bytes.extend_from_slice(&table.z[i].to_le_bytes());
        bytes.extend_from_slice(&table.scale_0[i].exp().to_le_bytes());
        bytes.extend_from_slice(&table.scale_1[i].exp().to_le_bytes());
        bytes.extend_from_slice(&table.scale_2[i].exp().to_le_bytes());
        bytes.push(dc_to_u8(table.f_dc_0[i]));
        bytes.push(dc_to_u8(table.f_dc_1[i]));
        bytes.push(dc_to_u8(table.f_dc_2[i]));
        bytes.push((table.linear_alpha(i).clamp(0.0, 1.0) * 255.0).round() as u8);
        let q = table.rotation(i);
        bytes.push(quat_to_byte(q.w));
        bytes.push(quat_to_byte(q.x));
        bytes.push(quat_to_byte(q.y));
        bytes.push(quat_to_byte(q.z));
    }
    bytes
}

fn dc_from_u8(value: u8) -> f32 {
    ((value as f32 / 255.0) - 0.5) / SH_C0
}

fn dc_to_u8(value: f32) -> u8 {
    ((value * SH_C0 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8
}

fn byte_to_quat(value: u8) -> f32 {
    (value as f32 / 255.0) * 2.0 - 1.0
}

fn quat_to_byte(value: f32) -> u8 {
    (((value.clamp(-1.0, 1.0) + 1.0) * 0.5) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splat_decodes_one_row() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        bytes.extend_from_slice(&2.0f32.to_le_bytes());
        bytes.extend_from_slice(&3.0f32.to_le_bytes());
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        bytes.extend_from_slice(&2.0f32.to_le_bytes());
        bytes.extend_from_slice(&4.0f32.to_le_bytes());
        bytes.extend_from_slice(&[255, 128, 0, 128, 128, 128, 128, 128]);
        let table = read_splat_bytes(&bytes).unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table.position(0), Vec3::new(1.0, 2.0, 3.0));
        assert!((table.scale_1[0] - 2.0f32.ln()).abs() < 1e-6);
        assert!(table.rotation(0).w.is_finite());
    }

    #[test]
    fn splat_rejects_bad_length() {
        let err = read_splat_bytes(&[0; 31]).unwrap_err().to_string();
        assert!(err.contains("not divisible"));
    }

    #[test]
    fn splat_writer_round_trips_core_columns() {
        let mut table = SplatTable::default();
        table.push_standard(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.0, 0.5, 1.0),
            0.0,
            Vec3::new(0.1, -0.2, 0.3),
            Quat::IDENTITY,
        );
        let decoded = read_splat_bytes(&write_splat_bytes(&table)).unwrap();
        assert_eq!(decoded.position(0), Vec3::new(1.0, 2.0, 3.0));
        assert!((decoded.scale_1[0] - 0.5).abs() < 1e-6);
        assert!((decoded.linear_alpha(0) - 0.5019608).abs() < 1e-6);
    }
}
