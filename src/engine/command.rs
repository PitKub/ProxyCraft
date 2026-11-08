//! FFmpeg command lines for proxies.

use std::path::Path;

use super::caps::Capabilities;
use super::media::MediaInfo;
use super::{Family, Preset};

/// How a file is decoded. Each file tries a GPU method first when one applies, then the CPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decode {
    /// NVIDIA: decode, resize and (H.264/H.265) encode on the GPU.
    Cuda,
    /// Any GPU: decode on the GPU, resize on the CPU.
    D3d11,
    Cpu,
}

impl Decode {
    pub fn label(self) -> &'static str {
        match self {
            Decode::Cuda => "GPU",
            Decode::D3d11 => "GPU decode",
            Decode::Cpu => "CPU",
        }
    }
}

/// Codecs NVIDIA's decoder can handle. Others (ProRes, DNxHR, CineForm…) always decode on
/// the CPU. Unsupported variants of these fall back to the CPU automatically.
pub const GPU_DECODABLE: &[&str] = &[
    "h264",
    "hevc",
    "av1",
    "vp9",
    "vp8",
    "mpeg2video",
    "mpeg1video",
    "mpeg4",
    "vc1",
    "mjpeg",
];

/// Codecs that Windows D3D11 video decoding handles on Intel, AMD and NVIDIA GPUs.
pub const D3D11_DECODABLE: &[&str] = &["h264", "hevc", "av1", "vp9", "mpeg2video", "vc1", "wmv3"];

/// Source pixel formats D3D11 decodes, and the format its frames are downloaded in.
pub fn d3d11_download_format(pix_fmt: &str) -> Option<&'static str> {
    match pix_fmt {
        "yuv420p" | "yuvj420p" | "nv12" => Some("nv12"),
        "yuv420p10le" | "p010le" => Some("p010le"),
        _ => None,
    }
}

/// Encoder settings tuned for proxies: fast to make and easy to scrub.
pub fn video_args(preset: &Preset, codec: &str, info: &MediaInfo, gpu_frames: bool) -> Vec<String> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    if preset.family == Family::ProRes {
        return s(&[
            "-c:v",
            "prores_ks",
            "-profile:v",
            preset.profile,
            "-pix_fmt",
            "yuv422p10le",
            "-vendor",
            "apl0",
        ]);
    }
    // A keyframe every second and no B-frames make proxies fast to decode and quick to scrub.
    let gop = ((if info.fps > 0.0 { info.fps } else { 25.0 }).round() as i64)
        .max(1)
        .to_string();
    let ten_bit = preset.family == Family::H265 && info.is_10bit();
    let q = if preset.family == Family::H264 {
        "23"
    } else {
        "26"
    };
    let mut args = s(&["-c:v", codec]);
    if codec.ends_with("_nvenc") {
        args.extend(s(&[
            "-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", q, "-b:v", "0",
        ]));
    } else if codec.ends_with("_qsv") {
        args.extend(s(&["-preset", "veryfast", "-global_quality", q]));
    } else if codec.ends_with("_amf") {
        let qp = (q.parse::<u32>().unwrap_or(23) + 2).to_string();
        args.extend(s(&[
            "-quality", "speed", "-rc", "cqp", "-qp_i", q, "-qp_p", &qp,
        ]));
    } else if codec == "libx264" {
        args.extend(s(&["-preset", "veryfast", "-crf", q]));
    } else {
        args.extend(s(&[
            "-preset",
            "faster",
            "-crf",
            q,
            "-x265-params",
            "log-level=error",
        ]));
    }
    args.extend(s(&["-g", &gop, "-bf", "0"]));
    if preset.family == Family::H264 {
        args.extend(s(&["-profile:v", "high"]));
    } else if ten_bit {
        args.extend(s(&["-profile:v", "main10"]));
    }
    if !gpu_frames {
        let pix = if !ten_bit {
            "yuv420p"
        } else if codec.starts_with("lib") {
            "yuv420p10le"
        } else {
            "p010le"
        };
        args.extend(s(&["-pix_fmt", pix]));
    }
    args
}

