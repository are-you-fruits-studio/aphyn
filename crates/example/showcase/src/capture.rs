use std::{fs::File, io::BufWriter, path::PathBuf, thread::JoinHandle};

use aphyn::*;

use crate::{CAPTURE_FPS, LOOP_STEPS, LoopClock, SIM_HZ, WARMUP_LOOPS, WINDOW_SIZE, flock::FlockMember};

const FIRST_STEP: u64 = (WARMUP_LOOPS * LOOP_STEPS) as u64;
const STEPS_PER_FRAME: u64 = (SIM_HZ / CAPTURE_FPS) as u64;
const FRAMES: u64 = LOOP_STEPS as u64 / STEPS_PER_FRAME;
const PENDING_WRITES_MAX: usize = 8;
const CAPTURE_FORMAT: GpuTextureFormat = GpuTextureFormat::Rgba8UnormSrgb;
// after every engine pass, gizmos included, so the capture matches what the window shows
const RENDER_ORDER_CAPTURE: u64 = RENDER_ORDER_GIZMOS + 50_000;

const SHADER: &str = r#"
#include "aphyn/fullscreen.wgsl"

@group(0) @binding(0) var src_texture: texture_2d<f32>;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4f {
    return vec4f(fullscreen_triangle_position(vertex_index), 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    return vec4f(textureLoad(src_texture, vec2i(position.xy), 0).xyz, 1.0);
}
"#;

#[derive(Resource, Default)]
pub struct LoopSeamResource {
    previous_loop: Vec<Vec3<f32>>,
}

pub fn register(mut data: WorldDataMut, _behavior: WorldBehaviorBuilderMut, render: &mut RenderFeatureRegistry, dir: PathBuf) {
    std::fs::create_dir_all(&dir).expect("cannot create the capture directory");
    data.resources_mut().insert(LoopSeamResource::default());

    assert!(render.add_feature(
        "showcase_capture",
        CaptureFeature {
            dir,
            shader: None,
            texture: None,
            writes: Vec::new(),
        }
    ));
}

pub fn measure_loop_seam(
    clock: Res<LoopClock>,
    mut seam: ResMut<LoopSeamResource>,
    boids: WorldQuery<(&FlockMember, &GlobalTransform)>,
) {
    if !clock.step.is_multiple_of(LOOP_STEPS as u64) {
        return;
    }

    let positions: Vec<_> = boids.iter().map(|(_, transform)| transform.position).collect();

    if !seam.previous_loop.is_empty() {
        let drift = positions
            .iter()
            .zip(seam.previous_loop.iter())
            .map(|(end, start)| (*end - *start).length())
            .fold(0.0, f64::max);
        println!(
            "loop {}: max boid drift from the previous loop {drift:.6} world units",
            clock.step / LOOP_STEPS as u64
        );
    }

    seam.previous_loop = positions;
}

struct CaptureFeature {
    dir: PathBuf,
    shader: Option<GpuShader>,
    texture: Option<GpuTexture>,
    writes: Vec<JoinHandle<()>>,
}

impl RenderFeature for CaptureFeature {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
        let Some(clock) = world.resources().get::<LoopClock>() else {
            return;
        };

        if clock.step < FIRST_STEP || !(clock.step - FIRST_STEP).is_multiple_of(STEPS_PER_FRAME) {
            return;
        }

        let frame = (clock.step - FIRST_STEP) / STEPS_PER_FRAME;

        passes.add(RENDER_ORDER_CAPTURE, move |this, ctx| this.capture(frame, ctx));
    }
}

impl CaptureFeature {
    fn capture(&mut self, frame: u64, ctx: &mut RenderPassCtx) {
        if frame == FRAMES {
            for write in self.writes.drain(..) {
                write.join().unwrap();
            }
            println!("captured {FRAMES} frames into {}", self.dir.display());
            std::process::exit(0);
        }

        let core = ctx.render_api.core();
        let Some(current) = ctx.world.resources().get::<CurrentRenderingResource>() else {
            return;
        };
        let target = current.target.texture();
        let size = target.size();

        let shader = self.shader.get_or_insert_with(|| {
            let code = WgslComposer::with_engine_libraries()
                .compose("showcase_capture.wgsl", SHADER, &[])
                .unwrap();
            let desc = GpuShaderDesc {
                cull_mode: GpuCullMode::None,
                depth_write: false,
                ..Default::default()
            };

            core.create_shader(Some("showcase capture"), &code, desc).unwrap()
        });

        if self.texture.as_ref().is_none_or(|texture| texture.size() != size) {
            if size != WINDOW_SIZE.map(|x| x as usize) {
                eprintln!(
                    "warning: capturing {}x{}, expected {}x{}",
                    size[0], size[1], WINDOW_SIZE[0], WINDOW_SIZE[1]
                );
            }
            self.texture = Some(core.create_texture(Some("showcase capture"), size, CAPTURE_FORMAT, 1));
        }

        let texture = self.texture.as_ref().unwrap();

        core.draw(
            &GpuRenderPass {
                shader,
                target: Some(GpuRenderPassTarget {
                    view: texture,
                    mip_level: 0,
                    ops: GpuPassOps::Clear([0.0, 0.0, 0.0, 1.0]),
                }),
                depth: None,
                bind_groups: &[&[(
                    0,
                    GpuBinding::Texture {
                        texture: target,
                        mip_level: None,
                    },
                )]],
                vertex_buffers: &[],
                viewport: None,
                scissor: None,
            },
            0..3,
            0..1,
        );

        let Some(rgba) = core.read_texture(texture, 0) else {
            eprintln!("failed to read back frame {frame}");
            return;
        };

        let rgb = rgba.chunks(4).flat_map(|pixel| &pixel[..3]).copied().collect::<Vec<_>>();
        let path = self.dir.join(format!("frame_{frame:04}.png"));
        let size = size.map(|x| x as u32);

        if self.writes.len() >= PENDING_WRITES_MAX {
            self.writes.remove(0).join().unwrap();
        }
        self.writes.push(std::thread::spawn(move || write_png(path, size, &rgb)));
    }
}

fn write_png(path: PathBuf, size: [u32; 2], rgb: &[u8]) {
    let file = BufWriter::new(File::create(&path).unwrap());
    let mut encoder = png::Encoder::new(file, size[0], size[1]);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(rgb).unwrap();
}
