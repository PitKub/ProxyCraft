//! End-to-end tests with real FFmpeg (skipped when ffmpeg/ffprobe are not on PATH).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use proxycraft::engine::batch::{BatchOptions, Event, Job, ProxyBatch, Status};
use proxycraft::engine::caps::detect_capabilities;
use proxycraft::engine::media::{check_proxy, probe_media};
use proxycraft::engine::plan::plan_outputs;
use proxycraft::engine::{Destination, PRESETS};
use proxycraft::tools::find_tool;

fn tools() -> Option<(String, String)> {
    let f = find_tool("ffmpeg")?;
    let p = find_tool("ffprobe")?;
    Some((
        f.to_string_lossy().into_owned(),
        p.to_string_lossy().into_owned(),
    ))
}

fn make(ffmpeg: &str, out: &Path, args: &[&str]) {
    let mut cmd = Command::new(ffmpeg);
    cmd.args(["-v", "error", "-y"]).args(args).arg(out);
    assert!(cmd.status().unwrap().success(), "could not make {out:?}");
}

fn sources(ffmpeg: &str, dir: &Path) -> Vec<PathBuf> {
    let a = dir.join("A camera clip.mov"); // H.264 + AAC + timecode, 1 s at 25 fps
    make(
        ffmpeg,
        &a,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=640x360:r=25",
            "-f",
            "lavfi",
            "-i",
            "sine=f=440:sample_rate=48000",
            "-t",
            "1",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-timecode",
            "01:00:00:00",
        ],
    );
    let b = dir.join("b_10bit.mkv"); // 10-bit HEVC, Matroska (no stored frame count), Opus audio
    make(
        ffmpeg,
        &b,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=640x480:r=30",
            "-f",
            "lavfi",
            "-i",
            "sine=f=220:sample_rate=48000",
            "-t",
            "1",
            "-c:v",
            "libx265",
            "-x265-params",
            "log-level=none",
            "-pix_fmt",
            "yuv420p10le",
            "-c:a",
            "libopus",
        ],
    );
    let c = dir.join("c_rotated.mp4"); // rotation flag and two audio tracks
    let plain = dir.join("c_plain.mp4");
    make(
        ffmpeg,
        &plain,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=480x270:r=24",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=48000",
            "-f",
            "lavfi",
            "-i",
            "sine=f=880:sample_rate=48000",
            "-t",
            "1",
            "-map",
            "0",
            "-map",
            "1",
            "-map",
            "2",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
        ],
    );
    // A rotation flag is only written when remuxing without re-encoding.
    let plain_s = plain.to_string_lossy().into_owned();
    make(
        ffmpeg,
        &c,
        &[
            "-display_rotation:v:0",
            "90",
            "-i",
            &plain_s,
            "-map",
            "0",
            "-c",
            "copy",
        ],
    );
    let _ = std::fs::remove_file(&plain);
    vec![a, b, c]
}

fn run_batch(
    ffmpeg: &str,
    ffprobe: &str,
    jobs: Vec<Job>,
    preset_key: &str,
    skip: bool,
) -> (Vec<Event>, Vec<Job>) {
    let caps = detect_capabilities(ffmpeg, false);
    assert!(caps.ok, "{}", caps.details);
    let preset = *proxycraft::engine::preset_by_key(preset_key).unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let ev = events.clone();
    let batch = ProxyBatch::new(
        ffmpeg,
        ffprobe,
        caps,
        jobs,
        preset,
        BatchOptions {
            parallel: 2,
            skip_existing: skip,
            low_priority: true,
            verify: true,
        },
        Box::new(move |e| ev.lock().unwrap().push(e)),
    );
    batch.run();
    let out = events.lock().unwrap().clone();
    (out, batch.jobs())
}

