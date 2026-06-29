use crate::config::{FillMode, VoxelCarveConfig, VoxelFillConfig};
use crate::error::{AgError, AgResult};
use crate::math::{Bounds, Vec3, QuatExt};
use crate::splat_table::SplatTable;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy)]
pub struct VoxelParams {
    pub size: f32,
    pub opacity_threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxelGrid {
    pub min: Vec3,
    pub size: f32,
    pub dims: [usize; 3],
    solid: Vec<bool>,
}

impl VoxelGrid {
    pub fn new(min: Vec3, size: f32, dims: [usize; 3]) -> Self {
        Self {
            min,
            size,
            dims,
            solid: vec![false; dims[0] * dims[1] * dims[2]],
        }
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> bool {
        if x >= self.dims[0] || y >= self.dims[1] || z >= self.dims[2] {
            return false;
        }
        self.solid[self.index(x, y, z)]
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, value: bool) {
        if x < self.dims[0] && y < self.dims[1] && z < self.dims[2] {
            let index = self.index(x, y, z);
            self.solid[index] = value;
        }
    }

    pub fn cell_center(&self, x: usize, y: usize, z: usize) -> Vec3 {
        self.min
            + Vec3::new(
                (x as f32 + 0.5) * self.size,
                (y as f32 + 0.5) * self.size,
                (z as f32 + 0.5) * self.size,
            )
    }

    pub fn world_to_cell(&self, p: Vec3) -> Option<(usize, usize, usize)> {
        let local = (p - self.min) / self.size;
        if local.x < 0.0 || local.y < 0.0 || local.z < 0.0 {
            return None;
        }
        let cell = (
            local.x.floor() as usize,
            local.y.floor() as usize,
            local.z.floor() as usize,
        );
        if cell.0 < self.dims[0] && cell.1 < self.dims[1] && cell.2 < self.dims[2] {
            Some(cell)
        } else {
            None
        }
    }

    pub fn neighbors6(&self, x: usize, y: usize, z: usize) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::with_capacity(6);
        if x > 0 {
            out.push((x - 1, y, z));
        }
        if y > 0 {
            out.push((x, y - 1, z));
        }
        if z > 0 {
            out.push((x, y, z - 1));
        }
        if x + 1 < self.dims[0] {
            out.push((x + 1, y, z));
        }
        if y + 1 < self.dims[1] {
            out.push((x, y + 1, z));
        }
        if z + 1 < self.dims[2] {
            out.push((x, y, z + 1));
        }
        out
    }

    pub fn solid_count(&self) -> usize {
        self.solid.iter().filter(|v| **v).count()
    }

    pub fn cell_count(&self) -> usize {
        self.solid.len()
    }

    pub fn flat_solid(&self, index: usize) -> bool {
        self.solid.get(index).copied().unwrap_or(false)
    }

    pub fn set_flat(&mut self, index: usize, value: bool) {
        if let Some(cell) = self.solid.get_mut(index) {
            *cell = value;
        }
    }

    pub fn mismatch_count(&self, other: &Self) -> usize {
        if self.min != other.min || self.size != other.size || self.dims != other.dims {
            return self.cell_count().max(other.cell_count());
        }
        self.solid
            .iter()
            .zip(other.solid.iter())
            .filter(|(a, b)| a != b)
            .count()
    }

    pub fn iter_solid(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.solid.iter().enumerate().filter_map(|(index, value)| {
            if !*value {
                return None;
            }
            let z = index / (self.dims[0] * self.dims[1]);
            let rem = index % (self.dims[0] * self.dims[1]);
            let y = rem / self.dims[0];
            let x = rem % self.dims[0];
            Some((x, y, z))
        })
    }

    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        z * self.dims[0] * self.dims[1] + y * self.dims[0] + x
    }
}

pub fn voxelize_cpu(table: &SplatTable, params: VoxelParams) -> AgResult<VoxelGrid> {
    let mut grid = voxel_grid_for_table(table, params)?;
    voxelize_cpu_into_grid(table, params, &mut grid);
    Ok(grid)
}

