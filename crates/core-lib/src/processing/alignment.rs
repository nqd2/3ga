use crate::config::{AlignmentRecipe, UpAxis};
use crate::error::{AgError, AgResult};
use crate::math::{Quat, QuatExt, Vec3, Vec3Ext};
use crate::splat_table::SplatTable;

#[derive(Debug, Clone, Copy)]
pub struct AlignmentTransform {
    pub rotation: Quat,
    pub scale: f32,
    pub origin: Vec3,
}

impl Default for AlignmentTransform {
    fn default() -> Self {
        Self {
            rotation: Quat::IDENTITY,
            scale: 1.0,
            origin: Vec3::ZERO,
        }
    }
}

impl AlignmentTransform {
    pub fn from_recipe(recipe: Option<&AlignmentRecipe>) -> AgResult<Self> {
        let Some(recipe) = recipe else {
            return Ok(Self::default());
        };
        let up_axis = up_axis_vec(recipe.up_axis);
        let rotation = if let Some(normal_arr) = recipe.floor_normal {
            let normal = Vec3::from_array(normal_arr);
            if normal.length_squared() > f32::EPSILON {
                Quat::from_rotation_between(normal.normalized(), up_axis)
            } else {
                Quat::IDENTITY
            }
        } else {
            Quat::IDENTITY
        };

        let scale = if let (Some(points), Some(distance)) =
            (recipe.scale_points, recipe.scale_distance_meters)
        {
            if distance <= 0.0 {
                return Err(AgError::InvalidConfig(
                    "scale distance must be positive".to_string(),
                ));
            }
            let a = rotation.rotate_vec3(Vec3::from_array(points[0]));
            let b = rotation.rotate_vec3(Vec3::from_array(points[1]));
            let measured = (b - a).length();
            if measured <= f32::EPSILON {
                return Err(AgError::InvalidConfig(
                    "scale calibration points are identical".to_string(),
                ));
            }
            distance / measured
        } else {
            1.0
        };

        Ok(Self {
            rotation,
            scale,
            origin: recipe.origin.map(Vec3::from_array).unwrap_or(Vec3::ZERO),
        })
    }

    pub fn apply_point(self, point: Vec3) -> Vec3 {
        self.rotation.rotate_vec3(point - self.origin) * self.scale
    }
}

fn up_axis_vec(axis: Option<UpAxis>) -> Vec3 {
    match axis.unwrap_or(UpAxis::Y) {
        UpAxis::X => Vec3::new(1.0, 0.0, 0.0),
        UpAxis::Y => Vec3::Y,
        UpAxis::Z => Vec3::new(0.0, 0.0, 1.0),
        UpAxis::NegX => Vec3::new(-1.0, 0.0, 0.0),
        UpAxis::NegY => Vec3::new(0.0, -1.0, 0.0),
        UpAxis::NegZ => Vec3::new(0.0, 0.0, -1.0),
    }
}

pub fn bake_alignment(
    table: &mut SplatTable,
    recipe: Option<&AlignmentRecipe>,
) -> AgResult<AlignmentTransform> {
    let transform = AlignmentTransform::from_recipe(recipe)?;
    if transform == AlignmentTransform::default() {
        return Ok(transform);
    }
    for i in 0..table.len() {
        let point = transform.apply_point(table.position(i));
        table.set_position(i, point);
        table.set_rotation(i, transform.rotation * table.rotation(i));
        let scale_delta = transform.scale.ln();
        table.scale_0[i] += scale_delta;
        table.scale_1[i] += scale_delta;
        table.scale_2[i] += scale_delta;
    }
    Ok(transform)
}

impl PartialEq for AlignmentTransform {
    fn eq(&self, other: &Self) -> bool {
        self.rotation == other.rotation && self.scale == other.scale && self.origin == other.origin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_calibration_matches_distance() {
        let recipe = AlignmentRecipe {
            floor_normal: None,
            up_axis: None,
            scale_points: Some([[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]]),
            scale_distance_meters: Some(2.0),
            origin: None,
        };
        let tx = AlignmentTransform::from_recipe(Some(&recipe)).unwrap();
        let a = tx.apply_point(Vec3::ZERO);
        let b = tx.apply_point(Vec3::new(4.0, 0.0, 0.0));
        assert!(((b - a).length() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn floor_alignment_uses_selected_up_axis() {
        let recipe = AlignmentRecipe {
            floor_normal: Some([0.0, 1.0, 0.0]),
            up_axis: Some(UpAxis::Z),
            scale_points: None,
            scale_distance_meters: None,
            origin: None,
        };

        let tx = AlignmentTransform::from_recipe(Some(&recipe)).unwrap();
        let rotated_normal = tx.rotation.rotate_vec3(Vec3::Y);

        assert!((rotated_normal - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-5);
    }

    #[test]
    fn floor_alignment_maps_origin_to_zero_plane() {
        let recipe = AlignmentRecipe {
            floor_normal: Some([0.0, 1.0, 0.0]),
            up_axis: Some(UpAxis::Y),
            scale_points: None,
            scale_distance_meters: None,
            origin: Some([2.0, 1.0, 3.0]),
        };

        let tx = AlignmentTransform::from_recipe(Some(&recipe)).unwrap();
        let origin = tx.apply_point(Vec3::new(2.0, 1.0, 3.0));
        let floor_point = tx.apply_point(Vec3::new(4.0, 1.0, 5.0));
        let rotated_normal = tx.rotation.rotate_vec3(Vec3::Y);

        assert!(origin.length() < 1e-6);
        assert!(floor_point.y.abs() < 1e-6);
        assert!((rotated_normal - Vec3::Y).length() < 1e-5);
    }
}
