# LazyBuddy CLI 与构建

唯一宿主为 `apps/desktop` 的 Tauri/Svelte 应用。先由根目录脚本构建前端，再构建 Rust；直接 cargo 构建前必须有 `apps/desktop/dist`。

```powershell
.\build_and_run.bat -BuildOnly
.\build_and_run.bat -Release
```

默认输出在 `target/debug` 或 `target/release`；可显式设置 `CARGO_TARGET_DIR`。脚本仅在 node_modules 缺失时安装精确 npm 依赖，使用 npm.cmd。无需媒体 SDK，不自动下载业务运行时。构建前通过CLI正常退出与目标EXE路径匹配的本项目实例，等待退出后直接构建，不生成独立候选文件，不关闭其他安装实例。正常退出或构建失败时停止，不启动旧产物。`-LaunchOnly` 只启动已有新入口，`-ServicePackages <绝对目录>` 是显式红果开发包入口，不能代替可信线上安装。

构建工具与SDK按[文件放置规范](file-layout.md)由所属服务独立管理；具体准备入口见[红果服务README](https://github.com/LazyBuddyDev/hongguo-service/blob/main/README.md)，移动宿主安装位置不需要复制这些构建输入。

生产模块和全局配置位于 `%LOCALAPPDATA%/LazyBuddy`，Web 设置文件为 `web-preferences.json`。红果沿用已验证的 `%LOCALAPPDATA%/LazyBuddyShellPreview/hongguo` 与安全存储 scope `shell-preview`，避免复制或导出凭据；这是服务会话命名空间，不再表示另一套宿主入口。隔离实例的红果数据改用其独立数据目录。

同一Windows用户下，开发版与安装版默认共享 `%LOCALAPPDATA%/LazyBuddy/modules`：注册表位于该目录，功能和Python等运行时按精确版本保存在 `packages/<id>/<version>/`。开发版安装过的模块可由之后安装的新Web版本直接使用，反之亦然，无需按EXE目录重复下载。运行时从公共模块目录启动，构建工具链不作为运行依赖；显式指定隔离实例或自定义 `LAZYBUDDY_DATA_DIR` 时使用各自目录。

```powershell
$env:LAZYBUDDY_INSTANCE='local-qa'
$env:LAZYBUDDY_DATA_DIR=(Join-Path (Get-Location) 'tmp/profiles/local-qa')
.\target\debug\lazybuddy.exe --background
```

隔离实例跳过全局热键、自启动修改和自动更新，托盘仍可退出。`--background` 不激活内容页面。CLI 继承相同实例环境。

## 开发包与统一启动

用户统一运行根目录`build_and_run.bat`。已准备、完整的本地功能包按`target/packages/development/<id>/<version>/module.json`归位（目录不进Git），也可显式`-ModulePackages <目录>`指定同样结构的根目录。脚本只为已安装的功能选择较新版本，正常退出当前工作区实例后通过CLI的`install-local`安装，再启动新宿主；不自动安装新功能或下载重运行时，不停止其他安装。

这些是明确准备的开发包，不是在线失败回退，也不从相邻服务仓库源码加载实现。服务改动必须先在独立仓库构建并打成新版本完整包，再交付到上述目录；仅修改服务源码不会改变已安装模块。开发包准备属于开发交付，用户不需要手动调用独立服务脚本。`-BuildOnly`构建前同样正常退出匹配实例，但不安装模块或启动应用。`-LaunchOnly`跳过构建，但仍应用已准备的较新本地功能包。

日志入口打开`%LOCALAPPDATA%/LazyBuddy/logs`，当前运行日志统一为`LB.log`；红果0.2.11起只返回API诊断，不写服务日志。宿主负责文本格式、代码位置和下次启动时按上次关闭时间归档。更新宿主时须配套更新本地开发模块，由唯一BAT入口完成。

## 功能包

卸载整个应用使用安装目录内的`uninstall.exe`，可选择是否回收已下载的共享模块；设置和账号默认保留。详见[Windows卸载](uninstall.md)。

```powershell
.\target\debug\lazybuddy-cli.exe modules list
.\target\debug\lazybuddy-cli.exe modules install-local <包含module.json的目录>
.\target\debug\lazybuddy-cli.exe modules install <ID> <HTTPS签名目录URL> <原始Ed25519公钥文件>
.\target\debug\lazybuddy-cli.exe modules uninstall <ID>
```

宿主运行时使用功能页，CLI 拒绝直接修改包。开发时先安装精确依赖，再安装功能包；红果 Web 入口要求 0.2.1 以上服务（当前分发 0.2.13）和其声明的 Python 运行时。Web 宿主不使用 media-runtime。更新/卸载会先停止对应服务并等账号事务正常收尾，包完整准备后原子激活；失败保留已安装状态。

随 EXE 提供 `module-catalog.json` 和 `app-update-source.json`，使用维护者公钥验签。不能把未签名本地包作为线上下载失败的回退。

## 宿主控制

```powershell
$request='{"version":1,"command":"status","args":{}}'
& .\target\debug\lazybuddy-cli.exe host $request
```

Web 宿主命令为 `status`、`modules.list`、`window.show`、`window.hide` 、`updates.check/apply` 和 `app.quit`。`status.data.backend` 为 `web`。旧 Iced 专属播放器/截图控制协议已移除；界面验收通过自有隔离 WebView/CDP 驱动进行。独立服务调用仍使用 `lazybuddy-cli call <ID> <JSON请求>`，不输出凭据或媒体能力 URL。
