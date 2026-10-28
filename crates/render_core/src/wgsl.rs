use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fmt,
};

use wgpu::{Device, ShaderModule, ShaderModuleDescriptor, ShaderSource};

const ENGINE_LIBRARIES: &[(&str, &str)] = &[
    ("aphyn/fullscreen.wgsl", include_str!("./assets/wgsl/fullscreen.wgsl")),
    (
        "aphyn/standard_types.wgsl",
        include_str!("./assets/wgsl/standard_types.wgsl"),
    ),
    (
        "aphyn/standard_bindings.wgsl",
        include_str!("./assets/wgsl/standard_bindings.wgsl"),
    ),
    ("aphyn/pbr.wgsl", include_str!("./assets/wgsl/pbr.wgsl")),
];

#[derive(Default, Clone)]
pub struct WgslComposer {
    libraries: HashMap<String, Cow<'static, str>>,
}

impl WgslComposer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_engine_libraries() -> Self {
        let mut composer = Self::new();
        for (path, source) in ENGINE_LIBRARIES {
            composer.add_library(*path, *source);
        }
        composer
    }

    pub fn add_library(&mut self, path: impl Into<String>, source: impl Into<Cow<'static, str>>) {
        self.libraries.insert(path.into(), source.into());
    }

    pub fn has_library(&self, path: &str) -> bool {
        self.libraries.contains_key(path)
    }

    pub fn compose(&self, name: &str, source: &str, defs: &[&str]) -> Result<String, WgslComposeError> {
        let mut output = String::with_capacity(source.len());
        let mut included = HashSet::new();
        self.compose_into(&mut output, &mut included, name, source, defs)?;
        Ok(output)
    }

    pub fn create_shader_module(
        &self,
        device: &Device,
        name: &str,
        source: &str,
        defs: &[&str],
    ) -> Result<ShaderModule, WgslComposeError> {
        let code = self.compose(name, source, defs)?;

        Ok(device.create_shader_module(ShaderModuleDescriptor {
            label: Some(name),
            source: ShaderSource::Wgsl(code.into()),
        }))
    }

    fn compose_into<'a>(
        &'a self,
        output: &mut String,
        included: &mut HashSet<&'a str>,
        file: &str,
        source: &str,
        defs: &[&str],
    ) -> Result<(), WgslComposeError> {
        let error = |line: usize, kind: WgslComposeErrorKind| WgslComposeError {
            file: file.to_string(),
            line,
            kind,
        };

        let mut branches = Vec::<Branch>::new();

        for (index, line) in source.lines().enumerate() {
            let line_number = index + 1;
            let is_active = branches.last().is_none_or(Branch::is_active);

            let Some(directive) = line.trim_start().strip_prefix('#') else {
                if is_active {
                    output.push_str(line);
                }
                output.push('\n');
                continue;
            };

            let directive = directive.trim();
            let (keyword, argument) = directive
                .split_once(char::is_whitespace)
                .map_or((directive, ""), |(keyword, argument)| (keyword, argument.trim()));
            let malformed = || error(line_number, WgslComposeErrorKind::MalformedDirective(line.trim().to_string()));

            match keyword {
                "ifdef" | "ifndef" => {
                    if argument.is_empty() || argument.contains(char::is_whitespace) {
                        return Err(malformed());
                    }

                    branches.push(Branch {
                        line: line_number,
                        parent_active: is_active,
                        condition: defs.contains(&argument) == (keyword == "ifdef"),
                        has_else: false,
                    });
                }
                "else" => {
                    if !argument.is_empty() {
                        return Err(malformed());
                    }

                    match branches.last_mut() {
                        Some(branch) if !branch.has_else => branch.has_else = true,
                        _ => return Err(error(line_number, WgslComposeErrorKind::UnexpectedElse)),
                    }
                }
                "endif" => {
                    if !argument.is_empty() {
                        return Err(malformed());
                    }

                    if branches.pop().is_none() {
                        return Err(error(line_number, WgslComposeErrorKind::UnexpectedEndif));
                    }
                }
                "include" => {
                    let path = parse_include_argument(argument).ok_or_else(malformed)?;

                    if is_active {
                        let (path, library) = self
                            .libraries
                            .get_key_value(path)
                            .ok_or_else(|| error(line_number, WgslComposeErrorKind::UnknownInclude(path.to_string())))?;

                        if included.insert(path) {
                            self.compose_into(output, included, path, library, defs)?;
                        }
                    }
                }
                _ => return Err(malformed()),
            }

            output.push('\n');
        }

        match branches.last() {
            Some(branch) => Err(error(branch.line, WgslComposeErrorKind::UnterminatedIf)),
            None => Ok(()),
        }
    }
}

