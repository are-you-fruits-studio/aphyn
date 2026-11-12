use std::{
    error::Error,
    path::{Path, PathBuf},
};

pub const FILE_NAME_SCRIPTS_LIB_SRC: &str = if cfg!(windows) { "lib_app.dll" } else { "liblib_app.so" };
pub const FILE_NAME_SCRIPTS_LIB_DST: &str = if cfg!(windows) { "lib_app.dll" } else { "lib_app.so" };
pub const FILE_NAME_SCRIPTS_BIN: &str = "launcher";

pub fn path_scripts_lib_src(project_path: &str) -> PathBuf {
    [project_path, "scripts", "target", "release", FILE_NAME_SCRIPTS_LIB_SRC]
        .into_iter()
        .collect::<PathBuf>()
}

pub fn path_scripts_bin_src(project_path: &str) -> PathBuf {
    let bin_file_name = format!("{FILE_NAME_SCRIPTS_BIN}{}", std::env::consts::EXE_SUFFIX);

    [project_path, "scripts", "target", "release", bin_file_name.as_str()]
        .into_iter()
        .collect::<PathBuf>()
}

pub fn build_app(project_path: &str) -> Result<(), Box<dyn Error>> {
    let project_name = PathBuf::from(project_path);
    let project_name = project_name
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("project path has no folder name")?;

    let scripts_path = [project_path, "scripts"].iter().collect::<PathBuf>();

    if !std::fs::exists(scripts_path.join("src").join("main.rs"))? {
        return Err(format!(
            "{} is missing: add it with `fn main() {{ aphyn::launch_app_dynamically(); }}` \
             and a `[[bin]]` section with `name = \"{FILE_NAME_SCRIPTS_BIN}\"` and `path = \"src/main.rs\"` to scripts/Cargo.toml",
            scripts_path.join("src").join("main.rs").display()
        )
        .into());
    }

    build_cargo_project(&scripts_path)?;

    let bin_src = path_scripts_bin_src(project_path);

    if !std::fs::exists(&bin_src)? {
        return Err(format!(
            "{} was not built: scripts/Cargo.toml needs a `[[bin]]` section with `name = \"{FILE_NAME_SCRIPTS_BIN}\"` and `path = \"src/main.rs\"`",
            bin_src.display()
        )
        .into());
    }

    let build_exe = format!("{project_name}{}", std::env::consts::EXE_SUFFIX);

    copy_file(
        path_scripts_lib_src(project_path),
        [project_path, "builds", FILE_NAME_SCRIPTS_LIB_DST]
            .iter()
            .collect::<PathBuf>(),
    )?;

    copy_file(
        bin_src,
        [project_path, "builds", build_exe.as_str()].iter().collect::<PathBuf>(),
    )?;

    let assets_src = [project_path, "assets"].iter().collect::<PathBuf>();
    let assets_dst = [project_path, "builds", "assets"].iter().collect::<PathBuf>();

    if std::fs::exists(&assets_src).unwrap_or(true) {
        copy_dir_all(assets_src, assets_dst)?;
    }

    Ok(())
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn build_cargo_project(project_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let mut path = PathBuf::from(project_path.as_ref());

    path.push("Cargo.toml");

    let exit_status = std::process::Command::new("cargo")
        .args(["build", "--release", "--manifest-path", path.to_str().unwrap()])
        .stderr(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .status()?;

    if !exit_status.success() {
        return Err("cargo build was unsuccessful".into());
    }

    Ok(())
}

fn copy_file(src_path: impl AsRef<Path>, dst_path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let dst_path = dst_path.as_ref();

    if let Some(dst_parent_path) = dst_path.parent() {
        _ = std::fs::create_dir_all(dst_parent_path)
    };

    std::fs::copy(src_path, dst_path)?;

    Ok(())
}