pub fn voxel_grid_for_table(table: &SplatTable, params: VoxelParams) -> AgResult<VoxelGrid> {
    if params.size <= 0.0 {
        return Err(AgError::InvalidConfig(
            "voxel size must be positive".to_string(),
        ));
    }
    let mut bounds = Bounds::empty();
    for i in 0..table.len() {
        let splat_bounds = table.gaussian_bounds(i);
        bounds.include(splat_bounds.min);
        bounds.include(splat_bounds.max);
    }
    if bounds.is_empty() {
        return Ok(VoxelGrid::new(Vec3::ZERO, params.size, [1, 1, 1]));
    }
    let block = params.size * 4.0;
    let min = Vec3::new(
        (bounds.min.x / block).floor() * block,
        (bounds.min.y / block).floor() * block,
        (bounds.min.z / block).floor() * block,
    );
    let max = Vec3::new(
        (bounds.max.x / block).ceil() * block,
        (bounds.max.y / block).ceil() * block,
        (bounds.max.z / block).ceil() * block,
    );
    let dims = [
        ((max.x - min.x) / params.size).ceil().max(1.0) as usize,
        ((max.y - min.y) / params.size).ceil().max(1.0) as usize,
        ((max.z - min.z) / params.size).ceil().max(1.0) as usize,
    ];
    Ok(VoxelGrid::new(min, params.size, dims))
}

fn voxelize_cpu_into_grid(table: &SplatTable, params: VoxelParams, grid: &mut VoxelGrid) {
    for i in 0..table.len() {
        let alpha = table.linear_alpha(i);
        if alpha < params.opacity_threshold {
            continue;
        }
        let (min_cell, max_cell) = splat_cell_bounds(table, i, grid);
        for z in min_cell.2..=max_cell.2 {
            for y in min_cell.1..=max_cell.1 {
                for x in min_cell.0..=max_cell.0 {
                    if gaussian_contribution_at(table, i, grid.cell_center(x, y, z))
                        >= params.opacity_threshold
                    {
                        grid.set(x, y, z, true);
                    }
                }
            }
        }
    }
}

pub(crate) fn splat_cell_bounds(
    table: &SplatTable,
    index: usize,
    grid: &VoxelGrid,
) -> ((usize, usize, usize), (usize, usize, usize)) {
    let bounds = table.gaussian_bounds(index);
    let min_cell = grid.world_to_cell(bounds.min).unwrap_or((0, 0, 0));
    let max_cell = grid.world_to_cell(bounds.max).unwrap_or((
        grid.dims[0] - 1,
        grid.dims[1] - 1,
        grid.dims[2] - 1,
    ));
    (min_cell, max_cell)
}

pub(crate) fn gaussian_contribution_at(table: &SplatTable, index: usize, point: Vec3) -> f32 {
    let center = table.position(index);
    let local = table.rotation(index).inverse_rotate_vec3(point - center);
    let sigma = Vec3::new(
        table.scale_0[index].exp().max(1e-6),
        table.scale_1[index].exp().max(1e-6),
        table.scale_2[index].exp().max(1e-6),
    );
    let n2 =
        (local.x / sigma.x).powi(2) + (local.y / sigma.y).powi(2) + (local.z / sigma.z).powi(2);
    table.linear_alpha(index) * (-0.5 * n2).exp()
}

pub fn fill_grid(grid: &VoxelGrid, config: &VoxelFillConfig, seed_pos: Vec3) -> VoxelGrid {
    match config.mode {
        FillMode::None => grid.clone(),
        FillMode::FloorFill => floor_fill(grid),
        FillMode::ExteriorFill => exterior_fill(grid, config.dilation_size, seed_pos),
    }
}

