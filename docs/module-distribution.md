# 模块签名与在线分发

宿主和功能包独立分发。根`module-catalog.json`包含可信Ed25519公钥及HTTPS目录地址；构建与基础打包均携带该配置。模块管理器先验目录签名，再核对ZIP的大小、SHA-256、路径、清单、平台和精确依赖。签名不等于Windows Authenticode，不能由此宣称EXE取得代码签名证书。

## 分发边界

Web测试包复用`v2.0.0-preview.1`入口，浏览器模块在既有`modules-v0.1.0`中追加发布；Web可信目录为`docs/modules/web-catalog.json`及对应签名，随宿主配置该地址。旧`catalog.json`与原生包保留给旧客户端，不能混用。当前组合为news0.1.0、hongguo0.2.6、python-runtime0.1.0；在线验收及正式发布状态见[TASKS](../TASKS.md)。

新闻包由新闻仓库构建，红果及其运行时包由红果仓库构建，宿主不导入服务源码。浏览器路线默认需要news、hongguo与精确匹配的python-runtime；`media-runtime`是红果独立服务的可选原生能力，当前Web宿主不使用。以每个实际清单的依赖闭包为准，不能混用旧包。

## 维护者身份

真实维护者私钥保存在工作区外的`%LOCALAPPDATA%/LazyBuddyDev/signing/module-catalog-ed25519.dpapi`，仅由当前用户DPAPI保护后保存，签名时只在进程内存解密，不导出、不上传或打包。DPAPI文件通常依赖原机器和用户，不是跨机器恢复备份；丢失身份需显式规划公钥轮换，不能静默换钥。

维护工具环境统一为`target/tools/release/`，使用已验证的`cryptography==46.0.3`。已有私钥必须复用，普通构建不生成新身份；只有首次建立身份另获授权时才使用`--initialize-key`。详见[文件放置规范](file-layout.md)。

## 组装与签名

以下为发布操作参考，执行前核对当前授权、实际ZIP与来源提交。占位参数须替换；目录JSON、签名和可信配置的输出文件必须尚不存在，使用新暂存位置，不覆盖未经核对的旧产物。

```powershell
python -B scripts/assemble-catalog.py --packages <news.zip> <hongguo.zip> <python-runtime.zip> --base-url <HTTPS模块资源目录> --output target/packages/catalog/web-catalog.json
.\target\tools\release\Scripts\python.exe -B scripts/sign-catalog.py --catalog target/packages/catalog/web-catalog.json --protected-key "$env:LOCALAPPDATA\LazyBuddyDev\signing\module-catalog-ed25519.dpapi" --source target/packages/catalog/module-catalog.json --url <HTTPS目录地址>
.\build_and_run.bat -BuildOnly -Release
python -B scripts/package_base.py --output target/packages/base
```

核对新配置公钥与既有客户端信任根一致，再按发布范围更新正式配置。基础包输出目录也须为新的或已核实清理的位置。先上传完整模块ZIP，再将签名目录原文与`.sig`一起原样部署至`docs/modules/`，由Pages提供固定目录端点。`.gitattributes`禁止换行转换；目录与签名不同步时允许校验失败，不能关闭校验。功能页按目录与已安装状态提供安装/更新/卸载。

正式版前测试安装包复用现有测试分发入口，不为每次修复新建预览发布；同步包、校验值、说明、来源提交与证据。实现变化仍增加模块内部版本与精确依赖，使客户端识别更新；不可变包目录不能复用。正式稳定版快照另按授权建立。

## 在线验收

匿名HTTPS重新下载目录、签名和ZIP，验证真实客户端安装、整组依赖激活、卸载回收与运行中入口。使用隔离数据，不读取真实凭据，不触发业务上游请求；结束后退出自身实例并清理下载及模块副本。离线签名/解包单元不能代替真实下载验收，账号、收藏写入及节目权限另验。

下载统一使用显式选择NativeTls的ureq Agent，证书与签名校验均保留。目录总超时30秒，每包1800秒，读取60秒、连接15秒。Windows遵循显式代理环境变量，否则使用当前用户已启用的手动系统代理；不修改系统配置，也不声称支持PAC/WPAD。必须包含清除HTTP/HTTPS/ALL_PROXY环境变量后的桌面等价安装验证。

应用更新使用另一份签名清单，不能通过模块包替换宿主；见[应用更新](app-updates.md)。
