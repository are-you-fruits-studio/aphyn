use std::f32::consts::PI;

use aphyn::*;

pub type MeshData = (Vec<StandardVertex>, Vec<u16>);

pub fn make_dart() -> MeshData {
    let tip = [0.0, 0.0, 0.14];
    let left = [-0.055, 0.0, -0.07];
    let right = [0.055, 0.0, -0.07];
    let top = [0.0, 0.032, -0.05];
    let bottom = [0.0, -0.022, -0.05];
    let center = [0.0, 0.0, -0.02];

    let faces = [
        [tip, left, top],
        [tip, top, right],
        [tip, right, bottom],
        [tip, bottom, left],
        [left, right, top],
        [left, bottom, right],
    ];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for face in faces {
        let outward = sub(centroid(&face), center);
        let normal = normalize(cross(sub(face[1], face[0]), sub(face[2], face[0])));
        let normal = if dot(normal, outward) < 0.0 {
            scale(normal, -1.0)
        } else {
            normal
        };

        let base = vertices.len() as u16;
        for position in face {
            vertices.push(vertex(position, normal, [1.0, 0.0, 0.0, 1.0], [0.0, 0.0], [1.0; 4]));
        }
        push_front_tri(&vertices, &mut indices, [base, base + 1, base + 2], normal);
    }

    (vertices, indices)
}

pub fn make_sphere(radius: f32, is_inside_out: bool) -> MeshData {
    const STACKS: usize = 24;
    const SLICES: usize = 40;

    let mut vertices = Vec::with_capacity((STACKS + 1) * (SLICES + 1));
    let mut indices = Vec::new();

    for i in 0..=STACKS {
        let phi = PI * i as f32 / STACKS as f32;
        for j in 0..=SLICES {
            let theta = 2.0 * PI * j as f32 / SLICES as f32;
            let n = [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()];
            let normal = if is_inside_out { scale(n, -1.0) } else { n };
            vertices.push(vertex(
                scale(n, radius),
                normal,
                [-theta.sin(), 0.0, theta.cos(), 1.0],
                [j as f32 / SLICES as f32, i as f32 / STACKS as f32],
                [1.0; 4],
            ));
        }
    }

    let cols = (SLICES + 1) as u16;
    for i in 0..STACKS as u16 {
        for j in 0..SLICES as u16 {
            let first = i * cols + j;
            let second = first + cols;
            for tri in [[first, second, first + 1], [first + 1, second, second + 1]] {
                let outward = vertices[tri[0] as usize].normal;
                push_front_tri(&vertices, &mut indices, tri, outward);
            }
        }
    }

    (vertices, indices)
}

pub fn make_ground(cells: usize, cell_size: f32, fade_radius: [f32; 2]) -> MeshData {
    let mut vertices = Vec::with_capacity(cells * cells * 4);
    let mut indices = Vec::with_capacity(cells * cells * 6);
    let half = cells as f32 * cell_size * 0.5;

    for cz in 0..cells {
        for cx in 0..cells {
            let base = vertices.len() as u16;
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let x = (cx as f32 + u) * cell_size - half;
                let z = (cz as f32 + v) * cell_size - half;
                let distance = (x * x + z * z).sqrt();
                let fade = 1.0 - smoothstep(fade_radius[0], fade_radius[1], distance);
                vertices.push(vertex(
                    [x, 0.0, z],
                    [0.0, 1.0, 0.0],
                    [1.0, 0.0, 0.0, 1.0],
                    [u, v],
                    [fade, fade, fade, 1.0],
                ));
            }
            push_front_tri(&vertices, &mut indices, [base, base + 1, base + 2], [0.0, 1.0, 0.0]);
            push_front_tri(&vertices, &mut indices, [base, base + 2, base + 3], [0.0, 1.0, 0.0]);
        }
    }

    (vertices, indices)
}

pub fn make_tile_normal_map(size: u32, tiles: u32) -> Vec<u8> {
    let tile_height = |u: f32, v: f32| {
        let (fu, fv) = ((u * tiles as f32).fract(), (v * tiles as f32).fract());
        let edge = fu.min(1.0 - fu).min(fv).min(1.0 - fv);
        let bevel = smoothstep(0.0, 0.14, edge);
        let center = ((fu - 0.5).powi(2) + (fv - 0.5).powi(2)).sqrt();
        let dimple = 1.0 - smoothstep(0.08, 0.2, center);
        bevel - 0.6 * dimple
    };

    let texel = 1.0 / size as f32;
    let strength = 0.035;
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let (u, v) = ((x as f32 + 0.5) * texel, (y as f32 + 0.5) * texel);
            let dx = (tile_height(u + texel, v) - tile_height(u - texel, v)) / (2.0 * texel);
            let dy = (tile_height(u, v + texel) - tile_height(u, v - texel)) / (2.0 * texel);
            let n = normalize([-dx * strength, -dy * strength, 1.0]);
            data.extend(n.map(|c| ((c * 0.5 + 0.5) * 255.0).round() as u8));
            data.push(255);
        }
    }

    data
}

fn vertex(position: [f32; 3], normal: [f32; 3], tangent: [f32; 4], uv: [f32; 2], color: [f32; 4]) -> StandardVertex {
    StandardVertex {
        position,
        normal,
        tangent,
        color,
        uv,
    }
}

// Front faces are wound so that cross(b - a, c - a) points into the solid (see fruits_pong3d's mesh.rs).
fn push_front_tri(vertices: &[StandardVertex], indices: &mut Vec<u16>, tri: [u16; 3], outward: [f32; 3]) {
    let p = |i: u16| vertices[i as usize].position;
    let face_normal = cross(sub(p(tri[1]), p(tri[0])), sub(p(tri[2]), p(tri[0])));

    if dot(face_normal, outward) > 0.0 {
        indices.extend_from_slice(&[tri[0], tri[2], tri[1]]);
    } else {
        indices.extend_from_slice(&tri);
    }
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    a.map(|c| c * s)
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    scale(a, 1.0 / dot(a, a).sqrt())
}

fn centroid(face: &[[f32; 3]; 3]) -> [f32; 3] {
    scale([0, 1, 2].map(|i| face[0][i] + face[1][i] + face[2][i]), 1.0 / 3.0)
}
