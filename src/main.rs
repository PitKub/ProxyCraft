//! ProxyCraft window (egui / eframe).

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod theme;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, text::LayoutJob, Align, Color32, FontId, Key, KeyboardShortcut, Layout, Modifiers,
    RichText, TextFormat,
};
use egui_extras::{Column, TableBuilder};

use proxycraft::engine::batch::{BatchOptions, Event, Job, ProxyBatch, Status};
use proxycraft::engine::caps::{detect_capabilities, Capabilities};
use proxycraft::engine::media::{check_proxy, probe_media, MediaInfo, ProxyCheck};
use proxycraft::engine::plan::{
    auto_parallel_jobs, collect_media, has_supported_extension, is_proxy_file, plan_outputs,
};
use proxycraft::engine::{
    format_time, preset_by_key, Destination, Family, Preset, DEFAULT_SUFFIX, PRESETS,
    SUPPORTED_EXTENSIONS,
};
use proxycraft::help_text::{Block, GUIDE};
use proxycraft::settings::Settings;
use proxycraft::tools::{bundled_tool, find_tool, same_path};
use proxycraft::{APP_NAME, DONATION_ADDRESS, REPO_URL, VERSION};

const STATS_IDLE: &str = "Files: —   |   Speed: —   |   Time left: —";
const ICON_PNG: &[u8] = include_bytes!("../assets/ProxyCraft.png");

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(APP_NAME)
        .with_inner_size([1240.0, 780.0])
        .with_min_inner_size([900.0, 560.0])
        .with_drag_and_drop(true);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(ICON_PNG) {
        viewport = viewport.with_icon(Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| Ok(Box::new(ProxyCraftApp::new(&cc.egui_ctx)))),
    )
}

// ------------------------------------------------------------------ state

/// One file in the list.
struct Row {
    path: PathBuf,
    info: Option<MediaInfo>,
    probing: bool,
    status: String,
    status_tip: String,
    /// The proxy of this row, once one is known (made, skipped or found by a check).
    output: Option<PathBuf>,
    engine: String,
    check: String,
    check_tip: String,
    selected: bool,
}

impl Row {
    fn new(path: PathBuf) -> Row {
        Row {
            path,
            info: None,
            probing: true,
            status: "Waiting".into(),
            status_tip: String::new(),
            output: None,
            engine: String::new(),
            check: String::new(),
            check_tip: String::new(),
            selected: false,
        }
    }
}

/// Results from worker threads.
enum Msg {
    Caps(String, Capabilities),
    Probed(u64, PathBuf, Result<MediaInfo, String>),
    Checked {
        key: PathBuf,
        output: PathBuf,
        check: Option<ProxyCheck>,
        message: String,
    },
    CheckDone,
    Batch(Event),
    BatchDone,
}

struct SettingsDraft {
    ffmpeg: String,
    ffprobe: String,
    suffix: String,
}

enum Dialog {
    Settings(SettingsDraft),
    About,
    Donate {
        copied: bool,
    },
    Finished {
        warning: bool,
        text: String,
        summary: String,
        folder: Option<PathBuf>,
    },
    Alert {
        title: String,
        text: String,
    },
    ConfirmQuit,
}

struct ProxyCraftApp {
    settings: Settings,
    ffmpeg: String,
    ffprobe: String,
    caps: Option<Capabilities>,
    rows: Vec<Row>,
    anchor: Option<usize>,

    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    ctx: egui::Context,
    probe_gen: Arc<AtomicU64>,
    batch: Option<Arc<ProxyBatch>>,
    check_stop: Option<Arc<AtomicBool>>,
    check_counts: (usize, usize, usize), // ok, problems, missing
    cancel_requested: bool,
    started: Instant,

    current: String,
    stats: String,
    progress: f32,
    flash: Option<(String, Instant)>,

    dialog: Option<Dialog>,
    show_guide: bool,
    allow_close: bool,
    logo: Option<egui::TextureHandle>,
}

