from PySide6.QtWidgets import QDialog, QVBoxLayout, QTextBrowser, QPushButton


class HelpDialog(QDialog):
    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("ProxyCraft — User Guide")
        self.resize(860, 680)
        browser = QTextBrowser()
        browser.setOpenExternalLinks(True)
        browser.setHtml("""
        <h1>ProxyCraft</h1>
        <p><b>Fast proxy creator for video editing.</b></p>

        <h2>Getting Started</h2>
        <ol>
          <li>Add video files with <b>Add Files</b>, add a whole folder with <b>Add Folder</b>,
          or drag files and folders onto the window.</li>
          <li>Choose a format and size in the Inspector.</li>
          <li>Choose where the proxies go.</li>
          <li>Select <b>Create Proxies</b> (Ctrl+Enter).</li>
        </ol>
        <p>Existing proxies and <b>Proxies</b> folders are never added as sources.
        Turn on <b>Include subfolders when adding a folder</b> to add a whole card or project.</p>

        <h2>Formats</h2>
        <ul>
          <li><b>ProRes 422 Proxy, ¼ or ½ size</b> and <b>ProRes 422 LT, ½ size</b> — MOV files that
          every editor handles smoothly. 10-bit 4:2:2. Encoded on the CPU.</li>
          <li><b>H.264, ¼ or ½ size</b> — the smallest MP4 files, playable everywhere. 8-bit 4:2:0.</li>
          <li><b>H.265, ¼ or ½ size</b> — small MP4 files; 10-bit sources stay 10-bit.</li>
        </ul>
        <p>H.264 and H.265 proxies have a keyframe every second and no B-frames, so they are
        quick to decode and scrub in a timeline.</p>

        <h2>Destination</h2>
        <ul>
          <li><b>Next to the original media, in a Proxies folder</b> — the default. Each proxy goes
          into a <i>Proxies</i> folder beside its original.</li>
          <li><b>Next to the original media, same folder</b> — beside the original file.</li>
          <li><b>One folder for all proxies</b> — choose any folder. Files with the same name get
          a number added.</li>
        </ul>
        <p>Proxies are named after the original with the ending set in Tools → Settings
        (default <i>_Proxy</i>), for example <i>Clip001_Proxy.mov</i>.</p>

        <h2>What Proxies Keep</h2>
        <p>Each proxy keeps the original's frame count and frame timing, timecode, rotation flag,
        color tags and every audio track, so editors can relink proxies and originals. Audio is
        copied unchanged when the container accepts it; otherwise it becomes PCM in MOV files and
        AAC in MP4 files.</p>

        <h2>Proxy Check</h2>
        <p>With <b>Check each proxy against its original</b> on (the default), every new proxy is
        compared with its original: length, frame rate, timecode, number of audio tracks, rotation
        and picture shape. The <b>Check</b> column shows <i>OK</i> or <i>Problems</i>; hover over a
        result for details. Existing proxies that are skipped are checked too, which catches a proxy
        made from an older version of a clip.</p>
        <p>When the original does not store its frame count (MKV and some other formats), the length
        is compared by duration instead.</p>
        <p><b>Tools → Check Proxies</b> (Ctrl+K) checks the proxies you already have, using the
        current format and destination to find them, without encoding anything.</p>

        <h2>Speed and Hardware</h2>
        <p>ProxyCraft checks what really works on your PC when it starts:</p>
        <ul>
          <li>With an NVIDIA GPU, video is decoded and resized on the GPU and H.264/H.265 is
          encoded with NVENC, without copying full-size frames through the CPU. ProRes is encoded
          on the CPU from GPU-resized frames.</li>
          <li>With an Intel or AMD GPU, video is decoded on the GPU (D3D11) and resized on the
          CPU. This covers H.264, H.265, AV1, VP9, MPEG-2 and VC-1 in 8-bit or 10-bit 4:2:0.</li>
          <li>Intel Quick Sync and AMD AMF are used for H.264/H.265 encoding when available.
          If a hardware encoder fails on a file (for example 10-bit H.265 on an older GPU), the
          file is made with x264 or x265 on the CPU instead; the status shows <i>CPU, x264</i>
          or <i>CPU, x265</i>.</li>
          <li>Files the GPU cannot decode, such as ProRes or DNxHR sources, are decoded on the CPU
          automatically. If GPU decoding fails for a file, it is retried on the CPU.</li>
          <li>Without a usable GPU, everything runs on the CPU (libx264, libx265, prores_ks).</li>
        </ul>
        <p><b>Files at once</b> sets how many files are encoded in parallel. Auto uses two, which
        was 20–50% faster than one at a time in testing.</p>
        <p><b>Run in the background (lower priority)</b> keeps your editor responsive while proxies
        are made. It does not slow encoding when the PC is otherwise idle.</p>

        <h2>Skipping and Cancelling</h2>
        <p>With <b>Skip files that already have a proxy</b> on, files whose proxy already exists are
        skipped, so you can add a whole folder again after new footage arrives. H.264 and H.265
        proxies share the .mp4 name, so turn the option off to replace one with the other.</p>
        <p>Proxies are written to a temporary <i>.part</i> file and renamed when finished.
        Cancelling or a failure never leaves a half-written proxy behind.</p>

        <h2>FFmpeg</h2>
        <p>The recommended ProxyCraft download includes FFmpeg in an <i>ffmpeg</i> folder next to
        ProxyCraft.exe, and ProxyCraft uses it automatically. To use a different FFmpeg, open
        <b>Tools → Settings</b> and select ffmpeg.exe and ffprobe.exe from the same build; the paths
        are remembered. <b>Use the FFmpeg included with ProxyCraft</b> switches back.
        Without an included FFmpeg, ProxyCraft looks on the system PATH and in C:\\ffmpeg\\bin.
        <b>Tools → Settings</b> also shows the hardware ProxyCraft will use.</p>
        <p>The included FFmpeg is an unmodified GPL-3.0 build. Its licence and source details are in
        the <i>ffmpeg</i> folder (LICENSE.txt and SOURCE.txt).</p>

        <h2>Keyboard Shortcuts</h2>
        <p><b>Ctrl+O</b> Add Files &nbsp; <b>Ctrl+Shift+O</b> Add Folder &nbsp;
        <b>Delete</b> Remove Selected &nbsp; <b>Ctrl+Enter</b> Create Proxies &nbsp;
        <b>Ctrl+K</b> Check Proxies</p>
        <p>Double-click a finished file to open its proxy folder. Right-click a file for more options.</p>
        """)
        close = QPushButton("Close")
        close.clicked.connect(self.accept)
        lay = QVBoxLayout(self)
        lay.addWidget(browser)
        lay.addWidget(close)
