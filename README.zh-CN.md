<p align="center">
  <img src=".github/assets/banner.png" alt="Magpie Banner" />
</p>

<h1 align="center">Magpie</h1>

<p align="center">
  一个基于 Tauri、React 和 Rust 构建的快速、现代的剪贴板管理器。
</p>

<p align="center">
  <a href="./README.md">English</a> | <a href="./README.zh-CN.md">简体中文</a>
</p>

## 安装

### Homebrew（macOS）

```bash
brew install --cask kirklin/tap/magpie
```

这会自动添加 [`kirklin/tap`](https://github.com/kirklin/homebrew-tap) 并安装最新版本（同时支持 Apple Silicon 与 Intel）。之后升级：

```bash
brew upgrade --cask magpie
```

> Magpie 目前为 ad-hoc 签名、尚未做 Apple 公证。cask 会在安装时自动移除隔离标记，所以上面的命令可直接使用。若 macOS 仍提示应用"已损坏"，用 `brew reinstall --cask --no-quarantine magpie` 重新安装即可。

### 手动下载

从 [最新 release](https://github.com/kirklin/magpie/releases/latest) 下载对应系统的文件：

| 系统                       | 文件                                                               |
| -------------------------- | ------------------------------------------------------------------ |
| macOS，Apple Silicon       | `Magpie_<版本>_aarch64.dmg`                                        |
| macOS，Intel               | `Magpie_<版本>_x64.dmg`                                            |
| Windows，x64               | `Magpie_<版本>_x64-setup.exe` 或 `Magpie_<版本>_x64_en-US.msi`     |
| Windows，ARM64             | `Magpie_<版本>_arm64-setup.exe` 或 `Magpie_<版本>_arm64_en-US.msi` |
| Debian、Ubuntu             | `Magpie_<版本>_amd64.deb` 或 `Magpie_<版本>_arm64.deb`             |
| Fedora 等使用 RPM 的发行版 | `Magpie-<版本>-1.x86_64.rpm` 或 `Magpie-<版本>-1.aarch64.rpm`      |
| 任意 Linux 发行版          | `Magpie_<版本>_amd64.AppImage` 或 `Magpie_<版本>_aarch64.AppImage` |

> Windows 安装包暂时没有代码签名，第一次运行时 SmartScreen 可能会拦截，点"更多信息"，再点"仍要运行"即可。Windows 10 上如果没有 WebView2 运行时，安装程序会自动下载。

## 打开 Magpie

默认快捷键在 macOS 上是 <kbd>⌘</kbd><kbd>⇧</kbd><kbd>V</kbd>，在 Windows 上是 <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>V</kbd>，在 Linux 上是 <kbd>Ctrl</kbd><kbd>Alt</kbd><kbd>V</kbd>，因为 Linux 的终端已经用 <kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>V</kbd> 粘贴。也可以从托盘图标打开。

## Linux 桌面环境

Magpie 支持 X11 和 Wayland。Wayland 把全局快捷键和向其他应用输入按键的能力交给各个桌面环境自己决定，所以这两项在不同桌面上的表现不同：

| 桌面环境                                     | 快捷键                                                                                                   | 粘贴回原来的应用                              |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| X11 下的任意桌面                             | 在 Magpie 设置里修改                                                                                     | 直接可用                                      |
| Wayland 下的 KDE Plasma 6                    | Plasma 首次会询问是否允许，之后可以从 Magpie 设置里修改                                                  | Plasma 首次会询问是否允许 Magpie 控制输入设备 |
| Wayland 下的 GNOME 48 及更新版本             | GNOME 首次会询问是否允许，之后在 GNOME 设置里 Magpie 的应用页面修改                                      | GNOME 首次会询问是否允许远程交互              |
| Wayland 下的 GNOME 47 及更早版本             | 在 GNOME 设置里添加一个自定义快捷键，命令填写 Magpie 设置里显示的命令                                    | GNOME 首次会询问是否允许远程交互              |
| Sway、Hyprland、niri 等基于 wlroots 的合成器 | 在合成器配置里给 Magpie 设置里显示的命令绑定按键，例如 Sway 写 `bindsym Ctrl+Alt+v exec magpie --toggle` | 直接可用                                      |

如果某个桌面完全没有提供向其他应用粘贴的途径，Magpie 会把选中的记录放到剪贴板上，由你自己按 <kbd>Ctrl</kbd><kbd>V</kbd> 粘贴。

托盘图标需要桌面带有系统托盘。GNOME 只有装了 AppIndicator 扩展才会显示托盘图标，Ubuntu 已经自带这个扩展；其他 GNOME 发行版上请用快捷键打开 Magpie。

平铺式合成器会像对待其他窗口一样平铺 Magpie 的窗口。想让它保持浮动，给应用标识 `magpie` 加一条规则，例如 Sway 写 `for_window [app_id="magpie"] floating enable`。

## 许可证

基于 [GPL-3.0](./LICENSE) 许可证开源 · © 2026 [Kirk Lin](https://github.com/kirklin)
