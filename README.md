# ProxyCraft — Windows Edition

<img src="assets/ProxyCraft.png" alt="ProxyCraft icon" width="96" align="right">

**Free • Open Source • MIT License**

ProxyCraft is a free, open-source Windows application for creating editing proxies quickly. It makes ProRes, H.264 and H.265 proxies at half or quarter size, using your GPU for decoding, resizing and encoding when it can. It is built for large, high-resolution footage such as 8K, where making proxies inside an editor can take hours.

> **Real-world example:** for the same set of 8K clips, creating proxies took **about 8 hours in FilmCraft** and **17 minutes in ProxyCraft** on the author's PC — roughly 28 times faster.

## Latest release

Download **ProxyCraft 1.0.0 for Windows** from [GitHub Releases](https://github.com/PitKub/ProxyCraft/releases/latest).

## Download and run

From [GitHub Releases](https://github.com/PitKub/ProxyCraft/releases), download one of:

- **ProxyCraft-1.0.0-Setup.exe** — recommended. A Windows installer with **FFmpeg included**. It asks whether to install for you only (no administrator rights; default folder `%LOCALAPPDATA%\Programs\ProxyCraft`) or for all users (`C:\Program Files\ProxyCraft`), and lets you choose a different folder. It adds ProxyCraft to the Start menu, optionally to the desktop, and to **Settings → Apps** for uninstalling.
- **ProxyCraft-1.0.0-Windows.zip** — the portable version with FFmpeg included. Extract the whole ZIP to a folder and run `ProxyCraft.exe`.
- **ProxyCraft-1.0.0-Windows-without-FFmpeg.zip** — the portable version without FFmpeg, for people who already have FFmpeg installed.

**Requirements:** Windows 10 or Windows 11, 64-bit. Everything else ProxyCraft needs, including the Visual C++ runtime and (in the recommended downloads) FFmpeg, is included. An NVIDIA, Intel or AMD GPU makes proxies faster but is not required.

Python is not needed to run the compiled application. In the portable versions, keep the `ffmpeg` folder next to `ProxyCraft.exe`; ProxyCraft finds it there automatically. Settings are kept in the Windows registry, so installing a new version or moving the portable folder keeps them.

Windows SmartScreen may warn about a new, unsigned app. Select **More info → Run anyway** if you downloaded it from this repository.

## Features

- **Seven proxy formats:**
  - ProRes 422 Proxy, ¼ size
  - ProRes 422 Proxy, ½ size
  - ProRes 422 LT, ½ size
  - H.264, ¼ size
  - H.264, ½ size
  - H.265, ¼ size
  - H.265, ½ size
- **Fast on any PC.** ProxyCraft tests which encoders and decoders really work on your computer:
  - With an NVIDIA GPU, video is decoded, resized and encoded on the GPU.
  - Intel Quick Sync and AMD AMF are used for H.264 and H.265 encoding when available.
  - Without a usable GPU, everything runs on the CPU.
- **Several files at once.** Two files are encoded in parallel by default.
- **Reliable time left.** The estimate uses the encoding speed of the last 30 seconds, counts 4K frames as more work than HD frames, and ignores skipped and failed files. The progress line also shows the expected finishing time.
- **Proxies that relink.** Each proxy keeps the original's frame count, frame timing, timecode, rotation, color tags and every audio track.
- **Safe batches.** Proxies are written to a temporary file and renamed when finished, so cancelling never leaves a half-written proxy. Files that already have a proxy can be skipped.
- **Easy to add media.** Add files or whole folders, including subfolders, or drag them onto the window. Existing proxies and `Proxies` folders are never added as sources.
- **Dark and Light themes**, remembered between launches.

## Speed

**8K project, compared with an editor.** Creating proxies for the same set of 8K clips took about 8 hours in FilmCraft and 17 minutes in ProxyCraft on the author's PC. ProxyCraft decodes and resizes on the GPU, encodes two files at once and keeps full-size frames away from the CPU, which is where large files gain the most.

**Benchmarks.** Measured on one PC: Intel Core Ultra 7 265F (20 threads) and NVIDIA GeForce RTX 5070 Ti, with an FFmpeg development build from 23 October 2026. The sources were 8-second synthetic 4K test clips with added noise: HEVC 10-bit at 100 Mb/s and H.264 8-bit at 80 Mb/s. Speeds include FFmpeg start-up and are in frames per second, so real footage and other PCs will differ.

| Proxy | CPU decode and Lanczos resize, one file | ProxyCraft, one file | ProxyCraft, two files at once |
|---|---|---|---|
| ProRes 422 Proxy, ¼ size, from 4K HEVC 10-bit | 73 fps | 151 fps | 184 fps |
| H.264, ¼ size, from 4K HEVC 10-bit | 85 fps | 168 fps | 189 fps |
| H.265, ½ size, from 4K H.264 | 104 fps | 169 fps | 213 fps |

On the CPU alone, encoding two ProRes proxies at once raised the total from 88 to 134 fps on the same PC.

## FFmpeg and FFprobe

ProxyCraft uses **FFmpeg** and **FFprobe** to read and encode video.

**Included FFmpeg.** The recommended release ZIP contains FFmpeg 9.0.2 in an `ffmpeg` folder next to `ProxyCraft.exe`. ProxyCraft uses it automatically, so there is nothing to install or set up. It is an unmodified GPL build from [BtbN FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) with NVIDIA, Intel and AMD hardware support and the x264 and x265 encoders.

**Your own FFmpeg.** With the ZIP without FFmpeg, or to use a different version, ProxyCraft looks on the system `PATH` and in `C:\ffmpeg\bin`. You can also open **Tools → Settings** and select `ffmpeg.exe` and `ffprobe.exe` from the same build. **Use the FFmpeg included with ProxyCraft** switches back to the included one.

**Tools → Settings** also shows the hardware ProxyCraft will use on your PC.

FFmpeg runs in the background without opening console windows. Errors appear in the app.

## Basic workflow

1. Select **Add Files** or **Add Folder**, or drag files and folders onto the window.
2. Choose a format and size in the Inspector.
3. Choose a destination:
   - **Next to the original media, in a Proxies folder** (default)
   - **Next to the original media, same folder**
   - **One folder for all proxies**
4. Select **Create Proxies**.

Proxies are named after the original with an ending you can change in **Tools → Settings**. The default is `_Proxy`, for example `Clip001_Proxy.mov` for ProRes and `Clip001_Proxy.mp4` for H.264 and H.265.

## Formats in detail

- **ProRes** — MOV, 10-bit 4:2:2, encoded on the CPU from GPU-resized frames when possible. Audio is copied, or converted to PCM.
- **H.264** — MP4, 8-bit 4:2:0, the smallest and most compatible files. Audio is copied, or converted to AAC.
- **H.265** — MP4; 10-bit sources stay 10-bit. Audio is copied, or converted to AAC.

H.264 and H.265 proxies have a keyframe every second and no B-frames, so they decode and scrub quickly in a timeline.

## Options

- **Skip files that already have a proxy** — add a whole folder again after new footage arrives and only the new files are processed. H.264 and H.265 proxies share the `.mp4` name, so turn this off to replace one with the other.
- **Include subfolders when adding a folder** — add a whole card or project folder at once.
- **Run in the background (lower priority)** — keeps your editor responsive while proxies are made.
- **Files at once** — how many files are encoded in parallel. Auto uses two.

## Controls

- **Ctrl+O** — Add Files
- **Ctrl+Shift+O** — Add Folder
- **Delete** — Remove selected files from the list
- **Ctrl+Enter** — Create Proxies
- **Double-click** a finished file to open its proxy folder; **right-click** for more options.

## Privacy

ProxyCraft works offline. It does not send your media, settings or usage data anywhere. The only network actions are **Help → Donate → Open PayPal** and the source-code link in **Help → About**, which open a website in your browser.

## Run from source

You need Windows and [Python](https://www.python.org/downloads/) 3.10 or newer.

```bat
git clone https://github.com/PitKub/ProxyCraft.git
cd ProxyCraft
python -m pip install PySide6==6.12.0
powershell -ExecutionPolicy Bypass -File get_ffmpeg.ps1
python -m proxycraft
```

`get_ffmpeg.ps1` downloads the same FFmpeg build the releases include into `vendor\ffmpeg` and checks its SHA-256 checksum. You can skip it if FFmpeg is already installed.

## Build the EXE

Run `build.bat` in the project folder. It:

1. installs the build tools from `requirements.txt` (PySide6, Nuitka and Zig);
2. compiles `dist\ProxyCraft.exe`;
3. runs `get_ffmpeg.ps1` if FFmpeg has not been downloaded yet;
4. creates `dist\ProxyCraft-1.0.0-Windows.zip` (with FFmpeg) and `dist\ProxyCraft-1.0.0-Windows-without-FFmpeg.zip`;
5. creates the installer `dist\ProxyCraft-1.0.0-Setup.exe` from `installer\ProxyCraft.iss`, if [Inno Setup 6](https://jrsoftware.org/isinfo.php) is installed (`winget install --id JRSoftware.InnoSetup -e`).

The first build needs an internet connection and takes several minutes. The FFmpeg source archives for the release end up in `vendor\release-sources`.

## Project layout

| Path | Contents |
|---|---|
| `proxycraft/core.py` | FFmpeg detection, media probing, FFmpeg commands and the parallel batch engine (no Qt) |
| `proxycraft/main.py` | The window, settings and dialogs |
| `proxycraft/help.py` | The User Guide |
| `launcher.py` | Entry point for the EXE build |
| `assets/` | App icon |
| `build.bat`, `pysidedeploy.spec`, `requirements.txt` | Windows build |
| `get_ffmpeg.ps1` | Downloads and verifies the included FFmpeg and its source (`vendor\`, not in git) |
| `installer/ProxyCraft.iss` | Inno Setup script for the Windows installer |
| `PROJECT_NOTES.txt` | How the encoding pipeline works |

## Contributing

Bug reports and pull requests are welcome. Please open an [issue](https://github.com/PitKub/ProxyCraft/issues) with your Windows version, GPU, FFmpeg version and the error shown in the app.

## License

ProxyCraft is open source under the [MIT License](LICENSE). You may use, copy, change and share it, including commercially, as long as the copyright notice and licence text are kept.

Copyright © 2026 Piotr Kubicki.

## Third-party software

The Windows EXE includes Qt and PySide6, licensed under the LGPL-3.0, and the Python runtime. The recommended release ZIP also includes FFmpeg, licensed under the GPL-3.0. FFmpeg is a separate program that ProxyCraft runs; it does not change ProxyCraft's MIT licence. Its source code is attached to every release. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and the `licenses` folder for details.

## Donations and contact

ProxyCraft is free to use. Donations are completely optional. If you enjoy the app and would like to support its continued development, choose **Help → Donate** and send a voluntary PayPal donation to **pitkub@gmail.com**. The dialog lets you copy the address or open PayPal.

For contact or support, email **pitkub@gmail.com**.