fn floor_fill(grid: &VoxelGrid) -> VoxelGrid {
    let mut out = grid.clone();
    for x in 0..grid.dims[0] {
        for z in 0..grid.dims[2] {
            for y in 0..grid.dims[1] {
                if grid.get(x, y, z) {
                    for yy in 0..y {
                        out.set(x, yy, z, true);
                    }
                    break;
                }
            }
        }
    }
    out
}

fn exterior_fill(grid: &VoxelGrid, dilation_size: f32, seed_pos: Vec3) -> VoxelGrid {
    let dilated = dilate(grid, (dilation_size / grid.size).ceil().max(0.0) as isize);
    let mut outside = HashSet::new();
    let mut queue = VecDeque::new();
    for x in 0..grid.dims[0] {
        for y in 0..grid.dims[1] {
            for z in [0, grid.dims[2] - 1] {
                queue.push_back((x, y, z));
            }
        }
    }
    for x in 0..grid.dims[0] {
        for z in 0..grid.dims[2] {
            for y in [0, grid.dims[1] - 1] {
                queue.push_back((x, y, z));
            }
        }
    }
    for y in 0..grid.dims[1] {
        for z in 0..grid.dims[2] {
            for x in [0, grid.dims[0] - 1] {
                queue.push_back((x, y, z));
            }
        }
    }
    while let Some(cell) = queue.pop_front() {
        if outside.contains(&cell) || dilated.get(cell.0, cell.1, cell.2) {
            continue;
        }
        outside.insert(cell);
        for next in dilated.neighbors6(cell.0, cell.1, cell.2) {
            queue.push_back(next);
        }
    }
    if grid
        .world_to_cell(seed_pos)
        .map(|seed| outside.contains(&seed))
        .unwrap_or(false)
    {
        return grid.clone();
    }
    let mut out = grid.clone();
    for z in 0..grid.dims[2] {
        for y in 0..grid.dims[1] {
            for x in 0..grid.dims[0] {
                if outside.contains(&(x, y, z)) {
                    out.set(x, y, z, true);
                }
            }
        }
    }
    out
}

fn dilate(grid: &VoxelGrid, radius: isize) -> VoxelGrid {
    if radius <= 0 {
        return grid.clone();
    }
    let mut out = grid.clone();
    for (x, y, z) in grid.iter_solid().collect::<Vec<_>>() {
        for dz in -radius..=radius {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    let nz = z as isize + dz;
                    if nx >= 0 && ny >= 0 && nz >= 0 {
                        out.set(nx as usize, ny as usize, nz as usize, true);
                    }
                }
            }
        }
    }
    out
}

pub fn carve_grid(grid: &VoxelGrid, config: &VoxelCarveConfig) -> VoxelGrid {
    if !config.enabled {
        return grid.clone();
    }
    let seed = Vec3::from_array(config.seed_pos);
    let Some(seed_cell) = grid.world_to_cell(seed) else {
        return grid.clone();
    };
    let radius_cells = (config.agent_radius / grid.size).ceil().max(0.0) as isize;
    let half_height_cells = (config.agent_height / (2.0 * grid.size)).ceil().max(0.0) as isize;
    let mut reachable = HashSet::new();
    let mut queue = VecDeque::from([seed_cell]);
    while let Some(cell) = queue.pop_front() {
        if reachable.contains(&cell) || capsule_blocked(grid, cell, radius_cells, half_height_cells)
        {
            continue;
        }
        reachable.insert(cell);
        for next in grid.neighbors6(cell.0, cell.1, cell.2) {
            queue.push_back(next);
        }
    }
    let mut out = VoxelGrid::new(grid.min, grid.size, grid.dims);
    for (x, y, z) in grid.iter_solid() {
        let borders_reachable = grid
            .neighbors6(x, y, z)
            .iter()
            .any(|cell| reachable.contains(cell));
        if borders_reachable {
            out.set(x, y, z, true);
        }
    }
    out
}

