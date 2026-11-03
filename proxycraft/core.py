"""ProxyCraft engine: FFmpeg detection, media probing and parallel proxy encoding.

This module has no Qt dependency. The window runs ProxyBatch on a worker
thread and receives events through a callback.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable, Optional

SUPPORTED_EXTENSIONS = {
    ".mp4", ".mov", ".m4v", ".mkv", ".avi", ".mxf", ".mts", ".m2ts", ".ts",
    ".webm", ".mpg", ".mpeg", ".3gp", ".wmv", ".flv", ".dv",
}

DEFAULT_SUFFIX = "_Proxy"
PROXIES_FOLDER = "Proxies"

# Where proxies are written.
DEST_PROXIES_FOLDER = "proxies_folder"   # <source folder>/Proxies/
DEST_SAME_FOLDER = "same_folder"         # next to the source file
DEST_CUSTOM = "custom"                   # one folder for the whole batch

# Codecs NVIDIA's decoder can handle. Others (ProRes, DNxHR, CineForm…)
# always decode on the CPU. Unsupported variants of these, such as some
# 4:2:2 files on older GPUs, fall back to the CPU automatically.
GPU_DECODABLE = {
    "h264", "hevc", "av1", "vp9", "vp8", "mpeg2video", "mpeg1video",
    "mpeg4", "vc1", "mjpeg",
}


@dataclass(frozen=True)
class Preset:
    key: str
    label: str
    family: str          # "prores", "h264" or "h265"
    divisor: int         # 2 = half size, 4 = quarter size
    profile: str = ""    # ProRes profile number

    @property
    def extension(self) -> str:
        return ".mov" if self.family == "prores" else ".mp4"

    @property
    def container(self) -> str:
        return "mov" if self.family == "prores" else "mp4"


PRESETS = [
    Preset("prores_proxy_quarter", "ProRes 422 Proxy, ¼ size", "prores", 4, "0"),
    Preset("prores_proxy_half", "ProRes 422 Proxy, ½ size", "prores", 2, "0"),
    Preset("prores_lt_half", "ProRes 422 LT, ½ size", "prores", 2, "1"),
    Preset("h264_quarter", "H.264, ¼ size", "h264", 4),
    Preset("h264_half", "H.264, ½ size", "h264", 2),
    Preset("h265_quarter", "H.265, ¼ size", "h265", 4),
    Preset("h265_half", "H.265, ½ size", "h265", 2),
]
PRESETS_BY_KEY = {p.key: p for p in PRESETS}


def hidden_process_kwargs(low_priority: bool = False) -> dict:
    """Hide FFmpeg console windows on Windows; optionally lower the priority."""
    if os.name != "nt":
        return {}
    startupinfo = subprocess.STARTUPINFO()
    startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startupinfo.wShowWindow = subprocess.SW_HIDE
    flags = subprocess.CREATE_NO_WINDOW
    if low_priority:
        # Keeps an editor responsive; costs nothing when the PC is idle.
        flags |= subprocess.BELOW_NORMAL_PRIORITY_CLASS
    return {"startupinfo": startupinfo, "creationflags": flags}


def _run(cmd: list[str], timeout: float = 30) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8",
                          errors="replace", timeout=timeout, **hidden_process_kwargs())


def clean_error(text: str) -> str:
    """Drop FFmpeg's "[mov,mp4 @ 0x…]" prefixes so messages read plainly."""
    lines = [re.sub(r"^\[[^\]]*@\s*[0-9a-fx]+\]\s*", "", l).strip() for l in text.splitlines()]
    return "\n".join(l for l in lines if l)


def format_time(seconds: float) -> str:
    seconds = max(0, int(round(seconds)))
    return f"{seconds // 3600:02d}:{seconds % 3600 // 60:02d}:{seconds % 60:02d}"


# --------------------------------------------------------------------------
# Media information
# --------------------------------------------------------------------------

@dataclass
class MediaInfo:
    path: Path
    width: int = 0          # coded size, before any rotation flag
    height: int = 0
    fps: float = 0.0
    duration: float = 0.0
    frames: int = 0
    codec: str = ""
    pix_fmt: str = ""
    bits: int = 8
    rotation: int = 0
    audio_codecs: list = field(default_factory=list)
    color: dict = field(default_factory=dict)

    @property
    def is_10bit(self) -> bool:
        return self.bits >= 10 or bool(
            re.search(r"(?:10|12|14|16)(?:le|be)$", self.pix_fmt.lower())
        )

    def proxy_size(self, divisor: int) -> tuple[int, int]:
        """Proxy dimensions, rounded to even numbers as the encoders require."""
        w = max(2, int(round(self.width / divisor / 2)) * 2)
        h = max(2, int(round(self.height / divisor / 2)) * 2)
        return w, h


