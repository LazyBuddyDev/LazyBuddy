# 文件放置规范

本文件是LazyBuddy及其独立服务仓库的文件归位与命名依据。路径相对文件所属仓库；按需创建目录，不为未使用的工具或功能预建空结构。新增、移动文件前先确定其用途和所有者。

## 源码和入口

| 文件或内容 | 放置位置 | 边界 |
| --- | --- | --- |
| 产品介绍、最低运行条件、快速启动 | 根`README.md` | 保持简短，链接专题说明 |
| 长期开发约束和文档规范 | 根`AGENTS.md` | 不写执行流水，直接链接本文件 |
| 当前未完成的功能目标、TODO、状态、必要证据与阻塞 | 根`TASKS.md` | 一个完整需求一个稳定条目；完成后删除，不设归档区 |
| 文档导航 | `docs/README.md` | 新增、合并、删除文档时一起更新 |
| 前端页面、组件、样式 | 宿主`apps/desktop/src/` | 负责绘制与API调用，不放业务取源实现 |
| 原生窗口、系统能力、API桥 | 宿主`apps/desktop/src-tauri/` | 唯一桌面程序入口 |
| 命令行程序 | 宿主`apps/cli/` | 与宿主共用协议和管理能力 |
| 通用Rust协议、客户端、安装及更新逻辑 | 宿主`crates/<职责>/` | 不导入具体新闻、短剧业务 |
| 新闻、账号、搜索、收藏、媒体处理业务 | 对应独立服务仓库 | 本仓库只链接服务API，不复制服务源码 |
| 构建、打包、维护脚本 | `scripts/` | 验证驱动放`scripts/verify/`或所属测试目录 |
| Cargo工作区清单与锁文件、信任根配置 | 宿主根目录 | `Cargo.toml`、`Cargo.lock`、`module-catalog.json`、`app-update-source.json`，不含私钥 |
| npm清单与锁文件、前端及Tauri配置 | `apps/desktop/`及其`src-tauri/` | 跟随实际构建入口，不另复制到根目录 |
| 许可、第三方来源与忽略规则 | 根`LICENSE`、`.gitignore`及组件自带许可 | 组件自带许可随素材或源码保留，不随清理移除 |
| 自动化工作流 | `.github/workflows/` | 与实际构建和文档入口保持一致 |
| 产品图标、固定业务无关素材及第三方许可 | `assets/` | 不放验收截图、录屏或真实账号数据 |
| 协议清单样例 | `contracts/` | 使用通用样例，不包含业务服务实现 |
| 单元与契约测试 | 所属源码测试模块或`tests/` | 浏览器驱动放`apps/desktop/tests/` |

独立服务内部沿用其现有语言结构，例如`src/`、`resolver/`、`search/`、`runtimes/`。这些是该服务的源码，不是主仓库的运行依赖目录。`target/service-exports/`中已有独立Git仓库属于源码，不能按构建缓存清理；接入或交接以对应仓库自己的文件和提交为准。

## 文档归位

| 内容 | 放置位置 |
| --- | --- |
| 架构、模块边界、数据流 | `docs/architecture.md` |
| 通用界面外观、公共控件与交互模式 | `docs/ui-guidelines.md` |
| 构建、启动、CLI与数据位置 | `docs/cli.md` |
| 宿主包清单、通用传输与生命周期契约 | `docs/module-api.md` |
| 各服务自己的方法、字段、错误与限制 | 对应服务的`docs/module-api.md`等专题 |
| 定向测试入口与验证边界 | `docs/testing.md` |
| 模块包、目录签名与分发维护 | `docs/module-distribution.md` |
| 主程序签名更新与替换维护 | `docs/app-updates.md` |
| 卸载操作、文件归属与验证边界 | `docs/uninstall.md` |
| 一次性排查记录、报告、截图、录屏 | `tmp/`，有长期价值的结论再归并到专题 |
| 当前任务的交付边界、分支与必要验证证据 | `TASKS.md`中的未完成任务条目 |

