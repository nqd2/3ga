use crate::error::{AgError, AgResult};
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
    Ok(
        filter_cluster_with_stats(table, coarse_voxel_size, opacity_threshold, seed_pos, 0.1)?
            .table,
    )
}

#[derive(Debug, Clone)]
pub struct ClusterFilterOutcome {
    pub table: SplatTable,
    pub input_count: usize,
    pub output_count: usize,
    pub removed_count: usize,
    pub requested_seed: [f32; 3],
    pub resolved_seed: [f32; 3],
    pub seed_was_resolved: bool,
    pub occupied_cells: usize,
    pub cluster_cells: usize,
}

pub fn filter_cluster_with_stats(
    table: &SplatTable,
    coarse_voxel_size: f32,
    opacity_threshold: f32,
    seed_pos: Vec3,
    min_contribution: f32,
) -> AgResult<ClusterFilterOutcome> {
    if !coarse_voxel_size.is_finite() || coarse_voxel_size <= 0.0 {
        return Err(AgError::InvalidConfig(
            "filterCluster coarse voxel size must be positive".to_string(),
        ));
    }
    if !opacity_threshold.is_finite() || opacity_threshold < 0.0 {
        return Err(AgError::InvalidConfig(
            "filterCluster opacity threshold must be non-negative".to_string(),
        ));
    }
    if !min_contribution.is_finite() || min_contribution < 0.0 {
        return Err(AgError::InvalidConfig(
            "filterCluster min contribution must be non-negative".to_string(),
        ));
    }

    let grid = voxelize_cpu(
        table,
        VoxelParams {
            size: coarse_voxel_size,
            opacity_threshold,
        },
    )?;
    let occupied_cells = grid.solid_count();
    if occupied_cells == 0 {
        return Err(AgError::InvalidInput(
            "filterCluster found no occupied voxels at the requested threshold".to_string(),
        ));
    }
    let requested_cell = grid
        .world_to_cell(seed_pos)
        .unwrap_or_else(|| world_to_cell_clamped(&grid, seed_pos));
    let seed = if grid.get(requested_cell.0, requested_cell.1, requested_cell.2) {
        requested_cell
    } else {
        nearest_solid_cell(&grid, requested_cell).ok_or_else(|| {
            AgError::InvalidInput(
                "filterCluster could not resolve seed to occupied voxel".to_string(),
            )
        })?
    };
    let seed_was_resolved = seed != requested_cell;
    let resolved_seed = grid.cell_center(seed.0, seed.1, seed.2);

    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([seed]);
    while let Some(cell) = queue.pop_front() {
        if visited.contains(&cell) || !grid.get(cell.0, cell.1, cell.2) {
            continue;
        }
        visited.insert(cell);
        for next in grid.neighbors6(cell.0, cell.1, cell.2) {
            if !visited.contains(&next) && grid.get(next.0, next.1, next.2) {
                queue.push_back(next);
            }
        }
    }
    if visited.is_empty() {
        return Err(AgError::InvalidInput(
            "filterCluster found no connected occupied voxels from seed".to_string(),
        ));
    }
    let keep = (0..table.len())
        .map(|i| splat_touches_cluster(table, i, &grid, &visited, min_contribution))
        .collect::<Vec<_>>();
    let filtered = table.compact_by_mask(&keep)?;
    if !table.is_empty() && filtered.is_empty() {
        return Err(AgError::InvalidInput(
            "filterCluster removed every splat; check seed, opacity threshold, or alignment"
                .to_string(),
        ));
    }
    Ok(ClusterFilterOutcome {
        input_count: table.len(),
        output_count: filtered.len(),
        removed_count: table.len().saturating_sub(filtered.len()),
        requested_seed: seed_pos.to_array(),
        resolved_seed: resolved_seed.to_array(),
        seed_was_resolved,
        occupied_cells,
        cluster_cells: visited.len(),
        table: filtered,
    })
}

pub fn filter_floaters_by_voxel_contribution(
    table: &SplatTable,
    grid: &VoxelGrid,
    params: VoxelParams,
) -> AgResult<SplatTable> {
    Ok(filter_floaters_by_voxel_contribution_with_stats(table, grid, params, 0.1)?.table)
}

#[derive(Debug, Clone)]
pub struct FloaterFilterOutcome {
    pub table: SplatTable,
    pub input_count: usize,
    pub output_count: usize,
    pub removed_count: usize,
}

