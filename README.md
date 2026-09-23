<p align="center">
  <img src=".github/assets/banner.png" alt="Magpie Banner" />
</p>

<h1 align="center">Magpie</h1>

<p align="center">
  A fast, modern clipboard manager built with Tauri, React, and Rust.
</p>

<p align="center">
  <a href="./README.md">English</a> | <a href="./README.zh-CN.md">简体中文</a>
</p>

## Installation

### Homebrew (macOS)

```bash
brew install --cask kirklin/tap/magpie
```

This adds the [`kirklin/tap`](https://github.com/kirklin/homebrew-tap) tap and installs the latest build (Apple Silicon & Intel). To upgrade later:

```bash
brew upgrade --cask magpie
```

> Magpie is ad-hoc signed but not yet Apple-notarized. The cask automatically strips the quarantine flag on install, so the command above works as-is. If macOS still reports the app as "damaged", reinstall with `brew reinstall --cask --no-quarantine magpie`.

### Manual download

Grab the file for your system from the [latest release](https://github.com/kirklin/magpie/releases/latest):

| System                                   | File                                                                     |
| ---------------------------------------- | ------------------------------------------------------------------------ |
| macOS, Apple Silicon                     | `Magpie_<version>_aarch64.dmg`                                           |
| macOS, Intel                             | `Magpie_<version>_x64.dmg`                                               |
| Windows, x64                             | `Magpie_<version>_x64-setup.exe` or `Magpie_<version>_x64_en-US.msi`     |
| Windows, ARM64                           | `Magpie_<version>_arm64-setup.exe` or `Magpie_<version>_arm64_en-US.msi` |
| Debian, Ubuntu                           | `Magpie_<version>_amd64.deb` or `Magpie_<version>_arm64.deb`             |
| Fedora and other RPM-based distributions | `Magpie-<version>-1.x86_64.rpm` or `Magpie-<version>-1.aarch64.rpm`      |
| Any Linux distribution                   | `Magpie_<version>_amd64.AppImage` or `Magpie_<version>_aarch64.AppImage` |

> The Windows installers aren't code-signed yet, so SmartScreen may block the first launch: choose "More info", then "Run anyway". On Windows 10 without the WebView2 runtime, the installer downloads it.

## Opening Magpie

The default shortcut is <kbd>⌘</kbd><kbd>⇧</kbd><kbd>V</kbd> on macOS, <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>V</kbd> on Windows and <kbd>Ctrl</kbd><kbd>Alt</kbd><kbd>V</kbd> on Linux, where terminals already use <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>V</kbd> to paste. The tray icon opens it too.

## Linux desktops

Magpie runs on X11 and Wayland. Wayland leaves global shortcuts and typing into other apps to each desktop, so those two work differently from desktop to desktop:

| Desktop                                                  | Shortcut                                                                                                            | Pasting into the app you came from                   |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| Any desktop on X11                                       | Set in Magpie's settings                                                                                            | Works right away                                     |
| KDE Plasma 6 on Wayland                                  | Plasma asks once to allow it; change it from Magpie's settings                                                      | Plasma asks once to let Magpie control input devices |
| GNOME 48 and later on Wayland                            | GNOME asks once to allow it; change it on Magpie's page in GNOME Settings                                           | GNOME asks once to allow remote interaction          |
| GNOME 47 and earlier on Wayland                          | Add a custom shortcut in GNOME Settings that runs the command shown in Magpie's settings                            | GNOME asks once to allow remote interaction          |
| Sway, Hyprland, niri and other wlroots-based compositors | Bind a key to the command shown in Magpie's settings, for example `bindsym Ctrl+Alt+v exec magpie --toggle` in Sway | Works right away                                     |

Where a desktop offers no way to paste into other apps, Magpie puts the entry on the clipboard and you paste it with <kbd>Ctrl</kbd><kbd>V</kbd>.

The tray icon needs a desktop with a system tray. GNOME shows it only with the AppIndicator extension, which Ubuntu includes; elsewhere on GNOME, open Magpie with the shortcut.

Tiling compositors tile Magpie's window like any other. To keep it floating, add a rule for the app id `magpie`, for example `for_window [app_id="magpie"] floating enable` in Sway.

## License

Licensed under the [GPL-3.0](./LICENSE) license · © 2026 [Kirk Lin](https://github.com/kirklin)
