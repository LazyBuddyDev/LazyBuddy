# 应用自动更新

正式构建默认在启动时和每15分钟后台检查、下载并准备应用更新；这是全局能力，切换根页面、进入设置或隐藏窗口不会停止它。隔离实例默认不自动联网。系统设置可关闭自动检查，也可随时手动检查。准备好后底部显示「重启更新」，点击后正常保存、退出并替换程序；不会自动打断播放或强行重启。

Debug开发构建禁用应用更新检查、下载和替换，也不显示应用更新开关、手动检查与重启更新入口；开发版通过根目录`start.bat`重新构建启动。该限制由宿主按构建模式执行，不能通过CLI绕过，不改写用户保存的自动更新偏好。可选功能的安装和更新独立保留。

当前最新版本为1.0.10；1.0.8和1.0.9可在同一通道自动升级，无需再次手动安装。1.0.8起使用新身份与`stable-v3-windows-x86_64.json`签名更新源。用户已接受本轮换钥：1.0.7及更早客户端需要手动安装1.0.8一次，不能通过旧公钥自动信任新身份；后续继续使用v3通道。旧v2和更早更新清单及签名保留原字节，不用新钥覆盖。

## 测试包与信任

测试包复用原下载入口，不能只比较版本号。基础包携带build-info.json，记录本包文件哈希；规范化文件表的SHA256作为构建标识，同版本内容变化仍会被识别为更新。app-update-source.json固定独立更新目录URL和维护者Ed25519公钥，使用已有发布身份，不从下载响应信任新公钥。

固定目录为https://lazybuddydev.github.io/LazyBuddy/updates/stable-v3-windows-x86_64.json及对应.sig。先验证原文签名、平台、版本/构建、ZIP大小及哈希，再验证build-info与每个文件哈希。ZIP拒绝越界、链接、重复及非应用文件；更新仅覆盖宿主/CLI/卸载程序/更新助手、可信源配置与许可说明，不替换模块、账号、用户设置或其他目录。元数据1MiB、ZIP256MiB、解压512MiB有界；沿用Windows TLS、系统代理与慢网下载保护。

## Windows替换

已校验的独立lazybuddy-updater.exe复制到更新缓存运行，防止更新自身时被占用。辅助进程核对父进程真实路径、获取独占安装锁，准备就绪后才让宿主正常退出，等待最多30秒。替换前备份已有文件，失败恢复已替换项；新宿主的PID和窗口控制接口就绪后才确认成功。准备或等待失败不覆盖仍在运行的应用，无法完整恢复时保留备份供修复，不能声称已恢复。

下载缓存位于当前数据目录updates；成功后清理解压、ZIP和备份，仍运行的辅助EXE下次检查再回收，不停止用户实例清理占用。只在受控隔离实例验证替换，不使用真实账号/播放做更新验收。Windows进程等待依据[Microsoft文档](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject)，独立进程处理Windows运行文件占用的模式亦参考[self-replace](https://docs.rs/self-replace/latest/self_replace/)。

签名身份及维护工具环境见[模块分发](module-distribution.md)，文件归位见[文件放置规范](file-layout.md)。

## 发布步骤

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/build-and-run.ps1 -BuildOnly -Release
python -B scripts/package_base.py --output target/packages/base --download-url <HTTPS正式不可变包地址>
.\target\tools\release\Scripts\python.exe -B scripts/sign-update.py --manifest target/packages/base/app-update.json --protected-key "$env:LOCALAPPDATA\LazyBuddyDev\signing\release-v3-ed25519.dpapi" --output docs/updates/stable-v3-windows-x86_64.json --replace
```

基础包输出目录须为新目录或已核实清理的旧目录，不能覆盖未经核对的文件。package_base生成基础ZIP、package.json和未签名app-update.json；默认URL仍是测试包，稳定发版须显式传--download-url。正式快照不可覆盖，Web线上清单只指向经确认的正式构建；后续测试包使用独立配置与清单，不将测试更新推给稳定客户端。签署目录后先覆盖上传ZIP/校验值，再推送docs/updates原文与签名到Pages；.gitattributes禁止对已签名JSON换行转换。短暂目录/包不同步时校验失败并保留现有应用，可稍后检查，绝不关闭校验或回退到未签名更新。

仅在首次启用自动更新的旧客户端需要引导安装；启用后未来测试包覆盖会自动获取，用户只需点击重启更新。模块仍由功能管理按独立签名目录更新。

## 定向验证

运行cargo test -p lazybuddy-app-updater --lib、cargo test -p lazybuddy host::tests、python -B scripts/verify/test-app-release.py。共享下载代码变化才额外检查module-manager的package单元；不默认跑工作区、真实业务或全量界面。实际进程替换由独立更新链路脚本验证，不能把文件复制单元成功当作线上更新成功。