pub fn filter_floaters_by_voxel_contribution_with_stats(
    table: &SplatTable,
    grid: &VoxelGrid,
    params: VoxelParams,
    min_contribution: f32,
) -> AgResult<FloaterFilterOutcome> {
    if !min_contribution.is_finite() || min_contribution < 0.0 {
        return Err(AgError::InvalidConfig(
            "filterFloatersByVoxelContribution min contribution must be non-negative".to_string(),
        ));
    }
    let keep = (0..table.len())
        .map(|i| {
            splat_contributes_to_occupied_voxel(table, i, grid, params.opacity_threshold)
                && splat_contributes_to_occupied_voxel(table, i, grid, min_contribution)
        })
        .collect::<Vec<_>>();
    let filtered = table.compact_by_mask(&keep)?;
    Ok(FloaterFilterOutcome {
        input_count: table.len(),
        output_count: filtered.len(),
        removed_count: table.len().saturating_sub(filtered.len()),
        table: filtered,
    })
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

fn splat_touches_cluster(
    table: &SplatTable,
    index: usize,
    grid: &VoxelGrid,
    cluster: &HashSet<(usize, usize, usize)>,
    min_contribution: f32,
) -> bool {
    if table.linear_alpha(index) < min_contribution {
        return false;
    }
    if grid
        .world_to_cell(table.position(index))
        .map(|cell| cluster.contains(&cell))
        .unwrap_or(false)
    {
        return true;
    }

    let (min_cell, max_cell) = splat_cell_bounds(table, index, grid);
    let span = [
        max_cell.0.saturating_sub(min_cell.0) + 1,
        max_cell.1.saturating_sub(min_cell.1) + 1,
        max_cell.2.saturating_sub(min_cell.2) + 1,
    ];
    let covered_cells = span[0] * span[1] * span[2];
    let min_hits = if covered_cells > 64 {
        (covered_cells / 16).max(2)
    } else {
        1
    };
    let mut hits = 0usize;
    for z in min_cell.2..=max_cell.2 {
        for y in min_cell.1..=max_cell.1 {
            for x in min_cell.0..=max_cell.0 {
                if !cluster.contains(&(x, y, z)) {
                    continue;
                }
                if gaussian_contribution_at(table, index, grid.cell_center(x, y, z))
                    >= min_contribution
                {
                    hits += 1;
                    if hits >= min_hits {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn nearest_solid_cell(
    grid: &VoxelGrid,
    seed: (usize, usize, usize),
) -> Option<(usize, usize, usize)> {
    grid.iter_solid().min_by(|a, b| {
        cell_distance2(*a, seed)
            .partial_cmp(&cell_distance2(*b, seed))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn cell_distance2(a: (usize, usize, usize), b: (usize, usize, usize)) -> f32 {
    let dx = a.0 as f32 - b.0 as f32;
    let dy = a.1 as f32 - b.1 as f32;
    let dz = a.2 as f32 - b.2 as f32;
    dx * dx + dy * dy + dz * dz
}

fn world_to_cell_clamped(grid: &VoxelGrid, p: Vec3) -> (usize, usize, usize) {
    let local = (p - grid.min) / grid.size;
    (
        clamp_cell_axis(local.x, grid.dims[0]),
        clamp_cell_axis(local.y, grid.dims[1]),
        clamp_cell_axis(local.z, grid.dims[2]),
    )
}

fn clamp_cell_axis(value: f32, dim: usize) -> usize {
    if dim <= 1 || !value.is_finite() {
        return 0;
    }
    value.floor().clamp(0.0, (dim - 1) as f32) as usize
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
        let filtered = filter_cluster(&table, 1.0, 0.001, Vec3::ZERO).unwrap();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn cluster_resolves_empty_seed_to_nearest_occupied_cell() {
        let table = three_point_table();
        let outcome =
            filter_cluster_with_stats(&table, 1.0, 0.001, Vec3::new(3.5, 3.5, 0.0), 0.1).unwrap();

        assert_eq!(outcome.output_count, 2);
        assert!(outcome.seed_was_resolved);
        assert_eq!(outcome.removed_count, 1);
    }

    #[test]
    fn floater_filter_is_separate_from_cluster_filter() {
        let table = three_point_table();
        let grid = voxelize_cpu(
            &table,
            VoxelParams {
                size: 0.5,
                opacity_threshold: 0.001,
            },
        )
        .unwrap();
        let filtered = filter_floaters_by_voxel_contribution(
            &table,
            &grid,
            VoxelParams {
                size: 0.5,
                opacity_threshold: 0.001,
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
                Vec3::new(0.75f32.ln(), 0.75f32.ln(), 0.75f32.ln()),
                8.0,
                Vec3::ZERO,
                Quat::IDENTITY,
            );
        }
        table
    }
}
