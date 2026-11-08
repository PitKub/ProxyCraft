//! Finding FFmpeg and FFprobe.

use std::path::{Path, PathBuf};

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Folders that may hold the included FFmpeg: beside ProxyCraft.exe, or the project folder
/// when running from source with `cargo run`.
pub fn app_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            folders.push(dir.to_path_buf());
        }
    }
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !folders.contains(&project) {
        folders.push(project);
    }
    folders
}

/// FFmpeg shipped with ProxyCraft: an "ffmpeg" folder next to ProxyCraft.exe. When running
/// from source, vendor/ffmpeg (filled by get_ffmpeg.ps1) is used.
pub fn bundled_tool(name: &str) -> Option<PathBuf> {
    let exe = exe_name(name);
    for folder in app_folders() {
        for sub in [PathBuf::from("ffmpeg"), Path::new("vendor").join("ffmpeg")] {
            let p = folder.join(sub).join(&exe);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn which(name: &str) -> Option<PathBuf> {
    let exe = exe_name(name);
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(&exe))
            .find(|p| p.is_file())
    })
}

/// Included FFmpeg first, then PATH, then the usual Windows FFmpeg folders.
pub fn find_tool(name: &str) -> Option<PathBuf> {
    if let Some(p) = bundled_tool(name).or_else(|| which(name)) {
        return Some(p);
    }
    let exe = exe_name(name);
    let program_files =
        std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    [
        PathBuf::from(r"C:\ffmpeg\bin"),
        Path::new(&program_files).join("ffmpeg").join("bin"),
        Path::new(&local)
            .join("Programs")
            .join("ffmpeg")
            .join("bin"),
    ]
    .into_iter()
    .map(|d| d.join(&exe))
    .find(|p| p.is_file())
}

pub fn same_path(a: &str, b: &Path) -> bool {
    let a = Path::new(a);
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}
