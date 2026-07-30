This directory is intentionally present in source control so ordinary Tauri
development builds can resolve the configured resource path.

scripts/build_windows_sidecar.ps1 places the generated PyInstaller onedir
bundle here for Windows packaging. Generated files are ignored; only this
placeholder is tracked.
