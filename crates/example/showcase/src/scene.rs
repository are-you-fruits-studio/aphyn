use std::{f32::consts::TAU, fmt::Write};

use aphyn::*;

use crate::{
    LoopClock,
    flock::{BOIDS_PER_FLOCK, FLOCKS, FlockLight, FlockMember, Seek, Separation, Velocity, boid_target},
    mesh,
};

// Aphyn Design System tokens.
const NIGHT_900: u32 = 0x14131C;
const NIGHT_800: u32 = 0x1F1E2A;
const NIGHT_700: u32 = 0x2E2D3D;
const NIGHT_300: u32 = 0x9C9BB0;
const NIGHT_100: u32 = 0xE3E2EA;
const BILBERRY_200: u32 = 0xB9B5E3;
const BILBERRY_300: u32 = 0x9790D4;
const BILBERRY_400: u32 = 0x7870C4;
const BILBERRY_500: u32 = 0x5A51AE;
const BILBERRY_600: u32 = 0x3B3486;
const BLOOM_300: u32 = 0xABBAD6;
const AXIS_X: u32 = 0xD2675B;
const AXIS_Y: u32 = 0x6E9B67;
const AXIS_Z: u32 = 0x7287B0;

const EXPOSURE: f32 = 0.5;
const GLOW_EVERY: usize = 9;
const STREAM_LENGTH: f32 = 0.09;
const SEED: u64 = 0xA9_4F_15;

const CAMERA_RADIUS: f32 = 7.4;
const CAMERA_HEIGHT: f32 = 0.9;
const CAMERA_TARGET: Vec3<f32> = Vec3::new(0.0, 0.3, 0.0);

#[derive(Component)]
pub struct OrbitCamera;

#[derive(Component)]
pub struct EntityCountText;

pub fn setup_post_processing(
    mut bloom: ResMut<BloomResource>,
    mut exposure: ResMut<ExposureResource>,
    mut grading: ResMut<ColorGradingResource>,
) {
    bloom.is_enabled = true;
    bloom.threshold = 2.2;
    bloom.threshold_softening = 0.5;
    bloom.intensity = 0.55;

    exposure.is_enabled = true;
    exposure.exposure = EXPOSURE;

    grading.ty = Some(ColorGradingType::Aces);
}

