use crate::error::{AgError, AgResult};
use crate::math::{Bounds, Quat, QuatExt, Vec3};
use serde::{Deserialize, Serialize};

pub const SIGMA_CUTOFF: f32 = 3.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SplatTable {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub z: Vec<f32>,
    pub scale_0: Vec<f32>,
    pub scale_1: Vec<f32>,
    pub scale_2: Vec<f32>,
    pub opacity: Vec<f32>,
    pub f_dc_0: Vec<f32>,
    pub f_dc_1: Vec<f32>,
    pub f_dc_2: Vec<f32>,
    pub rot_0: Vec<f32>,
    pub rot_1: Vec<f32>,
    pub rot_2: Vec<f32>,
    pub rot_3: Vec<f32>,
    pub f_rest: Vec<Vec<f32>>,
}

impl SplatTable {
    pub fn len(&self) -> usize {
        self.x.len()
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    pub fn validate(&self) -> AgResult<()> {
        let len = self.x.len();
        let columns = [
            ("y", self.y.len()),
            ("z", self.z.len()),
            ("scale_0", self.scale_0.len()),
            ("scale_1", self.scale_1.len()),
            ("scale_2", self.scale_2.len()),
            ("opacity", self.opacity.len()),
            ("f_dc_0", self.f_dc_0.len()),
            ("f_dc_1", self.f_dc_1.len()),
            ("f_dc_2", self.f_dc_2.len()),
            ("rot_0", self.rot_0.len()),
            ("rot_1", self.rot_1.len()),
            ("rot_2", self.rot_2.len()),
            ("rot_3", self.rot_3.len()),
        ];
        for (name, got) in columns {
            if got != len {
                return Err(AgError::InvalidTable(format!(
                    "column {name} has length {got}, expected {len}"
                )));
            }
        }
        for (index, rest) in self.f_rest.iter().enumerate() {
            if rest.len() != len {
                return Err(AgError::InvalidTable(format!(
                    "f_rest_{index} has length {}, expected {len}",
                    rest.len()
                )));
            }
        }
        Ok(())
    }

    pub fn push_standard(
        &mut self,
        position: Vec3,
        scale: Vec3,
        opacity: f32,
        color_dc: Vec3,
        rotation: Quat,
    ) {
        self.x.push(position.x);
        self.y.push(position.y);
        self.z.push(position.z);
        self.scale_0.push(scale.x);
        self.scale_1.push(scale.y);
        self.scale_2.push(scale.z);
        self.opacity.push(opacity);
        self.f_dc_0.push(color_dc.x);
        self.f_dc_1.push(color_dc.y);
        self.f_dc_2.push(color_dc.z);
        self.rot_0.push(rotation.w);
        self.rot_1.push(rotation.x);
        self.rot_2.push(rotation.y);
        self.rot_3.push(rotation.z);
    }

    pub fn position(&self, index: usize) -> Vec3 {
        Vec3::new(self.x[index], self.y[index], self.z[index])
    }

    pub fn set_position(&mut self, index: usize, position: Vec3) {
        self.x[index] = position.x;
        self.y[index] = position.y;
        self.z[index] = position.z;
    }

    pub fn rotation(&self, index: usize) -> Quat {
        Quat::from_wxyz(
            self.rot_0[index],
            self.rot_1[index],
            self.rot_2[index],
            self.rot_3[index],
        )
        .normalized()
    }

    pub fn set_rotation(&mut self, index: usize, rotation: Quat) {
        let rotation = rotation.normalized();
        self.rot_0[index] = rotation.w;
        self.rot_1[index] = rotation.x;
        self.rot_2[index] = rotation.y;
        self.rot_3[index] = rotation.z;
    }

    pub fn linear_alpha(&self, index: usize) -> f32 {
        1.0 / (1.0 + (-self.opacity[index]).exp())
    }

    pub fn compact_by_mask(&self, keep: &[bool]) -> AgResult<Self> {
        if keep.len() != self.len() {
            return Err(AgError::InvalidInput(format!(
                "mask length {} does not match table length {}",
                keep.len(),
                self.len()
            )));
        }
        let mut out = Self {
            f_rest: vec![Vec::new(); self.f_rest.len()],
            ..Self::default()
        };
        for (i, keep_row) in keep.iter().copied().enumerate() {
            if !keep_row {
                continue;
            }
            out.push_standard(
                self.position(i),
                Vec3::new(self.scale_0[i], self.scale_1[i], self.scale_2[i]),
                self.opacity[i],
                Vec3::new(self.f_dc_0[i], self.f_dc_1[i], self.f_dc_2[i]),
                self.rotation(i),
            );
            for (dst, src) in out.f_rest.iter_mut().zip(self.f_rest.iter()) {
                dst.push(src[i]);
            }
        }
        Ok(out)
    }

    pub fn center_bounds(&self) -> Bounds {
        let mut bounds = Bounds::empty();
        for i in 0..self.len() {
            bounds.include(self.position(i));
        }
        bounds
    }

    pub fn gaussian_bounds(&self, index: usize) -> Bounds {
        let center = self.position(index);
        let rotation = self.rotation(index);
        let half_local = Vec3::new(
            self.scale_0[index].exp() * SIGMA_CUTOFF,
            self.scale_1[index].exp() * SIGMA_CUTOFF,
            self.scale_2[index].exp() * SIGMA_CUTOFF,
        );
        let axis_x = rotation.rotate_vec3(Vec3::new(half_local.x, 0.0, 0.0));
        let axis_y = rotation.rotate_vec3(Vec3::new(0.0, half_local.y, 0.0));
        let axis_z = rotation.rotate_vec3(Vec3::new(0.0, 0.0, half_local.z));
        let half_world = Vec3::new(
            axis_x.x.abs() + axis_y.x.abs() + axis_z.x.abs(),
            axis_x.y.abs() + axis_y.y.abs() + axis_z.y.abs(),
            axis_x.z.abs() + axis_y.z.abs() + axis_z.z.abs(),
        );
        Bounds {
            min: center - half_world,
            max: center + half_world,
        }
    }

    pub fn scene_bounds(&self) -> Bounds {
        let mut bounds = Bounds::empty();
        for i in 0..self.len() {
            let splat_bounds = self.gaussian_bounds(i);
            bounds.include(splat_bounds.min);
            bounds.include(splat_bounds.max);
        }
        bounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_mismatched_columns() {
        let table = SplatTable {
            x: vec![0.0],
            y: vec![0.0, 1.0],
            ..SplatTable::default()
        };
        let err = table.validate().unwrap_err().to_string();
        assert!(err.contains("column y"));
    }

    #[test]
    fn rotated_ellipsoid_bounds_follow_rotation() {
        let mut table = SplatTable::default();
        let s = 0.5f32.sqrt();
        table.push_standard(
            Vec3::ZERO,
            Vec3::new(2.0f32.ln(), 1.0f32.ln(), 0.5f32.ln()),
            4.0,
            Vec3::ZERO,
            Quat::from_wxyz(s, 0.0, 0.0, s),
        );

        let bounds = table.gaussian_bounds(0);

        assert!((bounds.min.x + 3.0).abs() < 1e-5);
        assert!((bounds.max.x - 3.0).abs() < 1e-5);
        assert!((bounds.min.y + 6.0).abs() < 1e-5);
        assert!((bounds.max.y - 6.0).abs() < 1e-5);
        assert!((bounds.min.z + 1.5).abs() < 1e-5);
        assert!((bounds.max.z - 1.5).abs() < 1e-5);
    }
}
