use std::path::Path;

use aphyn_render::{
    ComputeShader, ComputeShaderAssetMetadata, RenderApiResource, Shader, ShaderAssetMetadata, WgslComposer, wgsl_include_paths,
};

pub struct ShaderLoader<'a> {
    pub render_api: &'a RenderApiResource,
}

impl<'a> ShaderLoader<'a> {
    pub fn load_from_deserialized(&self, meta: ShaderAssetMetadata, assets_dir_path: impl AsRef<Path>) -> Option<Shader> {
        let wgsl_key = meta.wgsl.as_str();
        let code = compose_shader_file(wgsl_key, assets_dir_path.as_ref())?;

        self.render_api
            .create_shader(wgsl_key, &code, meta.desc.clone(), Some(meta.clone()))
    }
}

pub struct ComputeShaderLoader<'a> {
    pub render_api: &'a RenderApiResource,
}

impl<'a> ComputeShaderLoader<'a> {
    pub fn load_from_deserialized(
        &self,
        meta: ComputeShaderAssetMetadata,
        assets_dir_path: impl AsRef<Path>,
    ) -> Option<ComputeShader> {
        let wgsl_key = meta.wgsl.as_str();
        let code = compose_shader_file(wgsl_key, assets_dir_path.as_ref())?;

        self.render_api.create_compute_shader(wgsl_key, &code, Some(meta.clone()))
    }
}

fn compose_shader_file(wgsl_key: &str, assets_dir_path: &Path) -> Option<String> {
    let source = match std::fs::read_to_string(assets_dir_path.join(wgsl_key)) {
        Ok(source) => source,
        Err(err) => {
            println!("failed to load shader {wgsl_key}, file read error: {err}");
            return None;
        }
    };

    let mut composer = WgslComposer::with_engine_libraries();
    add_user_libraries(&mut composer, &source, assets_dir_path);

    match composer.compose(wgsl_key, &source, &[]) {
        Ok(code) => Some(code),
        Err(err) => {
            println!("failed to load shader {wgsl_key}, compose error: {err}");
            None
        }
    }
}

fn add_user_libraries(composer: &mut WgslComposer, source: &str, assets_dir_path: &Path) {
    for path in wgsl_include_paths(source) {
        if composer.has_library(path) {
            continue;
        }

        // a missing file is left for the composer to report as an unknown include
        let Ok(library) = std::fs::read_to_string(assets_dir_path.join(path)) else {
            continue;
        };

        composer.add_library(path.to_string(), library.clone());
        add_user_libraries(composer, &library, assets_dir_path);
    }
}
