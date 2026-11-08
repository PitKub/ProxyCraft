//! Encoding a list of files, several at a time.

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::caps::Capabilities;
use super::command::{attempt_label, build_command, encode_attempts};
use super::media::{check_proxy, probe_media, MediaInfo, ProxyCheck};
use super::plan::auto_parallel_jobs;
use super::process::command;
use super::{clean_error, Preset};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Encoding,
    Done,
    Skipped,
    Failed,
    Cancelled,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Waiting => "Waiting",
            Status::Encoding => "Encoding",
            Status::Done => "Done",
            Status::Skipped => "Skipped",
            Status::Failed => "Failed",
            Status::Cancelled => "Cancelled",
        }
    }

    fn finished(self) -> bool {
        matches!(
            self,
            Status::Done | Status::Skipped | Status::Failed | Status::Cancelled
        )
    }

    fn excluded_from_work(self) -> bool {
        matches!(self, Status::Skipped | Status::Failed | Status::Cancelled)
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub source: PathBuf,
    pub output: PathBuf,
    pub info: Option<MediaInfo>,
    pub status: Status,
    pub message: String,
    pub frames_done: u64,
    pub check: Option<ProxyCheck>,
}

impl Job {
    pub fn new(source: PathBuf, output: PathBuf, info: Option<MediaInfo>) -> Job {
        Job {
            source,
            output,
            info,
            status: Status::Waiting,
            message: String::new(),
            frames_done: 0,
            check: None,
        }
    }

    /// Decoding dominates proxy speed and scales with the source frame size, so one 4K
    /// frame counts as about four 1080p frames of work.
    pub fn pixels_per_frame(&self) -> u64 {
        self.info
            .as_ref()
            .map(|i| (i.width as u64 * i.height as u64).max(1))
            .unwrap_or(1)
    }
}

/// Estimate the time left from recent encoding speed.
///
/// Work is measured in source pixels (frames × frame size), so batches that mix 4K and HD
/// files are estimated correctly. Speed comes from the last `window` seconds, so the estimate
/// follows changes such as a slower file starting, and the shown value is smoothed so it does
/// not jump around. Skipped and failed files are left out of the speed entirely.
pub struct TimeEstimator {
    window: f64,
    min_span: f64,
    samples: Vec<(f64, f64, u64)>, // time, work done, frames done
    smoothed_eta: Option<f64>,
}

impl Default for TimeEstimator {
    fn default() -> Self {
        TimeEstimator::new(30.0, 3.0)
    }
}

impl TimeEstimator {
    pub fn new(window: f64, min_span: f64) -> TimeEstimator {
        TimeEstimator {
            window,
            min_span,
            samples: Vec::new(),
            smoothed_eta: None,
        }
    }

    /// (eta seconds, None while measuring; frames per second)
    pub fn update(
        &mut self,
        now: f64,
        work_done: f64,
        frames_done: u64,
        work_left: f64,
    ) -> (Option<f64>, f64) {
        if self.samples.is_empty() && work_done <= 0.0 {
            // Start measuring at the first encoded frame, so FFmpeg and GPU start-up time is
            // not mistaken for slow encoding.
            return (None, 0.0);
        }
        self.samples.push((now, work_done, frames_done));
        while self.samples.len() > 2 && now - self.samples[1].0 >= self.window {
            self.samples.remove(0);
        }
        let (t0, w0, f0) = self.samples[0];
        let span = now - t0;
        if span < self.min_span || work_done <= w0 {
            return (self.smoothed_eta, 0.0);
        }
        let rate = (work_done - w0) / span;
        let fps = frames_done.saturating_sub(f0) as f64 / span;
        let eta = if rate > 0.0 {
            Some(work_left / rate)
        } else {
            None
        };
        if let Some(eta) = eta {
            // Light smoothing: follows real changes within a few seconds.
            let s = match self.smoothed_eta {
                None => eta,
                Some(prev) => 0.7 * prev + 0.3 * eta,
            };
            self.smoothed_eta = Some(s.min(eta * 1.5)); // never far above the raw value
        }
        if work_left <= 0.0 {
            self.smoothed_eta = Some(0.0);
        }
        (self.smoothed_eta, fps)
    }
}

