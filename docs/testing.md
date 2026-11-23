# LazyBuddy 定向验证

新增功能带对应单元测试，只跑受影响范围，不默认执行全工作区或全量界面回归。主仓库测试宿主和 API 边界，独立服务维护业务与媒体处理测试。

提醒核心用`node --experimental-strip-types --test apps/desktop/tests/attention.test.ts`；保存边界用`cargo test --locked -p lazybuddy preferences::tests`。无头入口`node apps/desktop/tests/attention-page.mjs`检查来源聚合、悬停/键盘确认、重载持久化、新版本/构建、保存失败回滚和隐藏隔离，使用本地样例，不访问账号；报告与浅深主题截图在`tmp/verification-attention`。应用更新就绪与目录变化样例不代替真实线上下载验收。

```powershell
npm.cmd --prefix apps/desktop run check
npm.cmd --prefix apps/desktop test
npm.cmd --prefix apps/desktop run build
cargo test --locked -p lazybuddy host::tests
```

窗口改动选 `dock::tests` / `tray::tests`，播放桥选 `playback::tests`，设置选 `preferences::tests`。签名包安装、私有会话、控制通道及更新器分别在对应 crates 中测试。CI 按变更选择 Rust 模块与相关 Python 单元，依赖变化只扩大编译检查。

本地阅读边界用`node --experimental-strip-types --test apps/desktop/tests/reader.test.ts apps/desktop/tests/lifecycle.test.ts`、`cargo test --locked -p lazybuddy reader::tests`及`cargo test --locked -p lazybuddy-module-contract reading_template`。服务解析和持久化测试在独立reader-service仓库运行`cargo test --locked`。

真实阅读包的隔离集成入口为`node apps/desktop/tests/reader-host.mjs <宿主EXE绝对路径> <完整reader包目录> <完整news包目录>`；新闻包仅用于离线根页面切换，脚本准备独立缓存，不访问上游。脚本通过BAT测试新模块首次安装，再以CDP验证阅读/设置/隐藏/迟到隔离/删除；报告与截图在`tmp/verification-reader`，结束时quit并清理隔离书库。同一入口通过仅操作自有进程控件的`native-reader-dialog.py`验证原生文件选择与EPUB导入，不移动物理鼠标；控件消息验收与手动操作验收分别报告。

公共界面按[UI规范](ui-guidelines.md)验证，选择受影响的模式及已有使用方，不按功能重复建立外观标准。以下是现有模式验证入口，脚本名只表示当前实现位置：

| 验证模式 | 无头样例入口 |
| --- | --- |
| 聚合卡片、标题滚动及局部操作状态 | `apps/desktop/tests/news-page.mjs` |
| 平台视觉、公共工具栏、主题与窄窗口 | `apps/desktop/tests/news-design.mjs` |
| 统一页签、集合工具栏、列表/网格与封面状态 | `apps/desktop/tests/hongguo-views.mjs` |
| 画面点击、音量浮层与媒体快捷键 | `apps/desktop/tests/video-click.mjs` |
| 公共焦点输入方式、指针+按键、Tab/ShiftTab、编辑/录入与监听清理 | `apps/desktop/tests/focus-navigation.mjs`、`apps/desktop/tests/focus-navigation.test.ts` |
| 来源选择、分页重置、同书号隔离、正文派发及App接入端声明（旧网页试读来源隐藏） | `apps/desktop/tests/reader-sources.mjs` |
| 书架和浏览记录按页读取、跨页筛选、返回缓存、旧包兼容与迟到隔离 | `apps/desktop/tests/reader-library.mjs` |
| 书页方向键、三种模式、重排锚点、有界分块、章节边界与静默 | `apps/desktop/tests/reader-paging.mjs` |
| 内容及设置的公共行、标签、下拉框、工具栏、宽窄窗口和浅深主题 | `apps/desktop/tests/ui-consistency.mjs` |
| 功能设置页签、全局设置隔离、局部透明度保存与静默 | `apps/desktop/tests/feature-settings.mjs` |
| 根页面切换后的提示归属 | `apps/desktop/tests/feature-notices.mjs` |
| 开发/正式构建的应用更新入口 | `apps/desktop/tests/development-updates.mjs` |
| API诊断文本与运行日志归档 | `apps/desktop/tests/host-logs.mjs <宿主EXE绝对路径> <CLI绝对路径>` |

这些样例不代替真实账号、系统窗口行为或音画验收。使用本地媒体样例的入口需先准备对应测试素材；宿主级诊断样例还需显式假服务产物，不能因此调用真实账号或加载相邻工作区业务实现。

桌面后台启动用 `python scripts/verify/test-background-startup.py --host-dir target/debug` 定向验证GUI子系统、从隐藏父控制台启动、关闭该控制台后继续驻留、后台不显示及正常quit；仅操作自有隔离进程，并核对真实自启动注册项未变化。它不代替下一次Windows登录的自然自启动验收。

