# 模块签名与在线分发

宿主和功能包独立分发。根`module-catalog.json`包含可信Ed25519公钥及HTTPS目录地址；构建与基础打包均携带该配置。模块管理器先验目录签名，再核对ZIP的大小、SHA-256、路径、清单、平台和精确依赖。签名不等于Windows Authenticode，不能由此宣称EXE取得代码签名证书。

## 分发边界

当前宿主版本为`v1.0.10`，使用`docs/modules/stable-v7-catalog.json`及其签名，更新bilibili0.1.4与hongguo0.2.17；reader0.1.16、news0.1.2和python-runtime0.1.0复用既有不可变包。用户已明确接受本轮身份轮换和手动安装一次：1.0.7及更早客户端不能自动获得新信任根，需手动安装1.0.8或更新版本；1.0.8起沿用同一身份，后续可自动更新。旧v6及更早目录、签名与资源保留原字节。

各功能及其运行时由所属独立服务仓库构建，宿主不导入服务源码。按每个实际清单的精确依赖闭包安装；某个功能的运行时不能成为其他功能或基础宿主的先决条件，也不能混用旧包。当前 Web 宿主不使用可选原生 `media-runtime`，具体包、来源与版本见签名目录。包版本独立于第三方引擎版本，控制协议或适配实现变化同样增加包版本。Python 依赖固定在所属项目的 `pyproject.toml`，用私有环境完成标准 wheel 安装，保留可用的 stdin/stdout，Windows 使用已验证的 `pythonw` 入口。包不包含构建 SDK、缓存、凭据或试验数据。

## 维护者身份

维护者私钥的本地副本保存在工作区外的`%LOCALAPPDATA%/LazyBuddyDev/signing/release-v3-ed25519.dpapi`，由当前用户DPAPI保护；同一身份已按用户明确授权加密存入GitHub受保护签名环境。签名时仅在进程内存使用，私钥不进入Git、Release、日志或普通构建产物。DPAPI文件通常依赖原机器和用户，不是跨机器恢复备份；丢失身份需显式规划公钥轮换，不能静默换钥。

维护工具环境统一为`target/tools/release/`，使用已验证的`cryptography==46.0.3`。已有私钥必须复用，普通构建不生成新身份；只有首次建立身份另获授权时才使用`--initialize-key`。详见[文件放置规范](file-layout.md)。

## 多端统一签名

所有执行端使用同一发布身份；不能按PC生成私钥。Windows DPAPI文件通常不能直接复制到另一台机器解密。本地文件的存在或同名不代表身份正确，必须先与仓库内应用和模块信任根同时核对公钥。

集中签名已启用：1.0.8先由本机签名发布，其后经用户明确授权将同一私钥加密迁入环境，云端产物已用现有客户端公钥验签，并与已发布签名逐字节核对一致。其他PC无需持有私钥或复制DPAPI文件，直接调用统一入口。集中入口为本仓库的 `Sign release metadata` 工作流，使用 `release-signing` 受保护环境中的 `LAZYBUDDY_RELEASE_SIGNING_KEY` Secret。环境限制为main分支；只有明确workflow_dispatch可调用，默认只读仓库权限。私钥只在签名进程内存中使用，不输出到Git、Release、日志或工作流产物；产物仅包含公开JSON原文和签名。调用端下载后必须用当前客户端公钥再次验签，不能信任工作流返回的新公钥。

首次配置已完成；仅在维护密钥存储时需要在持有当前有效私钥的那台PC更新到main后执行一次，使用已安装cryptography46.0.3的私有工具环境：

```bash
./target/tools/release/Scripts/python.exe -B scripts/release-signing.py register --protected-key "$LOCALAPPDATA/LazyBuddyDev/signing/release-v3-ed25519.dpapi"
```

该命令先读取远端main的两份公开信任根，拒绝过期执行端，再核对本机身份，失败时不上传；成功时通过本地stdin将现有密钥交给GitHub CLI，由CLI加密存入环境Secret，不产生明文临时文件或命令行密钥值。保留原DPAPI文件作为原机器恢复来源，不能把它当成跨机器备份。使用环境Secret意味着GitHub成为受信任签名执行端；具有修改main与签名环境权限的维护者必须受控，启用环境审核或进一步分离仓库时按团队权限配置。

