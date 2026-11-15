#include "aphyn/standard_types.wgsl"

@group(0) @binding(0) var<uniform> global_data: GlobalData;

struct SnowflakeAttributes {
    @location(5) position: vec3f,
    @location(7) angles: vec2f,
    @location(9) scale: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) normal: vec3f,
}

// tune to the size of snowflake.obj
const SNOWFLAKE_SCALE: f32 = 0.1;
const LIGHT_DIRECTION: vec3f = vec3f(0.3, 1.0, -0.5);
const SNOW_COLOR: vec3f = vec3f(0.85, 0.92, 1.0);

fn tumble(angles: vec2f) -> mat3x3f {
    let cx = cos(angles.x);
    let sx = sin(angles.x);
    let cy = cos(angles.y);
    let sy = sin(angles.y);

    let around_x = mat3x3f(vec3f(1.0, 0.0, 0.0), vec3f(0.0, cx, sx), vec3f(0.0, -sx, cx));
    let around_y = mat3x3f(vec3f(cy, 0.0, -sy), vec3f(0.0, 1.0, 0.0), vec3f(sy, 0.0, cy));

    return around_y * around_x;
}

@vertex
fn vs_main(vertex: VertexAttributes, snowflake: SnowflakeAttributes) -> VertexOutput {
    let rotation = tumble(snowflake.angles);
    let position_world = snowflake.position + rotation * (vertex.position * SNOWFLAKE_SCALE * snowflake.scale);

    var output: VertexOutput;
    output.clip_position = global_data.matrix_world_to_clip * vec4f(position_world, 1.0);
    output.normal = rotation * vertex.normal;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    // two-sided, since the flakes are thin and unculled
    let diffuse = abs(dot(normalize(input.normal), normalize(LIGHT_DIRECTION)));

    return vec4f(SNOW_COLOR * (0.35 + 0.65 * diffuse), 1.0);
}
