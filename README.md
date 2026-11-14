# LazyBuddy

Windows x64 桌面搭子。Tauri 2 + Svelte 5 Web 宿主负责界面、窗口和模块调度，新闻与红果短剧由独立服务提供，按需安装与卸载。旧 Iced 宿主已删除。

## 构建与启动

需要 Rust/MSVC、Windows SDK、Node.js 24 和 Microsoft Edge WebView2 Runtime。基础宿主不需要 Python、Go 或 GStreamer。

```powershell
.\build_and_run.bat
.\build_and_run.bat -BuildOnly
.\build_and_run.bat -BuildOnly -Release
.\build_and_run.bat -LaunchOnly
```

脚本使用 `npm.cmd`，不受 PowerShell 的 npm.ps1 执行策略影响。前端资源嵌入 EXE，可信目录配置与更新配置复制到 EXE 旁。构建失败不启动旧产物；正在运行的程序请从托盘「退出」后再构建。关闭按钮隐藏到托盘，托盘左键恢复，拖到左右屏幕边缘收起为圆形 Logo。

在「功能」中按需安装、更新、卸载；根页面下拉只显示已安装功能。新闻、账号和媒体请求仅在对应可见页面执行，隐藏停止读取视频，恢复后不自动播放。

当前 Web 源码尚未发布为软件版本。旧预览 Release 和线上红果 0.1.0 包属于原生播放路线，不能作为 Web 播放包使用；本地使用独立红果仓库生成的 0.2.1 以上浏览器包（当前验证 0.2.5）。Web 应用更新使用独立签名源 `updates/web-windows-x86_64.json`，发布前检查会报告源不可用，不会下载旧 Iced 宿主。模块和应用更新发布仍需另行执行。

## 开发边界

| 路径 | 职责 |
| --- | --- |
| `apps/desktop/src` | Svelte 界面、浏览器视频与页面状态 |
| `apps/desktop/src-tauri` | 唯一 Rust 桌面入口、Windows 托盘/热键/窗口、API 桥 |
| `apps/cli` | 离线模块管理、服务调用与宿主控制 |
| `crates/module-contract` | 清单、依赖和页面协议 |
| `crates/module-manager` | 签名目录、校验、原子安装更新与卸载 |
| `crates/service-client` / `host-control` | 私有服务会话与当前用户 CLI |
| `crates/app-updater` | 签名更新准备、正常退出后替换及失败回滚 |

[news-service](https://github.com/LazyBuddyDev/news-service) 与 [hongguo-service](https://github.com/LazyBuddyDev/hongguo-service) 独立构建和测试；主仓库不加载相邻服务源码。开发包安装见 [构建与 CLI](docs/cli.md)，架构见 [模块化架构](docs/modular-architecture.md)，验证见 [定向测试](docs/testing.md)。长期规则见 [AGENTS.md](AGENTS.md)。

自有代码 MIT，Copyright © 2026 Leager；第三方保留原许可。真实账号和节目权限的验证不能由合成测试替代。
