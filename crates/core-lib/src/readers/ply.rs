use crate::error::{AgError, AgResult};
use crate::math::{Quat, QuatExt, Vec3};
use crate::splat_table::SplatTable;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Write};
use std::path::Path;

const END_HEADER: &[u8] = b"end_header\n";
const MAX_HEADER_BYTES: usize = 1024 * 1024;
const ROW_CHUNK_SIZE: usize = 1024;
const STANDARD_PLY_COLUMNS: &[&str] = &[
    "x", "y", "z", "scale_0", "scale_1", "scale_2", "opacity", "f_dc_0", "f_dc_1", "f_dc_2",
    "rot_0", "rot_1", "rot_2", "rot_3",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlyType {
    Float,
    Double,
    Char,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
}

impl PlyType {
    fn size(self) -> usize {
        match self {
            PlyType::Char | PlyType::UChar => 1,
            PlyType::Short | PlyType::UShort => 2,
            PlyType::Float | PlyType::Int | PlyType::UInt => 4,
            PlyType::Double => 8,
        }
    }
}

#[derive(Debug, Clone)]
struct PlyProperty {
    name: String,
    ty: PlyType,
    byte_offset: usize,
}

#[derive(Debug, Clone)]
struct PlyHeader {
    vertex_count: usize,
    row_size: usize,
    properties: Vec<PlyProperty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlyMetadata {
    pub vertex_count: usize,
    pub row_size: usize,
}

#[derive(Debug, Clone, Copy)]
struct ValueSlot {
    ty: PlyType,
    offset: usize,
}

#[derive(Debug, Clone, Copy)]
struct StandardLayout {
    x: ValueSlot,
    y: ValueSlot,
    z: ValueSlot,
    scale_0: ValueSlot,
    scale_1: ValueSlot,
    scale_2: ValueSlot,
    opacity: ValueSlot,
    f_dc_0: ValueSlot,
    f_dc_1: ValueSlot,
    f_dc_2: ValueSlot,
    rot_0: ValueSlot,
    rot_1: ValueSlot,
    rot_2: ValueSlot,
    rot_3: ValueSlot,
}

pub fn read_ply(path: impl AsRef<Path>) -> AgResult<SplatTable> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let header_bytes = read_header_bytes(&mut reader)?;
    let header = parse_ply_header(&header_bytes)?;
    decode_ply_rows(&mut reader, &header)
}

pub fn read_ply_metadata(path: impl AsRef<Path>) -> AgResult<PlyMetadata> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let header_bytes = read_header_bytes(&mut reader)?;
    let header = parse_ply_header(&header_bytes)?;
    Ok(PlyMetadata {
        vertex_count: header.vertex_count,
        row_size: header.row_size,
    })
}

pub fn read_ply_bytes(bytes: &[u8]) -> AgResult<SplatTable> {
    let data_start = find_data_start(bytes)?;
    let header = parse_ply_header(&bytes[..data_start])?;
    let required = header
        .row_size
        .checked_mul(header.vertex_count)
        .ok_or_else(|| AgError::InvalidInput("PLY vertex data size overflows usize".to_string()))?;
    let data = bytes
        .get(data_start..data_start + required)
        .ok_or_else(|| AgError::InvalidInput("PLY vertex data is truncated".to_string()))?;
    let mut cursor = Cursor::new(data);
    decode_ply_rows(&mut cursor, &header)
}

pub fn write_ply(path: impl AsRef<Path>, table: &SplatTable) -> AgResult<()> {
    let file = File::create(path)?;
    write_ply_to(file, table)
}

