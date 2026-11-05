from __future__ import annotations

import os
import shutil
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path

from PySide6.QtCore import Qt, QThread, Signal, QSize, QUrl, QSettings
from PySide6.QtGui import QAction, QKeySequence, QIcon, QDesktopServices, QColor, QBrush
from PySide6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QFileDialog, QMessageBox, QVBoxLayout,
    QHBoxLayout, QLabel, QPushButton, QComboBox, QGroupBox, QFormLayout, QSplitter,
    QStatusBar, QProgressBar, QCheckBox, QToolBar, QDialog, QLineEdit, QScrollArea,
    QSizePolicy, QTreeWidget, QTreeWidgetItem, QRadioButton, QButtonGroup,
    QAbstractItemView, QHeaderView, QMenu,
)

from . import __version__
from .core import (
    PRESETS, PRESETS_BY_KEY, SUPPORTED_EXTENSIONS, DEFAULT_SUFFIX,
    DEST_PROXIES_FOLDER, DEST_SAME_FOLDER, DEST_CUSTOM,
    Capabilities, Job, ProxyBatch, detect_capabilities, probe_media, check_proxy,
    collect_media, plan_outputs, auto_parallel_jobs, format_time, is_proxy_file,
)
from .help import HelpDialog


THEME_COLORS = {
    "dark": {
        "window": "#11151b", "panel": "#171c23", "panel_alt": "#1b2430",
        "field": "#0d1117", "viewer": "#080a0d", "status": "#0b0e12",
        "text": "#e5e9ef", "text_soft": "#d8dee8", "muted": "#8e9aaa",
        "accent": "#8eb3dc", "accent_soft": "#9fb7d2", "border": "#303946",
        "border_soft": "#394454", "button": "#222a35", "hover": "#2b3543",
        "pressed": "#354154", "selection": "#f2f5f8", "donate_border": "#354b64",
        "done": "#7fc98f", "failed": "#e58a8a",
    },
    "light": {
        "window": "#eef2f6", "panel": "#ffffff", "panel_alt": "#e5ebf2",
        "field": "#ffffff", "viewer": "#dbe3eb", "status": "#e3e9f0",
        "text": "#1b2733", "text_soft": "#273746", "muted": "#5e6d7d",
        "accent": "#2d6696", "accent_soft": "#3a719e", "border": "#c4ced9",
        "border_soft": "#aebbc9", "button": "#e2e9f0", "hover": "#d2dfe9",
        "pressed": "#c5d4e0", "selection": "#263f56", "donate_border": "#9eafc0",
        "done": "#2e7d44", "failed": "#b23a3a",
    },
}


def theme_stylesheet(theme_name):
    c = THEME_COLORS[theme_name]
    return f"""
QMainWindow, QWidget {{ background:{c['window']}; color:{c['text']}; font-family:Segoe UI; }}
QMenuBar, QMenu {{ background:{c['panel']}; color:{c['text']}; }}
QMenuBar::item:selected, QMenu::item:selected {{ background:{c['hover']}; }}
QMenu {{ border:1px solid {c['border']}; padding:4px 0; }}
QMenu::item {{ padding:6px 32px 6px 16px; }}
QMenu::item:disabled {{ color:{c['muted']}; }}
QMenu::separator {{ height:1px; background:{c['border']}; margin:4px 10px; }}
QToolBar {{ background:{c['panel']}; border:0; spacing:6px; padding:6px; }}
QToolBar QWidget {{ background:transparent; }}
QPushButton, QToolButton {{ background:{c['button']}; color:{c['text']}; border:1px solid {c['border_soft']}; padding:7px 12px; border-radius:4px; }}
QPushButton:hover, QToolButton:hover {{ background:{c['hover']}; }}
QPushButton:pressed, QToolButton:pressed {{ background:{c['pressed']}; }}
QPushButton:disabled {{ color:{c['muted']}; }}
QPushButton#PrimaryButton {{ background:{c['accent']}; color:{c['window']}; border-color:{c['accent']}; font-weight:600; }}
QPushButton#PrimaryButton:disabled {{ background:{c['button']}; color:{c['muted']}; border-color:{c['border_soft']}; }}
QPushButton#ThemeToggle {{ min-width:100px; padding:6px 14px; border-radius:15px; font-weight:600; }}
QPushButton#ThemeToggle:checked {{ background:{c['accent']}; border-color:{c['accent']}; color:{c['window']}; }}
QGroupBox {{ border:1px solid {c['border']}; margin-top:10px; padding:8px; font-weight:bold; }}
QGroupBox::title {{ subcontrol-origin:margin; left:10px; padding:0 5px; color:{c['accent']}; }}
QComboBox, QLineEdit {{ background:{c['field']}; color:{c['text']}; border:1px solid {c['border']}; padding:5px; min-height:22px; }}
QComboBox QAbstractItemView {{ background:{c['field']}; color:{c['text']}; selection-background-color:{c['hover']}; }}
QTextBrowser {{ background:{c['field']}; color:{c['text']}; border:1px solid {c['border']}; }}
QCheckBox, QRadioButton {{ color:{c['text_soft']}; spacing:7px; padding:2px; font-weight:normal; }}
QRadioButton::indicator, QCheckBox::indicator {{ width:14px; height:14px; border:2px solid {c['border_soft']}; background:{c['field']}; }}
QRadioButton::indicator {{ border-radius:9px; }}
QCheckBox::indicator {{ border-radius:3px; }}
QRadioButton::indicator:hover, QCheckBox::indicator:hover {{ border-color:{c['accent']}; }}
QRadioButton::indicator:checked {{ border-color:{c['accent']}; background:qradialgradient(cx:0.5, cy:0.5, radius:0.5, fx:0.5, fy:0.5, stop:0 {c['accent']}, stop:0.55 {c['accent']}, stop:0.62 {c['field']}, stop:1 {c['field']}); }}
QCheckBox::indicator:checked {{ border-color:{c['accent']}; background:{c['accent']}; }}
QRadioButton::indicator:disabled, QCheckBox::indicator:disabled {{ border-color:{c['border']}; }}
QLabel {{ color:{c['text_soft']}; }}
QStatusBar {{ background:{c['status']}; color:{c['muted']}; }}
QScrollArea {{ border:0; }}
QProgressBar {{ background:{c['field']}; color:{c['text']}; border:1px solid {c['border']}; border-radius:3px; text-align:center; min-height:18px; }}
QProgressBar::chunk {{ background:{c['accent']}; }}
QToolTip {{ background:{c['panel']}; color:{c['text']}; border:1px solid {c['border']}; padding:4px; }}
QTreeWidget {{ background:{c['field']}; color:{c['text']}; border:1px solid {c['border']}; alternate-background-color:{c['panel']}; }}
QTreeWidget::item {{ padding:3px 2px; }}
QTreeWidget::item:selected {{ background:{c['hover']}; color:{c['text']}; }}
QHeaderView::section {{ background:{c['panel_alt']}; color:{c['text_soft']}; border:0; border-right:1px solid {c['border']}; padding:5px; }}
#SectionTitle {{ color:{c['accent']}; font-size:15px; font-weight:bold; }}
#FamilyLabel {{ color:{c['muted']}; font-weight:600; padding-top:4px; }}
#DropHint {{ color:{c['muted']}; background:{c['status']}; border:1px dashed {c['border_soft']}; padding:10px; }}
#ProgressPanel {{ background:{c['panel_alt']}; border-top:1px solid {c['border']}; }}
#CurrentLabel {{ color:{c['accent_soft']}; }}
#StatsLabel, #Hint {{ color:{c['muted']}; }}
#AboutTitle {{ font-size:17px; font-weight:600; }}
#DonateHeading {{ font-size:20px; font-weight:700; color:{c['accent']}; }}
#DonateAddress {{ background:{c['panel_alt']}; border:1px solid {c['donate_border']}; border-radius:8px; padding:14px; }}
"""


