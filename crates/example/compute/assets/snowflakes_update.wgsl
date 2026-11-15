// must match the instance buffer layout of the example (locations 5 to 8, tightly packed)
struct Snowflake {
    position: vec3f,
    fall_speed: f32,
    angles: vec2f,
    angular_velocity: vec2f,
}

struct Params {
    world_to_clip: mat4x4f,
    clip_to_world: mat4x4f,
    delta_time: f32,
    count: u32,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> snowflakes: array<Snowflake>;

const TAU: f32 = 6.28318530718;
// a bit past the view edge, so a snowflake leaves and enters the view whole
const VIEW_EDGE: f32 = 1.1;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= params.count {
        return;
    }

    var snowflake = snowflakes[id.x];

    snowflake.position.y -= snowflake.fall_speed * params.delta_time;
    snowflake.angles = (snowflake.angles + snowflake.angular_velocity * params.delta_time) % TAU;

    let clip = params.world_to_clip * vec4f(snowflake.position, 1.0);
    let ndc = clip.xyz / clip.w;

    if clip.w > 0.0 && ndc.y < -VIEW_EDGE {
        let top = params.clip_to_world * vec4f(ndc.x, VIEW_EDGE, ndc.z, 1.0);
        snowflake.position = top.xyz / top.w;
    }

    snowflakes[id.x] = snowflake;
}
