use crate::config::MeshMode;
use crate::error::AgResult;
use crate::math::Vec3;
use crate::voxel::VoxelGrid;
use lin_alg::f32::Vec3 as McVec3;
use mcubes::{MarchingCubes, MeshSide};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    #[serde(default)]
    pub triangles_before_merge: usize,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

pub fn extract_mesh(grid: &VoxelGrid, mode: MeshMode) -> AgResult<Mesh> {
    match mode {
        MeshMode::Faces => Ok(extract_faces(grid)),
        MeshMode::Smooth => Ok(extract_smooth(grid)?),
    }
}

pub fn extract_faces(grid: &VoxelGrid) -> Mesh {
    let mut mesh = Mesh::default();
    let dirs = [
        (-1isize, 0isize, 0isize),
        (1, 0, 0),
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
    ];
    for (x, y, z) in grid.iter_solid() {
        for dir in dirs {
            let nx = x as isize + dir.0;
            let ny = y as isize + dir.1;
            let nz = z as isize + dir.2;
            let neighbor_solid =
                nx >= 0 && ny >= 0 && nz >= 0 && grid.get(nx as usize, ny as usize, nz as usize);
            if !neighbor_solid {
                emit_face(&mut mesh, grid, x, y, z, dir);
            }
        }
    }
    mesh.triangles_before_merge = mesh.triangle_count();
    mesh
}

pub fn extract_smooth(grid: &VoxelGrid) -> AgResult<Mesh> {
    if grid.solid_count() == 0 {
        return Ok(Mesh::default());
    }
    let dims = (grid.dims[0] + 2, grid.dims[1] + 2, grid.dims[2] + 2);
    let mut values = vec![1.0f32; dims.0 * dims.1 * dims.2];
    for z in 0..grid.dims[2] {
        for y in 0..grid.dims[1] {
            for x in 0..grid.dims[0] {
                if grid.get(x, y, z) {
                    let px = x + 1;
                    let py = y + 1;
                    let pz = z + 1;
                    values[px + py * dims.0 + pz * dims.0 * dims.1] = 0.0;
                }
            }
        }
    }
    let mc = MarchingCubes::new(
        dims,
        (
            grid.size * (dims.0 - 1) as f32,
            grid.size * (dims.1 - 1) as f32,
            grid.size * (dims.2 - 1) as f32,
        ),
        (
            (dims.0 - 1) as f32,
            (dims.1 - 1) as f32,
            (dims.2 - 1) as f32,
        ),
        McVec3::new(
            grid.min.x - grid.size * 0.5,
            grid.min.y - grid.size * 0.5,
            grid.min.z - grid.size * 0.5,
        ),
        values,
        0.5,
    )
    .map_err(|err| crate::AgError::InvalidInput(format!("marching cubes failed: {err}")))?;
    let raw = mc.generate(MeshSide::OutsideOnly);
    let mut mesh = Mesh {
        vertices: raw
            .vertices
            .iter()
            .map(|v| [v.posit.x, v.posit.y, v.posit.z])
            .collect(),
        indices: raw.indices.iter().map(|i| *i as u32).collect(),
        triangles_before_merge: raw.indices.len() / 3,
    };
    weld_duplicate_vertices(&mut mesh);
    merge_axis_aligned_coplanar_rectangles(&mut mesh);
    Ok(mesh)
}

