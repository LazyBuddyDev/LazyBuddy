# 模块签名与在线分发

宿主和功能包独立分发。根`module-catalog.json`包含可信Ed25519公钥及HTTPS目录地址；构建与基础打包均携带该配置。模块管理器先验目录签名，再核对ZIP的大小、SHA-256、路径、清单、平台和精确依赖。签名不等于Windows Authenticode，不能由此宣称EXE取得代码签名证书。

## 分发边界

当前宿主版本为`v1.0.7`，使用`docs/modules/stable-v6-catalog.json`及其签名，新增B站bilibili0.1.2不可变资源（独立服务仓库`v0.1.2`）；reader0.1.16、news0.1.2、hongguo0.2.13和python-runtime0.1.0继续复用既有地址与精确依赖。1.0.6保留`stable-v5-catalog.json`、1.0.5保留v4和reader0.1.11、1.0.4保留v3和reader0.1.3；1.0.2–1.0.3保留v2，全部旧目录、签名和不可变资源保持原字节。现用通道沿用同一Ed25519身份；1.0.0及更早客户端需先手动安装新宿主，不能自动信任新身份。

各功能及其运行时由所属独立服务仓库构建，宿主不导入服务源码。按每个实际清单的精确依赖闭包安装；某个功能的运行时不能成为其他功能或基础宿主的先决条件，也不能混用旧包。当前 Web 宿主不使用可选原生 `media-runtime`，具体包、来源与版本见签名目录。包版本独立于第三方引擎版本，控制协议或适配实现变化同样增加包版本。Python 依赖固定在所属项目的 `pyproject.toml`，用私有环境完成标准 wheel 安装，保留可用的 stdin/stdout，Windows 使用已验证的 `pythonw` 入口。包不包含构建 SDK、缓存、凭据或试验数据。

## 维护者身份

真实维护者私钥保存在工作区外的`%LOCALAPPDATA%/LazyBuddyDev/signing/module-catalog-ed25519.dpapi`，仅由当前用户DPAPI保护后保存，签名时只在进程内存解密，不导出、不上传或打包。DPAPI文件通常依赖原机器和用户，不是跨机器恢复备份；丢失身份需显式规划公钥轮换，不能静默换钥。

维护工具环境统一为`target/tools/release/`，使用已验证的`cryptography==46.0.3`。已有私钥必须复用，普通构建不生成新身份；只有首次建立身份另获授权时才使用`--initialize-key`。详见[文件放置规范](file-layout.md)。

## 组装与签名

以下为发布操作参考，执行前核对当前授权、实际ZIP与来源提交。占位参数须替换；目录JSON、签名和可信配置的输出文件必须尚不存在，使用新暂存位置，不覆盖未经核对的旧产物。

```powershell
python -B scripts/assemble-catalog.py --packages <news.zip> <hongguo.zip> <python-runtime.zip> --base-url <HTTPS模块资源目录> --output target/packages/catalog/web-catalog.json
.\target\tools\release\Scripts\python.exe -B scripts/sign-catalog.py --catalog target/packages/catalog/web-catalog.json --protected-key "$env:LOCALAPPDATA\LazyBuddyDev\signing\module-catalog-ed25519.dpapi" --source target/packages/catalog/module-catalog.json --url <HTTPS目录地址>
.\start.bat -BuildOnly -Release
python -B scripts/package_base.py --output target/packages/base
```

核对新配置公钥与既有客户端信任根一致，再按发布范围更新正式配置。基础包输出目录也须为新的或已核实清理的位置。先上传完整模块ZIP，再将签名目录原文与`.sig`一起原样部署至`docs/modules/`，由Pages提供固定目录端点。`.gitattributes`禁止换行转换；目录与签名不同步时允许校验失败，不能关闭校验。功能页按目录与已安装状态提供安装/更新/卸载。

正式版前测试安装包复用现有测试分发入口，不为每次修复新建预览发布；同步包、校验值、说明、来源提交与证据。实现变化仍增加模块内部版本与精确依赖，使客户端识别更新；不可变包目录不能复用。正式稳定版快照另按授权建立。

## 在线验收

匿名HTTPS重新下载目录、签名和ZIP，验证真实客户端安装、整组依赖激活、卸载回收与运行中入口。使用隔离数据，不读取真实凭据，不触发业务上游请求；结束后退出自身实例并清理下载及模块副本。离线签名和解包单元不能代替真实下载验收。发布前核对项目与第三方许可、依赖精确来源及实际独立包。涉及账号的功能包发布须单独进行一次真实账号验收，账号恢复、集合写入和节目权限按实际能力分别验证，显式豁免随本次交付记录。

下载统一使用显式选择 NativeTls 的 ureq Agent，证书与签名校验均保留。仅启用 Cargo feature 不会选择 NativeTls，下载不得绕过该 Agent。目录总超时30秒，每包1800秒，读取60秒、连接15秒。Windows遵循显式代理环境变量，否则通过 ureq 的 `win-system-proxy` 使用当前用户已启用的手动系统代理；不修改系统配置，也不声称支持PAC/WPAD。必须包含清除HTTP/HTTPS/ALL_PROXY环境变量后的桌面等价安装验证。失败提示保留包 ID 和脱敏错误类别，禁止输出签名跳转地址或代理凭据。

应用更新使用另一份签名清单，不能通过模块包替换宿主；见[应用更新](app-updates.md)。
