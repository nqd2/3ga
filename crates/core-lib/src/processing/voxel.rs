use crate::config::{FillMode, VoxelCarveConfig, VoxelFillConfig};
use crate::error::{AgError, AgResult};
use crate::math::{Bounds, QuatExt, Vec3};
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

    pub fn world_to_cell_clamped(&self, p: Vec3) -> (usize, usize, usize) {
        let local = (p - self.min) / self.size;
        (
            clamp_cell_axis(local.x, self.dims[0]),
            clamp_cell_axis(local.y, self.dims[1]),
            clamp_cell_axis(local.z, self.dims[2]),
        )
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

    pub fn crop_to_occupied(&self) -> (Self, VoxelCropStats) {
        let before_solid = self.solid_count();
        let mut min_cell = [usize::MAX; 3];
        let mut max_cell = [0usize; 3];
        for (x, y, z) in self.iter_solid() {
            min_cell[0] = min_cell[0].min(x);
            min_cell[1] = min_cell[1].min(y);
            min_cell[2] = min_cell[2].min(z);
            max_cell[0] = max_cell[0].max(x);
            max_cell[1] = max_cell[1].max(y);
            max_cell[2] = max_cell[2].max(z);
        }
        if before_solid == 0 {
            let empty = Self::new(self.min, self.size, [1, 1, 1]);
            return (
                empty,
                VoxelCropStats {
                    before_dims: self.dims,
                    after_dims: [1, 1, 1],
                    before_solid,
                    after_solid: 0,
                    min_cell: [0, 0, 0],
                    max_cell: [0, 0, 0],
                },
            );
        }
        let dims = [
            max_cell[0] - min_cell[0] + 1,
            max_cell[1] - min_cell[1] + 1,
            max_cell[2] - min_cell[2] + 1,
        ];
        let min = self.min
            + Vec3::new(
                min_cell[0] as f32 * self.size,
                min_cell[1] as f32 * self.size,
                min_cell[2] as f32 * self.size,
            );
        let mut out = Self::new(min, self.size, dims);
        for (x, y, z) in self.iter_solid() {
            out.set(x - min_cell[0], y - min_cell[1], z - min_cell[2], true);
        }
        let after_solid = out.solid_count();
        (
            out,
            VoxelCropStats {
                before_dims: self.dims,
                after_dims: dims,
                before_solid,
                after_solid,
                min_cell,
                max_cell,
            },
        )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelCropStats {
    pub before_dims: [usize; 3],
    pub after_dims: [usize; 3],
    pub before_solid: usize,
    pub after_solid: usize,
    pub min_cell: [usize; 3],
    pub max_cell: [usize; 3],
}

#[derive(Debug, Clone)]
pub struct VoxelFillOutcome {
    pub grid: VoxelGrid,
    pub before_solid: usize,
    pub after_solid: usize,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VoxelCarveOutcome {
    pub grid: VoxelGrid,
    pub before_solid: usize,
    pub after_solid: usize,
    pub reachable_cells: usize,
    pub requested_seed: [f32; 3],
    pub resolved_seed: Option<[f32; 3]>,
    pub warning: Option<String>,
}

fn clamp_cell_axis(value: f32, dim: usize) -> usize {
    if dim <= 1 || !value.is_finite() {
        return 0;
    }
    value.floor().clamp(0.0, (dim - 1) as f32) as usize
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
    fill_grid_with_status(grid, config, seed_pos).grid
}

pub fn fill_grid_with_status(
    grid: &VoxelGrid,
    config: &VoxelFillConfig,
    seed_pos: Vec3,
) -> VoxelFillOutcome {
    let before_solid = grid.solid_count();
    let (filled, warning) = match config.mode {
        FillMode::None => (grid.clone(), None),
        FillMode::FloorFill => (floor_fill(grid), None),
        FillMode::ExteriorFill => exterior_fill(grid, config.dilation_size, seed_pos),
    };
    VoxelFillOutcome {
        after_solid: filled.solid_count(),
        before_solid,
        grid: filled,
        warning,
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

fn exterior_fill(
    grid: &VoxelGrid,
    dilation_size: f32,
    seed_pos: Vec3,
) -> (VoxelGrid, Option<String>) {
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
    let Some(seed_cell) = grid.world_to_cell(seed_pos) else {
        return (
            grid.clone(),
            Some(
                "voxelExternalFill skipped because the fill seed is outside grid bounds"
                    .to_string(),
            ),
        );
    };
    if outside.contains(&seed_cell) {
        return (
            grid.clone(),
            Some(
                "voxelExternalFill skipped because the fill seed is connected to the boundary"
                    .to_string(),
            ),
        );
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
    (out, None)
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
    carve_grid_with_status(grid, config).grid
}

pub fn carve_grid_with_status(grid: &VoxelGrid, config: &VoxelCarveConfig) -> VoxelCarveOutcome {
    if !config.enabled {
        let solid = grid.solid_count();
        return VoxelCarveOutcome {
            grid: grid.clone(),
            before_solid: solid,
            after_solid: solid,
            reachable_cells: 0,
            requested_seed: config.seed_pos,
            resolved_seed: None,
            warning: None,
        };
    }
    let seed = Vec3::from_array(config.seed_pos);
    let before_solid = grid.solid_count();
    let Some(requested_seed_cell) = grid.world_to_cell(seed) else {
        return VoxelCarveOutcome {
            grid: grid.clone(),
            before_solid,
            after_solid: before_solid,
            reachable_cells: 0,
            requested_seed: config.seed_pos,
            resolved_seed: None,
            warning: Some("voxelCarve seed was outside grid bounds; skipping carve".to_string()),
        };
    };
    let radius_cells = (config.agent_radius / grid.size).ceil().max(0.0) as isize;
    let half_height_cells = (config.agent_height / (2.0 * grid.size)).ceil().max(0.0) as isize;
    let (seed_cell, warning) = if capsule_blocked(
        grid,
        requested_seed_cell,
        radius_cells,
        half_height_cells,
    ) {
        match nearest_unblocked_cell(grid, requested_seed_cell, radius_cells, half_height_cells) {
            Some(cell) => (
                cell,
                Some("voxelCarve seed was blocked; resolved to nearest navigable cell".to_string()),
            ),
            None => {
                return VoxelCarveOutcome {
                    grid: grid.clone(),
                    before_solid,
                    after_solid: before_solid,
                    reachable_cells: 0,
                    requested_seed: config.seed_pos,
                    resolved_seed: None,
                    warning: Some(
                        "voxelCarve seed was blocked after dilation with no nearby free cell; skipping carve"
                            .to_string(),
                    ),
                };
            }
        }
    } else {
        (requested_seed_cell, None)
    };
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
    let after_solid = out.solid_count();
    let warning = if reachable.is_empty() {
        Some("voxelCarve found no reachable space from seed".to_string())
    } else if after_solid == 0 {
        Some("voxelCarve produced no bordering collision cells".to_string())
    } else {
        warning
    };
    VoxelCarveOutcome {
        grid: out,
        before_solid: grid.solid_count(),
        after_solid,
        reachable_cells: reachable.len(),
        requested_seed: config.seed_pos,
        resolved_seed: Some(
            grid.cell_center(seed_cell.0, seed_cell.1, seed_cell.2)
                .to_array(),
        ),
        warning,
    }
}

fn nearest_unblocked_cell(
    grid: &VoxelGrid,
    seed: (usize, usize, usize),
    radius: isize,
    half_height: isize,
) -> Option<(usize, usize, usize)> {
    let search_radius = (radius.max(half_height).max(2) * 2).min(64);
    for r in 1..=search_radius {
        for dz in -r..=r {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()).max(dz.abs()) != r {
                        continue;
                    }
                    let nx = seed.0 as isize + dx;
                    let ny = seed.1 as isize + dy;
                    let nz = seed.2 as isize + dz;
                    if nx < 0 || ny < 0 || nz < 0 {
                        continue;
                    }
                    let cell = (nx as usize, ny as usize, nz as usize);
                    if cell.0 >= grid.dims[0] || cell.1 >= grid.dims[1] || cell.2 >= grid.dims[2] {
                        continue;
                    }
                    if !capsule_blocked(grid, cell, radius, half_height) {
                        return Some(cell);
                    }
                }
            }
        }
    }
    None
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
        let outcome = fill_grid_with_status(
            &grid,
            &VoxelFillConfig {
                mode: FillMode::ExteriorFill,
                dilation_size: 0.0,
            },
            Vec3::new(2.5, 2.5, 2.5),
        );

        assert_eq!(outcome.grid.solid_count(), grid.solid_count());
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn exterior_fill_skips_when_seed_is_outside_grid() {
        let grid = closed_room();
        let outcome = fill_grid_with_status(
            &grid,
            &VoxelFillConfig {
                mode: FillMode::ExteriorFill,
                dilation_size: 0.0,
            },
            Vec3::new(99.0, 99.0, 99.0),
        );

        assert_eq!(outcome.grid.solid_count(), grid.solid_count());
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn carve_blocked_seed_without_nearby_free_cell_returns_raw_grid() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [3, 3, 3]);
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    grid.set(x, y, z, true);
                }
            }
        }

        let outcome = carve_grid_with_status(
            &grid,
            &VoxelCarveConfig {
                enabled: true,
                agent_height: 1.0,
                agent_radius: 0.0,
                seed_pos: [1.5, 1.5, 1.5],
            },
        );

        assert_eq!(outcome.grid.solid_count(), grid.solid_count());
        assert_eq!(outcome.after_solid, grid.solid_count());
        assert_eq!(outcome.reachable_cells, 0);
        assert!(outcome.resolved_seed.is_none());
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn carve_outside_seed_returns_raw_grid() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [3, 3, 3]);
        grid.set(1, 1, 1, true);

        let outcome = carve_grid_with_status(
            &grid,
            &VoxelCarveConfig {
                enabled: true,
                agent_height: 1.0,
                agent_radius: 0.0,
                seed_pos: [99.0, 99.0, 99.0],
            },
        );

        assert_eq!(outcome.grid.solid_count(), grid.solid_count());
        assert_eq!(outcome.after_solid, grid.solid_count());
        assert_eq!(outcome.reachable_cells, 0);
        assert!(outcome.resolved_seed.is_none());
        assert!(outcome.warning.is_some());
    }

    #[test]
    fn crop_to_occupied_removes_empty_padding() {
        let mut grid = VoxelGrid::new(Vec3::ZERO, 1.0, [6, 5, 4]);
        grid.set(4, 2, 1, true);

        let (cropped, stats) = grid.crop_to_occupied();

        assert_eq!(cropped.dims, [1, 1, 1]);
        assert_eq!(cropped.min, Vec3::new(4.0, 2.0, 1.0));
        assert!(cropped.get(0, 0, 0));
        assert_eq!(stats.before_dims, [6, 5, 4]);
        assert_eq!(stats.after_solid, 1);
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