def _number(value, default=0.0) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _fraction(value) -> float:
    try:
        a, b = str(value).split("/")
        return float(a) / float(b) if float(b) else 0.0
    except (TypeError, ValueError):
        return _number(value)


def probe_media(ffprobe: str, path) -> MediaInfo:
    path = Path(path)
    p = _run([ffprobe, "-v", "error", "-show_format", "-show_streams", "-of", "json", str(path)])
    if p.returncode != 0:
        raise RuntimeError(clean_error(p.stderr) or f"Could not read {path.name}.")
    data = json.loads(p.stdout or "{}")
    streams = data.get("streams", [])
    # Skip cover art and thumbnails, which ffprobe reports as video streams.
    video = next((s for s in streams if s.get("codec_type") == "video"
                  and not (s.get("disposition") or {}).get("attached_pic")), None)
    if not video:
        raise RuntimeError("No video stream found.")

    fps = _fraction(video.get("avg_frame_rate")) or _fraction(video.get("r_frame_rate"))
    fmt = data.get("format", {})
    duration = _number(video.get("duration")) or _number(fmt.get("duration"))
    frames = int(_number(video.get("nb_frames")))
    if frames <= 0:
        frames = int(round(duration * fps))

    rotation = _number((video.get("tags") or {}).get("rotate"))
    for side_data in video.get("side_data_list") or []:
        if "rotation" in side_data:
            rotation = _number(side_data.get("rotation"))

    color = {
        key: video.get(src) for key, src in (
            ("color_primaries", "color_primaries"), ("color_trc", "color_transfer"),
            ("colorspace", "color_space"), ("color_range", "color_range"))
        if video.get(src) and video.get(src) != "unknown"
    }
    return MediaInfo(
        path=path,
        width=int(video.get("width") or 0),
        height=int(video.get("height") or 0),
        fps=fps,
        duration=duration,
        frames=max(1, frames),
        codec=(video.get("codec_name") or "").lower(),
        pix_fmt=video.get("pix_fmt") or "",
        bits=int(_number(video.get("bits_per_raw_sample"), 8.0) or 8),
        rotation=int(round(rotation)) % 360,
        audio_codecs=[(s.get("codec_name") or "").lower()
                      for s in streams if s.get("codec_type") == "audio"],
        color=color,
    )


# --------------------------------------------------------------------------
# FFmpeg capabilities
# --------------------------------------------------------------------------

HW_LABELS = {"nvenc": "NVIDIA NVENC", "qsv": "Intel Quick Sync", "amf": "AMD AMF"}


def encoder_label(codec: str) -> str:
    for suffix, label in HW_LABELS.items():
        if codec.endswith("_" + suffix):
            return label
    return "CPU"


@dataclass
class Capabilities:
    ok: bool = False
    version: str = ""
    details: str = ""
    encoders: set = field(default_factory=set)
    hwaccels: set = field(default_factory=set)
    # Hardware encoders that really opened on this PC, best first.
    working: dict = field(default_factory=lambda: {"h264": [], "h265": []})
    # NVIDIA decode + resize + encode without copying frames to the CPU.
    cuda_pipeline: bool = False

    def encoder(self, family: str) -> str:
        """Best encoder for CPU-decoded frames."""
        if self.working.get(family):
            return self.working[family][0]
        return "libx264" if family == "h264" else "libx265"

    def summary(self) -> str:
        if not self.ok:
            return self.details or "FFmpeg not available."
        parts = []
        if self.cuda_pipeline:
            parts.append("Decode and resize: NVIDIA GPU, CPU fallback")
        else:
            parts.append("Decode and resize: CPU")
        for family, name in (("h264", "H.264"), ("h265", "H.265")):
            codec = self.encoder(family)
            parts.append(f"{name}: {encoder_label(codec)} ({codec})")
        parts.append("ProRes: CPU (prores_ks)")
        return "\n".join(parts)


_CAPS_CACHE: dict = {}

_HW_CANDIDATES = {
    "h264": ("h264_nvenc", "h264_qsv", "h264_amf"),
    "h265": ("hevc_nvenc", "hevc_qsv", "hevc_amf"),
}


