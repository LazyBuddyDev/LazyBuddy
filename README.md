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

脚本使用 `npm.cmd`，不受 PowerShell 的 npm.ps1 执行策略影响。前端资源嵌入 EXE，可信目录配置与更新配置复制到 EXE 旁。构建失败不启动旧产物；当前实例运行时先旁路构建，成功后再正常退出并替换。-BuildOnly仅构建，不退出、安装模块或启动。关闭按钮隐藏到托盘，托盘左键恢复，拖到左右屏幕边缘收起为圆形 Logo。

开发中的1.0.0改用Windows安装向导EXE，构建方式见[Windows安装与卸载](docs/uninstall.md)。

卸载整个应用时，双击安装目录内的 `uninstall.exe`；可选移除已下载模块，设置和账号保留。详见[Windows卸载](docs/uninstall.md)。

在「功能」中按需安装、更新、卸载；根页面下拉只显示已安装功能。新闻、账号和媒体请求仅在对应可见页面执行，隐藏停止读取视频，恢复后不自动播放。

当前发布版本为[LazyBuddy 1.0.0](https://github.com/LazyBuddyDev/LazyBuddy/releases/tag/v1.0.0)，首次安装下载[Windows安装程序](https://github.com/LazyBuddyDev/LazyBuddy/releases/download/v1.0.0/LazyBuddy-Setup-windows-x86_64.exe)。可信模块目录提供新闻0.1.2、红果0.2.13与Python运行时0.1.0，按需安装。ZIP仅供应用内部签名更新使用。

本次经授权更换了丢失的签名身份，新客户端使用独立stable模块和应用更新端点。旧版用户需手动安装1.0.0一次以取得新公钥，之后继续自动更新；旧web端点保持原签名。安装程序拒绝覆盖已有安装，已有登记时先正常退出并卸载宿主，再运行新安装程序；默认保留账号、设置及共享模块。已知上游取源空响应和部分来源超时见发布说明及[TASKS](TASKS.md)。

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

[news-service](https://github.com/LazyBuddyDev/news-service) 与 [hongguo-service](https://github.com/LazyBuddyDev/hongguo-service) 独立构建和测试；主仓库不加载相邻服务源码。开发包安装见 [构建与 CLI](docs/cli.md)，架构见 [架构](docs/architecture.md)，验证见 [定向测试](docs/testing.md)。外观与公共交互遵循[UI规范](docs/ui-guidelines.md)。完整入口见[文档导航](docs/README.md)，文件归位见[文件放置规范](docs/file-layout.md)，长期规则见 [AGENTS.md](AGENTS.md)。

自有代码 MIT，Copyright © 2026 Leager；第三方保留原许可。真实账号和节目权限的验证不能由合成测试替代。
