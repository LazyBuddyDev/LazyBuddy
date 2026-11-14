# LazyBuddy 定向验证

新增功能带对应单元测试，只跑受影响范围，不默认执行全工作区或全量界面回归。主仓库测试宿主和 API 边界，独立服务维护业务与媒体处理测试。

```powershell
npm.cmd --prefix apps/desktop run check
npm.cmd --prefix apps/desktop test
npm.cmd --prefix apps/desktop run build
cargo test --locked -p lazybuddy host::tests
```

窗口改动选 `dock::tests` / `tray::tests`，播放桥选 `playback::tests`，设置选 `preferences::tests`。签名包安装、私有会话、控制通道及更新器分别在对应 crates 中测试。CI 按变更选择 Rust 模块与相关 Python 单元，依赖变化只扩大编译检查。

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

真实登录、上游播放、音画同步、在线签名发布及资源预算各自验收，不由单元或合成样例外推。禁止 cargo clean 或整体清空 target，独立服务 Git 与私有 SDK 不作构建缓存删除。