def _opens(ffmpeg: str, args: list[str]) -> bool:
    try:
        return _run([ffmpeg, "-hide_banner", "-loglevel", "error", *args, "-f", "null", "-"],
                    timeout=30).returncode == 0
    except Exception:
        return False


def detect_capabilities(ffmpeg: str, refresh: bool = False) -> Capabilities:
    """Find which encoders and decoders actually work on this PC.

    FFmpeg builds list every vendor's hardware encoders whatever GPU is
    fitted, so each one is test-opened. A successful result is cached.
    """
    if not refresh and ffmpeg in _CAPS_CACHE:
        return _CAPS_CACHE[ffmpeg]
    caps = Capabilities()
    if not ffmpeg or not os.path.isfile(ffmpeg):
        caps.details = "FFmpeg executable was not found."
        return caps
    try:
        ver = _run([ffmpeg, "-version"])
        if ver.returncode != 0:
            caps.details = ver.stderr.strip() or "FFmpeg could not be started."
            return caps
        caps.version = (ver.stdout.splitlines() or ["FFmpeg"])[0]
        for line in _run([ffmpeg, "-hide_banner", "-encoders"]).stdout.splitlines():
            m = re.match(r"\s*V\S*\s+(\S+)", line)
            if m:
                caps.encoders.add(m.group(1))
        caps.hwaccels = {
            line.strip() for line in _run([ffmpeg, "-hide_banner", "-hwaccels"]).stdout.splitlines()[1:]
            if line.strip()
        }
    except Exception as exc:
        caps.details = f"FFmpeg detection failed: {exc}"
        return caps

    test_input = ["-f", "lavfi", "-i", "color=black:size=1280x720:rate=25", "-frames:v", "3"]
    for family, candidates in _HW_CANDIDATES.items():
        caps.working[family] = [
            c for c in candidates
            if c in caps.encoders and _opens(ffmpeg, [*test_input, "-c:v", c, "-pix_fmt", "yuv420p"])
        ]
    if "cuda" in caps.hwaccels and "h264_nvenc" in caps.working["h264"]:
        caps.cuda_pipeline = _opens(ffmpeg, [
            *test_input, "-vf",
            "hwupload_cuda,scale_cuda=640:360:format=nv12,hwdownload,format=nv12",
        ])
    caps.ok = True
    caps.details = caps.version
    _CAPS_CACHE[ffmpeg] = caps
    return caps


# --------------------------------------------------------------------------
# Commands
# --------------------------------------------------------------------------

def _video_args(preset: Preset, codec: str, info: MediaInfo, gpu_frames: bool) -> list[str]:
    """Encoder settings tuned for proxies: fast to make and easy to scrub."""
    if preset.family == "prores":
        return ["-c:v", "prores_ks", "-profile:v", preset.profile,
                "-pix_fmt", "yuv422p10le", "-vendor", "apl0"]

    # A keyframe every second and no B-frames make proxies fast to decode
    # and quick to scrub in an editor.
    gop = str(max(1, int(round(info.fps or 25))))
    ten_bit = preset.family == "h265" and info.is_10bit
    q = "23" if preset.family == "h264" else "26"
    args = ["-c:v", codec]
    if codec.endswith("_nvenc"):
        args += ["-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", q, "-b:v", "0"]
    elif codec.endswith("_qsv"):
        args += ["-preset", "veryfast", "-global_quality", q]
    elif codec.endswith("_amf"):
        args += ["-quality", "speed", "-rc", "cqp", "-qp_i", q, "-qp_p", str(int(q) + 2)]
    elif codec == "libx264":
        args += ["-preset", "veryfast", "-crf", q]
    else:
        args += ["-preset", "faster", "-crf", q, "-x265-params", "log-level=error"]
    args += ["-g", gop, "-bf", "0"]
    if preset.family == "h264":
        args += ["-profile:v", "high"]
    elif ten_bit:
        args += ["-profile:v", "main10"]
    if not gpu_frames:
        if not ten_bit:
            pix = "yuv420p"
        else:
            pix = "yuv420p10le" if codec.startswith("lib") else "p010le"
        args += ["-pix_fmt", pix]
    return args


def _audio_args(audio_codecs: list[str], container: str) -> list[str]:
    """Copy each audio track when the container accepts it, else convert it.

    MOV (ProRes) gets PCM, which every editor reads; MP4 gets AAC.
    """
    args = []
    for i, codec in enumerate(audio_codecs):
        if container == "mov":
            copy = codec in {"aac", "alac", "mp3", "ac3", "eac3"} or codec.startswith("pcm_")
            args += [f"-c:a:{i}", "copy"] if copy else [f"-c:a:{i}", "pcm_s16le"]
        else:
            copy = codec in {"aac", "alac", "mp3", "ac3", "eac3"}
            args += [f"-c:a:{i}", "copy"] if copy else [f"-c:a:{i}", "aac", f"-b:a:{i}", "192k"]
    return args