impl ProxyCraftApp {
    fn new(ctx: &egui::Context) -> ProxyCraftApp {
        let settings = Settings::load();
        theme::apply(ctx, &settings.theme);
        let pick = |saved: &str, name: &str| -> String {
            if !saved.is_empty() && Path::new(saved).is_file() {
                saved.to_string()
            } else {
                find_tool(name)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            }
        };
        let ffmpeg = pick(&settings.ffmpeg, "ffmpeg");
        let ffprobe = pick(&settings.ffprobe, "ffprobe");
        let logo = eframe::icon_data::from_png_bytes(ICON_PNG)
            .ok()
            .map(|icon| {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                );
                ctx.load_texture("logo", image, egui::TextureOptions::LINEAR)
            });
        let (tx, rx) = channel();
        let mut app = ProxyCraftApp {
            settings,
            ffmpeg,
            ffprobe,
            caps: None,
            rows: Vec::new(),
            anchor: None,
            tx,
            rx,
            ctx: ctx.clone(),
            probe_gen: Arc::new(AtomicU64::new(0)),
            batch: None,
            check_stop: None,
            check_counts: (0, 0, 0),
            cancel_requested: false,
            started: Instant::now(),
            current: "Ready.".into(),
            stats: STATS_IDLE.into(),
            progress: 0.0,
            flash: None,
            dialog: None,
            show_guide: false,
            allow_close: false,
            logo,
        };
        app.detect_hardware(false);
        // Files or folders given on the command line (e.g. "Open with" or Send To).
        let args: Vec<PathBuf> = std::env::args_os()
            .skip(1)
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
        if !args.is_empty() {
            app.add_paths(args);
        }
        app
    }

    fn colors(&self) -> theme::Colors {
        theme::colors(&self.settings.theme)
    }

    fn suffix(&self) -> String {
        if self.settings.suffix.is_empty() {
            DEFAULT_SUFFIX.into()
        } else {
            self.settings.suffix.clone()
        }
    }

    fn preset(&self) -> Preset {
        *preset_by_key(&self.settings.preset).unwrap_or(&PRESETS[0])
    }

    fn running(&self) -> bool {
        self.batch.is_some()
    }

    fn checking(&self) -> bool {
        self.check_stop.is_some()
    }

    /// Creating or checking proxies: the list and options are locked.
    fn busy(&self) -> bool {
        self.running() || self.checking()
    }

    fn can_start(&self) -> bool {
        !self.rows.is_empty()
            && !self.busy()
            && self.caps.as_ref().is_some_and(|c| c.ok)
            && !self.ffprobe.is_empty()
    }

    fn can_check(&self) -> bool {
        !self.rows.is_empty() && !self.busy() && !self.ffprobe.is_empty()
    }

    fn flash(&mut self, text: impl Into<String>) {
        self.flash = Some((text.into(), Instant::now()));
    }

    fn alert(&mut self, title: &str, text: impl Into<String>) {
        self.dialog = Some(Dialog::Alert {
            title: title.into(),
            text: text.into(),
        });
    }

    fn save_settings(&self) {
        self.settings.save();
    }

    // -------------------------------------------------------------- hardware

    fn detect_hardware(&mut self, refresh: bool) {
        if self.ffmpeg.is_empty() {
            self.caps = Some(Capabilities::not_available("FFmpeg not found."));
            return;
        }
        self.caps = None;
        let (ffmpeg, tx, ctx) = (self.ffmpeg.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let caps = detect_capabilities(&ffmpeg, refresh);
            let _ = tx.send(Msg::Caps(ffmpeg, caps));
            ctx.request_repaint();
        });
    }

    fn hardware_text(&self) -> String {
        if self.ffmpeg.is_empty() {
            return "FFmpeg was not found. Open Tools → Settings and select ffmpeg.exe.".into();
        }
        match &self.caps {
            None => "Checking which encoders and decoders work on this PC…".into(),
            Some(c) => c.summary(),
        }
    }

    fn status_text(&self) -> String {
        let Some(caps) = &self.caps else {
            return "FFmpeg: checking…".into();
        };
        let f = if !caps.ok {
            "FFmpeg: NOT FOUND"
        } else if bundled_tool("ffmpeg").is_some_and(|b| same_path(&self.ffmpeg, &b)) {
            "FFmpeg: included"
        } else {
            "FFmpeg: OK"
        };
        let p = if self.ffprobe.is_empty() {
            "FFprobe: NOT FOUND"
        } else {
            "FFprobe: OK"
        };
        let gpu = if caps.cuda_pipeline {
            "GPU decoding: NVIDIA"
        } else if caps.d3d11_decode {
            "GPU decoding: D3D11"
        } else {
            "GPU decoding: not available"
        };
        format!("{f}   |   {p}   |   {gpu}")
    }

    // ----------------------------------------------------------------- files

    fn add_files(&mut self) {
        if self.busy() {
            return;
        }
        let exts: Vec<&str> = SUPPORTED_EXTENSIONS
            .iter()
            .map(|e| e.trim_start_matches('.'))
            .collect();
        let mut dialog = rfd::FileDialog::new()
            .set_title("Add video files")
            .add_filter("Video files", &exts)
            .add_filter("All files", &["*"]);
        if Path::new(&self.settings.last_dir).is_dir() {
            dialog = dialog.set_directory(&self.settings.last_dir);
        }
        if let Some(files) = dialog.pick_files() {
            if let Some(dir) = files.first().and_then(|f| f.parent()) {
                self.settings.last_dir = dir.to_string_lossy().into_owned();
                self.save_settings();
            }
            self.add_paths(files);
        }
    }

    fn add_folder(&mut self) {
        if self.busy() {
            return;
        }
        let mut dialog = rfd::FileDialog::new().set_title("Add a folder of video files");
        if Path::new(&self.settings.last_dir).is_dir() {
            dialog = dialog.set_directory(&self.settings.last_dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.settings.last_dir = folder.to_string_lossy().into_owned();
            self.save_settings();
            self.add_paths(vec![folder]);
        }
    }

    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        if self.busy() {
            return;
        }
        let suffix = self.suffix();
        let mut files = Vec::new();
        for p in paths {
            if p.is_dir() {
                files.extend(collect_media(&p, self.settings.include_subfolders, &suffix));
            } else if has_supported_extension(&p) && !is_proxy_file(&p, &suffix) {
                files.push(p);
            }
        }
        let mut known: HashSet<String> = self
            .rows
            .iter()
            .map(|r| r.path.to_string_lossy().to_lowercase())
            .collect();
        let mut new = Vec::new();
        for f in &files {
            if known.insert(f.to_string_lossy().to_lowercase()) {
                self.rows.push(Row::new(f.clone()));
                new.push(f.clone());
            }
        }
        if !new.is_empty() {
            self.flash(format!("Added {} file(s).", new.len()));
            self.probe(new);
        } else if !files.is_empty() {
            self.flash("Those files are already in the list.");
        } else {
            self.flash("No supported video files found.");
        }
    }

    fn probe(&mut self, paths: Vec<PathBuf>) {
        if self.ffprobe.is_empty() {
            for r in self.rows.iter_mut().filter(|r| paths.contains(&r.path)) {
                r.probing = false;
                r.status = "FFprobe not found".into();
            }
            return;
        }
        let gen_counter = self.probe_gen.clone();
        let generation = gen_counter.load(Ordering::SeqCst);
        let (ffprobe, tx, ctx) = (self.ffprobe.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            for path in paths {
                if gen_counter.load(Ordering::SeqCst) != generation {
                    return;
                }
                let result = probe_media(&ffprobe, &path)
                    .map_err(|e| e.lines().next().unwrap_or("Unreadable file.").to_string());
                if tx.send(Msg::Probed(generation, path, result)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
    }

    fn remove_selected(&mut self) {
        if self.busy() {
            return;
        }
        self.rows.retain(|r| !r.selected);
        self.anchor = None;
    }

    fn clear_list(&mut self) {
        if self.busy() {
            return;
        }
        self.probe_gen.fetch_add(1, Ordering::SeqCst);
        self.rows.clear();
        self.anchor = None;
        self.progress = 0.0;
        self.current = "Ready.".into();
        self.stats = STATS_IDLE.into();
    }

    fn select_row(&mut self, index: usize, modifiers: Modifiers) {
        if modifiers.shift {
            let a = self.anchor.unwrap_or(index).min(self.rows.len() - 1);
            let (lo, hi) = if a <= index { (a, index) } else { (index, a) };
            if !modifiers.command {
                self.rows.iter_mut().for_each(|r| r.selected = false);
            }
            for r in &mut self.rows[lo..=hi] {
                r.selected = true;
            }
        } else if modifiers.command {
            self.rows[index].selected = !self.rows[index].selected;
            self.anchor = Some(index);
        } else {
            for (i, r) in self.rows.iter_mut().enumerate() {
                r.selected = i == index;
            }
            self.anchor = Some(index);
        }
    }

    // ------------------------------------------------------------------- run

    fn destination_ready(&mut self, for_check: bool) -> bool {
        if self.settings.destination != Destination::Custom {
            return true;
        }
        let custom = self.settings.custom_dir.trim().to_string();
        if custom.is_empty() {
            let text = if for_check {
                "Choose the folder that holds the proxies first."
            } else {
                "Choose a folder for the proxies first."
            };
            self.alert("Destination", text);
            return false;
        }
        if !for_check {
            if let Err(e) = std::fs::create_dir_all(&custom) {
                self.alert("Destination", format!("Cannot use that folder:\n\n{e}"));
                return false;
            }
        }
        true
    }

    fn start(&mut self) {
        if self.busy() || self.rows.is_empty() {
            return;
        }
        let Some(caps) = self.caps.clone().filter(|c| c.ok) else {
            self.alert(
                "FFmpeg required",
                "FFmpeg is not ready. Open Tools → Settings and select ffmpeg.exe.",
            );
            return;
        };
        if !self.destination_ready(false) {
            return;
        }
        let preset = self.preset();
        let sources: Vec<PathBuf> = self.rows.iter().map(|r| r.path.clone()).collect();
        let outputs = plan_outputs(
            &sources,
            &preset,
            self.settings.destination,
            self.settings.custom_dir.trim(),
            &self.suffix(),
        );
        let jobs: Vec<Job> = self
            .rows
            .iter()
            .zip(outputs)
            .map(|(r, o)| Job::new(r.path.clone(), o, r.info.clone()))
            .collect();
        for r in &mut self.rows {
            r.status = "Waiting".into();
            r.status_tip.clear();
            r.output = None;
            r.engine.clear();
            r.check.clear();
            r.check_tip.clear();
        }
        let opts = BatchOptions {
            parallel: self.settings.parallel,
            skip_existing: self.settings.skip_existing,
            low_priority: self.settings.low_priority,
            verify: self.settings.verify,
        };
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        let sink = Box::new(move |e: Event| {
            let _ = tx.send(Msg::Batch(e));
            ctx.request_repaint();
        });
        let count = jobs.len();
        let batch = Arc::new(ProxyBatch::new(
            &self.ffmpeg,
            &self.ffprobe,
            caps,
            jobs,
            preset,
            opts,
            sink,
        ));
        self.current = format!(
            "Creating {count} proxies: {}, {} file(s) at once…",
            preset.label, batch.parallel
        );
        self.stats =
            format!("Files: 0 of {count} done   |   Speed: measuring…   |   Time left: measuring…");
        self.progress = 0.0;
        self.started = Instant::now();
        self.cancel_requested = false;
        self.batch = Some(batch.clone());
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            batch.run();
            let _ = tx.send(Msg::BatchDone);
            ctx.request_repaint();
        });
    }

    fn cancel(&mut self) {
        if let Some(batch) = &self.batch {
            batch.cancel();
            self.cancel_requested = true;
            self.current = "Cancelling…".into();
        } else if let Some(stop) = &self.check_stop {
            stop.store(true, Ordering::SeqCst);
            self.cancel_requested = true;
            self.current = "Stopping the check…".into();
        }
    }

    /// Check the proxies the current settings point to, without encoding.
    fn check_proxies(&mut self) {
        if self.busy() || self.rows.is_empty() {
            return;
        }
        if self.ffprobe.is_empty() {
            self.alert(
                "FFprobe required",
                "FFprobe was not found. Open Tools → Settings and select ffprobe.exe.",
            );
            return;
        }
        if !self.destination_ready(true) {
            return;
        }
        let sources: Vec<PathBuf> = self.rows.iter().map(|r| r.path.clone()).collect();
        let outputs = plan_outputs(
            &sources,
            &self.preset(),
            self.settings.destination,
            self.settings.custom_dir.trim(),
            &self.suffix(),
        );
        let work: Vec<(PathBuf, PathBuf, Option<MediaInfo>)> = self
            .rows
            .iter()
            .zip(outputs)
            .map(|(r, o)| (r.path.clone(), o, r.info.clone()))
            .collect();
        for r in &mut self.rows {
            r.check = "Checking…".into();
            r.check_tip.clear();
        }
        self.check_counts = (0, 0, 0);
        self.current = format!("Checking {} proxies…", work.len());
        self.cancel_requested = false;
        let stop = Arc::new(AtomicBool::new(false));
        self.check_stop = Some(stop.clone());
        let (ffprobe, tx, ctx) = (self.ffprobe.clone(), self.tx.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            for (key, output, info) in work {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let msg = if !output.is_file() {
                    let message = format!("No proxy found at:\n{}", output.display());
                    Msg::Checked {
                        key,
                        output,
                        check: None,
                        message,
                    }
                } else {
                    let info = match info {
                        Some(i) => Ok(i),
                        None => probe_media(&ffprobe, &key),
                    };
                    match info {
                        Ok(info) => {
                            let check = check_proxy(&ffprobe, &info, &output);
                            Msg::Checked {
                                key,
                                output,
                                check: Some(check),
                                message: String::new(),
                            }
                        }
                        Err(e) => {
                            let first = e.lines().next().unwrap_or("").to_string();
                            let message = format!("The original cannot be read. {first}");
                            Msg::Checked {
                                key,
                                output,
                                check: None,
                                message,
                            }
                        }
                    }
                };
                let _ = tx.send(msg);
                ctx.request_repaint();
            }
            let _ = tx.send(Msg::CheckDone);
            ctx.request_repaint();
        });
    }

    // ------------------------------------------------------------ messages

    fn handle_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Caps(ffmpeg, caps) => {
                    if ffmpeg == self.ffmpeg {
                        self.caps = Some(caps);
                    }
                }
                Msg::Probed(generation, path, result) => {
                    if generation != self.probe_gen.load(Ordering::SeqCst) {
                        continue;
                    }
                    if let Some(r) = self.rows.iter_mut().find(|r| r.path == path) {
                        r.probing = false;
                        match result {
                            Ok(info) => r.info = Some(info),
                            Err(e) => {
                                r.info = None;
                                r.status = "Unreadable".into();
                                r.status_tip = e;
                            }
                        }
                    }
                }
                Msg::Checked {
                    key,
                    output,
                    check,
                    message,
                } => {
                    let Some(r) = self.rows.iter_mut().find(|r| r.path == key) else {
                        continue;
                    };
                    match check {
                        None => {
                            let missing = !output.is_file();
                            if missing {
                                self.check_counts.2 += 1;
                            } else {
                                self.check_counts.1 += 1;
                            }
                            r.check = if missing { "No proxy" } else { "Problems" }.into();
                            r.check_tip = message;
                        }
                        Some(check) => {
                            if check.ok() {
                                self.check_counts.0 += 1;
                            } else {
                                self.check_counts.1 += 1;
                            }
                            show_check(r, &check);
                            r.output = Some(output);
                        }
                    }
                }
                Msg::CheckDone => {
                    self.check_stop = None;
                    for r in &mut self.rows {
                        if r.check == "Checking…" {
                            r.check.clear();
                        }
                    }
                    let (ok, problems, missing) = self.check_counts;
                    let mut text = format!("Checked proxies: {ok} OK, {problems} with problems");
                    if missing > 0 {
                        text += &format!(", {missing} without a proxy");
                    }
                    self.current = text + ". Hover over a result for details.";
                }
                Msg::Batch(event) => self.on_batch_event(event),
                Msg::BatchDone => self.batch = None,
            }
        }
    }

    fn on_batch_event(&mut self, event: Event) {
        match event {
            Event::JobStatus {
                index,
                status,
                message,
                output,
            } => {
                let Some(r) = self.rows.get_mut(index) else {
                    return;
                };
                match status {
                    Status::Encoding => {
                        r.status = format!("Encoding ({message})");
                        r.engine = message;
                    }
                    Status::Done => {
                        r.status = format!("Done ({message})");
                        r.status_tip = output.display().to_string();
                        r.output = Some(output);
                    }
                    _ => {
                        r.status = status.name().into();
                        r.status_tip = message;
                        if status == Status::Skipped {
                            r.output = Some(output);
                        }
                    }
                }
            }
            Event::JobProgress { index, percent, .. } => {
                if let Some(r) = self.rows.get_mut(index) {
                    if r.status.starts_with("Encoding") {
                        let engine = if r.engine.is_empty() {
                            "CPU"
                        } else {
                            r.engine.as_str()
                        };
                        r.status = format!("Encoding ({engine}) {percent:.0}%");
                    }
                }
            }
            Event::JobCheck { index, check } => {
                if let Some(r) = self.rows.get_mut(index) {
                    show_check(r, &check);
                }
            }
            Event::Progress {
                percent,
                fps,
                eta,
                done,
                total,
            } => {
                self.progress = (percent / 100.0) as f32;
                let elapsed = format_time(self.started.elapsed().as_secs_f64());
                let speed = if fps > 0.0 {
                    format!("{fps:.0} fps")
                } else {
                    "measuring…".into()
                };
                let left = match eta {
                    None => "measuring…".to_string(),
                    Some(eta) => {
                        let finish = chrono::Local::now()
                            + chrono::Duration::milliseconds((eta * 1000.0) as i64);
                        format!("{} (about {})", format_time(eta), finish.format("%H:%M"))
                    }
                };
                self.stats = format!(
                    "Files: {done} of {total} done   |   Speed: {speed}   |   Elapsed: {elapsed}   |   Time left: {left}"
                );
            }
            Event::Finished {
                created,
                skipped,
                failed,
                cancelled,
                elapsed,
                checked,
                check_problems,
            } => {
                let elapsed = format_time(elapsed);
                if cancelled {
                    self.current =
                        format!("Cancelled after {elapsed}. Unfinished proxies were removed.");
                    return;
                }
                if failed == 0 {
                    self.progress = 1.0;
                }
                let mut summary =
                    format!("Created {created}, skipped {skipped}, failed {failed} in {elapsed}.");
                if checked > 0 {
                    summary += &format!(
                        " Proxy check: {} OK, {check_problems} with problems.",
                        checked - check_problems
                    );
                }
                self.current = summary.clone();
                let text = if failed > 0 {
                    "Proxy creation finished with errors. Hover over a failed file's status for details."
                } else if check_problems > 0 {
                    "Some proxies may not relink to their originals. Hover over Problems in the Check column for details."
                } else {
                    "Proxy creation finished."
                };
                let folder = self
                    .rows
                    .iter()
                    .find_map(|r| r.output.as_ref())
                    .and_then(|o| o.parent())
                    .map(Path::to_path_buf);
                self.dialog = Some(Dialog::Finished {
                    warning: failed > 0 || check_problems > 0,
                    text: text.into(),
                    summary,
                    folder,
                });
            }
        }
    }

    // ------------------------------------------------------------- settings

    fn apply_settings(&mut self, d: SettingsDraft) {
        let ffmpeg = d.ffmpeg.trim().trim_matches('"').to_string();
        let ffprobe = d.ffprobe.trim().trim_matches('"').to_string();
        let missing: Vec<&str> = [ffmpeg.as_str(), ffprobe.as_str()]
            .into_iter()
            .filter(|p| !p.is_empty() && !Path::new(p).is_file())
            .collect();
        if !missing.is_empty() {
            self.alert(
                "File not found",
                format!("These files do not exist:\n\n{}", missing.join("\n")),
            );
        }
        let suffix = d.suffix.trim();
        self.settings.suffix = if suffix.is_empty() {
            DEFAULT_SUFFIX.into()
        } else {
            suffix.into()
        };
        let changed = ffmpeg != self.ffmpeg;
        // Store nothing for the included FFmpeg, so it is still found after the
        // ProxyCraft folder is moved or updated.
        let included = |p: &str, name: &str| bundled_tool(name).is_some_and(|b| same_path(p, &b));
        self.settings.ffmpeg = if included(&ffmpeg, "ffmpeg") {
            String::new()
        } else {
            ffmpeg.clone()
        };
        self.settings.ffprobe = if included(&ffprobe, "ffprobe") {
            String::new()
        } else {
            ffprobe.clone()
        };
        self.ffmpeg = ffmpeg;
        self.ffprobe = ffprobe;
        self.save_settings();
        if changed {
            self.detect_hardware(true);
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme_name: &str) {
        self.settings.theme = theme_name.into();
        theme::apply(ctx, theme_name);
        self.save_settings();
    }

    // ------------------------------------------------------------------ input

    fn handle_input(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() && self.dialog.is_none() {
            self.add_paths(dropped);
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.running() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.dialog = Some(Dialog::ConfirmQuit);
            } else {
                self.shutdown_workers();
            }
        }

        if self.dialog.is_some() {
            return;
        }
        let shortcut = |m: Modifiers, k: Key| {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)))
        };
        if shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::O) {
            self.add_folder();
        } else if shortcut(Modifiers::COMMAND, Key::O) {
            self.add_files();
        }
        if shortcut(Modifiers::COMMAND, Key::Enter) && self.can_start() {
            self.start();
        }
        if shortcut(Modifiers::COMMAND, Key::K) && self.can_check() {
            self.check_proxies();
        }
        if !ctx.egui_wants_keyboard_input() {
            if shortcut(Modifiers::NONE, Key::Delete) {
                self.remove_selected();
            }
            if shortcut(Modifiers::COMMAND, Key::A) {
                self.rows.iter_mut().for_each(|r| r.selected = true);
            }
        }
    }

    fn shutdown_workers(&mut self) {
        if let Some(stop) = &self.check_stop {
            stop.store(true, Ordering::SeqCst);
        }
        self.probe_gen.fetch_add(1, Ordering::SeqCst);
        if let Some(batch) = &self.batch {
            batch.cancel();
            // Give FFmpeg a moment to exit and the batch to remove unfinished files.
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                match self.rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Msg::BatchDone) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break
                    }
                    _ => {}
                }
            }
        }
    }

    // --------------------------------------------------------------------- ui

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui
                    .add_enabled(
                        !busy,
                        egui::Button::new("Add Files…").shortcut_text("Ctrl+O"),
                    )
                    .clicked()
                {
                    ui.close();
                    self.add_files();
                }
                if ui
                    .add_enabled(
                        !busy,
                        egui::Button::new("Add Folder…").shortcut_text("Ctrl+Shift+O"),
                    )
                    .clicked()
                {
                    ui.close();
                    self.add_folder();
                }
                if ui
                    .add_enabled(
                        !busy,
                        egui::Button::new("Remove Selected").shortcut_text("Del"),
                    )
                    .clicked()
                {
                    ui.close();
                    self.remove_selected();
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Clear List"))
                    .clicked()
                {
                    ui.close();
                    self.clear_list();
                }
                ui.separator();
                let start = egui::Button::new("Create Proxies").shortcut_text("Ctrl+Enter");
                if ui.add_enabled(self.can_start(), start).clicked() {
                    ui.close();
                    self.start();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.close();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Tools", |ui| {
                let check = egui::Button::new("Check Proxies").shortcut_text("Ctrl+K");
                if ui.add_enabled(self.can_check(), check).clicked() {
                    ui.close();
                    self.check_proxies();
                }
                ui.separator();
                if ui.button("Settings…").clicked() {
                    ui.close();
                    self.dialog = Some(Dialog::Settings(SettingsDraft {
                        ffmpeg: self.ffmpeg.clone(),
                        ffprobe: self.ffprobe.clone(),
                        suffix: self.suffix(),
                    }));
                }
                if ui
                    .add_enabled(
                        self.caps.is_some(),
                        egui::Button::new("Detect Hardware Again"),
                    )
                    .clicked()
                {
                    ui.close();
                    self.detect_hardware(true);
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("User Guide").clicked() {
                    ui.close();
                    self.show_guide = true;
                }
                if ui.button("Donate…").clicked() {
                    ui.close();
                    self.dialog = Some(Dialog::Donate { copied: false });
                }
                ui.separator();
                if ui.button("About").clicked() {
                    ui.close();
                    self.dialog = Some(Dialog::About);
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let c = self.colors();
        let busy = self.busy();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Add Files"))
                .clicked()
            {
                self.add_files();
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Add Folder"))
                .clicked()
            {
                self.add_folder();
            }
            if ui.add_enabled(!busy, egui::Button::new("Remove")).clicked() {
                self.remove_selected();
            }
            if ui.add_enabled(!busy, egui::Button::new("Clear")).clicked() {
                self.clear_list();
            }
            ui.separator();
            let start = egui::Button::new(RichText::new("Create Proxies").strong().color(c.window))
                .fill(c.accent);
            if ui.add_enabled(self.can_start(), start).clicked() {
                self.start();
            }
            if ui
                .add_enabled(busy && !self.cancel_requested, egui::Button::new("Cancel"))
                .clicked()
            {
                self.cancel();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let light = self.settings.theme == "light";
                let (text, tip) = if light {
                    ("Light theme", "Click to switch to the dark theme")
                } else {
                    ("Dark theme", "Click to switch to the light theme")
                };
                if ui.button(text).on_hover_text(tip).clicked() {
                    let next = if light { "dark" } else { "light" };
                    self.set_theme(ui.ctx(), next);
                }
            });
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let c = self.colors();
        let busy = self.busy();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("INSPECTOR").small().strong().color(c.muted));

            group(ui, &c, "Format and size", |ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    let mut last: Option<Family> = None;
                    for preset in PRESETS.iter() {
                        if last != Some(preset.family) {
                            ui.add_space(2.0);
                            ui.label(RichText::new(preset.family.name()).small().strong().color(c.muted));
                            last = Some(preset.family);
                        }
                        if ui.radio(self.settings.preset == preset.key, preset.label).clicked() {
                            self.settings.preset = preset.key.into();
                            self.save_settings();
                        }
                    }
                });
                let preset = self.preset();
                let hint = match preset.family {
                    Family::ProRes => {
                        "ProRes is edit-friendly in every editor. Encoded on the CPU; decoding and resizing use the GPU where possible."
                    }
                    Family::H264 => "H.264 gives the smallest files and plays everywhere. 8-bit 4:2:0.",
                    Family::H265 => "H.265 gives small files and keeps 10-bit sources at 10-bit.",
                };
                ui.add_space(4.0);
                let ext = preset.extension().trim_start_matches('.').to_uppercase();
                ui.label(RichText::new(format!("{hint} Output: {ext}.")).small().color(c.muted));
            });

            group(ui, &c, "Destination", |ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    let before = self.settings.destination;
                    egui::ComboBox::from_id_salt("destination")
                        .width(ui.available_width())
                        .selected_text(self.settings.destination.label())
                        .show_ui(ui, |ui| {
                            for d in Destination::ALL {
                                ui.selectable_value(&mut self.settings.destination, d, d.label());
                            }
                        });
                    if before != self.settings.destination {
                        self.save_settings();
                    }
                    if self.settings.destination == Destination::Custom {
                        let edit = egui::TextEdit::singleline(&mut self.settings.custom_dir)
                            .hint_text("Folder for all proxies")
                            .desired_width(f32::INFINITY);
                        if ui.add(edit).lost_focus() {
                            self.save_settings();
                        }
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if ui.button("Browse…").clicked() {
                                let mut dialog = rfd::FileDialog::new().set_title("Select a folder for all proxies");
                                if Path::new(&self.settings.custom_dir).is_dir() {
                                    dialog = dialog.set_directory(&self.settings.custom_dir);
                                }
                                if let Some(folder) = dialog.pick_folder() {
                                    self.settings.custom_dir = folder.to_string_lossy().into_owned();
                                    self.save_settings();
                                }
                            }
                        });
                    }
                });
            });

            group(ui, &c, "Options", |ui| {
                ui.add_enabled_ui(!busy, |ui| {
                    let s = &mut self.settings;
                    let mut changed = false;
                    changed |= ui.checkbox(&mut s.skip_existing, "Skip files that already have a proxy").changed();
                    changed |=
                        ui.checkbox(&mut s.include_subfolders, "Include subfolders when adding a folder").changed();
                    changed |= ui
                        .checkbox(&mut s.low_priority, "Run in the background (lower priority)")
                        .on_hover_text(
                            "Keeps your editor responsive while proxies are made. \
                             It does not slow encoding when the PC is otherwise idle.",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(&mut s.verify, "Check each proxy against its original")
                        .on_hover_text(
                            "After each proxy is made, compare its length, frame rate, timecode, audio tracks \
                             and rotation with the original, so you know it will relink. Takes a moment per file.",
                        )
                        .changed();
                    ui.horizontal(|ui| {
                        ui.label("Files at once");
                        let label =
                            |n: usize| if n == 0 { format!("Auto ({})", auto_parallel_jobs()) } else { n.to_string() };
                        egui::ComboBox::from_id_salt("parallel")
                            .width(ui.available_width())
                            .selected_text(label(s.parallel))
                            .show_ui(ui, |ui| {
                                for n in 0..=4 {
                                    changed |= ui.selectable_value(&mut s.parallel, n, label(n)).changed();
                                }
                            })
                            .response
                            .on_hover_text(
                                "Encoding two files at once was about 20–50% faster than one at a time \
                                 in testing, on both GPU and CPU.",
                            );
                    });
                    if changed {
                        s.save();
                    }
                });
            });

            group(ui, &c, "Hardware", |ui| {
                ui.label(RichText::new(self.hardware_text()).small().color(c.muted));
            });

            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Proxies keep the original frame count, timecode, rotation and audio tracks, so editors can \
                     relink them to the full-resolution media. Tools → Check Proxies checks proxies you already have.",
                )
                .small()
                .color(c.muted),
            );
        });
    }

    fn file_table(&mut self, ui: &mut egui::Ui) {
        let c = self.colors();
        let busy = self.busy();
        let divisor = self.preset().divisor;
        let mut clicked: Option<(usize, Modifiers)> = None;
        let mut open_folder: Option<PathBuf> = None;
        let mut remove_row: Option<usize> = None;
        let modifiers = ui.input(|i| i.modifiers);
        let row_h = 24.0;
        let rows = &self.rows;

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::remainder().at_least(160.0).clip(true))
            .column(Column::auto().at_least(86.0))
            .column(Column::auto().at_least(80.0))
            .column(Column::auto().at_least(70.0))
            .column(Column::auto().at_least(80.0))
            .column(Column::initial(200.0).at_least(90.0).clip(true))
            .column(Column::initial(90.0).at_least(60.0).clip(true))
            .header(row_h, |mut h| {
                let names = ["File", "Resolution", "Codec", "Duration", "Proxy size", "Status", "Check"];
                for (i, name) in names.iter().enumerate() {
                    h.col(|ui| {
                        let r = ui.label(RichText::new(*name).strong().color(c.text_soft));
                        if i == 6 {
                            r.on_hover_text("Whether the proxy matches its original. Hover over a result for details.");
                        }
                    });
                }
            })
            .body(|body| {
                body.rows(row_h, rows.len(), |mut row| {
                    let i = row.index();
                    let r = &rows[i];
                    row.set_selected(r.selected);
                    let dash = if r.probing { "…" } else { "—" };
                    row.col(|ui| {
                        let name = r.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        ui.label(name).on_hover_text(r.path.display().to_string());
                    });
                    row.col(|ui| {
                        ui.label(r.info.as_ref().map(|i| format!("{}×{}", i.width, i.height)).unwrap_or(dash.into()));
                    });
                    row.col(|ui| match &r.info {
                        Some(info) => {
                            let bits = if info.is_10bit() { " 10-bit" } else { "" };
                            let tracks = info.audio_codecs.len();
                            let tip = format!(
                                "{}, {:.3} fps, {tracks} audio track{}",
                                info.pix_fmt,
                                info.fps,
                                if tracks == 1 { "" } else { "s" }
                            );
                            ui.label(format!("{}{bits}", info.codec.to_uppercase())).on_hover_text(tip);
                        }
                        None => {
                            ui.label(dash);
                        }
                    });
                    row.col(|ui| {
                        ui.label(r.info.as_ref().map(|i| format_time(i.duration)).unwrap_or(dash.into()));
                    });
                    row.col(|ui| {
                        let size = r.info.as_ref().map(|i| {
                            let (w, h) = i.proxy_size(divisor);
                            format!("{w}×{h}")
                        });
                        ui.label(size.unwrap_or(dash.into()));
                    });
                    row.col(|ui| {
                        let l = ui.label(RichText::new(&r.status).color(status_color(&c, &r.status)));
                        if !r.status_tip.is_empty() {
                            l.on_hover_text(&r.status_tip);
                        }
                    });
                    row.col(|ui| {
                        let color = match r.check.as_str() {
                            "OK" => c.done,
                            "Problems" => c.failed,
                            _ => c.muted,
                        };
                        let l = ui.label(RichText::new(&r.check).color(color));
                        if !r.check_tip.is_empty() {
                            l.on_hover_text(&r.check_tip);
                        }
                    });
                    let resp = row.response();
                    if resp.clicked() {
                        clicked = Some((i, modifiers));
                    }
                    let proxy_dir = r.output.as_ref().and_then(|o| o.parent()).filter(|d| d.is_dir());
                    if resp.double_clicked() {
                        if let Some(dir) = proxy_dir {
                            open_folder = Some(dir.to_path_buf());
                        }
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Open Source Folder").clicked() {
                            open_folder = r.path.parent().map(Path::to_path_buf);
                            ui.close();
                        }
                        if ui.add_enabled(proxy_dir.is_some(), egui::Button::new("Open Proxy Folder")).clicked() {
                            open_folder = proxy_dir.map(Path::to_path_buf);
                            ui.close();
                        }
                        ui.separator();
                        if ui.add_enabled(!busy, egui::Button::new("Remove from List")).clicked() {
                            remove_row = Some(i);
                            ui.close();
                        }
                    });
                });
            });

        if let Some((i, m)) = clicked {
            self.select_row(i, m);
        }
        if let Some(dir) = open_folder {
            open_path(&dir.to_string_lossy());
        }
        if let Some(i) = remove_row.filter(|i| *i < self.rows.len()) {
            for (j, r) in self.rows.iter_mut().enumerate() {
                r.selected = j == i;
            }
            self.remove_selected();
        }
    }

    fn progress_panel(&mut self, ui: &mut egui::Ui) {
        let c = self.colors();
        egui::Frame::new()
            .fill(c.panel)
            .stroke(egui::Stroke::new(1.0, c.border))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&self.current).strong());
                ui.add(
                    egui::ProgressBar::new(self.progress)
                        .show_percentage()
                        .fill(c.accent.gamma_multiply(0.8)),
                );
                ui.label(RichText::new(&self.stats).small().color(c.muted));
            });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let c = self.colors();
        let caps_summary = self
            .caps
            .as_ref()
            .map(|c| c.summary())
            .unwrap_or_else(|| "Still checking…".into());
        let logo = self.logo.clone();
        let modal = egui::Modal::new(egui::Id::unique("dialog"));
        let response = modal.show(ctx, |ui| match &mut dialog {
            Dialog::Settings(d) => {
                ui.set_width(620.0);
                ui.heading("ProxyCraft — Settings");
                ui.add_space(8.0);
                egui::Grid::new("settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    for (label, name) in [("FFmpeg executable", "ffmpeg"), ("FFprobe executable", "ffprobe")] {
                        ui.label(label);
                        ui.vertical(|ui| {
                            let field = if name == "ffmpeg" { &mut d.ffmpeg } else { &mut d.ffprobe };
                            ui.add(egui::TextEdit::singleline(field).desired_width(470.0));
                            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                if ui.button("Browse…").clicked() {
                                    browse_tool(d, name);
                                }
                            });
                        });
                        ui.end_row();
                    }
                    if let (Some(f), Some(p)) = (bundled_tool("ffmpeg"), bundled_tool("ffprobe")) {
                        ui.label("");
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            if ui.button("Use the FFmpeg included with ProxyCraft").clicked() {
                                d.ffmpeg = f.to_string_lossy().into_owned();
                                d.ffprobe = p.to_string_lossy().into_owned();
                            }
                        });
                        ui.end_row();
                    }
                    ui.label("Proxy name ending");
                    ui.add(egui::TextEdit::singleline(&mut d.suffix).desired_width(470.0));
                    ui.end_row();
                    ui.label("");
                    let s = if d.suffix.trim().is_empty() { DEFAULT_SUFFIX } else { d.suffix.trim() };
                    ui.label(RichText::new(format!("Example: Clip001{s}.mov")).small().color(c.muted));
                    ui.end_row();
                    ui.label("Detected hardware");
                    ui.add(egui::Label::new(RichText::new(&caps_summary).small().color(c.muted)).wrap());
                    ui.end_row();
                });
                ui.add_space(8.0);
                buttons(ui, &["Cancel", "Save"]).map(|b| b == 1)
            }
            Dialog::About => {
                ui.set_width(560.0);
                ui.horizontal_top(|ui| {
                    if let Some(logo) = &logo {
                        ui.add(egui::Image::new(logo).fit_to_exact_size(egui::vec2(72.0, 72.0)));
                        ui.add_space(12.0);
                    }
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("{APP_NAME} {VERSION}")).size(22.0).strong());
                        ui.add_space(4.0);
                        ui.label("Fast proxy creator for video editing.");
                        ui.label("ProRes, H.264 and H.265 proxies, with GPU decoding and encoding where available.");
                        ui.add_space(4.0);
                        ui.label("Free and open source under the MIT License. Donations are completely optional.");
                        ui.horizontal(|ui| {
                            ui.label("Contact:");
                            if ui.link(DONATION_ADDRESS).clicked() {
                                open_path(&format!("mailto:{DONATION_ADDRESS}"));
                            }
                        });
                        ui.label("Copyright © 2026 Piotr Kubicki.");
                        if ui.link("Source code on GitHub").clicked() {
                            open_path(REPO_URL);
                        }
                    });
                });
                ui.add_space(8.0);
                buttons(ui, &["OK"]).map(|_| true)
            }
            Dialog::Donate { copied } => {
                ui.set_width(470.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Enjoying ProxyCraft?").size(22.0).strong().color(c.accent));
                    ui.add_space(6.0);
                    ui.label(
                        "ProxyCraft is free to use. If it helps your work and you would like to support its \
                         continued development, you can make a voluntary donation through PayPal. There is no \
                         obligation—thank you for using the app.",
                    );
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(c.panel_alt)
                        .stroke(egui::Stroke::new(1.0, c.donate_border))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(18, 10))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("PayPal donation address").strong());
                                ui.add(egui::Label::new(RichText::new(DONATION_ADDRESS).size(17.0)).selectable(true));
                            });
                        });
                });
                ui.add_space(10.0);
                let mut result = None;
                ui.horizontal(|ui| {
                    let copy_text = if *copied { "Address copied" } else { "Copy PayPal address" };
                    if ui.button(copy_text).clicked() {
                        ui.ctx().copy_text(DONATION_ADDRESS.into());
                        *copied = true;
                    }
                    if ui.button("Open PayPal").clicked() {
                        open_path("https://www.paypal.com/");
                    }
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if ui.button("Close").clicked() {
                            result = Some(true);
                        }
                    });
                });
                result
            }
            Dialog::Finished { warning, text, summary, folder } => {
                ui.set_width(460.0);
                ui.label(RichText::new(text.as_str()).strong().color(if *warning { c.failed } else { c.text }));
                ui.add_space(4.0);
                ui.label(summary.as_str());
                ui.add_space(8.0);
                let labels: &[&str] = if folder.is_some() { &["OK", "Open Proxy Folder"] } else { &["OK"] };
                let pressed = buttons(ui, labels);
                if pressed == Some(1) {
                    if let Some(dir) = folder {
                        open_path(&dir.to_string_lossy());
                    }
                }
                pressed.map(|_| true)
            }
            Dialog::Alert { title, text } => {
                ui.set_width(420.0);
                ui.label(RichText::new(title.as_str()).strong().size(16.0));
                ui.add_space(4.0);
                ui.label(text.as_str());
                ui.add_space(8.0);
                buttons(ui, &["OK"]).map(|_| true)
            }
            Dialog::ConfirmQuit => {
                ui.set_width(420.0);
                ui.label(RichText::new("Proxies are being created").strong().size(16.0));
                ui.add_space(4.0);
                ui.label("Proxy creation is still running. Stop it and quit?");
                ui.add_space(8.0);
                buttons(ui, &["No", "Yes"]).map(|b| b == 1)
            }
        });

        let close = match response.inner {
            Some(true) => {
                match dialog {
                    Dialog::Settings(d) => self.apply_settings(d),
                    Dialog::ConfirmQuit => {
                        self.shutdown_workers();
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    _ => {}
                }
                return;
            }
            Some(false) => true,
            // Escape or a click outside closes everything except the quit question.
            None => response.should_close() && !matches!(dialog, Dialog::ConfirmQuit),
        };
        if !close && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }

    fn guide_window(&mut self, ctx: &egui::Context) {
        if !self.show_guide {
            return;
        }
        let c = self.colors();
        let mut open = true;
        egui::Window::new("ProxyCraft — User Guide")
            .open(&mut open)
            .default_pos([180.0, 90.0])
            .default_size([700.0, 560.0])
            .max_width(900.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut number = 0;
                        for block in GUIDE {
                            match block {
                                Block::Title(t) => {
                                    ui.label(rich(t, 24.0, c.accent, &c));
                                    number = 0;
                                }
                                Block::Heading(t) => {
                                    ui.add_space(10.0);
                                    ui.label(rich(t, 17.0, c.accent, &c));
                                    number = 0;
                                }
                                Block::Para(t) => {
                                    ui.add_space(4.0);
                                    ui.label(rich(t, 14.0, c.text_soft, &c));
                                    number = 0;
                                }
                                Block::Numbered(t) => {
                                    number += 1;
                                    list_item(
                                        ui,
                                        &format!("{number}."),
                                        rich(t, 14.0, c.text_soft, &c),
                                        &c,
                                    );
                                }
                                Block::Bullet(t) => {
                                    list_item(ui, "•", rich(t, 14.0, c.text_soft, &c), &c)
                                }
                            }
                        }
                        ui.add_space(8.0);
                    });
            });
        self.show_guide = open;
    }
}

