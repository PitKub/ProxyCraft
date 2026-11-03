# Third-party notices

ProxyCraft's own code is released under the MIT License (see `LICENSE`). The Windows EXE also contains, or works with, the third-party software below. Their licences are separate from ProxyCraft's.

## Qt and PySide6 (bundled in ProxyCraft.exe)

ProxyCraft uses **Qt for Python (PySide6) 6.12.0** and the **Qt 6** libraries it includes, © The Qt Company Ltd. and other contributors.

- Licence: GNU Lesser General Public License v3 (LGPL-3.0). The full texts are in `licenses/LGPL-3.0.txt` and `licenses/GPL-3.0.txt`.
- Source code: PySide6 at <https://code.qt.io/cgit/pyside/pyside-setup.git/> and Qt at <https://download.qt.io/official_releases/qt/>.
- ProxyCraft does not modify Qt or PySide6. Because ProxyCraft's own source is public, you can rebuild the EXE with any compatible PySide6 version; see "Build the EXE" in `README.md`.

## Python (bundled in ProxyCraft.exe)

The EXE contains the **Python** runtime and standard library, © Python Software Foundation, licensed under the Python Software Foundation License: <https://docs.python.org/3/license.html>.

## Nuitka (build tool)

The EXE is compiled with **Nuitka 4.3.4**: <https://nuitka.net>. See Nuitka's own documentation for the terms that apply to programs it compiles.

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