def build_command(ffmpeg: str, info: MediaInfo, preset: Preset, output: Path,
                  caps: Capabilities, use_gpu: bool) -> list[str]:
    w, h = info.proxy_size(preset.divisor)
    # -noautorotate keeps the proxy's frame layout identical to the original
    # and preserves its rotation flag, so editors relink it reliably.
    cmd = [ffmpeg, "-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-noautorotate"]
    if use_gpu:
        cmd += ["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"]
    cmd += ["-i", str(info.path),
            "-map", "0:v:0", "-map", "0:a?", "-map_metadata", "0",
            # Keep every source frame and timestamp, so proxy and original
            # have the same frame count and timecode.
            "-fps_mode", "passthrough"]
    if use_gpu:
        fmt = "p010le" if info.is_10bit and preset.family != "h264" else "nv12"
        vf = f"scale_cuda={w}:{h}:format={fmt}"
        if preset.family == "prores":
            vf += f",hwdownload,format={fmt}"
            codec = "prores_ks"
        else:
            codec = "h264_nvenc" if preset.family == "h264" else "hevc_nvenc"
        gpu_frames = preset.family != "prores"
    else:
        # "area" is the fastest good-quality filter for 2× and 4× reductions.
        vf = f"scale={w}:{h}:flags=area"
        codec = "prores_ks" if preset.family == "prores" else caps.encoder(preset.family)
        gpu_frames = False
    cmd += ["-vf", vf, *_video_args(preset, codec, info, gpu_frames)]
    for key, value in info.color.items():
        cmd += [f"-{key}", value]
    cmd += [*_audio_args(info.audio_codecs, preset.container),
            "-f", preset.container, str(output),
            "-progress", "pipe:1", "-nostats"]
    return cmd


# --------------------------------------------------------------------------
# Planning
# --------------------------------------------------------------------------

def is_proxy_file(path: Path, suffix: str = DEFAULT_SUFFIX) -> bool:
    return path.stem.endswith(suffix) or path.parent.name.lower() == PROXIES_FOLDER.lower()


def collect_media(folder: Path, recursive: bool, suffix: str = DEFAULT_SUFFIX) -> list[Path]:
    """Video files in a folder, skipping existing proxies and Proxies folders."""
    pattern = folder.rglob("*") if recursive else folder.iterdir()
    found = []
    for p in pattern:
        try:
            if (p.is_file() and p.suffix.lower() in SUPPORTED_EXTENSIONS
                    and not is_proxy_file(p, suffix)):
                found.append(p)
        except OSError:
            continue
    return sorted(found, key=lambda p: str(p).lower())


def plan_outputs(sources: list[Path], preset: Preset, mode: str, custom_dir: str = "",
                 suffix: str = DEFAULT_SUFFIX) -> list[Path]:
    """Proxy path for each source; names are made unique within the batch."""
    used = set()
    outputs = []
    for src in sources:
        if mode == DEST_CUSTOM and custom_dir:
            folder = Path(custom_dir)
        elif mode == DEST_SAME_FOLDER:
            folder = src.parent
        else:
            folder = src.parent / PROXIES_FOLDER
        out = folder / f"{src.stem}{suffix}{preset.extension}"
        n = 2
        # Two sources with the same name sent to one custom folder.
        while str(out).lower() in used:
            out = folder / f"{src.stem}{suffix} ({n}){preset.extension}"
            n += 1
        used.add(str(out).lower())
        outputs.append(out)
    return outputs


def auto_parallel_jobs() -> int:
    """Two files at once was fastest in testing for both GPU and CPU pipelines."""
    return 2 if (os.cpu_count() or 4) >= 4 else 1


# --------------------------------------------------------------------------
# Batch encoding
# --------------------------------------------------------------------------

@dataclass
class Job:
    source: Path
    output: Path
    info: Optional[MediaInfo] = None
    status: str = "Waiting"
    message: str = ""
    frames_done: int = 0

    @property
    def pixels_per_frame(self) -> int:
        # Decoding dominates proxy speed and scales with the source frame size,
        # so one 4K frame counts as about four 1080p frames of work.
        return max(1, self.info.width * self.info.height) if self.info else 1


