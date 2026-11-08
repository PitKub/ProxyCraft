# Third-party notices

ProxyCraft's own code is released under the MIT License (see `LICENSE`). The Windows EXE also contains, or works with, the third-party software below. Their licences are separate from ProxyCraft's.

## Rust libraries (built into ProxyCraft.exe)

ProxyCraft.exe is compiled from ProxyCraft's own code and about 200 unmodified open-source Rust libraries (crates) from <https://crates.io>, including **egui**, **eframe** and **egui_extras** (the window, © Emil Ernerfeldt and contributors), **wgpu** (drawing), **winit** (windowing), **rfd** (file dialogs), **serde**, **chrono** and **dirs**.

- Almost all are licensed under the MIT License or the Apache License 2.0 (usually both, as alternatives). A few use the Boost Software License 1.0, the Unicode License v3, the Zlib License, the ISC License or the Mozilla Public License 2.0 (`option-ext`, used unmodified; its source is at <https://github.com/soc/option-ext>).
- The full list of crates, versions, licences and source links is in `licenses/RUST_CRATES.md`, with the licence texts in the `licenses` folder.

## Fonts (built into ProxyCraft.exe)

egui embeds these fonts, used unmodified:

- **Ubuntu Light**, © Canonical Ltd., under the Ubuntu Font Licence 1.0 (`licenses/Ubuntu-Font-Licence-1.0.txt`).
- **Hack**, © Source Foundry Authors, under the MIT License and the Bitstream Vera licence.
- **Noto Emoji**, © Google Inc., and **emoji-icon-font**, under the SIL Open Font License 1.1 (`licenses/OFL-1.1.txt`) and the MIT License.

## FFmpeg and FFprobe (included in the recommended release ZIP)

The ZIP named `ProxyCraft-<version>-Windows.zip` contains **FFmpeg 9.0.2** (`n9.0.2-45-g1e65c6cd5d`) in its `ffmpeg` folder: `ffmpeg.exe`, `ffprobe.exe` and their DLLs. The ZIP ending `-without-FFmpeg` does not.

- These are unmodified binaries from BtbN FFmpeg-Builds, release `autobuild-2026-11-02-12-51`, file `ffmpeg-n9.0.2-45-g1e65c6cd5d-win64-gpl-shared-9.0.zip`, SHA-256 `5719531947c7168f05acac8f0f4cf8e9171b1eb8e0b469483fe8221dbb1d0991`.
- Licence: GNU General Public License v3 (GPL-3.0). The build includes GPL components such as x264 and x265. The licence text is in `ffmpeg/LICENSE.txt` and `licenses/GPL-3.0.txt`.
- FFmpeg is a separate program. ProxyCraft starts it as a separate process and does not link to it, so ProxyCraft's own code stays under the MIT License.
- Source code:
  - FFmpeg at commit `1e65c6cd5d3d2d12cbb374460dbf08bedf54772a`: <https://github.com/FFmpeg/FFmpeg/tree/1e65c6cd5d3d2d12cbb374460dbf08bedf54772a>
  - The build scripts, which pin the version of every library in this build: <https://github.com/BtbN/FFmpeg-Builds/tree/5ece62897c650b863c1ddda7b6964845344aef2d>
  - Both are attached to every ProxyCraft release on GitHub, as `FFmpeg-source-<commit>.tar.gz` and `FFmpeg-Builds-scripts-<tag>.zip`.
- More about FFmpeg's licensing: <https://ffmpeg.org/legal.html>.

FFmpeg is a trademark of Fabrice Bellard, originator of the FFmpeg project.