impl eframe::App for ProxyCraftApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages();
        self.handle_input(ctx);
        if self.running() {
            // keep "Elapsed" ticking between progress events
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let c = self.colors();
        egui::Panel::top("menu")
            .frame(
                egui::Frame::new()
                    .fill(c.status)
                    .inner_margin(egui::Margin::symmetric(6, 3)),
            )
            .show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(c.panel)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(c.status)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ui, |ui| {
                let text = match &self.flash {
                    Some((t, at)) if at.elapsed() < Duration::from_secs(3) => {
                        ctx.request_repaint_after(
                            Duration::from_secs(3).saturating_sub(at.elapsed()),
                        );
                        t.clone()
                    }
                    _ => self.status_text(),
                };
                ui.label(RichText::new(text).small().color(c.muted));
            });
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(360.0)
            .min_size(330.0)
            .frame(
                egui::Frame::new()
                    .fill(c.panel)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(c.window).inner_margin(egui::Margin { left: 8, right: 6, top: 8, bottom: 8 }))
            .show(ui, |ui| {
                egui::Panel::bottom("progress")
                    .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 0, top: 6, bottom: 0 }))
                    .show_separator_line(false)
                    .show(ui, |ui| self.progress_panel(ui));
                egui::CentralPanel::default().frame(egui::Frame::new()).show(ui, |ui| {
                    if self.rows.is_empty() {
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(1.0, c.border_soft))
                            .corner_radius(6)
                            .inner_margin(egui::Margin::same(16))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new(
                                            "Drop video files or folders here, or use Add Files / Add Folder. \
                                             Existing proxies and Proxies folders are skipped automatically.",
                                        )
                                        .color(c.muted),
                                    );
                                });
                            });
                        ui.add_space(6.0);
                    }
                    self.file_table(ui);
                });
            });

        // Files dragged over the window.
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) && !self.busy() {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("drop"),
            ));
            let rect = ctx.content_rect();
            painter.rect_filled(rect, 0.0, Color32::from_black_alpha(140));
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Drop to add",
                FontId::proportional(26.0),
                Color32::WHITE,
            );
        }

        self.guide_window(&ctx);
        self.dialogs(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.colors().window.to_normalized_gamma_f32()
    }
}

