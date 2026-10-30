use std::path::Path;

use aphyn_render::{RenderApiResource, StandardShader, WgslComposer, wgsl_include_paths};

pub fn is_shader_asset_key(key: &str) -> bool {
    key.ends_with(".wgsl") && !key.ends_with(".lib.wgsl")
}

pub struct ShaderLoader<'a> {
    pub render_api: &'a RenderApiResource,
}

impl<'a> ShaderLoader<'a> {
    pub fn load(&mut self, key: &str, assets_dir_path: impl AsRef<Path>) -> Option<StandardShader> {
        let assets_dir_path = assets_dir_path.as_ref();

        let source = match std::fs::read_to_string(assets_dir_path.join(key)) {
            Ok(source) => source,
            Err(err) => {
                println!("failed to load shader {key}, file read error: {err}");
                return None;
            }
        };

        let mut composer = WgslComposer::with_engine_libraries();
        add_user_libraries(&mut composer, &source, assets_dir_path);

        let code = match composer.compose(key, &source, &[]) {
            Ok(code) => code,
            Err(err) => {
                println!("failed to load shader {key}, compose error: {err}");
                return None;
            }
        };

        self.render_api.create_shader(key, &code)
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
