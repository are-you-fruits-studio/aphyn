use aphyn_render_core::WgslComposer;
use wgpu::{Device, ShaderModule};

pub fn render_wgsl_composer() -> WgslComposer {
    let mut composer = WgslComposer::with_engine_libraries();
    composer.add_library("aphyn/bloom.wgsl", include_str!("./bloom.wgsl"));
    composer
}

pub(crate) fn create_engine_shader_module(device: &Device, name: &str, source: &str, defs: &[&str]) -> ShaderModule {
    render_wgsl_composer()
        .create_shader_module(device, name, source, defs)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::render_wgsl_composer;

    #[test]
    fn render_shaders_are_valid() {
        let composer = render_wgsl_composer();

        let shaders: &[(&str, &str, &[&str])] = &[
            ("shader_standard.wgsl", include_str!("./shader_standard.wgsl"), &[]),
            ("shader_standard.wgsl", include_str!("./shader_standard.wgsl"), &["LIT"]),
            (
                "shader_standard.wgsl",
                include_str!("./shader_standard.wgsl"),
                &["TRANSPARENT"],
            ),
            (
                "shader_standard.wgsl",
                include_str!("./shader_standard.wgsl"),
                &["LIT", "TRANSPARENT"],
            ),
            ("shader_standard.wgsl", include_str!("./shader_standard.wgsl"), &["UI"]),
            (
                "shader_standard.wgsl",
                include_str!("./shader_standard.wgsl"),
                &["UI", "TRANSPARENT"],
            ),
            (
                "shader_transparent_final.wgsl",
                include_str!("./shader_transparent_final.wgsl"),
                &[],
            ),
            ("shader_exposure.wgsl", include_str!("./shader_exposure.wgsl"), &[]),
            (
                "shader_bloom_gather_threshold.wgsl",
                include_str!("./shader_bloom_gather_threshold.wgsl"),
                &[],
            ),
            ("shader_bloom_blur.wgsl", include_str!("./shader_bloom_blur.wgsl"), &[]),
            (
                "shader_bloom_downscale.wgsl",
                include_str!("./shader_bloom_downscale.wgsl"),
                &[],
            ),
            ("shader_bloom_apply.wgsl", include_str!("./shader_bloom_apply.wgsl"), &[]),
            (
                "shader_color_grading_aces.wgsl",
                include_str!("./shader_color_grading_aces.wgsl"),
                &[],
            ),
            ("shader_gizmo.wgsl", include_str!("./shader_gizmo.wgsl"), &[]),
        ];

        for (name, source, defs) in shaders {
            let code = composer.compose(name, source, defs).unwrap();
            let module = naga::front::wgsl::parse_str(&code)
                .unwrap_or_else(|error| panic!("{name} {defs:?}\n{}", error.emit_to_string(&code)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|error| panic!("{name} {defs:?}\n{}", error.emit_to_string(&code)));
        }
    }
}