// --------------------------------------------------------------- helpers

fn show_check(r: &mut Row, check: &ProxyCheck) {
    r.check = if check.ok() { "OK" } else { "Problems" }.into();
    r.check_tip = check.text();
}

fn status_color(c: &theme::Colors, status: &str) -> Color32 {
    if status.starts_with("Done") {
        c.done
    } else if status == "Failed" || status == "Unreadable" {
        c.failed
    } else if status == "Skipped" || status == "Cancelled" {
        c.muted
    } else {
        c.text
    }
}

/// Right-aligned dialog buttons; returns the index (in `labels` order) of the one pressed.
fn buttons(ui: &mut egui::Ui, labels: &[&str]) -> Option<usize> {
    let mut pressed = None;
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        for (i, label) in labels.iter().enumerate().rev() {
            if ui.button(*label).clicked() {
                pressed = Some(i);
            }
        }
    });
    pressed
}

fn group(ui: &mut egui::Ui, c: &theme::Colors, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(6.0);
    egui::Frame::new()
        .fill(c.panel_alt)
        .stroke(egui::Stroke::new(1.0, c.border))
        .corner_radius(5)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(c.accent_soft));
            ui.add_space(2.0);
            add(ui);
        });
}

fn list_item(ui: &mut egui::Ui, marker: &str, text: LayoutJob, c: &theme::Colors) {
    ui.horizontal_top(|ui| {
        ui.add_space(8.0);
        ui.add_sized(
            [18.0, 18.0],
            egui::Label::new(RichText::new(marker).size(14.0).color(c.muted)),
        );
        ui.add(egui::Label::new(text).wrap());
    });
}