一个主题只保留一份权威说明，跨仓库用链接引用；不把整份共享文档复制到多个仓库。小规模专题直接放`docs/`，确有多个独立主题时才增加目录层级，不为阶段或一次性任务建立文档树。

`docs/index.html`与`docs/icon.svg`是公开站点入口及品牌资源。`docs/modules/`、`docs/updates/`是公开机器分发端点，包含精确字节签名；它们不是普通说明文档，整理时不得随意改名、删除、格式化或变更换行。发布与改写签名内容按单独授权执行。

## 构建、工具和临时文件

| 用途 | 固定位置 |
| --- | --- |
| 宿主编译产物 | `target/debug/`、`target/release/` |
| 嵌套Rust工作区产物 | 该Cargo工作区默认位置，如服务的`search/target/release/` |
| 验证素材 | `tmp/fixtures/<用途>/`，媒体样例为`tmp/fixtures/media/` |
| 隔离配置、WebView用户数据 | `tmp/profiles/<验证入口>/` |
| Rust/Python临时测试数据 | `tmp/`下带用途前缀的临时目录，收尾删除 |
| 验证报告、截图、录屏 | `tmp/verification-<用途>/` |
| 测试安装包、解包目录和校验信息 | `target/packages/<用途>/`，宿主基础包为`target/packages/base/` |
| 维护者工具及独立环境 | `target/tools/<工具>/`，发布工具Python环境为`target/tools/release/` |
| 服务私有工具链 | 所属服务的`target/toolchains/<工具>/`，如`go/`、`python/` |
| 可选SDK | 所属服务的`target/sdks/<SDK>/`，如`gstreamer/` |
| 下载缓存、wheel和安装源 | 所属服务的`target/downloads/`，如`python-wheels/`、`runtime/` |
| 构建缓存 | 所属仓库的`target/cache/<工具>/`，如`go/`；Cargo缓存保持默认结构 |

`tmp/`与`target/`保持Git忽略。构建工具、SDK和下载源由所属仓库管理，宿主不能隐式使用相邻服务源码或开发机固定路径。不要将SDK头文件、缓存、工具环境或验证数据打进用户安装包。

## 工作区清理

从仓库根目录运行 `cleanup.bat`，它将参数和退出码传给 `scripts/cleanup.ps1`。脚本需要 Windows 上的 PowerShell 7 和 Git。默认预览，显式加 `-Apply` 才执行。输出为 JSON，每项包含相对路径、状态、字节数和跳过原因，并汇总候选及已删除字节数。删除失败、执行时遇到占用或边界问题，以及显式请求的目录被拒绝时返回非零退出码。已不存在的目录记为 `missing`，可重跑。字节数按文件长度计算，磁盘实际回收量会受硬链接和文件系统影响。

```bash
cmd.exe //c cleanup.bat
```

核对清单后执行。

```bash
cmd.exe //c cleanup.bat -Apply
```

| 对象 | 认领与回收规则 |
| --- | --- |
| 默认宿主 Cargo 增量缓存 | 仅检查 `target/debug/incremental` 和 `target/release/incremental` 的已识别子目录。每个 crate 和构建配置保留最近两份，其他目录及内部文件七天内均未修改时才列为候选。Cargo 可重建这些缓存。 |
| 失效的隔离运行 | `-ExpiredRun` 接受单个明确路径，位于 `tmp/qa-…` 或 `tmp/profiles/<运行名>`。维护者先核对目录归属、相关进程已正常退出、合成数据可重建、证据已留存，再显式传入。 |
| 已替换的开发包 | `-RetiredPackage` 接受 `target/packages/development/<模块>/<版本>` 的明确路径。先核对未跟踪验证记录和已有替代品，确认不再用于交付或回归。脚本复核清单、入口、更新版本和 Git 文件中的引用。共享运行时、正式安装包和已签名发布资产保留。 |
| 源码、证据和工具 | 保留 Git 仓库、未提交源码、用户数据、系统与运行时配置、私有 SDK、工具链、外部缓存、现用 EXE 和必要验证证据。禁止整体清理 `target` 或运行 `cargo clean`。 |

