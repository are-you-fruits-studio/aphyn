use aphyn_ecs::Resource;
use aphyn_ffi::*;
use aphyn_math::{Vec3, Vec4};
use aphyn_render::{RenderSpace, StandardGlobalBindings, StandardTexture};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GizmoLine {
    pub start: Vec3<f32>,
    pub end: Vec3<f32>,
    // todo: transparency.
    pub color: Vec4<f32>,
}

#[repr(C)]
#[derive(Resource)]
pub struct GizmosResource {
    lines: FfiIndexMap<RenderSpace, FfiVec<GizmoLine>>,
}

impl GizmosResource {
    pub fn space(&mut self, space: RenderSpace) -> &mut FfiVec<GizmoLine> {
        self.lines.get_mut(&space).unwrap()
    }

    pub fn spaces(&mut self) -> FfiIndexMapPairsMutIter<'_, RenderSpace, FfiVec<GizmoLine>> {
        self.lines.iter_mut()
    }
}

impl Default for GizmosResource {
    fn default() -> Self {
        let mut lines = FfiIndexMap::new();

        lines.insert(RenderSpace::Clip, FfiVec::new());
        lines.insert(RenderSpace::Window, FfiVec::new());
        lines.insert(RenderSpace::World, FfiVec::new());

        Self { lines }
    }
}

#[repr(C)]
#[derive(Resource, Default)]
pub struct GizmosFrameResource {
    pub lines: FfiVec<(RenderSpace, FfiVec<GizmoLine>)>,
}

#[repr(C)]
#[derive(Resource)]
pub struct GlobalBindingsResource {
    pub bindings: StandardGlobalBindings,
}

#[repr(C)]
#[derive(Resource)]
pub struct DepthTextureResource {
    pub texture: StandardTexture,
}

#[repr(C)]
#[derive(Resource)]
pub struct TransparentTargetResource {
    pub texture: StandardTexture,
}

#[repr(C)]
#[derive(Resource)]
pub struct ExposureResource {
    pub exposure: f32,
    pub is_enabled: bool,
}

impl Default for ExposureResource {
    fn default() -> Self {
        Self {
            exposure: 1.5,
            is_enabled: false,
        }
    }
}

#[repr(C)]
#[derive(Resource)]
pub struct AmbientLightResource {
    pub color: Vec3<f32>,
}

impl Default for AmbientLightResource {
    fn default() -> Self {
        Self { color: Vec3::splat(0.0) }
    }
}

#[repr(C)]
#[derive(Resource)]
pub struct BloomResource {
    pub is_enabled: bool,
    pub threshold: f32,
    pub threshold_softening: f32,
    pub intensity: f32,
}

impl Default for BloomResource {
    fn default() -> Self {
        Self {
            is_enabled: false,
            threshold: 3.0,
            threshold_softening: 2.0,
            intensity: 0.7,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum ColorGradingType {
    Aces,
}

#[repr(C)]
#[derive(Resource, Default)]
pub struct ColorGradingResource {
    pub ty: Option<ColorGradingType>,
}

#[repr(C)]
#[derive(Resource)]
pub struct ScreenSpaceResource {
    pub near: f32,
    pub far: f32,
}

impl Default for ScreenSpaceResource {
    fn default() -> Self {
        Self {
            near: -10_000.0,
            far: 10_000.0,
        }
    }
}

#[repr(C)]
#[derive(Resource, Clone, Copy, Debug)]
pub struct ShadowSettingsResource {
    pub atlas_size: u32,
    pub tile_size: u32,
    pub cascade_count: u32,
    pub distance: f32,
    // 0 = uniform cascade splits, 1 = logarithmic
    pub split_lambda: f32,
    // how far behind a cascade's bounds directional casters are still captured
    pub caster_distance: f32,
    pub bias: f32,
    pub normal_offset: f32,
    pub spot_near: f32,
}

impl Default for ShadowSettingsResource {
    fn default() -> Self {
        Self {
            atlas_size: 4096,
            tile_size: 1024,
            cascade_count: 4,
            distance: 50.0,
            split_lambda: 0.75,
            caster_distance: 100.0,
            bias: 0.001,
            normal_offset: 0.1,
            spot_near: 0.05,
        }
    }
}
