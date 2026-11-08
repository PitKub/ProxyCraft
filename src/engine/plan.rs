//! Which files to process and where their proxies go.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{Destination, Preset, PROXIES_FOLDER, SUPPORTED_EXTENSIONS};

pub fn has_supported_extension(path: &Path) -> bool {
    path.extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_ascii_lowercase()))
        .is_some_and(|e| SUPPORTED_EXTENSIONS.contains(&e.as_str()))
}

pub fn is_proxy_file(path: &Path, suffix: &str) -> bool {
    let stem_proxy = path
        .file_stem()
        .is_some_and(|s| s.to_string_lossy().ends_with(suffix));
    let in_proxies = path
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(PROXIES_FOLDER));
    stem_proxy || in_proxies
}

/// Video files in a folder, skipping existing proxies and Proxies folders.
pub fn collect_media(folder: &Path, recursive: bool, suffix: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![folder.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                if recursive {
                    stack.push(p);
                }
            } else if p.is_file() && has_supported_extension(&p) && !is_proxy_file(&p, suffix) {
                found.push(p);
            }
        }
    }
    found.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    found
}

/// Proxy path for each source; names are made unique within the batch.
pub fn plan_outputs(
    sources: &[PathBuf],
    preset: &Preset,
    mode: Destination,
    custom_dir: &str,
    suffix: &str,
) -> Vec<PathBuf> {
    let mut used: HashSet<String> = HashSet::new();
    let mut outputs = Vec::with_capacity(sources.len());
    for src in sources {
        let parent = src.parent().map(Path::to_path_buf).unwrap_or_default();
        let folder = match mode {
            Destination::Custom if !custom_dir.is_empty() => PathBuf::from(custom_dir),
            Destination::SameFolder => parent,
            _ => parent.join(PROXIES_FOLDER),
        };
        let stem = src
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut out = folder.join(format!("{stem}{suffix}{}", preset.extension()));
        let mut n = 2;
        // Two sources with the same name sent to one custom folder.
        while used.contains(&out.to_string_lossy().to_lowercase()) {
            out = folder.join(format!("{stem}{suffix} ({n}){}", preset.extension()));
            n += 1;
        }
        used.insert(out.to_string_lossy().to_lowercase());
        outputs.push(out);
    }
    outputs
}

/// Two files at once was fastest in testing for both GPU and CPU pipelines.
pub fn auto_parallel_jobs() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    if cpus >= 4 {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{preset_by_key, DEFAULT_SUFFIX};

    #[test]
    fn proxies_are_recognised_and_skipped() {
        assert!(is_proxy_file(
            Path::new("/m/Clip_Proxy.mp4"),
            DEFAULT_SUFFIX
        ));
        assert!(is_proxy_file(
            Path::new("/m/proxies/Clip.mov"),
            DEFAULT_SUFFIX
        ));
        assert!(!is_proxy_file(Path::new("/m/Clip.mov"), DEFAULT_SUFFIX));
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::create_dir_all(d.join("Proxies")).unwrap();
        for f in [
            "B.MOV",
            "a.mp4",
            "a_Proxy.mp4",
            "notes.txt",
            "sub/c.mkv",
            "Proxies/a_Proxy.mov",
        ] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        let names = |v: Vec<PathBuf>| {
            v.iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(collect_media(d, false, DEFAULT_SUFFIX)),
            ["a.mp4", "B.MOV"]
        );
        assert_eq!(
            names(collect_media(d, true, DEFAULT_SUFFIX)),
            ["a.mp4", "B.MOV", "c.mkv"]
        );
    }

    #[test]
    fn outputs_per_destination() {
        let p = preset_by_key("prores_proxy_half").unwrap();
        let srcs = vec![
            PathBuf::from("/a/x.mov"),
            PathBuf::from("/b/x.MP4"),
            PathBuf::from("/c/y.mov"),
        ];
        let o = plan_outputs(&srcs, p, Destination::ProxiesFolder, "", "_Proxy");
        assert_eq!(o[0], PathBuf::from("/a/Proxies/x_Proxy.mov"));
        let o = plan_outputs(&srcs, p, Destination::SameFolder, "", "_P");
        assert_eq!(o[1], PathBuf::from("/b/x_P.mov"));
        let o = plan_outputs(&srcs, p, Destination::Custom, "/out", "_Proxy");
        assert_eq!(
            o,
            vec![
                PathBuf::from("/out/x_Proxy.mov"),
                PathBuf::from("/out/x_Proxy (2).mov"),
                PathBuf::from("/out/y_Proxy.mov")
            ]
        );
        // custom without a folder falls back to the Proxies folder, as in the Python version
        assert_eq!(
            plan_outputs(&srcs[..1], p, Destination::Custom, "", "_Proxy")[0],
            PathBuf::from("/a/Proxies/x_Proxy.mov")
        );
    }
}
