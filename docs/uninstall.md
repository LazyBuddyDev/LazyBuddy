# Windows安装与卸载

分发入口为`LazyBuddy-Setup-windows-x86_64.exe`：中文安装向导选择目录，按当前用户安装，无需管理员权限。默认目录为`%LOCALAPPDATA%/Programs/LazyBuddy`，只含基础宿主；可选功能仍按需安装。安装不修改系统代理或用户数据，完成页可选择启动。

安装目录必须尚不存在；安装程序不覆盖既有目录或运行实例。已有安装使用应用内更新；重新安装前通过Windows“已安装的应用”或目录内卸载程序移除。当前用户已有安装记录时拒绝另装，避免覆盖卸载入口。

完整安装包直接携带`uninstall.exe`，位于`lazybuddy.exe`同目录；安装完成即可使用，无需启动主程序。双击它确认卸载；正在运行的本安装会正常退出，不强杀进程。

随后可选择是否同时移除已下载的功能模块和运行时。默认只卸载主程序；模块由同一Windows用户的各安装共享，确认不再需要后才选删除。设置、账号凭据及个人文件始终保留。本功能不会卸载WebView2、系统Python、开发SDK或其他程序。

程序只删除经过构建清单校验的自有文件，不递归清空安装目录。安装文件遭修改、存在链接、退出超时或其他安装正在占用控制通道时，报告失败并保留程序文件；未知文件不删，目录非空时保留目录。自启动仅在注册值指向本安装时移除。

## 实现与更新边界

`uninstall.exe`是独立原生程序，与宿主、CLI和更新助手一起直接打入同一安装包，并纳入`build-info.json`的哈希清单。安装EXE与应用更新ZIP来自同一份完整文件清单，不生成启动入口、不依赖第一次运行，也不维护旧包兼容分支。更新时卸载程序随主程序一起校验、替换或回滚。

实际删除由系统临时目录中的独立进程执行，等待安装内助手和所属宿主正常退出，再清理已核实的文件。临时收尾仅删除本次创建的固定文件，不递归清理系统临时目录。卸载失败保留临时错误证据，不将部分删除报告为完整成功。

## 构建安装程序

```powershell
./scripts/build-and-run.ps1 -BuildOnly -Release
python -B scripts/package_base.py --output target/packages/base
$compiler = ./scripts/prepare-nsis.ps1
python -B scripts/package_installer.py --bundle target/packages/base --compiler $compiler
```

输出目录须新建；已有产物不覆盖。EXE用于用户安装，ZIP仅用于签名自动更新。编译器为官方[NSIS 3.11](https://nsis.sourceforge.io/Download)，准备脚本校验固定SHA256；仅使用zlib压缩，第三方许可随包保存在licenses/installer。安装后创建当前用户的Windows卸载登记，不创建桌面快捷方式。卸载只清理InstallLocation仍匹配本安装的登记，保留其他安装登记。

本地生成安装包不等于已公开发布；发布网站、Release附件和签名更新清单需同轮按授权切换。

## 定向验证

```powershell
cargo test --locked -p lazybuddy-app-updater uninstall::tests --lib
python -B scripts/verify/test-uninstall-process.py --host-dir <已构建宿主目录>
```

测试只使用`tmp/`内的独立安装、假功能包和数据哨兵，覆盖默认保留模块、明确移除模块、退出、自有文件删除、个人文件/设置/账号哨兵保留及助手自清理，不读取或卸载真实用户实例。更新共用校验边界变化时额外运行app-updater相关单元，发布时验证当前包结构的实际在线更新。