明确路径统一相对仓库根目录。先用同一参数预览，核对输出后再追加 `-Apply`。传多个路径时可用 PowerShell 数组从 Bash 调用。

```bash
pwsh.exe -NoProfile -Command '& ./scripts/cleanup.ps1 -ExpiredRun @("tmp/qa-example-finished", "tmp/profiles/example-finished")'
cmd.exe //c cleanup.bat -RetiredPackage target/packages/development/example/1.2.3
```

执行时重新核对目录归属、Git 状态、绝对路径、链接和运行占用。遇到重解析点保留整项，交由其创建方核对后移除链接本身。构建期间跳过缓存和包，避免结束用户进程或处理被占用的文件。剩余项按输出原因复核，可在问题解除后重跑。无需在每次构建前清缓存，也不要将清理加到构建或发布的隐式步骤。

新建测试脚本时，只在已忽略的临时位置创建本次独占目录。用 `finally`、测试清理回调或等价机制，正常退出自有进程并回收样例、隔离数据和临时安装包。清理前输出脱敏结果，必要证据落到 `tmp/verification-<用途>` 的稳定路径。遇到占用时留下精确相对路径和原因，便于复核。新增生成入口时，在对应文档写清输出归属、保留条件和重建入口。

修改清理规则时，运行 `scripts/verify/test-cleanup.ps1` 的隔离回归。该测试使用临时 Git 仓库验证选取、链接边界、运行占用和重复执行。

## 运行资源、账号和密钥

安装目录内的`uninstall.exe`由完整安装包直接携带，并纳入构建清单；不在首次启动时生成，不放入全局模块目录。运行时卸载助手用系统临时目录，验证时显式隔离到工作区`tmp/`，收尾只删除本次拥有的固定文件。

- 同一Windows用户的开发版与安装版默认共用`%LOCALAPPDATA%/LazyBuddy/modules/`，包按`packages/<id>/<version>/`保存，由模块管理器校验、安装和回收，不按EXE位置重复下载。
- 通用设置与应用更新缓存归`%LOCALAPPDATA%/LazyBuddy/`。服务持久数据使用宿主传入的目录或服务自身公开约定；现有数据位置见[构建与CLI](cli.md)，不因整理仓库移动或清空账号数据。
- 隔离验证使用独立实例、`tmp/`数据目录和凭据scope，不能覆盖生产配置、账号或服务。真实凭据只在Windows安全存储中。
- 维护者真实签名私钥保存在工作区外的DPAPI保护文件或安全存储中，不进入`target/tools/release/`、`tmp/`、Git或安装包。随机测试身份仅用于隔离样例并在收尾删除，不能替代真实维护者身份。

## 命名和迁移

- 目录和专题文档使用表明用途的英文小写kebab-case。`README.md`、`AGENTS.md`、`TASKS.md`等约定入口保留原名。
- 固定用途复用稳定名称，不按实验阶段、日期或流水号另建同类目录。并行临时实例可以附唯一标识，协议要求的不可变版本目录照常保留。
- 不新建或恢复`target/qa-*`、`target/verification-*`、`target/release-tools`及混合工具链的`native-player-sdk`目录。
- 移动时同步脚本、CI、文档链接、示例和清理路径；Python虚拟环境须核对解释器、入口脚本和依赖，不能只移动文件夹。
- 删除或递归移动前核对绝对路径、链接边界、Git状态和运行占用；不得整体清空`target`、执行`cargo clean`，或把独立Git仓库、未推送源码、私有SDK、用户数据当成垃圾。
- TASKS.md不保留已完成任务历史。清理被替代条目前先核对完整目标，部分覆盖时保留剩余事项和接手所需证据；长期设计归专题，一次性验证文件留在tmp，不另建任务归档。现存证据路径按实际位置注明，不能改写验证边界或伪造当前状态。
