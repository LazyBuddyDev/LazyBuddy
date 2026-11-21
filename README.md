<p align="center">
  <img src="assets/lazybuddy.svg" alt="LazyBuddy" width="96" height="96">
</p>

<h1 align="center">LazyBuddy</h1>

<p align="center"><strong>摸鱼，也要有个搭子。</strong><br>你的 Windows 桌面摸鱼搭子，喜欢什么就装什么。</p>

<p align="center">
  <a href="https://github.com/LazyBuddyDev/LazyBuddy/releases/tag/v1.0.8"><img src="https://img.shields.io/badge/release-v1.0.8-24745a?style=flat-square" alt="版本 1.0.8"></a>
  <img src="https://img.shields.io/badge/platform-Windows%20x64-5869e8?style=flat-square" alt="Windows x64">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-24745a?style=flat-square" alt="MIT License"></a>
  <a href="https://github.com/LazyBuddyDev/LazyBuddy/actions/workflows/checks.yml"><img src="https://img.shields.io/github/actions/workflow/status/LazyBuddyDev/LazyBuddy/checks.yml?branch=main&style=flat-square&label=checks" alt="构建检查状态"></a>
</p>

<p align="center">
  <a href="https://lazybuddydev.github.io/LazyBuddy/"><strong>下载 Windows 版</strong></a> ·
  <a href="https://github.com/LazyBuddyDev/LazyBuddy/releases/tag/v1.0.8">发布说明</a> ·
  <a href="docs/README.md">文档</a> ·
  <a href="https://github.com/LazyBuddyDev/LazyBuddy/issues">反馈问题</a>
</p>

---

LazyBuddy 是你的桌面摸鱼搭子，把喜欢的消遣放在手边。功能按需安装，想歇一会就呼出，忙起来就贴边收起，让摸鱼也有自己的节奏。

## 为什么用 LazyBuddy

| | |
| --- | --- |
| **按需安装** | 各项功能独立安装、更新和卸载，所需运行时由应用统一管理。 |
| **随手可用** | 托盘、全局快捷键、窗口置顶与左右贴边收起。 |
| **安静待命** | 切换功能时保留页面状态，非当前页面暂停任务，隐藏或收起时安静待命。 |
| **调成你的样子** | 浅深主题、背景与卡片透明度、快捷键设置。 |
| **开源可接入** | Tauri 2 / Svelte 5 / Rust 宿主，功能服务独立，通过明确 API 接入。 |

## 下载与开始使用

**[前往下载页 →](https://lazybuddydev.github.io/LazyBuddy/)**　或直接下载 **[Windows x64 安装程序](https://github.com/LazyBuddyDev/LazyBuddy/releases/download/v1.0.8/LazyBuddy-Setup-windows-x86_64.exe)**。

1. 运行 EXE 安装向导，选择新的安装目录。界面需要 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/#download-section)，基础宿主不需要 Python、Go 或媒体 SDK。
2. 打开「功能」，选择需要的功能，再从左上角下拉菜单切换。目前提供新闻速览、红果短剧、小说阅读和B站视频。
3. 在设置中调整外观与快捷键。窗口的「×」隐藏到托盘；彻底退出使用托盘菜单中的「退出」。

目前仅支持 **Windows x64**。Release 中的 ZIP 用于应用内部更新，首次安装请使用 EXE。1.0.7 及更早版本升级到 1.0.8 需先退出并卸载原宿主，再手动安装一次；默认保留设置、账号与共享模块。详见[安装与卸载](docs/uninstall.md)及[发布说明](https://github.com/LazyBuddyDev/LazyBuddy/releases/tag/v1.0.8)。

用户已确认本轮B站和红果登录实播正常；历史短剧取源空响应根因尚未定位，来源可用性与单集权限需分别确认。当前限制见[发布说明](https://github.com/LazyBuddyDev/LazyBuddy/releases/tag/v1.0.8)。

## 从源码运行

准备 Rust/MSVC、Windows SDK、Node.js 24 和 WebView2 Runtime，然后在仓库根目录运行统一入口：

```powershell
.\start.bat
```

仅构建使用 `-BuildOnly`，发布构建加 `-Release`。构建前会正常退出本项目对应实例；构建失败不会启动旧产物。前端资源嵌入 EXE，可选功能仍通过模块管理安装。小说功能支持本地TXT/EPUB及在线阅读；B站功能提供搜索、扫码登录、推荐、关注动态和选集，均在应用的“功能”入口按需安装。更多参数与开发包用法见[构建与 CLI](docs/cli.md)。

## 文档与参与

| 入口 | 内容 |
| --- | --- |
| [文档导航](docs/README.md) | 安装、开发、测试与分发的完整入口 |
| [架构](docs/architecture.md) · [模块契约](docs/module-api.md) | 宿主边界、模块生命周期与服务接入 |
| [UI 规范](docs/ui-guidelines.md) · [定向测试](docs/testing.md) | 公共交互和验证方式 |
| [维护指南](AGENTS.md) | 开始贡献前阅读的长期协作规范 |
| [GitHub Issues](https://github.com/LazyBuddyDev/LazyBuddy/issues) | 报告问题、讨论需求 |

当前功能服务各自维护业务实现、依赖和文档：[news-service](https://github.com/LazyBuddyDev/news-service) · [hongguo-service](https://github.com/LazyBuddyDev/hongguo-service) · [reader-service](https://github.com/LazyBuddyDev/reader-service) · [bilibili-service](https://github.com/LazyBuddyDev/bilibili-service)。

## 许可

自有代码采用 [MIT License](LICENSE)，Copyright © 2026 Leager。第三方组件保留原许可。
