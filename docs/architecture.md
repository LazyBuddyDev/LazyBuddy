# 宿主与模块架构

LazyBuddy是Windows x64的Tauri/Svelte桌面宿主。前端资源嵌入EXE，WebView2负责界面和浏览器视频呈现；宿主通过Rust桥调用独立服务，不加载任意脚本插件或业务源码。

## 仓库所有权

| 仓库 | 负责内容 |
| --- | --- |
| [LazyBuddy](https://github.com/LazyBuddyDev/LazyBuddy) | 界面、窗口/托盘/热键、通用设置、模块与应用更新管理、CLI、API调度 |
| [news-service](https://github.com/LazyBuddyDev/news-service) | 新闻来源、解析、偏好、缓存及独立API |
| [hongguo-service](https://github.com/LazyBuddyDev/hongguo-service) | 账号、搜索、官方收藏、观看进度、取源与受控浏览器媒体资源 |
| [reader-service](https://github.com/LazyBuddyDev/reader-service) | 本地书库、TXT/EPUB 解析、在线来源、正文和图片、书架、阅读偏好及位置 |
| [bilibili-service](https://github.com/LazyBuddyDev/bilibili-service) | 视频搜索、推荐与关注动态、账号、分 P 和合集目录、受控浏览器媒体资源 |

独立服务通过明确 API 接入。新增能力需在宿主实现受信页面及调度，并按[宿主模块契约](module-api.md)声明页面模板。服务自主管理依赖版本、许可、构建与测试。

## 实现入口

| 职责 | 入口 |
| --- | --- |
| 根页面、功能管理、全局设置、提示与视图调度 | [App.svelte](../apps/desktop/src/App.svelte)、[lifecycle.ts](../apps/desktop/src/lifecycle.ts) |
| 新闻、短剧、阅读、视频根页面 | [News.svelte](../apps/desktop/src/News.svelte)、[Hongguo.svelte](../apps/desktop/src/Hongguo.svelte)、[Reader.svelte](../apps/desktop/src/Reader.svelte)、[Bilibili.svelte](../apps/desktop/src/Bilibili.svelte) |
| 原生启动、系统能力、宿主管理与设置持久化 | [main.rs](../apps/desktop/src-tauri/src/main.rs)、[platform.rs](../apps/desktop/src-tauri/src/platform.rs)、[host.rs](../apps/desktop/src-tauri/src/host.rs)、[preferences.rs](../apps/desktop/src-tauri/src/preferences.rs) |
| 私有服务桥接 | [playback.rs](../apps/desktop/src-tauri/src/playback.rs)、[reader.rs](../apps/desktop/src-tauri/src/reader.rs)、[bilibili.rs](../apps/desktop/src-tauri/src/bilibili.rs) |
| 包清单、精确依赖与页面模板 | [module-contract](../crates/module-contract) |
| 模块下载、安装、更新与注册表 | [module-manager](../crates/module-manager) |
| 服务进程树与私有持久会话 | [service-client](../crates/service-client) |
| 当前用户命令行、宿主控制协议与日志 | [CLI](../apps/cli/src/main.rs)、[host-control](../crates/host-control) |
| 应用更新验证、Windows 替换与卸载 | [app-updater](../crates/app-updater) |

公共样式、控件与媒体交互入口见 [UI 规范](ui-guidelines.md#实现入口与验证)。

## 安装与调用

基础宿主不包含业务服务或Python、Go、GStreamer。模块管理器从可信目录验签，核对ZIP哈希、大小、路径、目标平台与精确依赖后，准备整组不可变版本目录并原子激活注册表。

开发版与安装版通过同一Windows用户的公共模块目录复用已安装功能和运行时。服务客户端按清单启动包内入口，拥有其进程树，使用私有JSON Lines会话。宿主不通过工作区相对路径寻找服务实现或构建SDK。包与进程约定见[模块契约](module-api.md)。

## 页面与生命周期

根页面、功能页和设置页共用窗口外壳；功能页与设置页平级，关闭后回当前根页面。齿轮只打开全局设置；各内容页通过独立设置页签调整所属偏好。全局设置归宿主，内容偏好、业务缓存、账号及媒体归所属独立服务；仅由宿主绘制的局部外观偏好沿用宿主持久化，但只应用于所属内容。外观与公共交互统一遵循[UI规范](ui-guidelines.md)。

重复选择当前根页面保持现有展示。切换时保留搜索、筛选、列表与子页滚动位置，返回先显示已有内容，再按需核对新鲜度与会话。滚动及其他迟到事件同样受视图代数约束。

切页、进入设置、隐藏、最小化或贴边收起时，停止旧视图的派发、重试、订阅、计时器和动画，尽可能取消在途任务。可见设置只运行当前操作所需请求。恢复只激活当前可见内容。托盘、热键及[应用更新](app-updates.md)使用独立调度。请求并发与媒体会话边界见[宿主模块契约](module-api.md#生命周期)。

## 状态与信任

全局配置有序、原子保存，退出等待必要收尾。生产实例沿用已验收的安全存储scope和数据位置；隔离测试使用独立目录与身份空间，不导出或复制凭据。账号状态与已确认数据按服务契约隔离，集合的乐观反馈遵循 [UI 规范](ui-guidelines.md#集合标记与展示资源)。

模块目录与应用更新是两套签名入口；安装成功、账号恢复和真实节目权限分别验证。发布与验证步骤见[模块分发](module-distribution.md)、[应用更新](app-updates.md)和[定向测试](testing.md)。当前未完成任务及其交付边界只在[TASKS](../TASKS.md)维护，完成后删除，不设任务归档。