/// Events from the batch, sent from worker threads.
#[derive(Clone, Debug)]
pub enum Event {
    JobStatus {
        index: usize,
        status: Status,
        message: String,
        output: PathBuf,
    },
    JobProgress {
        index: usize,
        percent: f64,
        fps: f64,
    },
    JobCheck {
        index: usize,
        check: ProxyCheck,
    },
    Progress {
        percent: f64,
        fps: f64,
        eta: Option<f64>,
        done: usize,
        total: usize,
    },
    Finished {
        created: usize,
        skipped: usize,
        failed: usize,
        cancelled: bool,
        elapsed: f64,
        checked: usize,
        check_problems: usize,
    },
}

pub struct BatchOptions {
    pub parallel: usize,
    pub skip_existing: bool,
    pub low_priority: bool,
    pub verify: bool,
}

/// Encode a list of jobs, several at a time. Events go to `sink`.
pub struct ProxyBatch {
    ffmpeg: String,
    ffprobe: String,
    caps: Capabilities,
    preset: Preset,
    opts: BatchOptions,
    pub parallel: usize,
    jobs: Mutex<Vec<Job>>,
    stop: AtomicBool,
    procs: Mutex<Vec<Arc<Mutex<Child>>>>,
    estimator: Mutex<(TimeEstimator, f64)>, // estimator, last emit time
    start: Instant,
    sink: Box<dyn Fn(Event) + Send + Sync>,
}

impl ProxyBatch {
    pub fn new(
        ffmpeg: &str,
        ffprobe: &str,
        caps: Capabilities,
        jobs: Vec<Job>,
        preset: Preset,
        opts: BatchOptions,
        sink: Box<dyn Fn(Event) + Send + Sync>,
    ) -> ProxyBatch {
        let parallel = if opts.parallel == 0 {
            auto_parallel_jobs()
        } else {
            opts.parallel
        }
        .max(1);
        ProxyBatch {
            ffmpeg: ffmpeg.into(),
            ffprobe: ffprobe.into(),
            caps,
            preset,
            opts,
            parallel,
            jobs: Mutex::new(jobs),
            stop: AtomicBool::new(false),
            procs: Mutex::new(Vec::new()),
            estimator: Mutex::new((TimeEstimator::default(), f64::NEG_INFINITY)),
            start: Instant::now(),
            sink,
        }
    }

    pub fn cancel(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(procs) = self.procs.lock() {
            for p in procs.iter() {
                if let Ok(mut c) = p.lock() {
                    let _ = c.kill();
                }
            }
        }
    }

