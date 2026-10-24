fn fullscreen_triangle_position(vertex_index: u32) -> vec2f {
    var positions = array<vec2f, 3>(
        vec2f(-1.0, -3.0),
        vec2f(3.0, 1.0),
        vec2f(-1.0, 1.0)
    );

    return positions[vertex_index];
}

fn fullscreen_uv(position: vec2f) -> vec2f {
    return vec2f(position.x * 0.5 + 0.5, 0.5 - position.y * 0.5);
}
