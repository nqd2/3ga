use crate::error::{AgError, AgResult};
use crate::mesh::Mesh;
use serde_json::json;
use std::fs;
use std::path::Path;

const GLB_MAGIC: u32 = 0x46546c67;
const GLB_VERSION: u32 = 2;
const JSON_CHUNK: u32 = 0x4e4f534a;
const BIN_CHUNK: u32 = 0x004e4942;

pub fn write_mesh_glb(path: impl AsRef<Path>, mesh: &Mesh, name: &str) -> AgResult<()> {
    let bytes = mesh_to_glb(mesh, name)?;
    fs::write(path, bytes)?;
    Ok(())
}

pub fn mesh_to_glb(mesh: &Mesh, name: &str) -> AgResult<Vec<u8>> {
    if !mesh.indices.len().is_multiple_of(3) {
        return Err(AgError::InvalidInput(
            "mesh index buffer length must be divisible by 3".to_string(),
        ));
    }

    let mut bin = Vec::new();
    for vertex in &mesh.vertices {
        for value in vertex {
            bin.extend_from_slice(&value.to_le_bytes());
        }
    }
    pad_to_4(&mut bin, 0);
    let index_offset = bin.len();
    for index in &mesh.indices {
        bin.extend_from_slice(&index.to_le_bytes());
    }
    pad_to_4(&mut bin, 0);

    let (min, max) = mesh_bounds(mesh);
    let json = json!({
        "asset": { "version": "2.0", "generator": "augmented-gaussian" },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": name }],
        "meshes": [{
            "name": name,
            "primitives": [{
                "attributes": { "POSITION": 0 },
                "indices": 1,
                "mode": 4,
                "material": 0
            }]
        }],
        "materials": [{
            "name": name,
            "doubleSided": true,
            "pbrMetallicRoughness": {
                "baseColorFactor": [0.18, 0.82, 0.64, 0.42],
                "metallicFactor": 0.0,
                "roughnessFactor": 0.85
            },
            "extras": { "ga3dRole": name }
        }],
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": mesh.vertices.len() * 12, "target": 34962 },
            { "buffer": 0, "byteOffset": index_offset, "byteLength": mesh.indices.len() * 4, "target": 34963 }
        ],
        "accessors": [
            {
                "bufferView": 0,
                "byteOffset": 0,
                "componentType": 5126,
                "count": mesh.vertices.len(),
                "type": "VEC3",
                "min": min,
                "max": max
            },
            {
                "bufferView": 1,
                "byteOffset": 0,
                "componentType": 5125,
                "count": mesh.indices.len(),
                "type": "SCALAR"
            }
        ]
    });

    let mut json_bytes = serde_json::to_vec(&json)?;
    pad_to_4(&mut json_bytes, b' ');

    let total_len = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&GLB_MAGIC.to_le_bytes());
    out.extend_from_slice(&GLB_VERSION.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&BIN_CHUNK.to_le_bytes());
    out.extend_from_slice(&bin);
    Ok(out)
}

pub fn write_navmesh_bin(path: impl AsRef<Path>, mesh: &Mesh) -> AgResult<()> {
    let mut out = Vec::new();
    out.extend_from_slice(b"AGNM");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(mesh.vertices.len() as u32).to_le_bytes());
    out.extend_from_slice(&(mesh.indices.len() as u32).to_le_bytes());
    for vertex in &mesh.vertices {
        for value in vertex {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    for index in &mesh.indices {
        out.extend_from_slice(&index.to_le_bytes());
    }
    fs::write(path, out)?;
    Ok(())
}

fn mesh_bounds(mesh: &Mesh) -> ([f32; 3], [f32; 3]) {
    if mesh.vertices.is_empty() {
        return ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        for i in 0..3 {
            min[i] = min[i].min(vertex[i]);
            max[i] = max[i].max(vertex[i]);
        }
    }
    (min, max)
}

fn pad_to_4(bytes: &mut Vec<u8>, value: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glb_has_valid_header() {
        let mesh = Mesh {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            triangles_before_merge: 1,
        };
        let bytes = mesh_to_glb(&mesh, "test").unwrap();
        assert_eq!(&bytes[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 2);
    }
}
