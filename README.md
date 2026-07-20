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

Grab the `.dmg` for your architecture from the [latest release](https://github.com/kirklin/magpie/releases/latest):

- Apple Silicon → `Magpie_<version>_aarch64.dmg`
- Intel → `Magpie_<version>_x64.dmg`

## License

Licensed under the [GPL-3.0](./LICENSE) license · © 2026 [Kirk Lin](https://github.com/kirklin)
