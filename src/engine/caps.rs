//! Which encoders and decoders actually work on this PC.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use super::process::{run, strings};
use super::Family;

const HW_LABELS: [(&str, &str); 3] = [
    ("nvenc", "NVIDIA NVENC"),
    ("qsv", "Intel Quick Sync"),
    ("amf", "AMD AMF"),
];

pub fn encoder_label(codec: &str) -> &'static str {
    for (suffix, label) in HW_LABELS {
        if codec.ends_with(&format!("_{suffix}")) {
            return label;
        }
    }
    "CPU"
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capabilities {
    pub ok: bool,
    pub version: String,
    pub details: String,
    pub encoders: BTreeSet<String>,
    pub hwaccels: BTreeSet<String>,
    /// Hardware H.264 encoders that really opened on this PC, best first.
    pub working_h264: Vec<String>,
    /// Hardware H.265 encoders that really opened on this PC, best first.
    pub working_h265: Vec<String>,
    /// NVIDIA decode + resize + encode without copying frames to the CPU.
    pub cuda_pipeline: bool,
    /// D3D11 GPU decoding (Intel, AMD, NVIDIA), used when the CUDA pipeline is not.
    pub d3d11_decode: bool,
}

impl Capabilities {
    pub fn not_available(details: &str) -> Capabilities {
        Capabilities {
            details: details.to_string(),
            ..Default::default()
        }
    }

    pub fn working(&self, family: Family) -> &[String] {
        match family {
            Family::H264 => &self.working_h264,
            Family::H265 => &self.working_h265,
            Family::ProRes => &[],
        }
    }

    /// Best encoder for CPU-decoded frames.
    pub fn encoder(&self, family: Family) -> String {
        if let Some(first) = self.working(family).first() {
            return first.clone();
        }
        match family {
            Family::H264 => "libx264".into(),
            Family::H265 => "libx265".into(),
            Family::ProRes => "prores_ks".into(),
        }
    }

    pub fn summary(&self) -> String {
        if !self.ok {
            return if self.details.is_empty() {
                "FFmpeg not available.".into()
            } else {
                self.details.clone()
            };
        }
        let mut parts = vec![if self.cuda_pipeline {
            "Decode and resize: NVIDIA GPU, CPU fallback".to_string()
        } else if self.d3d11_decode {
            "Decode: GPU (D3D11), CPU fallback. Resize: CPU".to_string()
        } else {
            "Decode and resize: CPU".to_string()
        }];
        for (family, name) in [(Family::H264, "H.264"), (Family::H265, "H.265")] {
            let codec = self.encoder(family);
            parts.push(format!("{name}: {} ({codec})", encoder_label(&codec)));
        }
        parts.push("ProRes: CPU (prores_ks)".into());
        parts.join("\n")
    }
}

/// Parse `ffmpeg -encoders`: video encoder names.
pub fn parse_encoders(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let t = line.trim_start();
        let mut it = t.split_whitespace();
        if let (Some(flags), Some(name)) = (it.next(), it.next()) {
            if flags.starts_with('V') && flags.len() >= 2 && name != "=" {
                out.insert(name.to_string());
            }
        }
    }
    out
}