class TimeEstimator:
    """Estimate the time left from recent encoding speed.

    Work is measured in source pixels (frames × frame size), so batches that mix
    4K and HD files are estimated correctly. Speed comes from the last
    `window` seconds, so the estimate follows changes such as a slower file
    starting, and the shown value is smoothed so it does not jump around.
    Skipped and failed files are left out of the speed entirely.
    """

    def __init__(self, window: float = 30.0, min_span: float = 3.0):
        self.window = window
        self.min_span = min_span
        self.samples: list[tuple[float, float, int]] = []  # time, work done, frames done
        self.smoothed_eta: Optional[float] = None

    def update(self, now: float, work_done: float, frames_done: int, work_left: float):
        """Return (eta_seconds or None while measuring, frames per second)."""
        if not self.samples and work_done <= 0:
            # Start measuring at the first encoded frame, so FFmpeg and GPU
            # start-up time is not mistaken for slow encoding.
            return None, 0.0
        self.samples.append((now, work_done, frames_done))
        while len(self.samples) > 2 and now - self.samples[1][0] >= self.window:
            self.samples.pop(0)
        t0, w0, f0 = self.samples[0]
        span = now - t0
        if span < self.min_span or work_done <= w0:
            return self.smoothed_eta, 0.0
        rate = (work_done - w0) / span
        fps = (frames_done - f0) / span
        eta = work_left / rate if rate > 0 else None
        if eta is not None:
            # Light smoothing: follows real changes within a few seconds.
            if self.smoothed_eta is None:
                self.smoothed_eta = eta
            else:
                self.smoothed_eta = 0.7 * self.smoothed_eta + 0.3 * eta
            self.smoothed_eta = min(self.smoothed_eta, eta * 1.5)  # never far above the raw value
        if work_left <= 0:
            self.smoothed_eta = 0.0
        return self.smoothed_eta, fps


