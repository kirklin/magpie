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

从 [最新 release](https://github.com/kirklin/magpie/releases/latest) 下载对应架构的 `.dmg`：

- Apple Silicon → `Magpie_<版本>_aarch64.dmg`
- Intel → `Magpie_<版本>_x64.dmg`

## 许可证

基于 [GPL-3.0](./LICENSE) 许可证开源 · © 2026 [Kirk Lin](https://github.com/kirklin)