/// Parse `ffmpeg -hwaccels`: every line after the heading.
pub fn parse_hwaccels(text: &str) -> BTreeSet<String> {
    text.lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

fn opens(ffmpeg: &str, args: &[&str]) -> bool {
    let mut full = strings(&[ffmpeg, "-hide_banner", "-loglevel", "error"]);
    full.extend(strings(args));
    full.extend(strings(&["-f", "null", "-"]));
    run(&full, Duration::from_secs(30))
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// True when D3D11 really decodes H.264 on the GPU.
///
/// A short H.264 sample is made first. Its frames are kept on the GPU and then
/// downloaded, which fails if FFmpeg quietly fell back to software decoding.
fn d3d11_decodes(ffmpeg: &str, caps: &Capabilities) -> bool {
    let mut encoders: Vec<String> = Vec::new();
    if caps.encoders.contains("libx264") {
        encoders.push("libx264".into());
    }
    encoders.extend(caps.working_h264.iter().cloned());
    let Some(first) = encoders.first() else {
        return false;
    };
    let Ok(dir) = tempfile_dir() else {
        return false;
    };
    let sample = dir.join("sample.mp4");
    let pix = if first == "libx264" {
        "yuv420p"
    } else {
        "nv12"
    };
    let sample_s = sample.to_string_lossy().into_owned();
    let made = run(
        &strings(&[
            ffmpeg,
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=25",
            "-frames:v",
            "10",
            "-c:v",
            first,
            "-pix_fmt",
            pix,
            &sample_s,
        ]),
        Duration::from_secs(30),
    );
    let ok = matches!(made, Ok(o) if o.status.success())
        && sample.is_file()
        && opens(
            ffmpeg,
            &[
                "-hwaccel",
                "d3d11va",
                "-hwaccel_output_format",
                "d3d11",
                "-i",
                &sample_s,
                "-vf",
                "hwdownload,format=nv12",
            ],
        );
    let _ = std::fs::remove_dir_all(&dir);
    ok
}

fn tempfile_dir() -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "proxycraft-{}-{}",
        std::process::id(),
        chrono::Local::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

static CACHE: Mutex<Option<HashMap<String, Capabilities>>> = Mutex::new(None);

const HW_CANDIDATES: [(Family, [&str; 3]); 2] = [
    (Family::H264, ["h264_nvenc", "h264_qsv", "h264_amf"]),
    (Family::H265, ["hevc_nvenc", "hevc_qsv", "hevc_amf"]),
];

/// Find which encoders and decoders actually work on this PC.
///
/// FFmpeg builds list every vendor's hardware encoders whatever GPU is fitted,
/// so each one is test-opened. A successful result is cached.
pub fn detect_capabilities(ffmpeg: &str, refresh: bool) -> Capabilities {
    if !refresh {
        if let Some(c) = CACHE
            .lock()
            .ok()
            .and_then(|g| g.as_ref().and_then(|m| m.get(ffmpeg).cloned()))
        {
            return c;
        }
    }
    if ffmpeg.is_empty() || !Path::new(ffmpeg).is_file() {
        return Capabilities::not_available("FFmpeg executable was not found.");
    }
    let mut caps = Capabilities::default();
    let ver = match run(&strings(&[ffmpeg, "-version"]), Duration::from_secs(30)) {
        Ok(v) => v,
        Err(e) => return Capabilities::not_available(&format!("FFmpeg detection failed: {e}")),
    };
    if !ver.status.success() {
        let err = String::from_utf8_lossy(&ver.stderr).trim().to_string();
        return Capabilities::not_available(if err.is_empty() {
            "FFmpeg could not be started."
        } else {
            &err
        });
    }
    caps.version = String::from_utf8_lossy(&ver.stdout)
        .lines()
        .next()
        .unwrap_or("FFmpeg")
        .to_string();
    if let Ok(o) = run(
        &strings(&[ffmpeg, "-hide_banner", "-encoders"]),
        Duration::from_secs(30),
    ) {
        caps.encoders = parse_encoders(&String::from_utf8_lossy(&o.stdout));
    }
    if let Ok(o) = run(
        &strings(&[ffmpeg, "-hide_banner", "-hwaccels"]),
        Duration::from_secs(30),
    ) {
        caps.hwaccels = parse_hwaccels(&String::from_utf8_lossy(&o.stdout));
    }
    let test_input = [
        "-f",
        "lavfi",
        "-i",
        "color=black:size=1280x720:rate=25",
        "-frames:v",
        "3",
    ];
    for (family, candidates) in HW_CANDIDATES {
        let working: Vec<String> = candidates
            .iter()
            .filter(|c| caps.encoders.contains(**c))
            .filter(|c| {
                let mut a = test_input.to_vec();
                a.extend(["-c:v", c, "-pix_fmt", "yuv420p"]);
                opens(ffmpeg, &a)
            })
            .map(|c| c.to_string())
            .collect();
        match family {
            Family::H264 => caps.working_h264 = working,
            _ => caps.working_h265 = working,
        }
    }
    if caps.hwaccels.contains("cuda") && caps.working_h264.iter().any(|c| c == "h264_nvenc") {
        let mut a = test_input.to_vec();
        a.extend([
            "-vf",
            "hwupload_cuda,scale_cuda=640:360:format=nv12,hwdownload,format=nv12",
        ]);
        caps.cuda_pipeline = opens(ffmpeg, &a);
    }
    if !caps.cuda_pipeline && caps.hwaccels.contains("d3d11va") {
        caps.d3d11_decode = d3d11_decodes(ffmpeg, &caps);
    }
    caps.ok = true;
    caps.details = caps.version.clone();
    if let Ok(mut g) = CACHE.lock() {
        g.get_or_insert_with(HashMap::new)
            .insert(ffmpeg.to_string(), caps.clone());
    }
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_and_labels() {
        let enc = parse_encoders(
            " V....D libx264  H.264\n A....D aac  AAC\n V..... h264_nvenc NVENC\n ------\n",
        );
        assert!(enc.contains("libx264") && enc.contains("h264_nvenc") && !enc.contains("aac"));
        assert_eq!(
            parse_hwaccels("Hardware acceleration methods:\ncuda\nd3d11va\n\n").len(),
            2
        );
        assert_eq!(encoder_label("hevc_qsv"), "Intel Quick Sync");
        assert_eq!(encoder_label("libx265"), "CPU");
        let caps = Capabilities {
            ok: true,
            working_h265: vec!["hevc_amf".into()],
            ..Default::default()
        };
        assert_eq!(caps.encoder(Family::H264), "libx264");
        assert_eq!(caps.encoder(Family::H265), "hevc_amf");
        assert!(caps.summary().contains("H.265: AMD AMF (hevc_amf)"));
        assert_eq!(
            Capabilities::not_available("").summary(),
            "FFmpeg not available."
        );
    }
}