/// Guide text with `**bold**` and `*italic*` markers. egui has no bold face by default,
/// so bold text is shown in the strongest text colour.
fn rich(text: &str, size: f32, color: Color32, c: &theme::Colors) -> LayoutJob {
    let mut job = LayoutJob::default();
    let (mut bold, mut italic) = (false, false);
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("**") {
            bold = !bold;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('*') {
            italic = !italic;
            rest = r;
        } else {
            let end = rest.find('*').unwrap_or(rest.len());
            let format = TextFormat {
                font_id: FontId::proportional(size),
                color: if bold { c.text } else { color },
                italics: italic,
                ..Default::default()
            };
            job.append(&rest[..end], 0.0, format);
            rest = &rest[end..];
        }
    }
    job
}

fn browse_tool(d: &mut SettingsDraft, name: &str) {
    let current = if name == "ffmpeg" {
        &d.ffmpeg
    } else {
        &d.ffprobe
    };
    let mut dialog = rfd::FileDialog::new().set_title(format!("Select {name}"));
    if cfg!(windows) {
        dialog = dialog
            .add_filter("Executable", &["exe"])
            .add_filter("All files", &["*"]);
    }
    if let Some(dir) = Path::new(current.trim().trim_matches('"'))
        .parent()
        .filter(|d| d.is_dir())
    {
        dialog = dialog.set_directory(dir);
    }
    let Some(p) = dialog.pick_file() else { return };
    if name == "ffmpeg" {
        d.ffmpeg = p.to_string_lossy().into_owned();
        // Choosing ffmpeg also fills in ffprobe from the same folder.
        if d.ffprobe.trim().is_empty() {
            let sibling = p.with_file_name(if cfg!(windows) {
                "ffprobe.exe"
            } else {
                "ffprobe"
            });
            if sibling.is_file() {
                d.ffprobe = sibling.to_string_lossy().into_owned();
            }
        }
    } else {
        d.ffprobe = p.to_string_lossy().into_owned();
    }
}

/// Open a folder in Explorer, or a web or mail link in the default program.
fn open_path(target: &str) {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";
    let _ = std::process::Command::new(program).arg(target).spawn();
}