后续PC只提交两份公开清单的精确字节base64到workflow_dispatch的 `catalog_base64` / `update_base64`；至少一项。签名器核对既有公钥、应用版本、平台、官方仓库资产地址、大小/哈希字段和模块精确依赖，拒绝未知格式；输出名称固定为 `catalog.json(.sig)` 和 `app-update.json(.sig)`，复制回正式端点时保留原字节。环境Secret未配置或不匹配即失败，不回退本机旧钥或临时身份。

当前有效私钥若在所有机器和既有安全存储中均已丢失，公钥或旧签名不能还原它；必须另行明确授权身份轮换和客户端引导安装，不能承诺旧客户端无感更新。

## 组装与签名

以下为发布操作参考，执行前核对当前授权、实际ZIP与来源提交。占位参数须替换；目录JSON、签名和可信配置的输出文件必须尚不存在，使用新暂存位置，不覆盖未经核对的旧产物。

```bash
python -B scripts/assemble-catalog.py --packages "$NEWS_ZIP" "$HONGGUO_ZIP" "$PYTHON_RUNTIME_ZIP" --base-url "$MODULE_RELEASE_URL" --output target/packages/catalog/web-catalog.json
./target/tools/release/Scripts/python.exe -B scripts/sign-catalog.py --catalog target/packages/catalog/web-catalog.json --protected-key "$LOCALAPPDATA/LazyBuddyDev/signing/release-v3-ed25519.dpapi" --source target/packages/catalog/module-catalog.json --url "$CATALOG_URL"
MSYS_NO_PATHCONV=1 cmd.exe /d /c "start.bat -BuildOnly -Release"
python -B scripts/package_base.py --output target/packages/base
```

核对新配置公钥与既有客户端信任根一致，再按发布范围更新正式配置。基础包输出目录也须为新的或已核实清理的位置。先上传完整模块ZIP，再将签名目录原文与`.sig`一起原样部署至`docs/modules/`，由Pages提供固定目录端点。`.gitattributes`禁止换行转换；目录与签名不同步时允许校验失败，不能关闭校验。功能页按目录与已安装状态提供安装/更新/卸载。

正式版前测试安装包复用现有测试分发入口，不为每次修复新建预览发布；同步包、校验值、说明、来源提交与证据。实现变化仍增加模块内部版本与精确依赖，使客户端识别更新；不可变包目录不能复用。正式稳定版快照另按授权建立。

## 在线验收

匿名HTTPS重新下载目录、签名和ZIP，验证真实客户端安装、整组依赖激活、卸载回收与运行中入口。使用隔离数据，不读取真实凭据，不触发业务上游请求；结束后退出自身实例并清理下载及模块副本。离线签名和解包单元不能代替真实下载验收。发布前核对项目与第三方许可、依赖精确来源及实际独立包。涉及账号的功能包发布须单独进行一次真实账号验收，账号恢复、集合写入和节目权限按实际能力分别验证，显式豁免随本次交付记录。

下载统一使用显式选择 NativeTls 的 ureq Agent，证书与签名校验均保留。仅启用 Cargo feature 不会选择 NativeTls，下载不得绕过该 Agent。目录总超时30秒，每包1800秒，读取60秒、连接15秒。Windows遵循显式代理环境变量，否则通过 ureq 的 `win-system-proxy` 使用当前用户已启用的手动系统代理；不修改系统配置，也不声称支持PAC/WPAD。必须包含清除HTTP/HTTPS/ALL_PROXY环境变量后的桌面等价安装验证。失败提示保留包 ID 和脱敏错误类别，禁止输出签名跳转地址或代理凭据。

宿主可取消还未激活的包下载。下载与解包不持有宿主设置和退出所用的提交锁。退出先使未提交任务失效，再等待已进入提交阶段的操作完成。异常中断留下的下载暂存由下次独占管理清理。

应用更新使用另一份签名清单，不能通过模块包替换宿主；见[应用更新](app-updates.md)。