class ProxyBatch:
    """Encode a list of jobs, several at a time.

    Events passed to `callback(event, data)`, from worker threads:
      job_status    index, status, message
      job_progress  index, percent, fps
      progress      percent, fps, eta (seconds, or None while measuring),
                    done, total (files finished or skipped / files in the batch)
      finished      created, skipped, failed, cancelled, elapsed
    """

    def __init__(self, ffmpeg: str, ffprobe: str, caps: Capabilities, jobs: list[Job],
                 preset: Preset, parallel: int = 0, skip_existing: bool = True,
                 low_priority: bool = True, callback: Optional[Callable] = None):
        self.ffmpeg = ffmpeg
        self.ffprobe = ffprobe
        self.caps = caps
        self.jobs = jobs
        self.preset = preset
        self.parallel = max(1, parallel or auto_parallel_jobs())
        self.skip_existing = skip_existing
        self.low_priority = low_priority
        self.callback = callback or (lambda event, data: None)
        self._stop = threading.Event()
        self._lock = threading.Lock()
        self._procs = set()
        self._start = 0.0
        self._last_emit = 0.0
        self._estimator = TimeEstimator()

    def cancel(self):
        self._stop.set()
        with self._lock:
            for proc in list(self._procs):
                try:
                    proc.kill()
                except Exception:
                    pass

    @property
    def cancelled(self) -> bool:
        return self._stop.is_set()

    def _emit(self, event, **data):
        self.callback(event, data)

    def _status(self, index, status, message=""):
        job = self.jobs[index]
        job.status, job.message = status, message
        self._emit("job_status", index=index, status=status, message=message)

    def _emit_progress(self, force=False):
        now = time.perf_counter()
        if not force and now - self._last_emit < 0.25:
            return
        with self._lock:
            self._last_emit = now
            # Encoding work only: skipped and failed files neither speed up nor
            # hold back the estimate.
            active = [j for j in self.jobs if j.info and j.status not in ("Skipped", "Failed", "Cancelled")]
            work_total = sum(j.info.frames * j.pixels_per_frame for j in active)
            work_done = sum(min(j.frames_done, j.info.frames) * j.pixels_per_frame for j in active)
            frames_done = sum(min(j.frames_done, j.info.frames) for j in active)
            eta, fps = self._estimator.update(now, work_done, frames_done, work_total - work_done)
            finished = sum(1 for j in self.jobs if j.status in ("Done", "Skipped", "Failed", "Cancelled"))
        # The bar shows encoding progress; with nothing left to encode it is full.
        percent = 100.0 if work_total == 0 else min(100.0, work_done * 100.0 / work_total)
        self._emit("progress", percent=percent, fps=fps, eta=eta,
                   done=finished, total=len(self.jobs))

    def run(self):
        self._start = time.perf_counter()
        for index, job in enumerate(self.jobs):
            if self.cancelled:
                break
            if job.info is None:
                try:
                    job.info = probe_media(self.ffprobe, job.source)
                except Exception as exc:
                    self._status(index, "Failed", str(exc).splitlines()[0] if str(exc) else "Unreadable file.")
        # Mark files with an existing proxy before encoding starts, so the time
        # estimate never counts them as work still to do.
        if self.skip_existing:
            for index, job in enumerate(self.jobs):
                if job.info and job.status != "Failed" and job.output.exists():
                    job.frames_done = job.info.frames
                    self._status(index, "Skipped",
                                 "A proxy with this name already exists. Turn off "
                                 "Skip existing proxies to replace it.")
        self._emit_progress(force=True)

        with ThreadPoolExecutor(max_workers=self.parallel) as pool:
            list(pool.map(self._run_job, range(len(self.jobs))))

        created = sum(1 for j in self.jobs if j.status == "Done")
        skipped = sum(1 for j in self.jobs if j.status == "Skipped")
        failed = sum(1 for j in self.jobs if j.status == "Failed")
        self._emit_progress(force=True)
        self._emit("finished", created=created, skipped=skipped, failed=failed,
                   cancelled=self.cancelled, elapsed=time.perf_counter() - self._start)

    def _run_job(self, index: int):
        job = self.jobs[index]
        if job.info is None or job.status in ("Failed", "Skipped"):
            return
        if self.cancelled:
            self._status(index, "Cancelled")
            return
        try:
            job.output.parent.mkdir(parents=True, exist_ok=True)
        except OSError as exc:
            self._status(index, "Failed", f"Cannot create the destination folder: {exc}")
            return

        # Write to a temporary name and rename on success, so a cancelled or
        # failed proxy never looks finished.
        partial = job.output.with_name(job.output.name + ".part")
        use_gpu = self.caps.cuda_pipeline and job.info.codec in GPU_DECODABLE
        attempts = [True, False] if use_gpu else [False]
        error = ""
        for gpu in attempts:
            if self.cancelled:
                break
            job.frames_done = 0
            self._status(index, "Encoding", "GPU" if gpu else "CPU")
            cmd = build_command(self.ffmpeg, job.info, self.preset, partial, self.caps, gpu)
            ok, error = self._run_ffmpeg(index, cmd)
            if ok:
                try:
                    os.replace(partial, job.output)
                except OSError as exc:
                    error = f"Could not save the proxy: {exc}"
                    break
                job.frames_done = job.info.frames
                self._status(index, "Done", "GPU" if gpu else "CPU")
                self._emit_progress(force=True)
                return
        try:
            partial.unlink(missing_ok=True)
        except OSError:
            pass
        job.frames_done = 0
        if self.cancelled:
            self._status(index, "Cancelled")
        else:
            self._status(index, "Failed", error or "FFmpeg failed.")
        self._emit_progress(force=True)

    def _run_ffmpeg(self, index: int, cmd: list[str]) -> tuple[bool, str]:
        job = self.jobs[index]
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                stdin=subprocess.DEVNULL, text=True, encoding="utf-8",
                                errors="replace", bufsize=1,
                                **hidden_process_kwargs(self.low_priority))
        with self._lock:
            self._procs.add(proc)
        if self.cancelled:
            proc.kill()
        errors: list[str] = []
        reader = threading.Thread(target=lambda: errors.extend(proc.stderr), daemon=True)
        reader.start()
        fps = 0.0
        try:
            for line in proc.stdout:
                key, _, value = line.strip().partition("=")
                if key == "frame":
                    job.frames_done = int(_number(value))
                elif key == "fps":
                    fps = _number(value)
                elif key == "progress":
                    percent = min(100.0, job.frames_done * 100.0 / max(1, job.info.frames))
                    self._emit("job_progress", index=index, percent=percent, fps=fps)
                    self._emit_progress()
            proc.wait()
        finally:
            reader.join(timeout=2)
            with self._lock:
                self._procs.discard(proc)
        if proc.returncode == 0 and not self.cancelled:
            return True, ""
        text = clean_error("".join(errors)).splitlines()
        return False, "\n".join(text[-8:]) or f"FFmpeg exited with code {proc.returncode}."
