//! Media information from FFprobe, and the proxy check.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::clean_error;
use super::process::{run, strings};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaInfo {
    pub path: PathBuf,
    /// Coded size, before any rotation flag.
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration: f64,
    pub frames: u64,
    pub codec: String,
    pub pix_fmt: String,
    pub bits: u32,
    pub rotation: i32,
    pub audio_codecs: Vec<String>,
    /// FFmpeg output option name (without "-") -> value, e.g. color_primaries -> bt709.
    pub color: BTreeMap<String, String>,
    pub timecode: String,
    /// Frame count stored in the file, not estimated.
    pub frames_exact: bool,
    /// Video stream duration, not the whole file's.
    pub duration_exact: bool,
}

impl MediaInfo {
    pub fn is_10bit(&self) -> bool {
        if self.bits >= 10 {
            return true;
        }
        let p = self.pix_fmt.to_ascii_lowercase();
        [
            "10le", "10be", "12le", "12be", "14le", "14be", "16le", "16be",
        ]
        .iter()
        .any(|s| p.ends_with(s))
    }

    /// Proxy dimensions, rounded to even numbers as the encoders require.
    pub fn proxy_size(&self, divisor: u32) -> (u32, u32) {
        let d = divisor.max(1) as f64;
        let even = |v: u32| -> u32 { (((v as f64 / d / 2.0).round() as u32) * 2).max(2) };
        (even(self.width), even(self.height))
    }
}

