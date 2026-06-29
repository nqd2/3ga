use crate::config::NavmeshConfig;
use crate::error::{AgError, AgResult};
use crate::mesh::Mesh;
use glam::{UVec3, Vec3A};
use rerecast::{AreaType, ConfigBuilder, DetailNavmesh, HeightfieldBuilder, TriMesh};

pub fn bake_navmesh(input: &Mesh, config: &NavmeshConfig) -> AgResult<Option<Mesh>> {
    if !config.enabled || input.indices.is_empty() {
        return Ok(None);
    }

    let mut trimesh = to_rerecast_trimesh(input)?;
    let aabb = trimesh
        .compute_aabb()
        .ok_or_else(|| AgError::InvalidInput("navmesh input mesh is empty".to_string()))?;
    let cell_size = config.cell_size.max(0.05);
    let cell_height = config.cell_height.max(0.05);
    let build_config = ConfigBuilder {
        agent_height: config.agent_height,
        agent_radius: config.agent_radius,
        walkable_slope_angle: config.max_slope_degrees.to_radians(),
        walkable_climb: config.walkable_climb,
        cell_size_fraction: (config.agent_radius / cell_size).max(0.1),
        cell_height_fraction: (config.agent_radius / cell_height).max(0.1),
        min_region_size: config.min_region_size,
        merge_region_size: config.merge_region_size,
        aabb,
        ..ConfigBuilder::default()
    }
    .build();

    trimesh.mark_walkable_triangles(build_config.walkable_slope_angle);
    if !trimesh.area_types.contains(&AreaType::DEFAULT_WALKABLE) {
        return Ok(None);
    }

    let mut heightfield = HeightfieldBuilder {
        aabb,
        cell_size: build_config.cell_size,
        cell_height: build_config.cell_height,
    }
    .build()
    .map_err(|err| AgError::InvalidInput(format!("rerecast heightfield build failed: {err}")))?;

    heightfield
        .rasterize_triangles(&trimesh, build_config.walkable_climb)
        .map_err(|err| AgError::InvalidInput(format!("rerecast rasterize failed: {err}")))?;

    heightfield.filter_low_hanging_walkable_obstacles(build_config.walkable_climb);
    heightfield.filter_ledge_spans(build_config.walkable_height, build_config.walkable_climb);
    heightfield.filter_walkable_low_height_spans(build_config.walkable_height);

    let mut compact = heightfield
        .into_compact(build_config.walkable_height, build_config.walkable_climb)
        .map_err(|err| AgError::InvalidInput(format!("rerecast compact field failed: {err}")))?;
    compact.erode_walkable_area(build_config.walkable_radius);
    compact.build_distance_field();
    compact
        .build_regions(
            build_config.border_size,
            build_config.min_region_area,
            build_config.merge_region_area,
        )
        .map_err(|err| AgError::InvalidInput(format!("rerecast region build failed: {err}")))?;
    let contours = compact.build_contours(
        build_config.max_simplification_error,
        build_config.max_edge_len,
        build_config.contour_flags,
    );
    let poly_mesh = contours
        .into_polygon_mesh(build_config.max_vertices_per_polygon)
        .map_err(|err| AgError::InvalidInput(format!("rerecast polygon mesh failed: {err}")))?;
    let detail = DetailNavmesh::new(
        &poly_mesh,
        &compact,
        build_config.detail_sample_dist,
        build_config.detail_sample_max_error,
    )
    .map_err(|err| AgError::InvalidInput(format!("rerecast detail mesh failed: {err}")))?;

    let mesh = detail_to_mesh(&detail);
    if mesh.triangle_count() == 0 {
        Ok(None)
    } else {
        Ok(Some(mesh))
    }
}

fn to_rerecast_trimesh(mesh: &Mesh) -> AgResult<TriMesh> {
    if !mesh.indices.len().is_multiple_of(3) {
        return Err(AgError::InvalidInput(
            "navmesh input index buffer length must be divisible by 3".to_string(),
        ));
    }
    let vertices = mesh
        .vertices
        .iter()
        .map(|v| Vec3A::new(v[0], v[1], v[2]))
        .collect::<Vec<_>>();
    let indices = mesh
        .indices
        .chunks_exact(3)
        .map(|tri| UVec3::new(tri[0], tri[1], tri[2]))
        .collect::<Vec<_>>();
    let area_types = vec![AreaType::NOT_WALKABLE; indices.len()];
    Ok(TriMesh {
        vertices,
        indices,
        area_types,
    })
}

fn detail_to_mesh(detail: &DetailNavmesh) -> Mesh {
    let mut out = Mesh::default();
    for sub in &detail.meshes {
        let verts = &detail.vertices[sub.base_vertex_index as usize..][..sub.vertex_count as usize];
        let tris =
            &detail.triangles[sub.base_triangle_index as usize..][..sub.triangle_count as usize];
        let base = out.vertices.len() as u32;
        out.vertices.extend(verts.iter().map(|v| [v.x, v.y, v.z]));
        for tri in tris {
            out.indices.push(base + tri[0] as u32);
            out.indices.push(base + tri[1] as u32);
            out.indices.push(base + tri[2] as u32);
        }
    }
    out.triangles_before_merge = out.triangle_count();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_floor_generates_navmesh() {
        let mesh = Mesh {
            vertices: vec![
                [-2.0, 0.0, -2.0],
                [2.0, 0.0, -2.0],
                [2.0, 0.0, 2.0],
                [-2.0, 0.0, 2.0],
            ],
            indices: vec![0, 2, 1, 0, 3, 2],
            triangles_before_merge: 2,
        };
        let navmesh = bake_navmesh(&mesh, &NavmeshConfig::default())
            .unwrap()
            .unwrap();
        assert!(navmesh.triangle_count() > 0);
    }

    #[test]
    fn vertical_wall_returns_no_navmesh() {
        let mesh = Mesh {
            vertices: vec![
                [0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 2.0, 2.0],
                [0.0, 0.0, 2.0],
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            triangles_before_merge: 2,
        };
        assert!(
            bake_navmesh(&mesh, &NavmeshConfig::default())
                .unwrap()
                .is_none()
        );
    }
}
