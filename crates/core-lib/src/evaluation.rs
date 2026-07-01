use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::splat_table::SplatTable;

#[derive(Debug, Clone, Copy, Default)]
pub struct GeometricError {
    pub sample_count: usize,
    pub mean: f32,
    pub rms: f32,
    pub p95: f32,
}

pub fn mesh_error_against_splat_centers(table: &SplatTable, mesh: &Mesh) -> GeometricError {
    if table.is_empty() || mesh.indices.is_empty() {
        return GeometricError::default();
    }
    let target_samples = table.len().min(4096);
    let stride = (table.len() / target_samples).max(1);
    let mut distances = Vec::with_capacity(target_samples);
    for i in (0..table.len()).step_by(stride).take(target_samples) {
        let p = table.position(i);
        let mut best = f32::INFINITY;
        for tri in mesh.indices.chunks_exact(3) {
            let a = Vec3::from_array(mesh.vertices[tri[0] as usize]);
            let b = Vec3::from_array(mesh.vertices[tri[1] as usize]);
            let c = Vec3::from_array(mesh.vertices[tri[2] as usize]);
            best = best.min(point_triangle_distance(p, a, b, c));
        }
        if best.is_finite() {
            distances.push(best);
        }
    }
    if distances.is_empty() {
        return GeometricError::default();
    }
    distances.sort_by(|a, b| a.total_cmp(b));
    let sum = distances.iter().sum::<f32>();
    let sq_sum = distances.iter().map(|d| d * d).sum::<f32>();
    let p95_index = ((distances.len() - 1) as f32 * 0.95).round() as usize;
    GeometricError {
        sample_count: distances.len(),
        mean: sum / distances.len() as f32,
        rms: (sq_sum / distances.len() as f32).sqrt(),
        p95: distances[p95_index],
    }
}

fn point_triangle_distance(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (p - a).length();
    }

    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (p - b).length();
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (p - (a + ab * v)).length();
    }

    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (p - c).length();
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (p - (a + ac * w)).length();
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (p - (b + (c - b) * w)).length();
    }

    let normal = ab.cross(ac).normalize();
    (p - a).dot(normal).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_on_triangle_has_zero_error() {
        let d = point_triangle_distance(
            Vec3::new(0.25, 0.0, 0.25),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        assert!(d < 1e-6);
    }
}