pub fn setup_scene(mut world: WorldDataMut) {
    let (mut res, mut ec, _) = world.as_tuple_mut();

    let render_api = res.get::<RenderApiResource>().unwrap().clone();
    let ui_assets = res.get::<StandardUiAssetsResource>().unwrap();
    let (font, font_texture) = (ui_assets.font_px_8_12.clone(), ui_assets.texture_text_px_8_12.clone());

    let normal_map = render_api.create_texture(
        wgpu::FilterMode::Linear,
        [512, 512],
        &mesh::make_tile_normal_map(512, 1),
        StandardTextureAssetMetadata {
            raw_texture: Default::default(),
            format: StandardTextureFormat::Rgba8Unorm,
            should_generate_mipmaps: true,
        },
    );
    let normal_map = res
        .get_mut::<AssetStorageResource<StandardTexture>>()
        .unwrap()
        .insert(normal_map);

    let background_color = gamma(pre_tonemap(srgb(NIGHT_900)), 1.0);

    let materials = [
        StandardMaterialAssetMetadata {
            shader: StandardEngineShader::OpaqueUnlit.into(),
            color: background_color,
            ..Default::default()
        },
        StandardMaterialAssetMetadata {
            shader: StandardEngineShader::OpaqueLit.into(),
            color: gamma(srgb(NIGHT_800), 1.0),
            emission_color: background_color,
            metallic: 0.1,
            roughness: 0.5,
            normal_tex: normal_map,
            ..Default::default()
        },
        StandardMaterialAssetMetadata {
            shader: StandardEngineShader::OpaqueLit.into(),
            color: gamma(srgb(BILBERRY_600), 1.0),
            emission_color: gamma(srgb(BILBERRY_500), 0.15),
            roughness: 0.3,
            ..Default::default()
        },
        StandardMaterialAssetMetadata {
            shader: StandardEngineShader::OpaqueLit.into(),
            color: gamma(srgb(BILBERRY_500), 1.0),
            emission_color: gamma(srgb(BILBERRY_400), 0.35),
            roughness: 0.5,
            ..Default::default()
        },
        StandardMaterialAssetMetadata {
            shader: StandardEngineShader::OpaqueLit.into(),
            color: gamma(srgb(BILBERRY_200), 1.0),
            emission_color: gamma(srgb(BILBERRY_300), 3.5),
            roughness: 0.5,
            ..Default::default()
        },
        StandardMaterialAssetMetadata {
            shader: StandardEngineShader::UiOpaqueUnlit.into(),
            color_tex: font_texture,
            alpha_threshold: 0.5,
            ..Default::default()
        },
    ]
    .map(|meta| render_api.create_material(meta));

    let material_storage = res.get_mut::<AssetStorageResource<StandardMaterial>>().unwrap();
    let [
        background_material,
        ground_material,
        berry_material,
        boid_material,
        glow_material,
        text_material,
    ] = materials.map(|material| material_storage.insert(material));

    let mesh_storage = res.get_mut::<AssetStorageResource<StandardMesh>>().unwrap();
    let [background_mesh, ground_mesh, berry_mesh, dart_mesh] = [
        mesh::make_sphere(60.0, true),
        mesh::make_ground(6, 2.4, [2.0, 6.0]),
        mesh::make_sphere(0.85, false),
        mesh::make_dart(),
    ]
    .map(|(vertices, indices)| mesh_storage.insert(render_api.create_mesh(&vertices, &indices, None)));

    let spawn_mesh =
        |ec: &mut EntitiesHolderMut, mesh: &AssetHandle<StandardMesh>, material: &AssetHandle<StandardMaterial>, position| {
            let entity = ec.create_entity();
            ec.set_components(
                entity,
                (
                    StandardMeshComponent { mesh: mesh.clone() },
                    StandardMaterialComponent {
                        material: material.clone(),
                    },
                    GlobalTransform {
                        position,
                        scale_rotation: Mat3::IDENTITY,
                    },
                ),
            );
            entity
        };

    spawn_mesh(&mut ec, &background_mesh, &background_material, Vec3::splat(0.0));
    spawn_mesh(&mut ec, &ground_mesh, &ground_material, Vec3::new(0.0, -2.3, 0.0));
    spawn_mesh(&mut ec, &berry_mesh, &berry_material, Vec3::splat(0.0));

    let mut rng = SplitMix64(SEED);

    for flock in 0..FLOCKS {
        for i in 0..BOIDS_PER_FLOCK {
            let offset = loop {
                let v = Vec3::new(rng.next_signed(), rng.next_signed(), rng.next_signed());
                if v.length() <= 1.0 {
                    break Vec3::new(v.x * 0.5, v.y * 0.25, v.z * 0.5);
                }
            };
            let member = FlockMember {
                flock,
                lag: (rng.next_signed() * 0.5 + 0.5) * STREAM_LENGTH,
                offset,
                swirl_turns: [-2.0, -1.0, 1.0, 2.0][(rng.next_u64() % 4) as usize],
            };
            let material = if i % GLOW_EVERY == 0 { &glow_material } else { &boid_material };
            let entity = spawn_mesh(&mut ec, &dart_mesh, material, boid_target(&member, 0.0));

            ec.set_components(entity, (member, Velocity::default(), Seek::default(), Separation::default()));
        }
    }

    let light = |ec: &mut EntitiesHolderMut, light: StandardLightComponent, transform: GlobalTransform| {
        let entity = ec.create_entity();
        ec.set_components(entity, (light, transform));
        entity
    };

    light(
        &mut ec,
        StandardLightComponent::Point {
            color: srgb(NIGHT_100) * 6.0,
            range: 30.0,
        },
        GlobalTransform {
            position: Vec3::new(0.0, 1.4, 0.0),
            scale_rotation: Mat3::IDENTITY,
        },
    );
    light(
        &mut ec,
        StandardLightComponent::Directional {
            color: srgb(BLOOM_300) * 0.6,
        },
        GlobalTransform {
            position: Vec3::splat(0.0),
            scale_rotation: Mat3::rotation_y(0.7) * Mat3::rotation_x(-0.5),
        },
    );
    for flock in 0..FLOCKS {
        let entity = light(
            &mut ec,
            StandardLightComponent::Point {
                color: srgb(BILBERRY_400) * 4.5,
                range: 20.0,
            },
            GlobalTransform::IDENTITY,
        );
        ec.add_component(entity, FlockLight(flock)).ok().unwrap();
    }

    let camera = ec.create_entity();
    ec.set_components(
        camera,
        (
            GlobalTransform::IDENTITY,
            CameraComponent(Camera {
                near: 0.1,
                far: 200.0,
                projection: CameraProjection::Perspective {
                    fov: 50_f32.to_radians(),
                },
            }),
            OrbitCamera,
        ),
    );

    let text = |ec: &mut EntitiesHolderMut, content: &str, size: f32, top: f32, color: u32| {
        let entity = ec.create_entity();
        ec.set_components(
            entity,
            (
                BatchedMeshComponent::default(),
                StandardMaterialComponent {
                    material: text_material.clone(),
                },
                GlobalRectComponent::default(),
                LocalRectComponent {
                    anchor: Vec2::splat(0.0),
                    pivot: Vec2::splat(0.0),
                    offset: Vec2::new(UiVal::px(36.0), UiVal::px(top)),
                    scale: Vec2::new(UiVal::px(600.0), UiVal::px(size)).map(Into::into),
                    z: -10.0,
                    ..Default::default()
                },
                TextComponent {
                    font: font.clone(),
                    text: String::from(content).into(),
                    font_size: UiVal::px(size),
                    horizontal_align: HorizontalAlign::Left,
                    vertical_align: VerticalAlign::Top,
                    horizontal_spacing: UiVal::px(size * 0.12),
                    color: gamma(pre_tonemap(srgb(color)), 1.0),
                    ..Default::default()
                },
            ),
        );
        entity
    };

    text(&mut ec, "aphyn", 32.0, 30.0, NIGHT_100);
    let count = text(&mut ec, "", 16.0, 72.0, NIGHT_300);
    ec.add_component(count, EntityCountText).ok().unwrap();
}

