# Changelog

## 1.0.0 — first public release

- ProRes 422 Proxy (¼ and ½ size), ProRes 422 LT (½ size), H.264 (¼ and ½ size) and H.265 (¼ and ½ size) proxies.
- NVIDIA GPU decoding, resizing and encoding, with automatic CPU fallback per file.
- Intel Quick Sync and AMD AMF encoding when available; every hardware encoder is tested on the PC before use.
- Two files encoded at once by default; adjustable in **Files at once**.
- Proxies keep the original frame count, frame timing, timecode, rotation, color tags and all audio tracks.
- Destination: a Proxies folder next to the originals, the same folder, or one folder for all proxies.
- Skip files that already have a proxy; add folders with subfolders; drag and drop.
- Unfinished proxies are written to a temporary file, so cancelling never leaves broken files.
- Dark and Light themes, User Guide, Donate and About dialogs.
- Windows installer: install for the current user (no administrator rights) or all users, choose the folder, Start menu and optional desktop shortcut, uninstall from Settings → Apps.
- Time-left estimate based on the last 30 seconds of encoding, weighted by frame size, ignoring skipped and failed files, with the expected finishing time.
- FFmpeg 9.0.2 included in the recommended download, so there is nothing to install. A download without FFmpeg is also available.
- Open source under the MIT License.
