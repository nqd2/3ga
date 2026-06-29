use crate::config::{AlignmentRecipe, UpAxis};
use crate::error::{AgError, AgResult};
use crate::math::{Quat, Vec3, QuatExt, Vec3Ext};
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
        let rotation = if let Some(points) = recipe.floor_points {
            let normal = floor_normal_from_three_points(points)?;
            Quat::from_rotation_between(normal, up_axis)
        } else if let Some(points) = recipe.floor_fit_points.as_deref() {
            let mut normal = fit_floor_normal(points)?;
            if normal.dot(up_axis) < 0.0 {
                normal = normal * -1.0;
            }
            Quat::from_rotation_between(normal, up_axis)
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

fn floor_normal_from_three_points(points: [[f32; 3]; 3]) -> AgResult<Vec3> {
    let a = Vec3::from_array(points[0]);
    let b = Vec3::from_array(points[1]);
    let c = Vec3::from_array(points[2]);
    let normal = (b - a).cross(c - a).normalized();
    if normal.length() <= f32::EPSILON {
        return Err(AgError::InvalidConfig(
            "floor calibration points are collinear".to_string(),
        ));
    }
    Ok(normal)
}

fn fit_floor_normal(points: &[[f32; 3]]) -> AgResult<Vec3> {
    if points.len() < 3 {
        return Err(AgError::InvalidConfig(
            "floorFitPoints requires at least 3 points".to_string(),
        ));
    }

    let mut centroid = Vec3::ZERO;
    for point in points {
        centroid = centroid + Vec3::from_array(*point);
    }
    centroid = centroid / points.len() as f32;

    let mut covariance = [[0.0f32; 3]; 3];
    for point in points {
        let p = Vec3::from_array(*point) - centroid;
        let values = [p.x, p.y, p.z];
        for row in 0..3 {
            for col in row..3 {
                covariance[row][col] += values[row] * values[col];
            }
        }
    }
    covariance[1][0] = covariance[0][1];
    covariance[2][0] = covariance[0][2];
    covariance[2][1] = covariance[1][2];

    if covariance[0][0] + covariance[1][1] + covariance[2][2] <= f32::EPSILON {
        return Err(AgError::InvalidConfig(
            "floorFitPoints are coincident".to_string(),
        ));
    }

    let (eigenvalues, eigenvectors) = jacobi_symmetric_3x3(covariance);
    let mut sorted = eigenvalues;
    sorted.sort_by(|a, b| a.total_cmp(b));
    if sorted[1] <= 1e-6 * sorted[2].max(1.0) {
        return Err(AgError::InvalidConfig(
            "floorFitPoints are collinear".to_string(),
        ));
    }
    let mut smallest = 0;
    for index in 1..3 {
        if eigenvalues[index] < eigenvalues[smallest] {
            smallest = index;
        }
    }
    let normal = Vec3::new(
        eigenvectors[0][smallest],
        eigenvectors[1][smallest],
        eigenvectors[2][smallest],
    )
    .normalized();

    if normal.length() <= f32::EPSILON {
        return Err(AgError::InvalidConfig(
            "floorFitPoints do not define a plane".to_string(),
        ));
    }
    Ok(normal)
}

fn jacobi_symmetric_3x3(mut matrix: [[f32; 3]; 3]) -> ([f32; 3], [[f32; 3]; 3]) {
    let mut vectors = [[0.0f32; 3]; 3];
    vectors[0][0] = 1.0;
    vectors[1][1] = 1.0;
    vectors[2][2] = 1.0;

    for _ in 0..32 {
        let mut p = 0;
        let mut q = 1;
        let mut largest = matrix[0][1].abs();
        for (row, col) in [(0, 2), (1, 2)] {
            let value = matrix[row][col].abs();
            if value > largest {
                largest = value;
                p = row;
                q = col;
            }
        }
        if largest <= 1e-8 {
            break;
        }

        let app = matrix[p][p];
        let aqq = matrix[q][q];
        let apq = matrix[p][q];
        let tau = (aqq - app) / (2.0 * apq);
        let t = if tau >= 0.0 {
            1.0 / (tau + (1.0 + tau * tau).sqrt())
        } else {
            -1.0 / (-tau + (1.0 + tau * tau).sqrt())
        };
        let c = 1.0 / (1.0 + t * t).sqrt();
        let s = t * c;

        for k in [0usize, 1, 2] {
            if k != p && k != q {
                let mkp = matrix[k][p];
                let mkq = matrix[k][q];
                matrix[k][p] = c * mkp - s * mkq;
                matrix[p][k] = matrix[k][p];
                matrix[k][q] = s * mkp + c * mkq;
                matrix[q][k] = matrix[k][q];
            }
        }
        matrix[p][p] = c * c * app - 2.0 * s * c * apq + s * s * aqq;
        matrix[q][q] = s * s * app + 2.0 * s * c * apq + c * c * aqq;
        matrix[p][q] = 0.0;
        matrix[q][p] = 0.0;

        for row in &mut vectors {
            let vip = row[p];
            let viq = row[q];
            row[p] = c * vip - s * viq;
            row[q] = s * vip + c * viq;
        }
    }

    ([matrix[0][0], matrix[1][1], matrix[2][2]], vectors)
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
            floor_points: None,
            floor_fit_points: None,
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
            floor_points: Some([[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]]),
            floor_fit_points: None,
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
    fn fitted_floor_points_become_level() {
        let recipe = AlignmentRecipe {
            floor_points: None,
            floor_fit_points: Some(vec![
                [-2.0, -0.5, -1.0],
                [-1.0, -0.25, 0.5],
                [0.0, 0.0, 0.0],
                [1.0, 0.25, -0.5],
                [2.0, 0.5, 1.0],
            ]),
            up_axis: Some(UpAxis::Y),
            scale_points: None,
            scale_distance_meters: None,
            origin: None,
        };

        let tx = AlignmentTransform::from_recipe(Some(&recipe)).unwrap();
        let a = tx.apply_point(Vec3::new(-2.0, -0.5, -1.0));
        let b = tx.apply_point(Vec3::new(2.0, 0.5, 1.0));

        assert!((a.y - b.y).abs() < 1e-5);
    }

    #[test]
    fn fitted_floor_rejects_collinear_points() {
        let recipe = AlignmentRecipe {
            floor_points: None,
            floor_fit_points: Some(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]),
            up_axis: None,
            scale_points: None,
            scale_distance_meters: None,
            origin: None,
        };

        let err = AlignmentTransform::from_recipe(Some(&recipe))
            .unwrap_err()
            .to_string();
        assert!(err.contains("floorFitPoints"));
    }
}