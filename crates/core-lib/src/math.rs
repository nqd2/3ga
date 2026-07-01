use serde::{Deserialize, Serialize};

pub type Vec3 = glam::Vec3;
pub type Quat = glam::Quat;

pub trait Vec3Ext {
    fn normalized(self) -> glam::Vec3;
}

impl Vec3Ext for glam::Vec3 {
    fn normalized(self) -> glam::Vec3 {
        self.normalize_or_zero()
    }
}

pub trait QuatExt {
    fn from_wxyz(w: f32, x: f32, y: f32, z: f32) -> glam::Quat;
    fn from_rotation_between(from: glam::Vec3, to: glam::Vec3) -> glam::Quat;
    fn rotate_vec3(self, v: glam::Vec3) -> glam::Vec3;
    fn inverse_rotate_vec3(self, v: glam::Vec3) -> glam::Vec3;
    fn normalized(self) -> glam::Quat;
}

impl QuatExt for glam::Quat {
    fn from_wxyz(w: f32, x: f32, y: f32, z: f32) -> glam::Quat {
        glam::Quat::from_xyzw(x, y, z, w)
    }
    fn from_rotation_between(from: glam::Vec3, to: glam::Vec3) -> glam::Quat {
        glam::Quat::from_rotation_arc(from.normalize_or_zero(), to.normalize_or_zero())
    }
    fn rotate_vec3(self, v: glam::Vec3) -> glam::Vec3 {
        self * v
    }
    fn inverse_rotate_vec3(self, v: glam::Vec3) -> glam::Vec3 {
        self.normalize().conjugate() * v
    }
    fn normalized(self) -> glam::Quat {
        if self.is_normalized() {
            self
        } else {
            let normalized = self.normalize();
            if normalized.is_nan() {
                glam::Quat::IDENTITY
            } else {
                normalized
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min: glam::Vec3,
    pub max: glam::Vec3,
}

impl Bounds {
    pub fn empty() -> Self {
        Self {
            min: glam::Vec3::splat(f32::INFINITY),
            max: glam::Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub fn include(&mut self, p: glam::Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn is_empty(self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }
}
