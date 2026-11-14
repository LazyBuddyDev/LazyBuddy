# LazyBuddy 模块化架构

组织为 [LazyBuddyDev](https://github.com/LazyBuddyDev)，自有代码 MIT，Copyright © 2026 Leager。

| 仓库 | 边界 |
| --- | --- |
| LazyBuddy | Tauri/Svelte Web 前端、Windows 窗口/托盘/热键、通用设置、模块与更新管理、CLI、API 调度 |
| news-service | 新闻来源、解析、偏好与缓存、独立 API/构建/测试 |
| hongguo-service | 账号、搜索、官方收藏、断点、取源与受控浏览器媒体资源、私有 Python 依赖 |

基础宿主不打包新闻、红果、Python、Go 或 GStreamer。模块版本目录不可变，整组精确依赖准备成功后原子激活，卸载回收无引用运行时。线上目录验证 Ed25519 签名，包验证哈希/大小/路径/入口。开发包只通过明确本地入口安装。

前端通过固定 Rust 命令调用已安装服务 API，不通过相对路径加载业务源码，不增加任意脚本插件框架。新功能按 module-contract 定义服务清单与明确的前端模板。

同一时刻只有当前可见根页面工作；后台服务停止/暂停与前端代数双重约束迟到结果。设置与功能页平级，窗口隐藏保留状态但停止页面工作，系统托盘/热键/应用更新独立运行。

当前结构、构建与验证见 README、cli.md 和 testing.md。执行进度及验收边界以 TASKS.md 为准，软件发布与源码合并分别管理。