    pub fn cancelled(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub fn jobs(&self) -> Vec<Job> {
        self.jobs.lock().map(|j| j.clone()).unwrap_or_default()
    }

    fn set_status(&self, index: usize, status: Status, message: &str) {
        let output = {
            let mut jobs = self.jobs.lock().unwrap();
            jobs[index].status = status;
            jobs[index].message = message.to_string();
            jobs[index].output.clone()
        };
        (self.sink)(Event::JobStatus {
            index,
            status,
            message: message.to_string(),
            output,
        });
    }

    fn emit_progress(&self, force: bool) {
        let now = self.start.elapsed().as_secs_f64();
        let mut est = self.estimator.lock().unwrap();
        if !force && now - est.1 < 0.25 {
            return;
        }
        est.1 = now;
        let (work_total, work_done, frames_done, finished, total) = {
            let jobs = self.jobs.lock().unwrap();
            // Encoding work only: skipped and failed files neither speed up nor hold back the estimate.
            let mut wt = 0.0;
            let mut wd = 0.0;
            let mut fd = 0u64;
            for j in jobs
                .iter()
                .filter(|j| j.info.is_some() && !j.status.excluded_from_work())
            {
                let frames = j.info.as_ref().unwrap().frames;
                let px = j.pixels_per_frame() as f64;
                wt += frames as f64 * px;
                wd += j.frames_done.min(frames) as f64 * px;
                fd += j.frames_done.min(frames);
            }
            (
                wt,
                wd,
                fd,
                jobs.iter().filter(|j| j.status.finished()).count(),
                jobs.len(),
            )
        };
        let (eta, fps) = est
            .0
            .update(now, work_done, frames_done, work_total - work_done);
        drop(est);
        // The bar shows encoding progress; with nothing left to encode it is full.
        let percent = if work_total == 0.0 {
            100.0
        } else {
            (work_done * 100.0 / work_total).min(100.0)
        };
        (self.sink)(Event::Progress {
            percent,
            fps,
            eta,
            done: finished,
            total,
        });
    }

    /// Run the whole batch (blocking). Call from a worker thread.
    pub fn run(&self) {
        let count = self.jobs.lock().unwrap().len();
        for index in 0..count {
            if self.cancelled() {
                break;
            }
            let (needs, source) = {
                let j = &self.jobs.lock().unwrap()[index];
                (j.info.is_none(), j.source.clone())
            };
            if needs {
                match probe_media(&self.ffprobe, &source) {
                    Ok(info) => self.jobs.lock().unwrap()[index].info = Some(info),
                    Err(e) => {
                        let first = e.lines().next().unwrap_or("Unreadable file.").to_string();
                        self.set_status(index, Status::Failed, &first);
                    }
                }
            }
        }
        // Mark files with an existing proxy before encoding starts, so the time estimate never
        // counts them as work still to do.
        if self.opts.skip_existing {
            for index in 0..count {
                let skip = {
                    let mut jobs = self.jobs.lock().unwrap();
                    let j = &mut jobs[index];
                    let frames = j.info.as_ref().map(|i| i.frames);
                    if let Some(frames) =
                        frames.filter(|_| j.status != Status::Failed && j.output.exists())
                    {
                        j.frames_done = frames;
                        true
                    } else {
                        false
                    }
                };
                if skip {
                    self.set_status(
                        index,
                        Status::Skipped,
                        "A proxy with this name already exists. Turn off Skip existing proxies to replace it.",
                    );
                }
            }
        }
        self.emit_progress(true);

        let next = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..self.parallel.min(count.max(1)) {
                scope.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= count {
                        break;
                    }
                    self.run_job(i);
                });
            }
        });

        let jobs = self.jobs();
        let checked: Vec<&ProxyCheck> = jobs.iter().filter_map(|j| j.check.as_ref()).collect();
        self.emit_progress(true);
        (self.sink)(Event::Finished {
            created: jobs.iter().filter(|j| j.status == Status::Done).count(),
            skipped: jobs.iter().filter(|j| j.status == Status::Skipped).count(),
            failed: jobs.iter().filter(|j| j.status == Status::Failed).count(),
            cancelled: self.cancelled(),
            elapsed: self.start.elapsed().as_secs_f64(),
            checked: checked.len(),
            check_problems: checked.iter().filter(|c| !c.ok()).count(),
        });
    }

    fn verify(&self, index: usize) {
        let (info, output) = {
            let j = &self.jobs.lock().unwrap()[index];
            (j.info.clone(), j.output.clone())
        };
        let Some(info) = info else { return };
        if self.cancelled() {
            return;
        }
        let check = check_proxy(&self.ffprobe, &info, &output);
        self.jobs.lock().unwrap()[index].check = Some(check.clone());
        (self.sink)(Event::JobCheck { index, check });
    }

    fn run_job(&self, index: usize) {
        let job = self.jobs.lock().unwrap()[index].clone();
        if job.status == Status::Skipped && self.opts.verify {
            // An existing proxy may be from other media or an older version of the clip, so it
            // is checked too.
            self.verify(index);
            return;
        }
        let Some(info) = job.info.clone() else { return };
        if matches!(job.status, Status::Failed | Status::Skipped) {
            return;
        }
        if self.cancelled() {
            self.set_status(index, Status::Cancelled, "");
            return;
        }
        if let Some(parent) = job.output.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                self.set_status(
                    index,
                    Status::Failed,
                    &format!("Cannot create the destination folder: {e}"),
                );
                return;
            }
        }
        // Write to a temporary name and rename on success, so a cancelled or failed proxy never
        // looks finished.
        let mut partial = job.output.clone().into_os_string();
        partial.push(".part");
        let partial = PathBuf::from(partial);
        let mut error = String::new();
        for (decode, encoder) in encode_attempts(&self.caps, &info, &self.preset) {
            if self.cancelled() {
                break;
            }
            self.jobs.lock().unwrap()[index].frames_done = 0;
            let label = attempt_label(decode, &encoder);
            self.set_status(index, Status::Encoding, &label);
            let cmd = build_command(
                &self.ffmpeg,
                &info,
                &self.preset,
                &partial,
                &self.caps,
                decode,
                &encoder,
            );
            let (ok, err) = self.run_ffmpeg(index, &cmd, info.frames);
            error = err;
            if ok {
                if let Err(e) = std::fs::rename(&partial, &job.output) {
                    error = format!("Could not save the proxy: {e}");
                    break;
                }
                self.jobs.lock().unwrap()[index].frames_done = info.frames;
                self.set_status(index, Status::Done, &label);
                self.emit_progress(true);
                if self.opts.verify {
                    self.verify(index);
                }
                return;
            }
        }
        let _ = std::fs::remove_file(&partial);
        self.jobs.lock().unwrap()[index].frames_done = 0;
        if self.cancelled() {
            self.set_status(index, Status::Cancelled, "");
        } else {
            self.set_status(
                index,
                Status::Failed,
                if error.is_empty() {
                    "FFmpeg failed."
                } else {
                    &error
                },
            );
        }
        self.emit_progress(true);
    }

    fn run_ffmpeg(&self, index: usize, cmd: &[String], total_frames: u64) -> (bool, String) {
        let spawned = command(&cmd[0], self.opts.low_priority)
            .args(&cmd[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => return (false, format!("FFmpeg could not be started: {e}")),
        };
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child = Arc::new(Mutex::new(child));
        self.procs.lock().unwrap().push(child.clone());
        if self.cancelled() {
            let _ = child.lock().unwrap().kill();
        }
        let err_thread = std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut e) = stderr {
                let _ = e.read_to_string(&mut text);
            }
            text
        });
        let mut fps = 0.0;
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let (key, value) = line.trim().split_once('=').unwrap_or((line.trim(), ""));
                match key {
                    "frame" => {
                        let f = value.trim().parse::<f64>().unwrap_or(0.0) as u64;
                        self.jobs.lock().unwrap()[index].frames_done = f;
                    }
                    "fps" => fps = value.trim().parse::<f64>().unwrap_or(0.0),
                    "progress" => {
                        let done = self.jobs.lock().unwrap()[index].frames_done;
                        let percent = (done as f64 * 100.0 / total_frames.max(1) as f64).min(100.0);
                        (self.sink)(Event::JobProgress {
                            index,
                            percent,
                            fps,
                        });
                        self.emit_progress(false);
                    }
                    _ => {}
                }
            }
        }
        // Poll instead of wait(): the lock must stay free so cancel() can kill the process.
        let status = loop {
            match child.lock().unwrap().try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) => {}
                Err(e) => break Err(e),
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let errors = err_thread.join().unwrap_or_default();
        self.procs
            .lock()
            .unwrap()
            .retain(|p| !Arc::ptr_eq(p, &child));
        let code = status.as_ref().ok().and_then(|s| s.code());
        if matches!(&status, Ok(s) if s.success()) && !self.cancelled() {
            return (true, String::new());
        }
        let cleaned = clean_error(&errors);
        let lines: Vec<&str> = cleaned.lines().collect();
        let tail = lines[lines.len().saturating_sub(8)..].join("\n");
        if tail.is_empty() {
            (
                false,
                format!(
                    "FFmpeg exited with code {}.",
                    code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                ),
            )
        } else {
            (false, tail)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_waits_for_first_frame_then_follows_speed() {
        let mut e = TimeEstimator::new(30.0, 3.0);
        assert_eq!(e.update(0.0, 0.0, 0, 100.0), (None, 0.0));
        assert_eq!(e.update(1.0, 10.0, 10, 90.0).0, None); // still measuring (span < 3 s)
        let (eta, fps) = e.update(5.0, 50.0, 50, 50.0);
        assert!((eta.unwrap() - 5.0).abs() < 1e-9 && (fps - 10.0).abs() < 1e-9);
        let (eta, _) = e.update(10.0, 100.0, 100, 0.0);
        assert_eq!(eta, Some(0.0));
    }
}
