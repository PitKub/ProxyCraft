# Changelog

## 1.0.1

- **GPU decoding on Intel and AMD.** Files are now decoded on any GPU that supports Windows D3D11 video decoding, not only NVIDIA. This covers H.264, H.265, AV1, VP9, MPEG-2 and VC-1 in 8-bit or 10-bit 4:2:0. Resizing stays on the CPU. ProxyCraft tests that GPU decoding really works when it starts. If it fails for a file, that file is retried on the CPU automatically.
- **Proxy check.** After each proxy is made, ProxyCraft compares it with its original: length (frame count, or duration when the original does not store a frame count), frame rate, timecode, number of audio tracks, rotation and picture shape. The results are shown in a new **Check** column, and the details appear when you hover over a result. Existing proxies that are skipped are checked too. You can turn this off with **Check each proxy against its original**.
- **Software encoder fallback.** If a hardware H.264 or H.265 encoder (NVENC, Quick Sync or AMF) fails on a file, for example 10-bit H.265 on an older GPU, that file is made again with x264 or x265 on the CPU instead of failing. The status shows "CPU, x264" or "CPU, x265".
- **Tools → Check Proxies** (Ctrl+K) checks the proxies you already have for the files in the list, without encoding anything.

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