pub fn write_ply_to(mut writer: impl Write, table: &SplatTable) -> AgResult<()> {
    table.validate()?;
    writeln!(writer, "ply")?;
    writeln!(writer, "format binary_little_endian 1.0")?;
    writeln!(writer, "element vertex {}", table.len())?;
    for name in STANDARD_PLY_COLUMNS {
        writeln!(writer, "property float {name}")?;
    }
    for index in 0..table.f_rest.len() {
        writeln!(writer, "property float f_rest_{index}")?;
    }
    writeln!(writer, "end_header")?;

    for i in 0..table.len() {
        for value in [
            table.x[i],
            table.y[i],
            table.z[i],
            table.scale_0[i],
            table.scale_1[i],
            table.scale_2[i],
            table.opacity[i],
            table.f_dc_0[i],
            table.f_dc_1[i],
            table.f_dc_2[i],
            table.rot_0[i],
            table.rot_1[i],
            table.rot_2[i],
            table.rot_3[i],
        ] {
            writer.write_all(&value.to_le_bytes())?;
        }
        for column in &table.f_rest {
            writer.write_all(&column[i].to_le_bytes())?;
        }
    }
    Ok(())
}

fn read_header_bytes(reader: &mut impl Read) -> AgResult<Vec<u8>> {
    let mut header = Vec::with_capacity(4096);
    let mut byte = [0u8; 1];
    loop {
        reader.read_exact(&mut byte).map_err(|err| {
            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                AgError::InvalidInput("missing PLY end_header".to_string())
            } else {
                err.into()
            }
        })?;
        header.push(byte[0]);
        if header.ends_with(END_HEADER) {
            return Ok(header);
        }
        if header.len() > MAX_HEADER_BYTES {
            return Err(AgError::InvalidInput(
                "PLY header exceeds 1 MiB limit".to_string(),
            ));
        }
    }
}

fn find_data_start(bytes: &[u8]) -> AgResult<usize> {
    bytes
        .windows(END_HEADER.len())
        .position(|w| w == END_HEADER)
        .map(|offset| offset + END_HEADER.len())
        .ok_or_else(|| AgError::InvalidInput("missing PLY end_header".to_string()))
}

fn parse_ply_header(bytes: &[u8]) -> AgResult<PlyHeader> {
    let header = std::str::from_utf8(bytes)
        .map_err(|_| AgError::InvalidInput("PLY header is not utf8".to_string()))?;
    let mut is_binary_le = false;
    let mut vertex_count = None;
    let mut in_vertex = false;
    let mut row_size = 0usize;
    let mut properties = Vec::new();

    for line in header.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            ["format", "binary_little_endian", _] => is_binary_le = true,
            ["format", _, _] => {
                return Err(AgError::InvalidInput(
                    "only binary_little_endian PLY is supported".to_string(),
                ));
            }
            ["element", "vertex", count] => {
                vertex_count = Some(count.parse::<usize>().map_err(|_| {
                    AgError::InvalidInput(format!("invalid vertex count: {count}"))
                })?);
                in_vertex = true;
            }
            ["element", _, _] => in_vertex = false,
            ["property", ty, name] if in_vertex => {
                let ty = parse_ply_type(ty)?;
                properties.push(PlyProperty {
                    name: (*name).to_string(),
                    ty,
                    byte_offset: row_size,
                });
                row_size += ty.size();
            }
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
    if row_size == 0 {
        return Err(AgError::InvalidInput(
            "vertex element has no properties".to_string(),
        ));
    }

    Ok(PlyHeader {
        vertex_count,
        row_size,
        properties,
    })
}

fn parse_ply_type(ty: &str) -> AgResult<PlyType> {
    match ty {
        "float" | "float32" => Ok(PlyType::Float),
        "double" | "float64" => Ok(PlyType::Double),
        "char" | "int8" => Ok(PlyType::Char),
        "uchar" | "uint8" => Ok(PlyType::UChar),
        "short" | "int16" => Ok(PlyType::Short),
        "ushort" | "uint16" => Ok(PlyType::UShort),
        "int" | "int32" => Ok(PlyType::Int),
        "uint" | "uint32" => Ok(PlyType::UInt),
        _ => Err(AgError::InvalidInput(format!(
            "unsupported PLY property type: {ty}"
        ))),
    }
}