fn weld_duplicate_vertices(mesh: &mut Mesh) {
    let mut remap = HashMap::<[i32; 3], u32>::new();
    let mut vertices = Vec::new();
    let mut indices = Vec::with_capacity(mesh.indices.len());
    for index in &mesh.indices {
        let v = mesh.vertices[*index as usize];
        let key = [
            (v[0] * 1_000_000.0).round() as i32,
            (v[1] * 1_000_000.0).round() as i32,
            (v[2] * 1_000_000.0).round() as i32,
        ];
        let next = *remap.entry(key).or_insert_with(|| {
            let id = vertices.len() as u32;
            vertices.push(v);
            id
        });
        indices.push(next);
    }
    mesh.vertices = vertices;
    mesh.indices = indices;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PlaneKey {
    axis: usize,
    sign: i32,
    coord: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Q([i32; 3]);

#[derive(Debug, Clone)]
struct TriangleRecord {
    vertices: [[f32; 3]; 3],
    key: Option<PlaneKey>,
}

fn merge_axis_aligned_coplanar_rectangles(mesh: &mut Mesh) {
    if mesh.indices.len() < 6 {
        return;
    }
    let records = mesh
        .indices
        .chunks_exact(3)
        .map(|tri| {
            let vertices = [
                mesh.vertices[tri[0] as usize],
                mesh.vertices[tri[1] as usize],
                mesh.vertices[tri[2] as usize],
            ];
            TriangleRecord {
                vertices,
                key: plane_key(vertices),
            }
        })
        .collect::<Vec<_>>();

    let mut groups = HashMap::<PlaneKey, Vec<usize>>::new();
    let mut output = Mesh {
        triangles_before_merge: mesh.triangles_before_merge,
        ..Mesh::default()
    };
    for (index, record) in records.iter().enumerate() {
        if let Some(key) = record.key {
            groups.entry(key).or_default().push(index);
        } else {
            push_triangle(&mut output, record.vertices);
        }
    }

    for (key, triangle_indices) in groups {
        for component in plane_components(&records, &triangle_indices) {
            if component.len() <= 2 {
                for index in component {
                    push_triangle(&mut output, records[index].vertices);
                }
                continue;
            }
            if !try_push_merged_rectangle(&mut output, &records, &component, key) {
                for index in component {
                    push_triangle(&mut output, records[index].vertices);
                }
            }
        }
    }

    *mesh = output;
    weld_duplicate_vertices(mesh);
}

fn plane_key(vertices: [[f32; 3]; 3]) -> Option<PlaneKey> {
    let a = arr_to_vec(vertices[0]);
    let b = arr_to_vec(vertices[1]);
    let c = arr_to_vec(vertices[2]);
    let n = (b - a).cross(c - a);
    let len = n.length();
    if len <= 1e-8 {
        return None;
    }
    let unit = n / len;
    let values = [unit.x, unit.y, unit.z];
    let (axis, value) = values
        .iter()
        .copied()
        .enumerate()
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .unwrap();
    if value.abs() < 0.999 {
        return None;
    }
    let coord = vertices[0][axis];
    if vertices.iter().any(|v| (v[axis] - coord).abs() > 1e-5) {
        return None;
    }
    Some(PlaneKey {
        axis,
        sign: if value >= 0.0 { 1 } else { -1 },
        coord: quantize(coord),
    })
}

fn plane_components(records: &[TriangleRecord], indices: &[usize]) -> Vec<Vec<usize>> {
    let mut edge_to_triangles = HashMap::<(Q, Q), Vec<usize>>::new();
    for index in indices {
        let record = &records[*index];
        for edge in triangle_edges(record.vertices) {
            edge_to_triangles.entry(edge).or_default().push(*index);
        }
    }
    let mut graph = HashMap::<usize, Vec<usize>>::new();
    for attached in edge_to_triangles.values() {
        if attached.len() < 2 {
            continue;
        }
        for a in attached {
            for b in attached {
                if a != b {
                    graph.entry(*a).or_default().push(*b);
                }
            }
        }
    }
    let mut seen = HashMap::<usize, bool>::new();
    let mut components = Vec::new();
    for start in indices {
        if seen.get(start).copied().unwrap_or(false) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([*start]);
        seen.insert(*start, true);
        while let Some(current) = queue.pop_front() {
            component.push(current);
            for next in graph.get(&current).into_iter().flatten() {
                if seen.get(next).copied().unwrap_or(false) {
                    continue;
                }
                seen.insert(*next, true);
                queue.push_back(*next);
            }
        }
        components.push(component);
    }
    components
}

fn try_push_merged_rectangle(
    output: &mut Mesh,
    records: &[TriangleRecord],
    component: &[usize],
    key: PlaneKey,
) -> bool {
    let mut min_u = f32::INFINITY;
    let mut max_u = f32::NEG_INFINITY;
    let mut min_v = f32::INFINITY;
    let mut max_v = f32::NEG_INFINITY;
    let mut area = 0.0;
    for index in component {
        let vertices = records[*index].vertices;
        area += triangle_area(vertices);
        for vertex in vertices {
            let (u, v) = project_uv(vertex, key.axis);
            min_u = min_u.min(u);
            max_u = max_u.max(u);
            min_v = min_v.min(v);
            max_v = max_v.max(v);
        }
    }
    let rect_area = (max_u - min_u) * (max_v - min_v);
    if rect_area <= 1e-8 || (area - rect_area).abs() > rect_area.max(1.0) * 1e-4 {
        return false;
    }
    let coord = key.coord as f32 / 1_000_000.0;
    let mut vertices = [
        unproject_uv(coord, min_u, min_v, key.axis),
        unproject_uv(coord, max_u, min_v, key.axis),
        unproject_uv(coord, max_u, max_v, key.axis),
        unproject_uv(coord, min_u, max_v, key.axis),
    ];
    let normal = (arr_to_vec(vertices[1]) - arr_to_vec(vertices[0]))
        .cross(arr_to_vec(vertices[2]) - arr_to_vec(vertices[0]));
    let desired = match key.axis {
        0 => Vec3::new(key.sign as f32, 0.0, 0.0),
        1 => Vec3::new(0.0, key.sign as f32, 0.0),
        _ => Vec3::new(0.0, 0.0, key.sign as f32),
    };
    if normal.dot(desired) < 0.0 {
        vertices.swap(1, 3);
    }
    push_triangle(output, [vertices[0], vertices[1], vertices[2]]);
    push_triangle(output, [vertices[0], vertices[2], vertices[3]]);
    true
}

fn push_triangle(mesh: &mut Mesh, vertices: [[f32; 3]; 3]) {
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(vertices);
    mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
}

fn triangle_edges(vertices: [[f32; 3]; 3]) -> [(Q, Q); 3] {
    [
        sorted_edge(q(vertices[0]), q(vertices[1])),
        sorted_edge(q(vertices[1]), q(vertices[2])),
        sorted_edge(q(vertices[2]), q(vertices[0])),
    ]
}

fn sorted_edge(a: Q, b: Q) -> (Q, Q) {
    if a.0 <= b.0 { (a, b) } else { (b, a) }
}

fn triangle_area(vertices: [[f32; 3]; 3]) -> f32 {
    let a = arr_to_vec(vertices[0]);
    let b = arr_to_vec(vertices[1]);
    let c = arr_to_vec(vertices[2]);
    (b - a).cross(c - a).length() * 0.5
}

fn project_uv(vertex: [f32; 3], axis: usize) -> (f32, f32) {
    match axis {
        0 => (vertex[1], vertex[2]),
        1 => (vertex[0], vertex[2]),
        _ => (vertex[0], vertex[1]),
    }
}

fn unproject_uv(coord: f32, u: f32, v: f32, axis: usize) -> [f32; 3] {
    match axis {
        0 => [coord, u, v],
        1 => [u, coord, v],
        _ => [u, v, coord],
    }
}

fn arr_to_vec(value: [f32; 3]) -> Vec3 {
    Vec3::new(value[0], value[1], value[2])
}

fn q(value: [f32; 3]) -> Q {
    Q([quantize(value[0]), quantize(value[1]), quantize(value[2])])
}

fn quantize(value: f32) -> i32 {
    (value * 1_000_000.0).round() as i32
}

fn emit_face(
    mesh: &mut Mesh,
    grid: &VoxelGrid,
    x: usize,
    y: usize,
    z: usize,
    dir: (isize, isize, isize),
) {
    let x0 = grid.min.x + x as f32 * grid.size;
    let y0 = grid.min.y + y as f32 * grid.size;
    let z0 = grid.min.z + z as f32 * grid.size;
    let x1 = x0 + grid.size;
    let y1 = y0 + grid.size;
    let z1 = z0 + grid.size;
    let verts: [Vec3; 4] = match dir {
        (-1, 0, 0) => [v(x0, y0, z1), v(x0, y0, z0), v(x0, y1, z0), v(x0, y1, z1)],
        (1, 0, 0) => [v(x1, y0, z0), v(x1, y0, z1), v(x1, y1, z1), v(x1, y1, z0)],
        (0, -1, 0) => [v(x0, y0, z0), v(x1, y0, z0), v(x1, y0, z1), v(x0, y0, z1)],
        (0, 1, 0) => [v(x0, y1, z1), v(x1, y1, z1), v(x1, y1, z0), v(x0, y1, z0)],
        (0, 0, -1) => [v(x1, y0, z0), v(x0, y0, z0), v(x0, y1, z0), v(x1, y1, z0)],
        (0, 0, 1) => [v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)],
        _ => unreachable!(),
    };
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(verts.map(|p| p.to_array()));
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_voxel_has_six_exposed_faces() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [1, 1, 1]);
        grid.set(0, 0, 0, true);
        let mesh = extract_faces(&grid);
        assert_eq!(mesh.triangle_count(), 12);
    }

    #[test]
    fn adjacent_voxels_cull_internal_faces() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [2, 1, 1]);
        grid.set(0, 0, 0, true);
        grid.set(1, 0, 0, true);

        let mesh = extract_faces(&grid);

        assert_eq!(mesh.triangle_count(), 20);
    }

    #[test]
    fn smooth_mesh_tracks_pre_merge_triangle_count() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [2, 2, 1]);
        for x in 0..2 {
            for y in 0..2 {
                grid.set(x, y, 0, true);
            }
        }

        let mesh = extract_smooth(&grid).unwrap();

        assert!(mesh.triangles_before_merge >= mesh.triangle_count());
        assert!(mesh.triangle_count() > 0);
    }

    #[test]
    fn smooth_empty_grid_returns_empty_mesh() {
        let grid = VoxelGrid::new(Vec3::ZERO, 1.0, [2, 2, 2]);

        let mesh = extract_smooth(&grid).unwrap();

        assert_eq!(mesh.triangle_count(), 0);
        assert_eq!(mesh.triangles_before_merge, 0);
    }
}