/// Copy each audio track when the container accepts it, else convert it.
/// MOV (ProRes) gets PCM, which every editor reads; MP4 gets AAC.
pub fn audio_args(audio_codecs: &[String], container: &str) -> Vec<String> {
    let copyable = |c: &str| matches!(c, "aac" | "alac" | "mp3" | "ac3" | "eac3");
    let mut args = Vec::new();
    for (i, codec) in audio_codecs.iter().enumerate() {
        let key = format!("-c:a:{i}");
        if container == "mov" {
            let copy = copyable(codec) || codec.starts_with("pcm_");
            args.push(key);
            args.push(if copy { "copy" } else { "pcm_s16le" }.to_string());
        } else if copyable(codec) {
            args.push(key);
            args.push("copy".into());
        } else {
            args.extend([key, "aac".into(), format!("-b:a:{i}"), "192k".into()]);
        }
    }
    args
}

/// Decoding methods to try for a file, in order. The CPU is always last.
pub fn decode_methods(caps: &Capabilities, info: &MediaInfo) -> Vec<Decode> {
    if caps.cuda_pipeline && GPU_DECODABLE.contains(&info.codec.as_str()) {
        return vec![Decode::Cuda, Decode::Cpu];
    }
    if caps.d3d11_decode
        && D3D11_DECODABLE.contains(&info.codec.as_str())
        && d3d11_download_format(&info.pix_fmt).is_some()
    {
        return vec![Decode::D3d11, Decode::Cpu];
    }
    vec![Decode::Cpu]
}

pub fn software_encoder(family: Family) -> Option<&'static str> {
    match family {
        Family::H264 => Some("libx264"),
        Family::H265 => Some("libx265"),
        Family::ProRes => None,
    }
}

/// (decoding method, encoder) pairs to try in order; "" means the usual encoder.
///
/// The start-up test only opens each hardware encoder with 8-bit 720p frames, so one can
/// still fail on a file (10-bit on an older GPU, a frame size it does not support…). The
/// last attempt then uses x264 or x265 on the CPU.
pub fn encode_attempts(
    caps: &Capabilities,
    info: &MediaInfo,
    preset: &Preset,
) -> Vec<(Decode, String)> {
    let mut attempts: Vec<(Decode, String)> = decode_methods(caps, info)
        .into_iter()
        .map(|d| (d, String::new()))
        .collect();
    if let Some(sw) = software_encoder(preset.family) {
        if caps.encoder(preset.family) != sw && caps.encoders.contains(sw) {
            attempts.push((Decode::Cpu, sw.to_string()));
        }
    }
    attempts
}

pub fn attempt_label(decode: Decode, encoder: &str) -> String {
    if encoder.is_empty() {
        decode.label().to_string()
    } else {
        format!("CPU, {}", encoder.strip_prefix("lib").unwrap_or(encoder))
    }
}