fn decode_ply_rows(reader: &mut impl Read, header: &PlyHeader) -> AgResult<SplatTable> {
    let layout = StandardLayout::new(&header.properties)?;
    let mut table = SplatTable::default();
    reserve_standard_columns(&mut table, header.vertex_count);

    let mut chunk = vec![0u8; header.row_size * ROW_CHUNK_SIZE];
    let mut rows_remaining = header.vertex_count;
    while rows_remaining > 0 {
        let rows = rows_remaining.min(ROW_CHUNK_SIZE);
        let byte_len = rows * header.row_size;
        reader.read_exact(&mut chunk[..byte_len]).map_err(|err| {
            if err.kind() == std::io::ErrorKind::UnexpectedEof {
                AgError::InvalidInput("PLY vertex data is truncated".to_string())
            } else {
                err.into()
            }
        })?;

        for row in chunk[..byte_len].chunks_exact(header.row_size) {
            table.push_standard(
                Vec3::new(
                    read_slot(row, layout.x),
                    read_slot(row, layout.y),
                    read_slot(row, layout.z),
                ),
                Vec3::new(
                    read_slot(row, layout.scale_0),
                    read_slot(row, layout.scale_1),
                    read_slot(row, layout.scale_2),
                ),
                read_slot(row, layout.opacity),
                Vec3::new(
                    read_slot(row, layout.f_dc_0),
                    read_slot(row, layout.f_dc_1),
                    read_slot(row, layout.f_dc_2),
                ),
                Quat::from_wxyz(
                    read_slot(row, layout.rot_0),
                    read_slot(row, layout.rot_1),
                    read_slot(row, layout.rot_2),
                    read_slot(row, layout.rot_3),
                )
                .normalized(),
            );
        }
        rows_remaining -= rows;
    }

    table.validate()?;
    Ok(table)
}

fn reserve_standard_columns(table: &mut SplatTable, count: usize) {
    table.x.reserve(count);
    table.y.reserve(count);
    table.z.reserve(count);
    table.scale_0.reserve(count);
    table.scale_1.reserve(count);
    table.scale_2.reserve(count);
    table.opacity.reserve(count);
    table.f_dc_0.reserve(count);
    table.f_dc_1.reserve(count);
    table.f_dc_2.reserve(count);
    table.rot_0.reserve(count);
    table.rot_1.reserve(count);
    table.rot_2.reserve(count);
    table.rot_3.reserve(count);
}

impl StandardLayout {
    fn new(properties: &[PlyProperty]) -> AgResult<Self> {
        Ok(Self {
            x: find_slot(properties, "x")?,
            y: find_slot(properties, "y")?,
            z: find_slot(properties, "z")?,
            scale_0: find_slot(properties, "scale_0")?,
            scale_1: find_slot(properties, "scale_1")?,
            scale_2: find_slot(properties, "scale_2")?,
            opacity: find_slot(properties, "opacity")?,
            f_dc_0: find_slot(properties, "f_dc_0")?,
            f_dc_1: find_slot(properties, "f_dc_1")?,
            f_dc_2: find_slot(properties, "f_dc_2")?,
            rot_0: find_slot(properties, "rot_0")?,
            rot_1: find_slot(properties, "rot_1")?,
            rot_2: find_slot(properties, "rot_2")?,
            rot_3: find_slot(properties, "rot_3")?,
        })
    }
}

fn find_slot(properties: &[PlyProperty], name: &str) -> AgResult<ValueSlot> {
    properties
        .iter()
        .find(|prop| prop.name == name)
        .map(|prop| ValueSlot {
            ty: prop.ty,
            offset: prop.byte_offset,
        })
        .ok_or_else(|| AgError::InvalidInput(format!("missing required PLY property {name}")))
}