pub fn wgsl_include_paths(source: &str) -> impl Iterator<Item = &str> {
    source.lines().filter_map(|line| {
        let directive = line.trim_start().strip_prefix('#')?.trim();
        let argument = directive.strip_prefix("include")?;

        if !argument.starts_with(char::is_whitespace) {
            return None;
        }

        parse_include_argument(argument.trim())
    })
}

fn parse_include_argument(argument: &str) -> Option<&str> {
    argument
        .strip_prefix('"')
        .and_then(|path| path.strip_suffix('"'))
        .filter(|path| !path.is_empty())
}

struct Branch {
    line: usize,
    parent_active: bool,
    condition: bool,
    has_else: bool,
}

impl Branch {
    fn is_active(&self) -> bool {
        self.parent_active && self.condition != self.has_else
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgslComposeError {
    pub file: String,
    pub line: usize,
    pub kind: WgslComposeErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WgslComposeErrorKind {
    UnknownInclude(String),
    UnexpectedElse,
    UnexpectedEndif,
    UnterminatedIf,
    MalformedDirective(String),
}

impl fmt::Display for WgslComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: ", self.file, self.line)?;

        match &self.kind {
            WgslComposeErrorKind::UnknownInclude(path) => write!(f, "unknown include \"{path}\""),
            WgslComposeErrorKind::UnexpectedElse => write!(f, "#else without a matching #ifdef/#ifndef"),
            WgslComposeErrorKind::UnexpectedEndif => write!(f, "#endif without a matching #ifdef/#ifndef"),
            WgslComposeErrorKind::UnterminatedIf => write!(f, "#ifdef/#ifndef without a matching #endif"),
            WgslComposeErrorKind::MalformedDirective(directive) => write!(f, "malformed directive `{directive}`"),
        }
    }
}

impl std::error::Error for WgslComposeError {}

#[cfg(test)]
mod tests {
    // todo: decide how we handle tests (should we keep them? should we remove them? ...)

    use super::WgslComposer;

    fn validate(composer: &WgslComposer, name: &str, source: &str) {
        let code = composer.compose(name, source, &[]).unwrap();
        let module = naga::front::wgsl::parse_str(&code).unwrap_or_else(|error| panic!("{}", error.emit_to_string(&code)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&code)));
    }

    #[test]
    fn engine_shaders_are_valid() {
        let composer = WgslComposer::with_engine_libraries();

        for (path, _) in super::ENGINE_LIBRARIES {
            validate(&composer, "library", &format!("#include \"{path}\""));
        }

        validate(
            &composer,
            "mip_fill_shader.wgsl",
            include_str!("./assets/mip_fill_shader.wgsl"),
        );

        let standard_defs: &[&[&str]] = &[&[], &["LIT"], &["TRANSPARENT"], &["LIT", "TRANSPARENT"], &["UI"], &["UI", "TRANSPARENT"]];

        for defs in standard_defs {
            let code = composer.compose("shader_standard.wgsl", include_str!("./assets/wgsl/shader_standard.wgsl"), defs).unwrap();
            let module = naga::front::wgsl::parse_str(&code).unwrap_or_else(|error| panic!("{defs:?}\n{}", error.emit_to_string(&code)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|error| panic!("{defs:?}\n{}", error.emit_to_string(&code)));
        }
    }

    #[test]
    fn custom_shader_with_user_library_is_valid() {
        let library = "fn tint(color: vec4<f32>) -> vec4<f32> {\n    return color * material_data.color;\n}\n";
        let source = r#"#include "aphyn/standard_bindings.wgsl"
#include "shaders/tint.lib.wgsl"

struct VertexOutput {
    @builtin(position) position_clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(vertex: VertexAttributes, instance_raw: InstanceRawAttributes) -> VertexOutput {
    let instance = map_instance_data(instance_raw);
    var out: VertexOutput;
    out.position_clip = global_data.matrix_world_to_clip * instance.local_to_world * vec4<f32>(vertex.position, 1.0);
    out.color = vertex.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return tint(in.color);
}
"#;

        assert_eq!(
            super::wgsl_include_paths(source).collect::<Vec<_>>(),
            ["aphyn/standard_bindings.wgsl", "shaders/tint.lib.wgsl"],
        );

        let mut composer = WgslComposer::with_engine_libraries();
        composer.add_library("shaders/tint.lib.wgsl", library);

        validate(&composer, "shaders/custom.wgsl", source);
    }
}
