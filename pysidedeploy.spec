[app]
title = ProxyCraft
project_dir = .
input_file = launcher.py
exec_directory = dist
project_file = 
icon = assets/ProxyCraft.ico

[python]
packages = Nuitka==4.3.4, ziglang==0.16.0
android_packages = 

[qt]
qml_files = 
excluded_qml_plugins = 
modules = Core,Gui,Widgets
plugins = accessiblebridge,egldeviceintegrations,generic,iconengines,imageformats,platforminputcontexts,platforms,platforms/darwin,platformthemes,styles,wayland-decoration-client,wayland-graphics-integration-client,wayland-shell-integration,xcbglintegrations

[nuitka]
mode = onefile
extra_args = --quiet --noinclude-qt-translations --zig --windows-console-mode=disable --windows-icon-from-ico=assets/ProxyCraft.ico --include-data-dir=assets=assets --windows-company-name=PitKub --windows-product-name=ProxyCraft --windows-file-description=ProxyCraft --windows-product-version=1.0.1.0 --windows-file-version=1.0.1.0

