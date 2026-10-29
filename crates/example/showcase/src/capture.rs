use std::{fs::File, io::BufWriter, path::PathBuf, thread::JoinHandle};

use aphyn::*;
use wgpu::*;

use crate::{CAPTURE_FPS, LOOP_STEPS, LoopClock, SIM_HZ, WARMUP_LOOPS, WINDOW_SIZE, flock::FlockMember};

const FIRST_STEP: u64 = (WARMUP_LOOPS * LOOP_STEPS) as u64;
const STEPS_PER_FRAME: u64 = (SIM_HZ / CAPTURE_FPS) as u64;
const FRAMES: u64 = LOOP_STEPS as u64 / STEPS_PER_FRAME;
const PENDING_WRITES_MAX: usize = 8;
const CAPTURE_FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

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

#[derive(Resource)]
pub struct CaptureRequest(pub PathBuf);

#[derive(Resource)]
pub struct CaptureResource {
    dir: PathBuf,
    bind_group_layout: BindGroupLayout,
    pipeline: RenderPipeline,
    readback: Option<Readback>,
    writes: Vec<JoinHandle<()>>,
    previous_loop: Vec<Vec3<f32>>,
}

struct Readback {
    size: [u32; 2],
    texture: Texture,
    view: TextureView,
    buffer: Buffer,
    padded_row: u32,
}

pub fn register(mut world: WorldBuilderMut, dir: PathBuf) {
    std::fs::create_dir_all(&dir).expect("cannot create the capture directory");
    world.data_mut().resources_mut().insert(CaptureRequest(dir));

    let mut behavior = world.behavior_mut();

    behavior.get_mut(Schedule::Start).insert_system(setup_capture);

    let mut render_camera = behavior.get_mut(Schedule::RenderCamera);
    render_camera.insert_system(capture_frame);
    render_camera.order_group(SYSTEM_GROUP_RENDER).before_system(capture_frame);
}

pub fn setup_capture(mut world: WorldDataMut) {
    let mut res = world.resources_mut();
    let dir = res.get::<CaptureRequest>().unwrap().0.clone();
    let render_state = unsafe { res.get::<RenderApiResource>().unwrap().raw() };
    let device = render_state.device();

    let bind_group_layout = create_bind_group_layout(
        device,
        Some("Showcase Capture Bind Group Layout"),
        &[CreateBindGroupLayoutEntry::Texture],
    );
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Showcase Capture Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let shader = WgslComposer::with_engine_libraries()
        .create_shader_module(device, "showcase_capture.wgsl", SHADER, &[])
        .unwrap();
    let pipeline = create_render_pipeline(
        device,
        Some("Showcase Capture Pipeline"),
        &pipeline_layout,
        &shader,
        &[],
        ColorTargetState {
            format: CAPTURE_FORMAT,
            blend: None,
            write_mask: ColorWrites::ALL,
        },
        None,
        PrimitiveTopology::TriangleList,
        None,
    );

    res.insert(CaptureResource {
        dir,
        bind_group_layout,
        pipeline,
        readback: None,
        writes: Vec::new(),
        previous_loop: Vec::new(),
    });
}

pub fn measure_loop_seam(
    clock: Res<LoopClock>,
    mut capture: ResMut<CaptureResource>,
    boids: WorldQuery<(&FlockMember, &GlobalTransform)>,
) {
    if !clock.step.is_multiple_of(LOOP_STEPS as u64) {
        return;
    }

    let positions: Vec<_> = boids.iter().map(|(_, transform)| transform.position).collect();

    if !capture.previous_loop.is_empty() {
        let drift = positions
            .iter()
            .zip(capture.previous_loop.iter())
            .map(|(end, start)| (*end - *start).length())
            .fold(0.0, f64::max);
        println!(
            "loop {}: max boid drift from the previous loop {drift:.6} world units",
            clock.step / LOOP_STEPS as u64
        );
    }

    capture.previous_loop = positions;
}

pub fn capture_frame(
    clock: Res<LoopClock>,
    mut capture: ResMut<CaptureResource>,
    render_api: Res<RenderApiResource>,
    current: Res<CurrentRenderingResource>,
) {
    if clock.step < FIRST_STEP || !(clock.step - FIRST_STEP).is_multiple_of(STEPS_PER_FRAME) {
        return;
    }

    let frame = (clock.step - FIRST_STEP) / STEPS_PER_FRAME;

    if frame == FRAMES {
        for write in capture.writes.drain(..) {
            write.join().unwrap();
        }
        println!("captured {FRAMES} frames into {}", capture.dir.display());
        std::process::exit(0);
    }

    let render_state = unsafe { render_api.raw() };
    let (device, queue) = (render_state.device(), render_state.queue());
    let size = current.target_size();

    if capture.readback.as_ref().is_none_or(|readback| readback.size != size) {
        if size != WINDOW_SIZE {
            eprintln!(
                "warning: capturing {}x{}, expected {}x{}",
                size[0], size[1], WINDOW_SIZE[0], WINDOW_SIZE[1]
            );
        }
        capture.readback = Some(Readback::new(device, size));
    }

    let readback = capture.readback.as_ref().unwrap();
    let bind_group = create_bind_group(
        device,
        Some("Showcase Capture Bind Group"),
        &capture.bind_group_layout,
        &[CreateBindGroupEntry::Texture(&current.target_view)],
    );

    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Showcase Capture Encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Showcase Capture Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &readback.view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            ..Default::default()
        });
        pass.set_pipeline(&capture.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    encoder.copy_texture_to_buffer(
        readback.texture.as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &readback.buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(readback.padded_row),
                rows_per_image: Some(size[1]),
            },
        },
        Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    let rgb = readback.read(device);
    let path = capture.dir.join(format!("frame_{frame:04}.png"));

    if capture.writes.len() >= PENDING_WRITES_MAX {
        capture.writes.remove(0).join().unwrap();
    }
    capture.writes.push(std::thread::spawn(move || write_png(path, size, &rgb)));
}

impl Readback {
    fn new(device: &Device, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Showcase Capture Texture"),
            size: Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: CAPTURE_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        let padded_row = (size[0] * 4).div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Showcase Capture Buffer"),
            size: padded_row as u64 * size[1] as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            size,
            texture,
            view,
            buffer,
            padded_row,
        }
    }

    fn read(&self, device: &Device) -> Vec<u8> {
        let slice = self.buffer.slice(..);
        slice.map_async(MapMode::Read, |result| result.unwrap());
        device.poll(PollType::wait_indefinitely()).unwrap();

        let mut rgb = Vec::with_capacity((self.size[0] * self.size[1] * 3) as usize);
        for row in slice.get_mapped_range().chunks(self.padded_row as usize) {
            for pixel in row[..self.size[0] as usize * 4].chunks(4) {
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        self.buffer.unmap();

        rgb
    }
}

fn write_png(path: PathBuf, size: [u32; 2], rgb: &[u8]) {
    let file = BufWriter::new(File::create(&path).unwrap());
    let mut encoder = png::Encoder::new(file, size[0], size[1]);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(rgb).unwrap();
}