pub fn orbit_camera(clock: Res<LoopClock>, mut query: WorldQuery<(&OrbitCamera, &mut GlobalTransform)>) {
    let t = TAU * clock.phase();

    for (_, transform) in query.iter_mut() {
        let angle = t + 0.6;
        transform.position = Vec3::new(
            CAMERA_RADIUS * angle.cos(),
            CAMERA_HEIGHT + 0.5 * (2.0 * t).sin(),
            CAMERA_RADIUS * angle.sin(),
        );
        transform.scale_rotation = Quat::look_rotation(CAMERA_TARGET - transform.position, Vec3::Y).to_matrix();
    }
}

pub fn update_entity_count(entities: WorldQuery<EntityId>, mut texts: WorldQuery<(&EntityCountText, &mut TextComponent)>) {
    let count = entities.iter().count();

    for (_, text) in texts.iter_mut() {
        text.text.clear();
        write!(text.text, "{count} entities").unwrap();
    }
}

pub fn draw_gizmos(mut gizmos: ResMut<GizmosResource>) {
    let lines = gizmos.space(RenderSpace::World);

    for (axis, color) in [(Vec3::<f32>::X, AXIS_X), (Vec3::Y, AXIS_Y), (Vec3::Z, AXIS_Z)] {
        lines.push(GizmoLine {
            start: axis * 0.9,
            end: axis * 2.6,
            color: srgb(color).xyzn(1.0),
        });
    }

    let (min, max) = (Vec3::new(-5.0, -2.3, -5.0), Vec3::new(5.0, 2.2, 5.0));
    let corner = |i: usize| {
        Vec3::new(
            [min.x, max.x][i & 1],
            [min.y, max.y][(i >> 1) & 1],
            [min.z, max.z][(i >> 2) & 1],
        )
    };
    let color = srgb(NIGHT_700).xyzn(1.0);

    for i in 0..8 {
        for bit in [1, 2, 4] {
            if i & bit == 0 {
                lines.push(GizmoLine {
                    start: corner(i),
                    end: corner(i | bit),
                    color,
                });
            }
        }
    }
}

fn srgb(hex: u32) -> Vec3<f32> {
    let channel = |shift: u32| {
        let c = ((hex >> shift) & 0xFF) as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    Vec3::new(channel(16), channel(8), channel(0))
}

// Undoes exposure and the ACES fit, so a flat surface lands on the given linear color on screen.
fn pre_tonemap(linear: Vec3<f32>) -> Vec3<f32> {
    let (a, b, c, d, e) = (2.51, 0.03, 2.43, 0.59, 0.14);
    let inverse_aces = |y: f32| {
        let (qa, qb, qc) = (a - y * c, b - y * d, -y * e);
        (-qb + (qb * qb - 4.0 * qa * qc).sqrt()) / (2.0 * qa)
    };
    let multiplier = 2.0_f32.powf(EXPOSURE);
    Vec3::new(inverse_aces(linear.x), inverse_aces(linear.y), inverse_aces(linear.z)) / multiplier
}

// Material and batched vertex colors are decoded with a 2.2 gamma before shading.
fn gamma(linear: Vec3<f32>, w: f32) -> Vec4<f32> {
    Vec4::new(
        linear.x.powf(1.0 / 2.2),
        linear.y.powf(1.0 / 2.2),
        linear.z.powf(1.0 / 2.2),
        w,
    )
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_signed(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
}