fn read_slot(row: &[u8], slot: ValueSlot) -> f32 {
    let offset = slot.offset;
    match slot.ty {
        PlyType::Float => f32::from_le_bytes(row[offset..offset + 4].try_into().unwrap()),
        PlyType::Double => f64::from_le_bytes(row[offset..offset + 8].try_into().unwrap()) as f32,
        PlyType::Char => row[offset] as i8 as f32,
        PlyType::UChar => row[offset] as f32,
        PlyType::Short => i16::from_le_bytes(row[offset..offset + 2].try_into().unwrap()) as f32,
        PlyType::UShort => u16::from_le_bytes(row[offset..offset + 2].try_into().unwrap()) as f32,
        PlyType::Int => i32::from_le_bytes(row[offset..offset + 4].try_into().unwrap()) as f32,
        PlyType::UInt => u32::from_le_bytes(row[offset..offset + 4].try_into().unwrap()) as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn canonical_header(vertex_count: usize) -> String {
        format!(
            concat!(
                "ply\n",
                "format binary_little_endian 1.0\n",
                "element vertex {}\n",
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
            ),
            vertex_count
        )
    }

    fn canonical_row(values: [f32; 14]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn ply_requires_vertex() {
        let input = b"ply\nformat binary_little_endian 1.0\nend_header\n";
        let err = read_ply_bytes(input).unwrap_err().to_string();
        assert!(err.contains("missing vertex"));
    }

    #[test]
    fn ply_decodes_canonical_row() {
        let mut bytes = canonical_header(1).into_bytes();
        bytes.extend_from_slice(&canonical_row([
            1.0, 2.0, 3.0, 0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 1.0, 0.0, 0.0, 0.0,
        ]));
        let table = read_ply_bytes(&bytes).unwrap();
        assert_eq!(table.len(), 1);
        assert_eq!(table.position(0), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(table.scale_2[0], 0.2);
        assert_eq!(table.rotation(0), Quat::IDENTITY);
    }

    #[test]
    fn ply_reads_header_metadata_without_decoding_rows() {
        let mut input = tempfile::Builder::new().suffix(".ply").tempfile().unwrap();
        input.write_all(canonical_header(99).as_bytes()).unwrap();

        let metadata = read_ply_metadata(input.path()).unwrap();

        assert_eq!(metadata.vertex_count, 99);
        assert_eq!(metadata.row_size, 14 * 4);
    }

    #[test]
    fn ply_decodes_mixed_property_types_with_precomputed_offsets() {
        let header = concat!(
            "ply\n",
            "format binary_little_endian 1.0\n",
            "element vertex 1\n",
            "property double x\n",
            "property int y\n",
            "property uint z\n",
            "property float scale_0\n",
            "property float scale_1\n",
            "property float scale_2\n",
            "property short opacity\n",
            "property float f_dc_0\n",
            "property float f_dc_1\n",
            "property float f_dc_2\n",
            "property float rot_0\n",
            "property float rot_1\n",
            "property float rot_2\n",
            "property float rot_3\n",
            "property uchar ignored\n",
            "end_header\n",
        );
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend_from_slice(&1.5f64.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        for value in [0.0f32, 0.1, 0.2] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&1i16.to_le_bytes());
        for value in [0.3f32, 0.4, 0.5, 1.0, 0.0, 0.0, 0.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(7);

        let table = read_ply_bytes(&bytes).unwrap();

        assert_eq!(table.position(0), Vec3::new(1.5, 2.0, 3.0));
        assert_eq!(table.opacity[0], 1.0);
        assert_eq!(table.rotation(0), Quat::IDENTITY);
    }

    #[test]
    fn file_reader_matches_byte_reader_for_float_fast_path() {
        let mut bytes = canonical_header(2).into_bytes();
        bytes.extend_from_slice(&canonical_row([
            1.0, 2.0, 3.0, 0.0, 0.1, 0.2, 1.0, 0.3, 0.4, 0.5, 1.0, 0.0, 0.0, 0.0,
        ]));
        bytes.extend_from_slice(&canonical_row([
            4.0, 5.0, 6.0, 0.2, 0.3, 0.4, 2.0, 0.6, 0.7, 0.8, 1.0, 0.0, 0.0, 0.0,
        ]));
        let mut input = tempfile::Builder::new().suffix(".ply").tempfile().unwrap();
        input.write_all(&bytes).unwrap();

        let from_bytes = read_ply_bytes(&bytes).unwrap();
        let from_file = read_ply(input.path()).unwrap();

        assert_eq!(from_file.len(), from_bytes.len());
        assert_eq!(from_file.position(1), from_bytes.position(1));
        assert_eq!(from_file.f_dc_2[1], from_bytes.f_dc_2[1]);
    }
}
