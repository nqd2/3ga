use crate::error::AgResult;
use crate::math::Vec3;
use crate::splat_table::SplatTable;
use crate::voxel::{
    VoxelGrid, VoxelParams, gaussian_contribution_at, splat_cell_bounds, voxelize_cpu,
};
use std::collections::{HashSet, VecDeque};

pub fn filter_nan(table: &SplatTable) -> AgResult<SplatTable> {
    let keep = (0..table.len())
        .map(|i| {
            [
                table.x[i],
                table.y[i],
                table.z[i],
                table.scale_0[i],
                table.scale_1[i],
                table.scale_2[i],
                table.opacity[i],
            ]
            .iter()
            .all(|v| v.is_finite())
        })
        .collect::<Vec<_>>();
    table.compact_by_mask(&keep)
}

pub fn filter_opacity_min(table: &SplatTable, min_alpha: f32) -> AgResult<SplatTable> {
    let keep = (0..table.len())
        .map(|i| table.linear_alpha(i) >= min_alpha)
        .collect::<Vec<_>>();
    table.compact_by_mask(&keep)
}

pub fn filter_box(table: &SplatTable, min: Vec3, max: Vec3) -> AgResult<SplatTable> {
    let keep = (0..table.len())
        .map(|i| {
            let p = table.position(i);
            p.x >= min.x
                && p.x <= max.x
                && p.y >= min.y
                && p.y <= max.y
                && p.z >= min.z
                && p.z <= max.z
        })
        .collect::<Vec<_>>();
    table.compact_by_mask(&keep)
}

pub fn filter_sphere(table: &SplatTable, center: Vec3, radius: f32) -> AgResult<SplatTable> {
    let radius2 = radius * radius;
    let keep = (0..table.len())
        .map(|i| {
            let d = table.position(i) - center;
            d.dot(d) <= radius2
        })
        .collect::<Vec<_>>();
    table.compact_by_mask(&keep)
}

pub fn filter_cluster(
    table: &SplatTable,
    coarse_voxel_size: f32,
    opacity_threshold: f32,
    seed_pos: Vec3,
) -> AgResult<SplatTable> {
    let grid = voxelize_cpu(
        table,
        VoxelParams {
            size: coarse_voxel_size,
            opacity_threshold,
        },
    )?;
    let Some(seed) = grid.world_to_cell(seed_pos) else {
        return Ok(table.clone());
    };
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([seed]);
    while let Some(cell) = queue.pop_front() {
        if !visited.insert(cell) || !grid.get(cell.0, cell.1, cell.2) {
            continue;
        }
        for next in grid.neighbors6(cell.0, cell.1, cell.2) {
            if !visited.contains(&next) && grid.get(next.0, next.1, next.2) {
                queue.push_back(next);
            }
        }
    }
    if visited.is_empty() {
        return Ok(table.clone());
    }
    let keep = (0..table.len())
        .map(|i| {
            grid.world_to_cell(table.position(i))
                .map(|cell| visited.contains(&cell))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    table.compact_by_mask(&keep)
}

pub fn filter_floaters_by_voxel_contribution(
    table: &SplatTable,
    grid: &VoxelGrid,
    params: VoxelParams,
) -> AgResult<SplatTable> {
    let keep = (0..table.len())
        .map(|i| splat_contributes_to_occupied_voxel(table, i, grid, params.opacity_threshold))
        .collect::<Vec<_>>();
    table.compact_by_mask(&keep)
}

fn splat_contributes_to_occupied_voxel(
    table: &SplatTable,
    index: usize,
    grid: &VoxelGrid,
    opacity_threshold: f32,
) -> bool {
    let alpha = table.linear_alpha(index);
    if alpha < opacity_threshold {
        return false;
    }
    let (min_cell, max_cell) = splat_cell_bounds(table, index, grid);
    for z in min_cell.2..=max_cell.2 {
        for y in min_cell.1..=max_cell.1 {
            for x in min_cell.0..=max_cell.0 {
                if !grid.get(x, y, z) {
                    continue;
                }
                if gaussian_contribution_at(table, index, grid.cell_center(x, y, z))
                    >= opacity_threshold
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Quat;

    #[test]
    fn box_and_sphere_keep_only_masked_rows() {
        let table = three_point_table();
        let boxed = filter_box(
            &table,
            Vec3::new(-0.5, -0.5, -0.5),
            Vec3::new(1.5, 0.5, 0.5),
        )
        .unwrap();
        assert_eq!(boxed.len(), 2);

        let sphere = filter_sphere(&table, Vec3::ZERO, 0.25).unwrap();
        assert_eq!(sphere.len(), 1);
        assert_eq!(sphere.position(0), Vec3::ZERO);
    }

    #[test]
    fn cluster_keeps_seed_component_not_distant_island() {
        let table = three_point_table();
        let filtered = filter_cluster(&table, 2.0, 0.05, Vec3::ZERO).unwrap();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn floater_filter_is_separate_from_cluster_filter() {
        let table = three_point_table();
        let grid = voxelize_cpu(
            &table,
            VoxelParams {
                size: 0.5,
                opacity_threshold: 0.05,
            },
        )
        .unwrap();
        let filtered = filter_floaters_by_voxel_contribution(
            &table,
            &grid,
            VoxelParams {
                size: 0.5,
                opacity_threshold: 0.05,
            },
        )
        .unwrap();
        assert_eq!(filtered.len(), 3);
    }

    fn three_point_table() -> SplatTable {
        let mut table = SplatTable::default();
        for p in [
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ] {
            table.push_standard(
                p,
                Vec3::new(0.2f32.ln(), 0.2f32.ln(), 0.2f32.ln()),
                8.0,
                Vec3::ZERO,
                Quat::IDENTITY,
            );
        }
        table
    }
}