DONATION_ADDRESS = "pitkub@gmail.com"

DESTINATION_MODES = [
    (DEST_PROXIES_FOLDER, "Next to the original media, in a Proxies folder"),
    (DEST_SAME_FOLDER, "Next to the original media, same folder"),
    (DEST_CUSTOM, "One folder for all proxies"),
]

COL_FILE, COL_RES, COL_CODEC, COL_DURATION, COL_PROXY, COL_STATUS, COL_CHECK = range(7)


def app_icon_path():
    # Nuitka onefile extracts bundled data beside the compiled module at launch.
    roots = [Path(__file__).resolve().parent.parent, Path(sys.executable).resolve().parent]
    for root in roots:
        candidate = root / "assets" / "ProxyCraft.ico"
        if candidate.is_file():
            return candidate
    return roots[0] / "assets" / "ProxyCraft.ico"


def app_folders():
    """Folders that may hold the included FFmpeg: beside the EXE, or the project."""
    folders = []
    for candidate in (Path(sys.argv[0]).resolve().parent,      # ProxyCraft.exe (Nuitka onefile)
                      Path(sys.executable).resolve().parent,
                      Path(__file__).resolve().parent.parent):  # running from source
        if candidate not in folders:
            folders.append(candidate)
    return folders


def bundled_tool(name):
    """FFmpeg shipped with ProxyCraft: an "ffmpeg" folder next to ProxyCraft.exe.

    When running from source, vendor/ffmpeg (filled by get_ffmpeg.ps1) is used.
    """
    exe = f"{name}.exe" if os.name == "nt" else name
    for folder in app_folders():
        for sub in (Path("ffmpeg"), Path("vendor") / "ffmpeg"):
            path = folder / sub / exe
            if path.is_file():
                return str(path)
    return ""


def find_tool(name):
    """Included FFmpeg first, then PATH, then the usual Windows FFmpeg folders."""
    found = bundled_tool(name) or shutil.which(name)
    if found:
        return found
    exe = f"{name}.exe" if os.name == "nt" else name
    for folder in (
        Path(r"C:\ffmpeg\bin"),
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "ffmpeg" / "bin",
        Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "ffmpeg" / "bin",
    ):
        if (folder / exe).is_file():
            return str(folder / exe)
    return ""


class CapabilityWorker(QThread):
    """Test the encoders and decoders off the GUI thread."""
    detected = Signal(str, object)

    def __init__(self, ffmpeg, refresh=False, parent=None):
        super().__init__(parent)
        self.ffmpeg = ffmpeg
        self.refresh = refresh

    def run(self):
        self.detected.emit(self.ffmpeg, detect_capabilities(self.ffmpeg, self.refresh))


class ProbeWorker(QThread):
    """Read resolution, codec and duration of newly added files."""
    probed = Signal(str, object, str)  # path, MediaInfo or None, error

    def __init__(self, ffprobe, paths, parent=None):
        super().__init__(parent)
        self.ffprobe = ffprobe
        self.paths = list(paths)
        self._stop = False

    def stop(self):
        self._stop = True

    def run(self):
        for path in self.paths:
            if self._stop:
                return
            try:
                self.probed.emit(str(path), probe_media(self.ffprobe, path), "")
            except Exception as exc:
                self.probed.emit(str(path), None, (str(exc).splitlines() or ["Unreadable file."])[0])


class CheckWorker(QThread):
    """Check existing proxies against their originals (Tools → Check Proxies)."""
    checked = Signal(str, str, object, str)  # source key, proxy path, ProxyCheck or None, message

    def __init__(self, ffprobe, pairs, infos, parent=None):
        super().__init__(parent)
        self.ffprobe = ffprobe
        self.pairs = list(pairs)     # (source key, proxy Path)
        self.infos = dict(infos)
        self._stop = False

    def stop(self):
        self._stop = True

    def run(self):
        for key, output in self.pairs:
            if self._stop:
                return
            if not output.is_file():
                self.checked.emit(key, str(output), None, f"No proxy found at:\n{output}")
                continue
            info = self.infos.get(key)
            if info is None:
                try:
                    info = probe_media(self.ffprobe, key)
                except Exception as exc:
                    first = (str(exc).splitlines() or [""])[0]
                    self.checked.emit(key, str(output), None, f"The original cannot be read. {first}")
                    continue
            self.checked.emit(key, str(output), check_proxy(self.ffprobe, info, output), "")


class BatchWorker(QThread):
    event = Signal(str, object)

    def __init__(self, batch: ProxyBatch, parent=None):
        super().__init__(parent)
        self.batch = batch
        batch.callback = lambda name, data: self.event.emit(name, data)

    def run(self):
        try:
            self.batch.run()
        except Exception as exc:
            self.event.emit("crashed", {"message": str(exc)})


class SettingsDialog(QDialog):
    def __init__(self, ffmpeg, ffprobe, suffix, hardware_text="", parent=None):
        super().__init__(parent)
        self.setWindowTitle("ProxyCraft — Settings")
        self.resize(660, 300)
        self.ffmpeg = QLineEdit(ffmpeg)
        self.ffprobe = QLineEdit(ffprobe)
        self.suffix = QLineEdit(suffix)
        lay = QFormLayout(self)
        b1 = QPushButton("Browse…")
        b2 = QPushButton("Browse…")
        # Each Browse button sits directly under the path it changes.
        for label, field, button in (("FFmpeg executable", self.ffmpeg, b1),
                                     ("FFprobe executable", self.ffprobe, b2)):
            lay.addRow(label, field)
            row = QHBoxLayout()
            row.addStretch()
            row.addWidget(button)
            lay.addRow("", row)
        included_ffmpeg, included_ffprobe = bundled_tool("ffmpeg"), bundled_tool("ffprobe")
        if included_ffmpeg and included_ffprobe:
            use_included = QPushButton("Use the FFmpeg included with ProxyCraft")

            def choose_included():
                self.ffmpeg.setText(included_ffmpeg)
                self.ffprobe.setText(included_ffprobe)

            use_included.clicked.connect(choose_included)
            row = QHBoxLayout()
            row.addStretch()
            row.addWidget(use_included)
            lay.addRow("", row)
        lay.addRow("Proxy name ending", self.suffix)
        example = QLabel()
        example.setObjectName("Hint")
        lay.addRow("", example)
        self.suffix.textChanged.connect(
            lambda text: example.setText(f"Example: Clip001{text or DEFAULT_SUFFIX}.mov")
        )
        self.suffix.textChanged.emit(self.suffix.text())
        if hardware_text:
            hw = QLabel(hardware_text)
            hw.setObjectName("Hint")
            hw.setWordWrap(True)
            lay.addRow("Detected hardware", hw)
        save = QPushButton("Save")
        cancel = QPushButton("Cancel")
        row = QHBoxLayout()
        row.addStretch()
        row.addWidget(save)
        row.addWidget(cancel)
        lay.addRow(row)
        b1.clicked.connect(lambda: self.browse(self.ffmpeg, "ffmpeg"))
        b2.clicked.connect(lambda: self.browse(self.ffprobe, "ffprobe"))
        save.clicked.connect(self.accept)
        cancel.clicked.connect(self.reject)

    def browse(self, target, name):
        start_dir = os.path.dirname(target.text().strip().strip('"'))
        p, _ = QFileDialog.getOpenFileName(
            self, f"Select {name}.exe", start_dir if os.path.isdir(start_dir) else "",
            "Executable (*.exe);;All files (*.*)"
        )
        if not p:
            return
        target.setText(p)
        # Choosing ffmpeg.exe also fills in ffprobe.exe from the same folder.
        if name == "ffmpeg" and not self.ffprobe.text().strip():
            sibling = Path(p).with_name("ffprobe.exe")
            if sibling.is_file():
                self.ffprobe.setText(str(sibling))


class MainWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("ProxyCraft")
        icon_path = app_icon_path()
        if icon_path.is_file():
            self.setWindowIcon(QIcon(str(icon_path)))
        screen = QApplication.primaryScreen()
        if screen is not None:
            available = screen.availableGeometry()
            self.resize(min(1280, max(960, available.width() - 24)),
                        min(800, max(600, available.height() - 24)))
        else:
            self.resize(1200, 760)
        self.setAcceptDrops(True)

        self.settings = QSettings("PitKub", "ProxyCraft")
        saved_theme = self.settings.value("appearance/theme", "dark")
        self.theme_name = saved_theme if saved_theme in THEME_COLORS else "dark"
        self.theme_colors = THEME_COLORS[self.theme_name]

        saved_ffmpeg = str(self.settings.value("tools/ffmpeg", "") or "")
        saved_ffprobe = str(self.settings.value("tools/ffprobe", "") or "")
        self.ffmpeg = saved_ffmpeg if os.path.isfile(saved_ffmpeg) else find_tool("ffmpeg")
        self.ffprobe = saved_ffprobe if os.path.isfile(saved_ffprobe) else find_tool("ffprobe")
        self.suffix = str(self.settings.value("naming/suffix", DEFAULT_SUFFIX) or DEFAULT_SUFFIX)

        self.caps: Capabilities | None = None
        self.capability_worker = None
        self.probe_workers = []
        self.batch_worker = None
        self.check_worker = None
        self.check_counts = {"ok": 0, "problems": 0, "missing": 0}
        self.items: dict[str, QTreeWidgetItem] = {}
        self.infos: dict = {}
        self.job_items: list[QTreeWidgetItem] = []
        self.job_engines: list[str] = []

        self._build_menu()
        self._build_ui()
        self._load_options()
        self._apply_theme(self.theme_name)
        self._detect_hardware()
        self._update_controls()

    # ------------------------------------------------------------------ UI

    def _build_menu(self):
        filem = self.menuBar().addMenu("&File")
        add_files = QAction("Add Files…", self)
        add_files.setShortcut(QKeySequence("Ctrl+O"))
        add_files.triggered.connect(self.add_files)
        add_folder = QAction("Add Folder…", self)
        add_folder.setShortcut(QKeySequence("Ctrl+Shift+O"))
        add_folder.triggered.connect(self.add_folder)
        self.remove_action = QAction("Remove Selected", self)
        self.remove_action.setShortcut(QKeySequence.Delete)
        self.remove_action.triggered.connect(self.remove_selected)
        self.clear_action = QAction("Clear List", self)
        self.clear_action.triggered.connect(self.clear_list)
        self.start_action = QAction("Create Proxies", self)
        self.start_action.setShortcut(QKeySequence("Ctrl+Return"))
        self.start_action.triggered.connect(self.start)
        quit_a = QAction("Exit", self)
        quit_a.triggered.connect(self.close)
        for a in (add_files, add_folder, self.remove_action, self.clear_action):
            filem.addAction(a)
        filem.addSeparator()
        filem.addAction(self.start_action)
        filem.addSeparator()
        filem.addAction(quit_a)

        tools = self.menuBar().addMenu("&Tools")
        settings = QAction("Settings…", self)
        settings.triggered.connect(self.settings_dialog)
        redetect = QAction("Detect Hardware Again", self)
        redetect.triggered.connect(lambda: self._detect_hardware(refresh=True))
        self.check_action = QAction("Check Proxies", self)
        self.check_action.setShortcut(QKeySequence("Ctrl+K"))
        self.check_action.triggered.connect(self.check_proxies)
        tools.addAction(self.check_action)
        tools.addSeparator()
        tools.addAction(settings)
        tools.addAction(redetect)

        helpm = self.menuBar().addMenu("&Help")
        guide = QAction("User Guide", self)
        guide.triggered.connect(lambda: HelpDialog(self).exec())
        donate = QAction("Donate…", self)
        donate.triggered.connect(self.donate_dialog)
        about = QAction("About", self)
        about.triggered.connect(self.about)
        helpm.addAction(guide)
        helpm.addAction(donate)
        helpm.addSeparator()
        helpm.addAction(about)

    def _build_ui(self):
        toolbar = QToolBar()
        toolbar.setIconSize(QSize(18, 18))
        toolbar.setMovable(False)
        self.addToolBar(toolbar)
        self.add_files_button = QPushButton("Add Files")
        self.add_files_button.clicked.connect(self.add_files)
        self.add_folder_button = QPushButton("Add Folder")
        self.add_folder_button.clicked.connect(self.add_folder)
        self.remove_button = QPushButton("Remove")
        self.remove_button.clicked.connect(self.remove_selected)
        self.clear_button = QPushButton("Clear")
        self.clear_button.clicked.connect(self.clear_list)
        for b in (self.add_files_button, self.add_folder_button, self.remove_button, self.clear_button):
            toolbar.addWidget(b)
        toolbar.addSeparator()
        self.start_button = QPushButton("Create Proxies")
        self.start_button.setObjectName("PrimaryButton")
        self.start_button.clicked.connect(self.start)
        self.cancel_button = QPushButton("Cancel")
        self.cancel_button.clicked.connect(self.cancel)
        toolbar.addWidget(self.start_button)
        toolbar.addWidget(self.cancel_button)
        spacer = QWidget()
        spacer.setSizePolicy(QSizePolicy.Expanding, QSizePolicy.Preferred)
        toolbar.addWidget(spacer)
        self.theme_toggle = QPushButton()
        self.theme_toggle.setObjectName("ThemeToggle")
        self.theme_toggle.setCheckable(True)
        self.theme_toggle.toggled.connect(
            lambda light: self._apply_theme("light" if light else "dark")
        )
        toolbar.addWidget(self.theme_toggle)
        for button in toolbar.findChildren(QPushButton):
            button.setFocusPolicy(Qt.NoFocus)

        splitter = QSplitter(Qt.Horizontal)

        left = QWidget()
        ll = QVBoxLayout(left)
        ll.setContentsMargins(8, 8, 4, 0)
        ll.setSpacing(6)
        self.drop_hint = QLabel(
            "Drop video files or folders here, or use Add Files / Add Folder. "
            "Existing proxies and Proxies folders are skipped automatically."
        )
        self.drop_hint.setObjectName("DropHint")
        self.drop_hint.setWordWrap(True)
        self.drop_hint.setAlignment(Qt.AlignCenter)
        ll.addWidget(self.drop_hint)

        self.tree = QTreeWidget()
        self.tree.setHeaderLabels(["File", "Resolution", "Codec", "Duration", "Proxy size",
                                   "Status", "Check"])
        self.tree.setRootIsDecorated(False)
        self.tree.setAlternatingRowColors(True)
        self.tree.setSelectionMode(QAbstractItemView.ExtendedSelection)
        self.tree.setUniformRowHeights(True)
        self.tree.setContextMenuPolicy(Qt.CustomContextMenu)
        self.tree.customContextMenuRequested.connect(self._tree_menu)
        self.tree.itemDoubleClicked.connect(lambda item, _col: self._open_folder_for(item))
        header = self.tree.header()
        header.setStretchLastSection(True)
        header.setSectionResizeMode(COL_FILE, QHeaderView.Stretch)
        for col in (COL_RES, COL_CODEC, COL_DURATION, COL_PROXY):
            header.setSectionResizeMode(col, QHeaderView.ResizeToContents)
        header.setSectionResizeMode(COL_STATUS, QHeaderView.Interactive)
        self.tree.setColumnWidth(COL_STATUS, 200)
        header.setSectionResizeMode(COL_CHECK, QHeaderView.Interactive)
        self.tree.setColumnWidth(COL_CHECK, 90)
        self.tree.headerItem().setToolTip(
            COL_CHECK, "Whether the proxy matches its original. Hover over a result for details.")
        ll.addWidget(self.tree, 1)

        progress_panel = QWidget()
        progress_panel.setObjectName("ProgressPanel")
        pl = QVBoxLayout(progress_panel)
        pl.setContentsMargins(10, 8, 10, 8)
        pl.setSpacing(4)
        self.current_label = QLabel("Ready.")
        self.current_label.setObjectName("CurrentLabel")
        self.progress = QProgressBar()
        self.progress.setRange(0, 1000)
        self.progress.setFormat("%p%")
        self.stats_label = QLabel("Files: —   |   Speed: —   |   Time left: —")
        self.stats_label.setObjectName("StatsLabel")
        pl.addWidget(self.current_label)
        pl.addWidget(self.progress)
        pl.addWidget(self.stats_label)
        ll.addWidget(progress_panel)
        splitter.addWidget(left)

        right = QScrollArea()
        right.setWidgetResizable(True)
        right.setMinimumWidth(330)
        right.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)
        inspector = QWidget()
        rl = QVBoxLayout(inspector)
        rl.setContentsMargins(8, 6, 8, 6)
        rl.setSpacing(4)
        title = QLabel("INSPECTOR")
        title.setObjectName("SectionTitle")
        rl.addWidget(title)

        format_box = QGroupBox("Format and size")
        fl = QVBoxLayout(format_box)
        fl.setSpacing(1)
        self.preset_group = QButtonGroup(self)
        self.preset_buttons = {}
        family_names = {"prores": "ProRes", "h264": "H.264", "h265": "H.265"}
        last_family = None
        for preset in PRESETS:
            if preset.family != last_family:
                caption = QLabel(family_names[preset.family])
                caption.setObjectName("FamilyLabel")
                fl.addWidget(caption)
                last_family = preset.family
            rb = QRadioButton(preset.label)
            self.preset_group.addButton(rb)
            self.preset_buttons[preset.key] = rb
            rb.toggled.connect(lambda checked, key=preset.key: checked and self._on_preset(key))
            fl.addWidget(rb)
        self.preset_hint = QLabel()
        self.preset_hint.setObjectName("Hint")
        self.preset_hint.setWordWrap(True)
        fl.addWidget(self.preset_hint)
        rl.addWidget(format_box)

        dest_box = QGroupBox("Destination")
        dl = QVBoxLayout(dest_box)
        self.dest_combo = QComboBox()
        for key, label in DESTINATION_MODES:
            self.dest_combo.addItem(label, key)
        self.dest_combo.currentIndexChanged.connect(self._on_destination_mode)
        dl.addWidget(self.dest_combo)
        self.custom_dir = QLineEdit()
        self.custom_dir.setPlaceholderText("Folder for all proxies")
        self.custom_dir.textChanged.connect(lambda t: self.settings.setValue("output/custom_dir", t))
        self.custom_browse = QPushButton("Browse…")
        self.custom_browse.clicked.connect(self.browse_custom_dir)
        dl.addWidget(self.custom_dir)
        row = QHBoxLayout()
        row.addStretch()
        row.addWidget(self.custom_browse)
        dl.addLayout(row)
        rl.addWidget(dest_box)

        options_box = QGroupBox("Options")
        ol = QVBoxLayout(options_box)
        self.skip_existing = QCheckBox("Skip files that already have a proxy")
        self.include_subfolders = QCheckBox("Include subfolders when adding a folder")
        self.low_priority = QCheckBox("Run in the background (lower priority)")
        self.low_priority.setToolTip(
            "Keeps your editor responsive while proxies are made. "
            "It does not slow encoding when the PC is otherwise idle."
        )
        self.verify = QCheckBox("Check each proxy against its original")
        self.verify.setToolTip(
            "After each proxy is made, compare its length, frame rate, timecode, audio tracks "
            "and rotation with the original, so you know it will relink. Takes a moment per file."
        )
        for cb in (self.skip_existing, self.include_subfolders, self.low_priority, self.verify):
            ol.addWidget(cb)
        par_row = QHBoxLayout()
        par_row.addWidget(QLabel("Files at once"))
        self.parallel_combo = QComboBox()
        self.parallel_combo.addItem(f"Auto ({auto_parallel_jobs()})", 0)
        for n in (1, 2, 3, 4):
            self.parallel_combo.addItem(str(n), n)
        self.parallel_combo.setToolTip(
            "Encoding two files at once was about 20–50% faster than one at a time "
            "in testing, on both GPU and CPU."
        )
        par_row.addWidget(self.parallel_combo, 1)
        ol.addLayout(par_row)
        rl.addWidget(options_box)

        hw_box = QGroupBox("Hardware")
        hl = QVBoxLayout(hw_box)
        self.hardware_label = QLabel("Checking FFmpeg…")
        self.hardware_label.setObjectName("Hint")
        self.hardware_label.setWordWrap(True)
        hl.addWidget(self.hardware_label)
        rl.addWidget(hw_box)

        note = QLabel("Proxies keep the original frame count, timecode, rotation and audio "
                      "tracks, so editors can relink them to the full-resolution media. "
                      "Tools → Check Proxies checks proxies you already have.")
        note.setObjectName("Hint")
        note.setWordWrap(True)
        rl.addWidget(note)
        rl.addStretch(1)
        right.setWidget(inspector)
        splitter.addWidget(right)
        splitter.setSizes([880, 360])
        splitter.setStretchFactor(0, 1)
        self.setCentralWidget(splitter)

        self.status = QStatusBar()
        self.setStatusBar(self.status)

    def _load_options(self):
        preset_key = self.settings.value("output/preset", PRESETS[0].key)
        if preset_key not in PRESETS_BY_KEY:
            preset_key = PRESETS[0].key
        self.preset_buttons[preset_key].setChecked(True)
        self._on_preset(preset_key)
        mode = self.settings.value("output/destination", DEST_PROXIES_FOLDER)
        index = max(0, self.dest_combo.findData(mode))
        self.dest_combo.setCurrentIndex(index)
        self.custom_dir.setText(str(self.settings.value("output/custom_dir", "") or ""))
        self._on_destination_mode()

        def as_bool(key, default):
            value = self.settings.value(key, default)
            return value if isinstance(value, bool) else str(value).lower() == "true"

        self.skip_existing.setChecked(as_bool("options/skip_existing", True))
        self.include_subfolders.setChecked(as_bool("options/include_subfolders", False))
        self.low_priority.setChecked(as_bool("options/low_priority", True))
        self.verify.setChecked(as_bool("options/verify", True))
        self.verify.toggled.connect(lambda v: self.settings.setValue("options/verify", v))
        self.skip_existing.toggled.connect(lambda v: self.settings.setValue("options/skip_existing", v))
        self.include_subfolders.toggled.connect(lambda v: self.settings.setValue("options/include_subfolders", v))
        self.low_priority.toggled.connect(lambda v: self.settings.setValue("options/low_priority", v))
        parallel = int(self.settings.value("options/parallel", 0) or 0)
        self.parallel_combo.setCurrentIndex(max(0, self.parallel_combo.findData(parallel)))
        self.parallel_combo.currentIndexChanged.connect(
            lambda _i: self.settings.setValue("options/parallel", self.parallel_combo.currentData())
        )

    def _apply_theme(self, theme_name):
        self.theme_name = theme_name if theme_name in THEME_COLORS else "dark"
        self.theme_colors = THEME_COLORS[self.theme_name]
        self.theme_toggle.blockSignals(True)
        self.theme_toggle.setChecked(self.theme_name == "light")
        self.theme_toggle.setText("Light theme" if self.theme_name == "light" else "Dark theme")
        self.theme_toggle.setToolTip(
            "Click to switch to the dark theme" if self.theme_name == "light"
            else "Click to switch to the light theme"
        )
        self.theme_toggle.blockSignals(False)
        self.setStyleSheet(theme_stylesheet(self.theme_name))
        self.settings.setValue("appearance/theme", self.theme_name)
        for item in self.items.values():
            self._color_status(item)

    # ------------------------------------------------------------- options

    def current_preset(self):
        for key, rb in self.preset_buttons.items():
            if rb.isChecked():
                return PRESETS_BY_KEY[key]
        return PRESETS[0]

    def _on_preset(self, key):
        preset = PRESETS_BY_KEY[key]
        self.settings.setValue("output/preset", key)
        hints = {
            "prores": "ProRes is edit-friendly in every editor. Encoded on the CPU; "
                      "decoding and resizing use the GPU where possible.",
            "h264": "H.264 gives the smallest files and plays everywhere. 8-bit 4:2:0.",
            "h265": "H.265 gives small files and keeps 10-bit sources at 10-bit.",
        }
        self.preset_hint.setText(f"{hints[preset.family]} Output: {preset.extension[1:].upper()}.")
        for path, item in self.items.items():
            self._fill_proxy_size(item, self.infos.get(path))

    def _on_destination_mode(self, *_):
        mode = self.dest_combo.currentData()
        self.settings.setValue("output/destination", mode)
        custom = mode == DEST_CUSTOM
        self.custom_dir.setVisible(custom)
        self.custom_browse.setVisible(custom)

    def browse_custom_dir(self):
        start = self.custom_dir.text() or str(Path.home())
        folder = QFileDialog.getExistingDirectory(self, "Select a folder for all proxies", start)
        if folder:
            self.custom_dir.setText(folder)

    # ---------------------------------------------------------------- files

    def add_files(self):
        patterns = " ".join(f"*{ext}" for ext in sorted(SUPPORTED_EXTENSIONS))
        files, _ = QFileDialog.getOpenFileNames(
            self, "Add video files", str(self.settings.value("paths/last_dir", "") or ""),
            f"Video files ({patterns});;All files (*.*)"
        )
        if files:
            self.settings.setValue("paths/last_dir", str(Path(files[0]).parent))
            self.add_paths([Path(f) for f in files])

    def add_folder(self):
        folder = QFileDialog.getExistingDirectory(
            self, "Add a folder of video files", str(self.settings.value("paths/last_dir", "") or "")
        )
        if folder:
            self.settings.setValue("paths/last_dir", folder)
            self.add_paths([Path(folder)])

    def add_paths(self, paths):
        if self.is_busy():
            return
        files = []
        for p in paths:
            if p.is_dir():
                files += collect_media(p, self.include_subfolders.isChecked(), self.suffix)
            elif p.suffix.lower() in SUPPORTED_EXTENSIONS and not is_proxy_file(p, self.suffix):
                files.append(p)
        new = []
        for f in files:
            key = str(f)
            if key.lower() in (k.lower() for k in self.items):
                continue
            item = QTreeWidgetItem([f.name, "…", "…", "…", "…", "Waiting", ""])
            item.setToolTip(COL_FILE, str(f))
            item.setData(COL_FILE, Qt.UserRole, key)
            self.tree.addTopLevelItem(item)
            self.items[key] = item
            new.append(f)
        if new:
            self.status.showMessage(f"Added {len(new)} file(s).", 3000)
            self._probe(new)
        elif files:
            self.status.showMessage("Those files are already in the list.", 3000)
        else:
            self.status.showMessage("No supported video files found.", 3000)
        self._update_controls()

    def _probe(self, paths):
        if not self.ffprobe:
            for p in paths:
                self.items[str(p)].setText(COL_STATUS, "FFprobe not found")
            return
        worker = ProbeWorker(self.ffprobe, paths, self)
        worker.probed.connect(self._on_probed)
        worker.finished.connect(lambda w=worker: self.probe_workers.remove(w) if w in self.probe_workers else None)
        self.probe_workers.append(worker)
        worker.start()

    def _on_probed(self, path, info, error):
        item = self.items.get(path)
        if item is None:
            return
        if info is None:
            self.infos.pop(path, None)
            for col in (COL_RES, COL_CODEC, COL_DURATION, COL_PROXY):
                item.setText(col, "—")
            item.setText(COL_STATUS, "Unreadable")
            item.setToolTip(COL_STATUS, error)
            self._color_status(item)
            return
        self.infos[path] = info
        item.setText(COL_RES, f"{info.width}×{info.height}")
        bits = " 10-bit" if info.is_10bit else ""
        item.setText(COL_CODEC, f"{info.codec.upper()}{bits}")
        item.setText(COL_DURATION, format_time(info.duration))
        tracks = len(info.audio_codecs)
        item.setToolTip(COL_CODEC, f"{info.pix_fmt}, {info.fps:.3f} fps, "
                                   f"{tracks} audio track{'s' if tracks != 1 else ''}")
        self._fill_proxy_size(item, info)

    def _fill_proxy_size(self, item, info):
        if info is None:
            return
        w, h = info.proxy_size(self.current_preset().divisor)
        item.setText(COL_PROXY, f"{w}×{h}")

    def remove_selected(self):
        if self.is_busy():
            return
        for item in self.tree.selectedItems():
            key = item.data(COL_FILE, Qt.UserRole)
            self.items.pop(key, None)
            self.infos.pop(key, None)
            self.tree.takeTopLevelItem(self.tree.indexOfTopLevelItem(item))
        self._update_controls()

    def clear_list(self):
        if self.is_busy():
            return
        for worker in self.probe_workers:
            worker.stop()
        self.tree.clear()
        self.items.clear()
        self.infos.clear()
        self.progress.setValue(0)
        self.current_label.setText("Ready.")
        self.stats_label.setText("Files: —   |   Speed: —   |   Time left: —")
        self._update_controls()

    def _tree_menu(self, pos):
        item = self.tree.itemAt(pos)
        if item is None:
            return
        menu = QMenu(self)
        open_src = menu.addAction("Open Source Folder")
        open_proxy = menu.addAction("Open Proxy Folder")
        open_proxy.setEnabled(item.data(COL_STATUS, Qt.UserRole) is not None)
        menu.addSeparator()
        remove = menu.addAction("Remove from List")
        remove.setEnabled(not self.is_busy())
        chosen = menu.exec(self.tree.viewport().mapToGlobal(pos))
        if chosen == open_src:
            self._open_path(Path(item.data(COL_FILE, Qt.UserRole)).parent)
        elif chosen == open_proxy:
            self._open_folder_for(item)
        elif chosen == remove:
            self.tree.setCurrentItem(item)
            self.remove_selected()

    def _open_folder_for(self, item):
        output = item.data(COL_STATUS, Qt.UserRole)
        if output and Path(output).parent.is_dir():
            self._open_path(Path(output).parent)

    @staticmethod
    def _open_path(path):
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(path)))

    def dragEnterEvent(self, event):
        if event.mimeData().hasUrls() and not self.is_busy():
            event.acceptProposedAction()

    def dropEvent(self, event):
        paths = [Path(u.toLocalFile()) for u in event.mimeData().urls() if u.isLocalFile()]
        if paths:
            self.add_paths(paths)
            event.acceptProposedAction()

    # -------------------------------------------------------------- hardware

    def _detect_hardware(self, refresh=False):
        if not self.ffmpeg:
            self.caps = Capabilities(details="FFmpeg not found.")
            self.hardware_label.setText(
                "FFmpeg was not found. Open Tools → Settings and select ffmpeg.exe."
            )
            self._update_status()
            return
        if self.capability_worker is not None and self.capability_worker.isRunning():
            return
        self.caps = None
        self.hardware_label.setText("Checking which encoders and decoders work on this PC…")
        self.status.showMessage("FFmpeg: checking…")
        self.capability_worker = CapabilityWorker(self.ffmpeg, refresh, self)
        self.capability_worker.detected.connect(self._on_capabilities)
        self.capability_worker.start()
        self._update_controls()

    def _on_capabilities(self, ffmpeg, caps):
        if ffmpeg != self.ffmpeg:
            return
        self.caps = caps
        self.hardware_label.setText(caps.summary())
        self._update_status()
        self._update_controls()

    def _update_status(self):
        if self.caps is None:
            self.status.showMessage("FFmpeg: checking…")
            return
        if not self.caps.ok:
            f = "FFmpeg: NOT FOUND"
        elif self.ffmpeg and self.ffmpeg == bundled_tool("ffmpeg"):
            f = "FFmpeg: included"
        else:
            f = "FFmpeg: OK"
        p = "FFprobe: OK" if self.ffprobe else "FFprobe: NOT FOUND"
        if self.caps.cuda_pipeline:
            gpu = "GPU decoding: NVIDIA"
        elif self.caps.d3d11_decode:
            gpu = "GPU decoding: D3D11"
        else:
            gpu = "GPU decoding: not available"
        self.status.showMessage(f"{f}   |   {p}   |   {gpu}")

    # ----------------------------------------------------------------- run

    def is_running(self):
        return self.batch_worker is not None and self.batch_worker.isRunning()

    def is_checking(self):
        return self.check_worker is not None and self.check_worker.isRunning()

    def is_busy(self):
        """Creating or checking proxies: the list and options are locked."""
        return self.is_running() or self.is_checking()

    def _update_controls(self):
        running = self.is_busy()
        has_files = bool(self.items)
        ready = has_files and not running and bool(self.caps and self.caps.ok and self.ffprobe)
        self.start_button.setEnabled(ready)
        self.start_action.setEnabled(ready)
        self.check_action.setEnabled(has_files and not running and bool(self.ffprobe))
        self.cancel_button.setEnabled(running)
        for w in (self.add_files_button, self.add_folder_button, self.remove_button,
                  self.clear_button, self.dest_combo, self.custom_dir, self.custom_browse,
                  self.skip_existing, self.parallel_combo, self.low_priority, self.verify):
            w.setEnabled(not running)
        for rb in self.preset_buttons.values():
            rb.setEnabled(not running)
        self.remove_action.setEnabled(not running)
        self.clear_action.setEnabled(not running)
        self.drop_hint.setVisible(not has_files)

    def start(self):
        if self.is_busy() or not self.items:
            return
        if not (self.caps and self.caps.ok):
            QMessageBox.warning(self, "FFmpeg required",
                                "FFmpeg is not ready. Open Tools → Settings and select ffmpeg.exe.")
            return
        mode = self.dest_combo.currentData()
        custom = self.custom_dir.text().strip()
        if mode == DEST_CUSTOM:
            if not custom:
                QMessageBox.information(self, "Destination", "Choose a folder for the proxies first.")
                return
            try:
                Path(custom).mkdir(parents=True, exist_ok=True)
            except OSError as exc:
                QMessageBox.critical(self, "Destination", f"Cannot use that folder:\n\n{exc}")
                return

        preset = self.current_preset()
        self.job_items = [self.tree.topLevelItem(i) for i in range(self.tree.topLevelItemCount())]
        sources = [Path(item.data(COL_FILE, Qt.UserRole)) for item in self.job_items]
        outputs = plan_outputs(sources, preset, mode, custom, self.suffix)
        jobs = [Job(s, o, self.infos.get(str(s))) for s, o in zip(sources, outputs)]
        self.job_engines = [""] * len(jobs)
        for item in self.job_items:
            item.setText(COL_STATUS, "Waiting")
            item.setData(COL_STATUS, Qt.UserRole, None)
            item.setToolTip(COL_STATUS, "")
            item.setText(COL_CHECK, "")
            item.setToolTip(COL_CHECK, "")
            self._color_status(item)

        batch = ProxyBatch(
            self.ffmpeg, self.ffprobe, self.caps, jobs, preset,
            parallel=int(self.parallel_combo.currentData() or 0),
            skip_existing=self.skip_existing.isChecked(),
            low_priority=self.low_priority.isChecked(),
            verify=self.verify.isChecked(),
        )
        self.batch_worker = BatchWorker(batch, self)
        self.batch_worker.event.connect(self._on_batch_event)
        self.batch_worker.finished.connect(self._update_controls)
        self.progress.setValue(0)
        self.current_label.setText(f"Creating {len(jobs)} proxies: {preset.label}, "
                                   f"{batch.parallel} file(s) at once…")
        self.stats_label.setText(f"Files: 0 of {len(jobs)} done   |   Speed: measuring…   |   "
                                 "Time left: measuring…")
        self._started_at = time.monotonic()
        self.batch_worker.start()
        self._update_controls()

    def cancel(self):
        if self.is_running():
            self.batch_worker.batch.cancel()
            self.cancel_button.setEnabled(False)
            self.current_label.setText("Cancelling…")
        elif self.is_checking():
            self.check_worker.stop()
            self.cancel_button.setEnabled(False)
            self.current_label.setText("Stopping the check…")

    # ---------------------------------------------------------- proxy check

    def check_proxies(self):
        """Check the proxies the current settings point to, without encoding."""
        if self.is_busy() or not self.items:
            return
        if not self.ffprobe:
            QMessageBox.warning(self, "FFprobe required",
                                "FFprobe was not found. Open Tools → Settings and select ffprobe.exe.")
            return
        mode = self.dest_combo.currentData()
        custom = self.custom_dir.text().strip()
        if mode == DEST_CUSTOM and not custom:
            QMessageBox.information(self, "Destination", "Choose the folder that holds the proxies first.")
            return
        items = [self.tree.topLevelItem(i) for i in range(self.tree.topLevelItemCount())]
        keys = [item.data(COL_FILE, Qt.UserRole) for item in items]
        outputs = plan_outputs([Path(k) for k in keys], self.current_preset(), mode, custom, self.suffix)
        for item in items:
            item.setText(COL_CHECK, "Checking…")
            item.setToolTip(COL_CHECK, "")
            self._color_status(item)
        self.check_counts = {"ok": 0, "problems": 0, "missing": 0}
        self.check_worker = CheckWorker(self.ffprobe, zip(keys, outputs), self.infos, self)
        self.check_worker.checked.connect(self._on_checked)
        self.check_worker.finished.connect(self._on_check_finished)
        self.current_label.setText(f"Checking {len(items)} proxies…")
        self.check_worker.start()
        self._update_controls()

    def _on_checked(self, key, output, check, message):
        item = self.items.get(key)
        if item is None:
            return
        if check is None:
            missing = not Path(output).is_file()
            self.check_counts["missing" if missing else "problems"] += 1
            item.setText(COL_CHECK, "No proxy" if missing else "Problems")
            item.setToolTip(COL_CHECK, message)
        else:
            self.check_counts["ok" if check.ok else "problems"] += 1
            self._show_check(item, check)
            item.setData(COL_STATUS, Qt.UserRole, output)
        self._color_status(item)

    def _on_check_finished(self):
        for item in self.items.values():
            if item.text(COL_CHECK) == "Checking…":
                item.setText(COL_CHECK, "")
        c = self.check_counts
        text = f"Checked proxies: {c['ok']} OK, {c['problems']} with problems"
        if c["missing"]:
            text += f", {c['missing']} without a proxy"
        self.current_label.setText(text + ". Hover over a result for details.")
        self._update_controls()

    def _show_check(self, item, check):
        item.setText(COL_CHECK, "OK" if check.ok else "Problems")
        item.setToolTip(COL_CHECK, check.text())

    def _on_batch_event(self, name, data):
        if name == "job_status":
            item = self.job_items[data["index"]]
            status, message = data["status"], data["message"]
            job = self.batch_worker.batch.jobs[data["index"]]
            if status == "Encoding":
                self.job_engines[data["index"]] = message
                item.setText(COL_STATUS, f"Encoding ({message})")
            elif status == "Done":
                item.setText(COL_STATUS, f"Done ({message})")
                item.setData(COL_STATUS, Qt.UserRole, str(job.output))
                item.setToolTip(COL_STATUS, str(job.output))
            else:
                item.setText(COL_STATUS, status)
                item.setToolTip(COL_STATUS, message)
                if status == "Skipped":
                    item.setData(COL_STATUS, Qt.UserRole, str(job.output))
            self._color_status(item)
        elif name == "job_progress":
            item = self.job_items[data["index"]]
            if item.text(COL_STATUS).startswith("Encoding"):
                where = self.job_engines[data["index"]] or "CPU"
                item.setText(COL_STATUS, f"Encoding ({where}) {data['percent']:.0f}%")
        elif name == "job_check":
            item = self.job_items[data["index"]]
            self._show_check(item, data["check"])
            self._color_status(item)
        elif name == "progress":
            self.progress.setValue(int(data["percent"] * 10))
            elapsed = format_time(time.monotonic() - self._started_at)
            speed = f"{data['fps']:.0f} fps" if data["fps"] > 0 else "measuring…"
            if data["eta"] is None:
                left = "measuring…"
            else:
                finish = datetime.now() + timedelta(seconds=data["eta"])
                left = f"{format_time(data['eta'])} (about {finish:%H:%M})"
            self.stats_label.setText(
                f"Files: {data['done']} of {data['total']} done   |   Speed: {speed}   |   "
                f"Elapsed: {elapsed}   |   Time left: {left}"
            )
        elif name == "finished":
            self._on_finished(data)
        elif name == "crashed":
            QMessageBox.critical(self, "ProxyCraft", data["message"])

    def _on_finished(self, data):
        elapsed = format_time(data["elapsed"])
        if data["cancelled"]:
            self.current_label.setText(f"Cancelled after {elapsed}. Unfinished proxies were removed.")
            return
        self.progress.setValue(1000 if not data["failed"] else self.progress.value())
        summary = (f"Created {data['created']}, skipped {data['skipped']}, "
                   f"failed {data['failed']} in {elapsed}.")
        problems = data.get("check_problems", 0)
        if data.get("checked"):
            summary += (f" Proxy check: {data['checked'] - problems} OK, "
                        f"{problems} with problems.")
        self.current_label.setText(summary)
        box = QMessageBox(self)
        box.setWindowTitle("ProxyCraft")
        box.setIcon(QMessageBox.Warning if data["failed"] or problems else QMessageBox.Information)
        if data["failed"]:
            box.setText("Proxy creation finished with errors. "
                        "Hover over a failed file's status for details.")
        elif problems:
            box.setText("Some proxies may not relink to their originals. "
                        "Hover over Problems in the Check column for details.")
        else:
            box.setText("Proxy creation finished.")
        box.setInformativeText(summary)
        open_button = None
        first_output = next((i.data(COL_STATUS, Qt.UserRole) for i in self.job_items
                             if i.data(COL_STATUS, Qt.UserRole)), None)
        if first_output:
            open_button = box.addButton("Open Proxy Folder", QMessageBox.ActionRole)
        box.addButton(QMessageBox.Ok)
        box.exec()
        if open_button is not None and box.clickedButton() == open_button:
            self._open_path(Path(first_output).parent)

    def _color_status(self, item):
        text = item.text(COL_STATUS)
        c = self.theme_colors
        if text.startswith("Done"):
            color = c["done"]
        elif text in ("Failed", "Unreadable"):
            color = c["failed"]
        elif text in ("Skipped", "Cancelled"):
            color = c["muted"]
        else:
            color = c["text"]
        item.setForeground(COL_STATUS, QBrush(QColor(color)))
        check = item.text(COL_CHECK)
        check_color = (c["done"] if check == "OK" else c["failed"] if check == "Problems"
                       else c["muted"])
        item.setForeground(COL_CHECK, QBrush(QColor(check_color)))

    # -------------------------------------------------------------- dialogs

    def settings_dialog(self):
        hardware = self.caps.summary() if self.caps else "Still checking…"
        d = SettingsDialog(self.ffmpeg, self.ffprobe, self.suffix, hardware, self)
        if not d.exec():
            return
        ffmpeg = d.ffmpeg.text().strip().strip('"')
        ffprobe = d.ffprobe.text().strip().strip('"')
        missing = [p for p in (ffmpeg, ffprobe) if p and not os.path.isfile(p)]
        if missing:
            QMessageBox.warning(self, "File not found", "These files do not exist:\n\n" + "\n".join(missing))
        self.suffix = d.suffix.text().strip() or DEFAULT_SUFFIX
        self.settings.setValue("naming/suffix", self.suffix)
        changed = ffmpeg != self.ffmpeg
        self.ffmpeg, self.ffprobe = ffmpeg, ffprobe
        # Store nothing for the included FFmpeg, so it is still found after
        # the ProxyCraft folder is moved or updated.
        self.settings.setValue("tools/ffmpeg", "" if ffmpeg == bundled_tool("ffmpeg") else ffmpeg)
        self.settings.setValue("tools/ffprobe", "" if ffprobe == bundled_tool("ffprobe") else ffprobe)
        if changed:
            self._detect_hardware(refresh=True)
        self._update_status()
        self._update_controls()

    def about(self):
        dlg = QDialog(self)
        dlg.setWindowTitle("About ProxyCraft")
        dlg.setWindowIcon(self.windowIcon())
        dlg.setMinimumWidth(560)
        layout = QVBoxLayout(dlg)
        layout.setContentsMargins(24, 22, 24, 18)
        layout.setSpacing(18)
        content = QHBoxLayout()
        content.setSpacing(20)
        icon_label = QLabel()
        icon_label.setPixmap(self.windowIcon().pixmap(72, 72))
        icon_label.setAlignment(Qt.AlignTop | Qt.AlignHCenter)
        content.addWidget(icon_label)
        details = QVBoxLayout()
        details.setSpacing(10)
        title = QLabel(f"ProxyCraft {__version__}")
        title.setObjectName("AboutTitle")
        details.addWidget(title)
        description = QLabel(
            "Fast proxy creator for video editing.<br>"
            "ProRes, H.264 and H.265 proxies, with GPU decoding and encoding where available."
        )
        description.setWordWrap(True)
        details.addWidget(description)
        details.addWidget(QLabel("Free and open source under the MIT License. "
                                 "Donations are completely optional."))
        contact = QHBoxLayout()
        contact.setSpacing(4)
        contact.addWidget(QLabel("Contact:"))
        email = QLabel(f"<a href='mailto:{DONATION_ADDRESS}'>{DONATION_ADDRESS}</a>")
        email.setTextFormat(Qt.RichText)
        email.setTextInteractionFlags(Qt.TextBrowserInteraction | Qt.TextSelectableByMouse)
        email.linkActivated.connect(lambda _l: QDesktopServices.openUrl(QUrl(f"mailto:{DONATION_ADDRESS}")))
        contact.addWidget(email)
        contact.addStretch()
        details.addLayout(contact)
        details.addWidget(QLabel("Copyright © 2026 Piotr Kubicki."))
        source = QLabel("<a href='https://github.com/PitKub/ProxyCraft'>Source code on GitHub</a>")
        source.setTextFormat(Qt.RichText)
        source.setOpenExternalLinks(True)
        details.addWidget(source)
        content.addLayout(details, 1)
        layout.addLayout(content)
        buttons = QHBoxLayout()
        buttons.addStretch()
        ok_button = QPushButton("OK")
        ok_button.clicked.connect(dlg.accept)
        buttons.addWidget(ok_button)
        layout.addLayout(buttons)
        dlg.exec()

    def donate_dialog(self):
        dlg = QDialog(self)
        dlg.setWindowTitle("Support ProxyCraft")
        dlg.setWindowIcon(self.windowIcon())
        dlg.resize(510, 330)
        layout = QVBoxLayout(dlg)
        layout.setContentsMargins(26, 24, 26, 20)
        layout.setSpacing(14)
        heading = QLabel("Enjoying ProxyCraft?")
        heading.setAlignment(Qt.AlignCenter)
        heading.setObjectName("DonateHeading")
        layout.addWidget(heading)
        message = QLabel(
            "ProxyCraft is free to use. If it helps your work and you would like to "
            "support its continued development, you can make a voluntary donation "
            "through PayPal. There is no obligation—thank you for using the app."
        )
        message.setWordWrap(True)
        message.setAlignment(Qt.AlignCenter)
        layout.addWidget(message)
        address = QLabel(f"<b>PayPal donation address</b><br>"
                         f"<span style='font-size:17px;'>{DONATION_ADDRESS}</span>")
        address.setAlignment(Qt.AlignCenter)
        address.setTextInteractionFlags(Qt.TextSelectableByMouse)
        address.setObjectName("DonateAddress")
        layout.addWidget(address)
        buttons = QHBoxLayout()
        copy_button = QPushButton("Copy PayPal address")
        paypal_button = QPushButton("Open PayPal")
        close_button = QPushButton("Close")

        def copy_address():
            QApplication.clipboard().setText(DONATION_ADDRESS)
            copy_button.setText("Address copied")

        copy_button.clicked.connect(copy_address)
        paypal_button.clicked.connect(lambda: QDesktopServices.openUrl(QUrl("https://www.paypal.com/")))
        close_button.clicked.connect(dlg.accept)
        buttons.addWidget(copy_button)
        buttons.addWidget(paypal_button)
        buttons.addStretch()
        buttons.addWidget(close_button)
        layout.addLayout(buttons)
        dlg.exec()

    def closeEvent(self, event):
        if self.is_running():
            r = QMessageBox.question(self, "Proxies are being created",
                                     "Proxy creation is still running. Stop it and quit?")
            if r != QMessageBox.Yes:
                event.ignore()
                return
            self.batch_worker.batch.cancel()
            self.batch_worker.wait(10000)
        if self.is_checking():
            self.check_worker.stop()
            self.check_worker.wait(35000)
        for worker in self.probe_workers:
            worker.stop()
            worker.wait(3000)
        if self.capability_worker is not None and self.capability_worker.isRunning():
            self.capability_worker.wait(60000)
        event.accept()


def main():
    app = QApplication(sys.argv)
    app.setApplicationName("ProxyCraft")
    app.setOrganizationName("PitKub")
    icon_path = app_icon_path()
    if icon_path.is_file():
        app.setWindowIcon(QIcon(str(icon_path)))
    w = MainWindow()
    w.show()
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