#[test]
fn every_format_relinks_to_its_original() {
    let Some((ffmpeg, ffprobe)) = tools() else {
        eprintln!("ffmpeg not found; skipped");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let srcs = sources(&ffmpeg, dir.path());
    for preset in PRESETS.iter() {
        let outs = plan_outputs(&srcs, preset, Destination::ProxiesFolder, "", "_Proxy");
        let jobs = srcs
            .iter()
            .zip(&outs)
            .map(|(s, o)| Job::new(s.clone(), o.clone(), None))
            .collect();
        let (events, jobs) = run_batch(&ffmpeg, &ffprobe, jobs, preset.key, false);
        for j in &jobs {
            assert_eq!(
                j.status,
                Status::Done,
                "{} {:?}: {}",
                preset.key,
                j.source.file_name(),
                j.message
            );
            assert!(j.output.is_file() && !j.output.with_extension("mov.part").exists());
            let check = j.check.as_ref().expect("checked");
            assert!(
                check.ok(),
                "{} {:?}: {}",
                preset.key,
                j.source.file_name(),
                check.text()
            );
        }
        let Some(Event::Finished {
            created,
            failed,
            checked,
            check_problems,
            cancelled,
            ..
        }) = events.last().cloned()
        else {
            panic!("no finish")
        };
        assert_eq!(
            (created, failed, checked, check_problems, cancelled),
            (3, 0, 3, 0, false)
        );
        // the proxy really is smaller and keeps 10-bit for H.265
        let p = probe_media(&ffprobe, &jobs[1].output).unwrap();
        let src = probe_media(&ffprobe, &jobs[1].source).unwrap();
        assert_eq!(p.width, src.width / preset.divisor);
        if preset.key.starts_with("h265") {
            assert!(p.is_10bit(), "{} lost 10-bit", preset.key);
        }
        let a = probe_media(&ffprobe, &jobs[0].output).unwrap();
        assert_eq!(
            a.timecode, "01:00:00:00",
            "{} lost the timecode",
            preset.key
        );
        let c = probe_media(&ffprobe, &jobs[2].output).unwrap();
        let c_src = probe_media(&ffprobe, &jobs[2].source).unwrap();
        assert_ne!(c_src.rotation, 0, "test clip has no rotation flag");
        assert_eq!(c.audio_codecs.len(), 2);
        assert_eq!(
            c.rotation, c_src.rotation,
            "{} lost the rotation flag",
            preset.key
        );
        // progress events reach 100 %
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Progress { percent, .. } if *percent >= 99.9)));
        for o in outs {
            let _ = std::fs::remove_file(o);
        }
    }
}

#[test]
fn skip_existing_unreadable_files_and_custom_folder() {
    let Some((ffmpeg, ffprobe)) = tools() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let srcs = sources(&ffmpeg, dir.path());
    let broken = dir.path().join("broken.mp4");
    std::fs::write(&broken, b"not a video").unwrap();
    let preset = proxycraft::engine::preset_by_key("h264_quarter").unwrap();
    let custom = dir.path().join("all proxies");
    let mut list = srcs.clone();
    list.push(broken);
    let outs = plan_outputs(
        &list,
        preset,
        Destination::Custom,
        &custom.to_string_lossy(),
        "_Proxy",
    );
    let jobs = list
        .iter()
        .zip(&outs)
        .map(|(s, o)| Job::new(s.clone(), o.clone(), None))
        .collect();
    let (_, jobs) = run_batch(&ffmpeg, &ffprobe, jobs, preset.key, true);
    assert_eq!(
        jobs.iter().map(|j| j.status).collect::<Vec<_>>(),
        [Status::Done, Status::Done, Status::Done, Status::Failed]
    );
    assert!(jobs[0].output.starts_with(&custom));
    // second run: existing proxies are skipped, and still checked
    let jobs2 = list
        .iter()
        .zip(&outs)
        .map(|(s, o)| Job::new(s.clone(), o.clone(), None))
        .collect();
    let (events, jobs2) = run_batch(&ffmpeg, &ffprobe, jobs2, preset.key, true);
    assert_eq!(
        jobs2.iter().filter(|j| j.status == Status::Skipped).count(),
        3
    );
    assert!(jobs2
        .iter()
        .take(3)
        .all(|j| j.check.as_ref().is_some_and(|c| c.ok())));
    assert!(matches!(
        events.last(),
        Some(Event::Finished {
            skipped: 3,
            failed: 1,
            ..
        })
    ));
    // a proxy of a different clip is caught by the check
    let wrong = check_proxy(
        &ffprobe,
        &probe_media(&ffprobe, &srcs[0]).unwrap(),
        &jobs[2].output,
    );
    assert!(!wrong.ok());
}

#[test]
fn cancel_stops_ffmpeg_and_leaves_no_partial_file() {
    let Some((ffmpeg, ffprobe)) = tools() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let long = dir.path().join("long.mp4");
    make(
        &ffmpeg,
        &long,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=1920x1080:r=25",
            "-t",
            "40",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
        ],
    );
    let caps = detect_capabilities(&ffmpeg, false);
    let preset = *proxycraft::engine::preset_by_key("prores_lt_half").unwrap();
    let out = dir.path().join("Proxies").join("long_Proxy.mov");
    let batch = Arc::new(ProxyBatch::new(
        &ffmpeg,
        &ffprobe,
        caps,
        vec![Job::new(long, out.clone(), None)],
        preset,
        BatchOptions {
            parallel: 1,
            skip_existing: false,
            low_priority: false,
            verify: true,
        },
        Box::new(|_| {}),
    ));
    let b = batch.clone();
    let worker = std::thread::spawn(move || b.run());
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let t0 = std::time::Instant::now();
    batch.cancel();
    worker.join().unwrap();
    assert!(
        t0.elapsed().as_secs_f64() < 5.0,
        "cancel took {:?}",
        t0.elapsed()
    );
    assert_eq!(batch.jobs()[0].status, Status::Cancelled);
    assert!(!out.exists());
    assert!(!out.with_extension("mov.part").exists());
}