fn number(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn fraction(v: Option<&Value>) -> f64 {
    if let Some(Value::String(s)) = v {
        if let Some((a, b)) = s.split_once('/') {
            let (a, b) = (
                a.trim().parse::<f64>().unwrap_or(0.0),
                b.trim().parse::<f64>().unwrap_or(0.0),
            );
            return if b != 0.0 { a / b } else { 0.0 };
        }
    }
    number(v)
}

/// A metadata tag, whatever its capitalisation (Matroska uses upper case).
fn tag(tags: Option<&Value>, name: &str) -> String {
    if let Some(Value::Object(map)) = tags {
        for (k, v) in map {
            if k.to_ascii_lowercase() == name {
                return match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
            }
        }
    }
    String::new()
}

/// Seconds from "HH:MM:SS.fraction", as Matroska stores stream durations.
fn clock_seconds(value: &str) -> f64 {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 {
        return 0.0;
    }
    match (
        parts[0].parse::<f64>(),
        parts[1].parse::<f64>(),
        parts[2].parse::<f64>(),
    ) {
        (Ok(h), Ok(m), Ok(s)) => h * 3600.0 + m * 60.0 + s,
        _ => 0.0,
    }
}

/// Parse `ffprobe -show_format -show_streams -of json` output.
pub fn parse_probe(path: &Path, json: &str) -> Result<MediaInfo, String> {
    let data: Value = serde_json::from_str(if json.trim().is_empty() { "{}" } else { json })
        .map_err(|e| format!("FFprobe output could not be read: {e}"))?;
    let empty = Vec::new();
    let streams = data
        .get("streams")
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    // Skip cover art and thumbnails, which ffprobe reports as video streams.
    let video = streams
        .iter()
        .find(|s| {
            s.get("codec_type").and_then(Value::as_str) == Some("video")
                && number(s.get("disposition").and_then(|d| d.get("attached_pic"))) == 0.0
        })
        .ok_or_else(|| "No video stream found.".to_string())?;

    let mut fps = fraction(video.get("avg_frame_rate"));
    if fps == 0.0 {
        fps = fraction(video.get("r_frame_rate"));
    }
    let null = Value::Null;
    let fmt = data.get("format").unwrap_or(&null);
    let tags = video.get("tags");
    let mut duration = number(video.get("duration"));
    if duration == 0.0 {
        duration = clock_seconds(&tag(tags, "duration"));
    }
    let duration_exact = duration > 0.0;
    if duration == 0.0 {
        duration = number(fmt.get("duration"));
    }
    let mut frames = number(video.get("nb_frames")) as i64;
    let frames_exact = frames > 0;
    if frames <= 0 {
        frames = (duration * fps).round() as i64;
    }
    // MOV keeps timecode in its own track; other formats on the video stream or the file.
    let mut timecode = tag(tags, "timecode");
    if timecode.is_empty() {
        timecode = tag(fmt.get("tags"), "timecode");
    }
    if timecode.is_empty() {
        timecode = streams
            .iter()
            .map(|s| tag(s.get("tags"), "timecode"))
            .find(|t| !t.is_empty())
            .unwrap_or_default();
    }
    let mut rotation = number(tags.and_then(|t| t.get("rotate")));
    if let Some(Value::Array(sd)) = video.get("side_data_list") {
        for side in sd {
            if side.get("rotation").is_some() {
                rotation = number(side.get("rotation"));
            }
        }
    }
    let mut color = BTreeMap::new();
    for (key, src) in [
        ("color_primaries", "color_primaries"),
        ("color_trc", "color_transfer"),
        ("colorspace", "color_space"),
        ("color_range", "color_range"),
    ] {
        if let Some(v) = video.get(src).and_then(Value::as_str) {
            if !v.is_empty() && v != "unknown" {
                color.insert(key.to_string(), v.to_string());
            }
        }
    }
    let bits = number(video.get("bits_per_raw_sample"));
    Ok(MediaInfo {
        path: path.to_path_buf(),
        width: number(video.get("width")) as u32,
        height: number(video.get("height")) as u32,
        fps,
        duration,
        frames: frames.max(1) as u64,
        codec: video
            .get("codec_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase(),
        pix_fmt: video
            .get("pix_fmt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        bits: if bits >= 1.0 { bits as u32 } else { 8 },
        rotation: ((rotation.round() as i32) % 360 + 360) % 360,
        audio_codecs: streams
            .iter()
            .filter(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"))
            .map(|s| {
                s.get("codec_name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_ascii_lowercase()
            })
            .collect(),
        color,
        timecode,
        frames_exact,
        duration_exact,
    })
}

pub fn probe_media(ffprobe: &str, path: &Path) -> Result<MediaInfo, String> {
    let mut args = strings(&[
        ffprobe,
        "-v",
        "error",
        "-show_format",
        "-show_streams",
        "-of",
        "json",
    ]);
    args.push(path.to_string_lossy().into_owned());
    let out = run(&args, Duration::from_secs(30))
        .map_err(|e| format!("FFprobe could not be started: {e}"))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !out.status.success() {
        let msg = clean_error(&String::from_utf8_lossy(&out.stderr));
        return Err(if msg.is_empty() {
            format!("Could not read {name}.")
        } else {
            msg
        });
    }
    parse_probe(path, &String::from_utf8_lossy(&out.stdout))
}

/// Whether a proxy will relink to its original.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProxyCheck {
    pub problems: Vec<String>,
    /// What could only be checked roughly.
    pub notes: Vec<String>,
}

impl ProxyCheck {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }

    pub fn text(&self) -> String {
        let mut lines = if self.problems.is_empty() {
            vec!["The proxy matches the original.".to_string()]
        } else {
            self.problems.clone()
        };
        lines.extend(self.notes.iter().cloned());
        lines.join("\n")
    }
}

/// Compare what editors use to relink: length, frame rate, timecode, audio tracks,
/// rotation and picture shape.
pub fn compare_media(source: &MediaInfo, proxy: &MediaInfo) -> ProxyCheck {
    let mut check = ProxyCheck::default();
    let fps = if source.fps > 0.0 {
        source.fps
    } else if proxy.fps > 0.0 {
        proxy.fps
    } else {
        25.0
    };
    if source.frames_exact && proxy.frames_exact {
        if source.frames != proxy.frames {
            check.problems.push(format!(
                "Frame count: original {}, proxy {}.",
                source.frames, proxy.frames
            ));
        }
    } else {
        // Without a stored frame count, compare durations: within 1.5 frames,
        // or half a second when only the whole file's duration is known.
        let exact = source.duration_exact && proxy.duration_exact;
        let tolerance = if exact { (1.5 / fps).max(0.05) } else { 0.5 };
        if (source.duration - proxy.duration).abs() > tolerance {
            check.problems.push(format!(
                "Duration: original {:.3} s, proxy {:.3} s.",
                source.duration, proxy.duration
            ));
        }
        check.notes.push(
            "Length compared by duration, because the original does not store its frame count."
                .into(),
        );
    }
    if source.fps > 0.0 && proxy.fps > 0.0 && (source.fps - proxy.fps).abs() > source.fps * 0.005 {
        check.problems.push(format!(
            "Frame rate: original {:.3} fps, proxy {:.3} fps.",
            source.fps, proxy.fps
        ));
    }
    if !source.timecode.is_empty() && source.timecode != proxy.timecode {
        let p = if proxy.timecode.is_empty() {
            "none"
        } else {
            proxy.timecode.as_str()
        };
        check.problems.push(format!(
            "Timecode: original {}, proxy {}.",
            source.timecode, p
        ));
    }
    if source.audio_codecs.len() != proxy.audio_codecs.len() {
        check.problems.push(format!(
            "Audio tracks: original {}, proxy {}.",
            source.audio_codecs.len(),
            proxy.audio_codecs.len()
        ));
    }
    if source.rotation != proxy.rotation {
        check.problems.push(format!(
            "Rotation: original {}°, proxy {}°.",
            source.rotation, proxy.rotation
        ));
    }
    if source.height > 0 && proxy.height > 0 {
        let a = source.width as f64 / source.height as f64;
        let b = proxy.width as f64 / proxy.height as f64;
        if (a - b).abs() > a * 0.02 {
            check.problems.push(format!(
                "Picture shape: original {}×{}, proxy {}×{}.",
                source.width, source.height, proxy.width, proxy.height
            ));
        }
    }
    check
}

pub fn check_proxy(ffprobe: &str, source: &MediaInfo, proxy_path: &Path) -> ProxyCheck {
    match probe_media(ffprobe, proxy_path) {
        Ok(proxy) => compare_media(source, &proxy),
        Err(e) => {
            let first = e.lines().next().unwrap_or("");
            ProxyCheck {
                problems: vec![format!("The proxy cannot be read. {first}")
                    .trim()
                    .to_string()],
                notes: vec![],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOV: &str = r#"{"streams":[
        {"index":0,"codec_type":"video","codec_name":"HEVC","width":3840,"height":2160,"pix_fmt":"yuv420p10le",
         "avg_frame_rate":"30000/1001","r_frame_rate":"30000/1001","duration":"10.010000","nb_frames":"300",
         "bits_per_raw_sample":"10","color_primaries":"bt2020","color_transfer":"arib-std-b67",
         "color_space":"bt2020nc","color_range":"tv","tags":{"rotate":"-90"},
         "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]},
        {"index":1,"codec_type":"audio","codec_name":"aac"},
        {"index":2,"codec_type":"data","tags":{"timecode":"01:00:00;00"}},
        {"index":3,"codec_type":"video","codec_name":"mjpeg","disposition":{"attached_pic":1}}],
        "format":{"duration":"10.05"}}"#;

    #[test]
    fn parses_a_typical_camera_file() {
        let m = parse_probe(Path::new("C:/a.mov"), MOV).unwrap();
        assert_eq!((m.width, m.height, m.frames), (3840, 2160, 300));
        assert!((m.fps - 29.97).abs() < 0.01);
        assert!(m.frames_exact && m.duration_exact && m.is_10bit());
        assert_eq!(m.codec, "hevc");
        assert_eq!(m.rotation, 270);
        assert_eq!(m.audio_codecs, vec!["aac"]);
        assert_eq!(m.timecode, "01:00:00;00");
        assert_eq!(
            m.color.get("color_trc").map(String::as_str),
            Some("arib-std-b67")
        );
        assert_eq!(m.proxy_size(4), (960, 540));
        assert_eq!(m.proxy_size(2), (1920, 1080));
    }

    #[test]
    fn matroska_durations_and_estimated_frames() {
        let mkv = r#"{"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080,
            "pix_fmt":"yuv420p","avg_frame_rate":"25/1","tags":{"DURATION":"00:00:04.000000000"}}],
            "format":{"duration":"4.1","tags":{"TIMECODE":"00:00:10:00"}}}"#;
        let m = parse_probe(Path::new("a.mkv"), mkv).unwrap();
        assert_eq!(m.duration, 4.0);
        assert!(m.duration_exact && !m.frames_exact);
        assert_eq!(m.frames, 100);
        assert_eq!(m.timecode, "00:00:10:00");
        assert!(!m.is_10bit());
        assert_eq!(
            parse_probe(Path::new("x"), r#"{"streams":[{"codec_type":"audio"}]}"#).unwrap_err(),
            "No video stream found."
        );
    }

    #[test]
    fn proxy_check_rules() {
        let src = parse_probe(Path::new("a.mov"), MOV).unwrap();
        let mut proxy = src.clone();
        proxy.width = 960;
        proxy.height = 540;
        assert!(compare_media(&src, &proxy).ok());
        proxy.frames = 299;
        proxy.timecode.clear();
        proxy.audio_codecs.clear();
        proxy.rotation = 0;
        proxy.fps = 25.0;
        proxy.width = 1000;
        let c = compare_media(&src, &proxy);
        assert_eq!(c.problems.len(), 6, "{:?}", c.problems);
        assert!(c.problems[1].contains("Frame rate") && c.problems[2].contains("proxy none"));
        // without stored frame counts the duration decides, with a note
        let mut a = src.clone();
        a.frames_exact = false;
        let mut b = a.clone();
        b.duration += 0.04;
        let c = compare_media(&a, &b);
        assert!(c.ok() && c.notes.len() == 1);
        b.duration += 0.1;
        assert!(!compare_media(&a, &b).ok());
        assert_eq!(
            ProxyCheck::default().text(),
            "The proxy matches the original."
        );
    }
}