pub fn build_command(
    ffmpeg: &str,
    info: &MediaInfo,
    preset: &Preset,
    output: &Path,
    caps: &Capabilities,
    decode: Decode,
    encoder: &str,
) -> Vec<String> {
    let (w, h) = info.proxy_size(preset.divisor);
    // -noautorotate keeps the proxy's frame layout identical to the original and preserves
    // its rotation flag, so editors relink it reliably.
    let mut cmd: Vec<String> = [
        ffmpeg,
        "-hide_banner",
        "-nostdin",
        "-y",
        "-loglevel",
        "error",
        "-noautorotate",
    ]
    .map(String::from)
    .to_vec();
    match decode {
        Decode::Cuda => {
            cmd.extend(["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"].map(String::from))
        }
        // Frames stay on the GPU until hwdownload, so a file the GPU cannot decode fails
        // clearly (and is retried on the CPU) instead of quietly decoding in software.
        Decode::D3d11 => {
            cmd.extend(["-hwaccel", "d3d11va", "-hwaccel_output_format", "d3d11"].map(String::from))
        }
        Decode::Cpu => {}
    }
    cmd.push("-i".into());
    cmd.push(info.path.to_string_lossy().into_owned());
    // Keep every source frame and timestamp, so proxy and original have the same frame
    // count and timecode.
    cmd.extend(
        [
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-map_metadata",
            "0",
            "-fps_mode",
            "passthrough",
        ]
        .map(String::from),
    );
    let (vf, codec, gpu_frames) = if decode == Decode::Cuda {
        let fmt = if info.is_10bit() && preset.family != Family::H264 {
            "p010le"
        } else {
            "nv12"
        };
        let mut vf = format!("scale_cuda={w}:{h}:format={fmt}");
        let codec = match preset.family {
            Family::ProRes => {
                vf.push_str(&format!(",hwdownload,format={fmt}"));
                "prores_ks".to_string()
            }
            Family::H264 => "h264_nvenc".to_string(),
            Family::H265 => "hevc_nvenc".to_string(),
        };
        (vf, codec, preset.family != Family::ProRes)
    } else {
        // "area" is the fastest good-quality filter for 2× and 4× reductions.
        let mut vf = format!("scale={w}:{h}:flags=area");
        if decode == Decode::D3d11 {
            let fmt = d3d11_download_format(&info.pix_fmt).unwrap_or("nv12");
            vf = format!("hwdownload,format={fmt},{vf}");
        }
        let codec = if preset.family == Family::ProRes {
            "prores_ks".to_string()
        } else if !encoder.is_empty() {
            encoder.to_string()
        } else {
            caps.encoder(preset.family)
        };
        (vf, codec, false)
    };
    cmd.push("-vf".into());
    cmd.push(vf);
    cmd.extend(video_args(preset, &codec, info, gpu_frames));
    for (key, value) in &info.color {
        cmd.push(format!("-{key}"));
        cmd.push(value.clone());
    }
    cmd.extend(audio_args(&info.audio_codecs, preset.container()));
    cmd.extend([
        "-f".to_string(),
        preset.container().to_string(),
        output.to_string_lossy().into_owned(),
    ]);
    cmd.extend(["-progress", "pipe:1", "-nostats"].map(String::from));
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::preset_by_key;

    fn info(codec: &str, pix: &str, bits: u32) -> MediaInfo {
        MediaInfo {
            path: "C:/m/a.mov".into(),
            width: 3840,
            height: 2160,
            fps: 29.97,
            codec: codec.into(),
            pix_fmt: pix.into(),
            bits,
            audio_codecs: vec!["aac".into(), "pcm_s24le".into()],
            ..Default::default()
        }
    }

    #[test]
    fn audio_rules() {
        let a = vec![
            "aac".to_string(),
            "pcm_s24le".to_string(),
            "opus".to_string(),
        ];
        assert_eq!(
            audio_args(&a, "mov"),
            ["-c:a:0", "copy", "-c:a:1", "copy", "-c:a:2", "pcm_s16le"]
        );
        assert_eq!(
            audio_args(&a, "mp4"),
            [
                "-c:a:0", "copy", "-c:a:1", "aac", "-b:a:1", "192k", "-c:a:2", "aac", "-b:a:2",
                "192k"
            ]
        );
    }

    #[test]
    fn decode_methods_and_fallbacks() {
        let h265 = preset_by_key("h265_half").unwrap();
        let src = info("hevc", "yuv420p10le", 10);
        let cpu = Capabilities {
            ok: true,
            encoders: ["libx264", "libx265"].map(String::from).into(),
            ..Default::default()
        };
        assert_eq!(
            encode_attempts(&cpu, &src, h265),
            vec![(Decode::Cpu, String::new())]
        );
        let nv = Capabilities {
            cuda_pipeline: true,
            working_h265: vec!["hevc_nvenc".into()],
            ..cpu.clone()
        };
        assert_eq!(
            encode_attempts(&nv, &src, h265),
            vec![
                (Decode::Cuda, String::new()),
                (Decode::Cpu, String::new()),
                (Decode::Cpu, "libx265".into())
            ]
        );
        let d3d = Capabilities {
            d3d11_decode: true,
            ..cpu.clone()
        };
        assert_eq!(decode_methods(&d3d, &src), vec![Decode::D3d11, Decode::Cpu]);
        assert_eq!(
            decode_methods(&d3d, &info("prores", "yuv422p10le", 10)),
            vec![Decode::Cpu]
        );
        assert_eq!(
            decode_methods(&d3d, &info("hevc", "yuv422p10le", 10)),
            vec![Decode::Cpu]
        );
        assert_eq!(attempt_label(Decode::Cpu, "libx265"), "CPU, x265");
        assert_eq!(attempt_label(Decode::D3d11, ""), "GPU decode");
    }

    #[test]
    fn commands_match_the_python_version() {
        let src = info("hevc", "yuv420p10le", 10);
        let caps = Capabilities {
            ok: true,
            ..Default::default()
        };
        let prores = preset_by_key("prores_proxy_quarter").unwrap();
        let cmd = build_command(
            "ffmpeg",
            &src,
            prores,
            Path::new("out.mov.part"),
            &caps,
            Decode::Cpu,
            "",
        );
        let j = cmd.join(" ");
        assert!(j.starts_with(
            "ffmpeg -hide_banner -nostdin -y -loglevel error -noautorotate -i C:/m/a.mov"
        ));
        assert!(j.contains("-map 0:v:0 -map 0:a? -map_metadata 0 -fps_mode passthrough -vf scale=960:540:flags=area"));
        assert!(j.contains("-c:v prores_ks -profile:v 0 -pix_fmt yuv422p10le -vendor apl0"));
        assert!(
            j.ends_with("-c:a:0 copy -c:a:1 copy -f mov out.mov.part -progress pipe:1 -nostats")
        );

        let nv = Capabilities {
            cuda_pipeline: true,
            ..caps.clone()
        };
        let h265 = preset_by_key("h265_half").unwrap();
        let j = build_command(
            "ffmpeg",
            &src,
            h265,
            Path::new("o.mp4"),
            &nv,
            Decode::Cuda,
            "",
        )
        .join(" ");
        assert!(j.contains("-hwaccel cuda -hwaccel_output_format cuda"));
        assert!(j.contains("-vf scale_cuda=1920:1080:format=p010le -c:v hevc_nvenc -preset p4 -tune hq -rc vbr -cq 26 -b:v 0 -g 30 -bf 0 -profile:v main10"));
        assert!(!j.contains("-pix_fmt"));

        let d3d = Capabilities {
            d3d11_decode: true,
            ..caps.clone()
        };
        let h264 = preset_by_key("h264_quarter").unwrap();
        let j = build_command(
            "ffmpeg",
            &src,
            h264,
            Path::new("o.mp4"),
            &d3d,
            Decode::D3d11,
            "",
        )
        .join(" ");
        assert!(j.contains("-vf hwdownload,format=p010le,scale=960:540:flags=area -c:v libx264 -preset veryfast -crf 23 -g 30 -bf 0 -profile:v high -pix_fmt yuv420p"));

        let amf = video_args(h264, "h264_amf", &src, false).join(" ");
        assert_eq!(amf, "-c:v h264_amf -quality speed -rc cqp -qp_i 23 -qp_p 25 -g 30 -bf 0 -profile:v high -pix_fmt yuv420p");
        let x265 = video_args(h265, "libx265", &src, false).join(" ");
        assert!(x265.ends_with("-profile:v main10 -pix_fmt yuv420p10le"));
        let qsv = video_args(h265, "hevc_qsv", &src, false).join(" ");
        assert!(qsv.ends_with("-pix_fmt p010le"));
    }
}
