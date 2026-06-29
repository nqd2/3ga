struct Splat {
    position: vec4<f32>,
    sigma_alpha: vec4<f32>,
    rotation: vec4<f32>,
};

struct Uniforms {
    grid_min: vec4<f32>,
    dims: vec4<u32>,
    voxel_size: f32,
    opacity_threshold: f32,
    splat_count: u32,
    voxel_count: u32,
};

@group(0) @binding(0)
var<storage, read> splats: array<Splat>;
@group(0) @binding(1)
var<uniform> uniforms: Uniforms;
@group(0) @binding(2)
var<storage, read_write> output: array<u32>;

fn rotate_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    let qv = vec3<f32>(q.y, q.z, q.w);
    let t = cross(qv, v) * 2.0;
    return v + t * q.x + cross(qv, t);
}

fn inverse_rotate_by_quat(v: vec3<f32>, q: vec4<f32>) -> vec3<f32> {
    return rotate_by_quat(v, vec4<f32>(q.x, -q.y, -q.z, -q.w));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let splat_index = global_id.x;
    if (splat_index >= uniforms.splat_count) {
        return;
    }

    let splat = splats[splat_index];
    if (splat.sigma_alpha.w < uniforms.opacity_threshold) {
        return;
    }
    let sigma = max(splat.sigma_alpha.xyz, vec3<f32>(0.000001));
    let q = normalize(splat.rotation);
    let axis_x = rotate_by_quat(vec3<f32>(sigma.x * 3.0, 0.0, 0.0), q);
    let axis_y = rotate_by_quat(vec3<f32>(0.0, sigma.y * 3.0, 0.0), q);
    let axis_z = rotate_by_quat(vec3<f32>(0.0, 0.0, sigma.z * 3.0), q);
    let half_world = abs(axis_x) + abs(axis_y) + abs(axis_z);
    let min_local = (splat.position.xyz - half_world - uniforms.grid_min.xyz) / uniforms.voxel_size;
    let max_local = (splat.position.xyz + half_world - uniforms.grid_min.xyz) / uniforms.voxel_size;
    let min_cell = vec3<i32>(
        max(0, i32(floor(min_local.x))),
        max(0, i32(floor(min_local.y))),
        max(0, i32(floor(min_local.z)))
    );
    let max_cell = vec3<i32>(
        min(i32(uniforms.dims.x) - 1, i32(floor(max_local.x))),
        min(i32(uniforms.dims.y) - 1, i32(floor(max_local.y))),
        min(i32(uniforms.dims.z) - 1, i32(floor(max_local.z)))
    );
    if (min_cell.x > max_cell.x || min_cell.y > max_cell.y || min_cell.z > max_cell.z) {
        return;
    }

    var z = min_cell.z;
    loop {
        if (z > max_cell.z) { break; }
        var y = min_cell.y;
        loop {
            if (y > max_cell.y) { break; }
            var x = min_cell.x;
            loop {
                if (x > max_cell.x) { break; }
                let center = uniforms.grid_min.xyz + (vec3<f32>(f32(x), f32(y), f32(z)) + vec3<f32>(0.5)) * uniforms.voxel_size;
                let local = inverse_rotate_by_quat(center - splat.position.xyz, q);
                let d = local / sigma;
                let n2 = dot(d, d);
                let contribution = splat.sigma_alpha.w * exp(-0.5 * n2);
                if (contribution >= uniforms.opacity_threshold) {
                    let index = u32(x) + u32(y) * uniforms.dims.x + u32(z) * uniforms.dims.x * uniforms.dims.y;
                    output[index] = 1u;
                }
                x = x + 1;
            }
            y = y + 1;
        }
        z = z + 1;
    }
}