fn capsule_blocked(
    grid: &VoxelGrid,
    cell: (usize, usize, usize),
    radius: isize,
    half_height: isize,
) -> bool {
    for dy in -half_height..=half_height {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let nx = cell.0 as isize + dx;
                let ny = cell.1 as isize + dy;
                let nz = cell.2 as isize + dz;
                if nx < 0 || ny < 0 || nz < 0 {
                    return true;
                }
                if grid.get(nx as usize, ny as usize, nz as usize) {
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
    use crate::splat_table::SplatTable;

    #[test]
    fn gaussian_contribution_uses_rotation() {
        let mut table = SplatTable::default();
        let s = 0.5f32.sqrt();
        table.push_standard(
            Vec3::ZERO,
            Vec3::new(0.4f32.ln(), 0.1f32.ln(), 0.1f32.ln()),
            8.0,
            Vec3::ZERO,
            Quat::from_wxyz(s, 0.0, 0.0, s),
        );

        let along_rotated_major_axis =
            gaussian_contribution_at(&table, 0, Vec3::new(0.0, 0.4, 0.0));
        let along_world_x = gaussian_contribution_at(&table, 0, Vec3::new(0.4, 0.0, 0.0));

        assert!(along_rotated_major_axis > 0.5);
        assert!(along_world_x < 0.001);
    }

    #[test]
    fn floor_fill_fills_column_from_bottom_to_first_solid() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [1, 4, 1]);
        grid.set(0, 2, 0, true);

        let filled = fill_grid(
            &grid,
            &VoxelFillConfig {
                mode: FillMode::FloorFill,
                dilation_size: 0.0,
            },
            Vec3::ZERO,
        );

        assert!(filled.get(0, 0, 0));
        assert!(filled.get(0, 1, 0));
        assert!(filled.get(0, 2, 0));
        assert!(!filled.get(0, 3, 0));
    }

    #[test]
    fn exterior_fill_keeps_closed_room_interior_empty_for_carve() {
        let grid = closed_room();
        let filled = fill_grid(
            &grid,
            &VoxelFillConfig {
                mode: FillMode::ExteriorFill,
                dilation_size: 0.0,
            },
            Vec3::new(2.5, 2.5, 2.5),
        );

        assert!(!filled.get(2, 2, 2));
        let carved = carve_grid(
            &filled,
            &VoxelCarveConfig {
                enabled: true,
                agent_height: 1.0,
                agent_radius: 0.0,
                seed_pos: [2.5, 2.5, 2.5],
            },
        );
        assert!(carved.solid_count() > 0);
    }

    #[test]
    fn exterior_fill_skips_when_seed_reaches_boundary() {
        let mut grid = closed_room();
        grid.set(0, 2, 2, false);
        let filled = fill_grid(
            &grid,
            &VoxelFillConfig {
                mode: FillMode::ExteriorFill,
                dilation_size: 0.0,
            },
            Vec3::new(2.5, 2.5, 2.5),
        );

        assert_eq!(filled.solid_count(), grid.solid_count());
    }

    #[test]
    fn capsule_carve_does_not_cross_too_narrow_gap() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [7, 3, 3]);
        for y in 0..3 {
            for z in 0..3 {
                grid.set(3, y, z, true);
            }
        }
        grid.set(3, 1, 1, false);
        grid.set(5, 1, 1, true);

        let carved = carve_grid(
            &grid,
            &VoxelCarveConfig {
                enabled: true,
                agent_height: 1.0,
                agent_radius: 1.0,
                seed_pos: [1.5, 1.5, 1.5],
            },
        );

        assert!(!carved.get(5, 1, 1));
    }

    fn closed_room() -> VoxelGrid {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [5, 5, 5]);
        for x in 0..5 {
            for y in 0..5 {
                for z in 0..5 {
                    if x == 0 || y == 0 || z == 0 || x == 4 || y == 4 || z == 4 {
                        grid.set(x, y, z, true);
                    }
                }
            }
        }
        grid
    }
}
 