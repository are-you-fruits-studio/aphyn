use std::{path::Path, process::Command};

pub fn copy_to_clipboard(text: &str) {
    if let Err(err) = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text)) {
        eprintln!("failed to copy to clipboard: {err}");
    }
}

pub fn read_clipboard() -> Option<String> {
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => Some(text),
        Err(err) => {
            eprintln!("failed to read clipboard: {err}");
            None
        }
    }
}

pub fn reveal_in_file_explorer(path: &Path) {
    #[cfg(target_os = "windows")]
    let result = {
        use std::os::windows::process::CommandExt;
        // explorer falls back to the desktop on relative paths or forward slashes; absolute() fixes both
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        // explorer parses "/select,<path>" itself and breaks on the quoting Command::arg would add
        Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).spawn()
    };

    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg("-R").arg(path).spawn();

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn();

    if let Err(err) = result {
        eprintln!("failed to reveal {} in file explorer: {err}", path.display());
    }
}
