use crate::error::{AgError, AgResult};
use crate::math::{Quat, Vec3, QuatExt};
use crate::splat_table::SplatTable;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
struct PlyProperty {
    name: String,
    ty: PlyType,
}

#[derive(Debug, Clone, Copy)]
enum PlyType {
    Float,
    Double,
    UChar,
    Int,
    UInt,
}

impl PlyType {
    fn size(self) -> usize {
        match self {
            PlyType::Float | PlyType::Int | PlyType::UInt => 4,
            PlyType::Double => 8,
            PlyType::UChar => 1,
        }
    }
}

pub fn read_ply(path: impl AsRef<Path>) -> AgResult<SplatTable> {
    let bytes = fs::read(path)?;
    read_ply_bytes(&bytes)
}

pub fn read_ply_bytes(bytes: &[u8]) -> AgResult<SplatTable> {
    let header_end = find_header_end(bytes)?;
    let header = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| AgError::InvalidInput("PLY header is not utf8".to_string()))?;
    let mut is_binary_le = false;
    let mut vertex_count = None;
    let mut in_vertex = false;
    let mut properties = Vec::new();
    for line in header.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            ["format", "binary_little_endian", _] => is_binary_le = true,
            ["element", "vertex", count] => {
                vertex_count = Some(count.parse::<usize>().map_err(|_| {
                    AgError::InvalidInput(format!("invalid vertex count: {count}"))
                })?);
                in_vertex = true;
            }
            ["element", _, _] => in_vertex = false,
            ["property", ty, name] if in_vertex => properties.push(PlyProperty {
                name: (*name).to_string(),
                ty: parse_ply_type(ty)?,
            }),
            _ => {}
        }
    }
    if !is_binary_le {
        return Err(AgError::InvalidInput(
            "only binary_little_endian PLY is supported".to_string(),
        ));
    }
    let vertex_count =
        vertex_count.ok_or_else(|| AgError::InvalidInput("missing vertex element".to_string()))?;
    let row_size: usize = properties.iter().map(|p| p.ty.size()).sum();
    let data_start = header_end + "end_header\n".len();
    let required = data_start + row_size * vertex_count;
    if bytes.len() < required {
        return Err(AgError::InvalidInput(
            "PLY vertex data is truncated".to_string(),
        ));
    }
    let mut table = SplatTable::default();
    for row in 0..vertex_count {
        let mut offset = data_start + row * row_size;
        let mut values = HashMap::new();
        for prop in &properties {
            let value = read_ply_value(bytes, offset, prop.ty)?;
            values.insert(prop.name.as_str(), value);
            offset += prop.ty.size();
        }
        table.push_standard(
            Vec3::new(
                get_required(&values, "x")?,
                get_required(&values, "y")?,
                get_required(&values, "z")?,
            ),
            Vec3::new(
                get_required(&values, "scale_0")?,
                get_required(&values, "scale_1")?,
                get_required(&values, "scale_2")?,
            ),
            get_required(&values, "opacity")?,
            Vec3::new(
                get_required(&values, "f_dc_0")?,
                get_required(&values, "f_dc_1")?,
                get_required(&values, "f_dc_2")?,
            ),
            Quat::from_wxyz(
                get_required(&values, "rot_0")?,
                get_required(&values, "rot_1")?,
                get_required(&values, "rot_2")?,
                get_required(&values, "rot_3")?,
            )
            .normalized(),
        );
    }
    table.validate()?;
    Ok(table)
}

fn find_header_end(bytes: &[u8]) -> AgResult<usize> {
    let marker = b"end_header\n";
    bytes
        .windows(marker.len())
        .position(|w| w == marker)
        .ok_or_else(|| AgError::InvalidInput("missing PLY end_header".to_string()))
}

fn parse_ply_type(ty: &str) -> AgResult<PlyType> {
    match ty {
        "float" | "float32" => Ok(PlyType::Float),
        "double" | "float64" => Ok(PlyType::Double),
        "uchar" | "uint8" => Ok(PlyType::UChar),
        "int" | "int32" => Ok(PlyType::Int),
        "uint" | "uint32" => Ok(PlyType::UInt),
        _ => Err(AgError::InvalidInput(format!(
            "unsupported PLY property type: {ty}"
        ))),
    }
}

fn read_ply_value(bytes: &[u8], offset: usize, ty: PlyType) -> AgResult<f32> {
    Ok(match ty {
        PlyType::Float => f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
        PlyType::Double => f64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as f32,
        PlyType::UChar => bytes[offset] as f32,
        PlyType::Int => i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as f32,
        PlyType::UInt => u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as f32,
    })
}

fn get_required(values: &HashMap<&str, f32>, name: &str) -> AgResult<f32> {
    values
        .get(name)
        .copied()
        .ok_or_else(|| AgError::InvalidInput(format!("missing required PLY property {name}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ply_requires_vertex() {
        let input = b"ply\nformat binary_little_endian 1.0\nend_header\n";
        let err = read_ply_bytes(input).unwrap_err().to_string();
        assert!(err.contains("missing vertex"));
    }

    #[test]
    fn ply_decodes_canonical_row() {
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
            1.0f32, 2.0, 3.0, 0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 1.0, 0.0, 0.0, 0.0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let table = read_ply_bytes(&bytes).unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table.position(0), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(table.scale_2[0], 0.2);
        assert_eq!(table.rotation(0), Quat::IDENTITY);
    }
}