开发构建的原生更新限制用 `python scripts/verify/test-development-updates.py --host-dir target/debug` 验证；使用隐藏隔离实例，检查CLI拒绝更新、缓存未生成与偏好保留，结束后正常退出并清理数据。

缩略窗的系统贴靠隔离用 `python scripts/verify/test-dock-snap.py --host-dir target/debug` 验证；隔离实例完成三轮左右移动和展开，读取原生边框/缩放/最大化样式及恢复尺寸，不移动物理鼠标。该检查不代替实际拖动时的系统贴靠预览验收。

显式界面验证（需 Windows 桌面与 WebView2，不移动物理鼠标）：

```powershell
node apps/desktop/tests/tray.mjs <宿主EXE绝对路径>
$env:LAZYBUDDY_TEST_NEWS_PACKAGE='<独立新闻包绝对路径>'
node apps/desktop/tests/host.mjs <宿主EXE绝对路径>
$env:LAZYBUDDY_BROWSER_PACKAGES='<独立红果合成服务包绝对路径>'
$env:LAZYBUDDY_BROWSER_FIXTURE_FILE='<自制合成MP4绝对路径>'
node apps/desktop/tests/browser-media.mjs <宿主EXE绝对路径>
```

host 驱动安装外部新闻包，预置新鲜合成新闻缓存，验证 API、主题恢复、左右贴边、圆形命中和运行时卸载，不访问新闻上游。browser-media 使用独立服务仓库生成的合成账号/媒体包，验证二维码取消、收藏反馈、实际视频推进/跳转/隐藏恢复、错误详情与重试，不读真实账号。隔离配置统一放 tmp/profiles，Rust/Python临时测试数据也在tmp下创建并在退出后清理；验证报告、截图和录屏统一放 tmp/verification-*，不再写入 target/verification-* 或 target/qa-*。本机合成媒体位于 tmp/fixtures/media，不随源码分发。测试安装包统一放 target/packages/base，维护者发布工具环境为 target/tools/release；更新进程验证脚本的默认报告为 tmp/verification-updates/update-process.json。

纯文档修改只检查导航、有效链接、示例入口与已签名文件字节完整性，不运行业务测试；目录用途统一见[文件放置规范](file-layout.md)。

真实接口按服务所声明的能力验证。身份恢复、目录、单条资源权限、远端集合读写、进度同步及实际媒体呈现分别取证。每项按实际涉及的能力执行。浏览器媒体验证分别检查控件、请求、管线以及真实帧时间。音频使用产品的封装与解复用路径，测实际 PCM 时长和频率。音画同步、实机视觉、安装升级、线上签名分发及资源预算分别报告，单元和合成样例不替代实机证据。

生命周期改动定向覆盖 A→B、A→B→A、设置切换与隐藏恢复。核对旧视图不再派发、迟到结果隔离、恢复无重复请求，且托盘、热键及相关宿主订阅继续工作。清理验证产物按[文件放置规范](file-layout.md#命名和迁移)执行。

轻量视频页面的定向入口：`node apps/desktop/tests/bilibili-page.mjs`使用显式样例媒体状态，检查未登录零内容请求、登录恢复/过期清除、公共播放器按钮顺序、进度预览/提交、手动重试保留位置、浅深/窄窗/长标题截图、页签缓存、选集CID派发、设置、隐藏和迟到隔离。`node apps/desktop/tests/video-click.mjs --capture-layout`核对另一媒体页面使用相同控制组件后的本地播放、音量和布局，截图统一输出至`tmp/verification-media-controls/`。`node apps/desktop/tests/bilibili-live.mjs <独立服务EXE绝对路径> <已明确授权的隔离凭据scope>`使用实际组件与官方服务，验证推荐/动态、真实画面与PCM、倍速、第二P和隐藏断流；此入口访问真实账号，不能默认运行或传入生产scope，音频输出在测试音频图中静默，报告不包含账号名或资源地址。它不代替原生宿主的实机视觉/扬声器验收。

`node apps/desktop/tests/hongguo-prefetch.mjs`使用本地真实媒体与固定600ms模拟准备延迟，对比普通打开和预加载接用时真实EOS到下一集首帧的耗时，检查单次预加载、最终断点只提交一次、隐藏零派发及恢复不自播。该入口也用样例弹幕检查默认关闭、滚动、显隐、跳转和换集。连播计时保存在`tmp/verification-hongguo-prefetch/timings.json`，来源和权限边界由独立服务测试维护。

`powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/verify/test-build-entry.ps1`使用与BAT相同的Windows PowerShell5.1，验证正常退出边界，并以原生UTF-8 JSON复现CP936解码失败及构建入口编码修复；无需真实账号或媒体请求。
