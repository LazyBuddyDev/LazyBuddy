# 宿主模块契约

API v1，目标平台Windows x64。本文只定义宿主的包、传输与生命周期边界；[新闻API](https://github.com/LazyBuddyDev/news-service/blob/main/docs/module-api.md)、[红果API](https://github.com/LazyBuddyDev/hongguo-service/blob/main/docs/module-api.md)和[阅读API](https://github.com/LazyBuddyDev/reader-service/blob/main/docs/module-api.md)由所属服务维护。源码边界见[架构](architecture.md)，文件归位见[规范](file-layout.md)。

## 清单与进程

ZIP根目录包含`module.json`，使用`schema: 1`、`service_api: 1`、三段式不可变包版本与`target: "windows-x86_64"`。ID以字母开头，仅含小写字母、数字及连字符。入口必须是包内相对路径。完整字段与校验由宿主`crates/module-contract`定义，通用样例见`contracts/fixtures`；各服务维护自己的实际清单。

功能包声明页面模板；`news_grid`指定`catalog_method`与`fetch_method`，`media_library`指定`search_method`，仅具备官方收藏能力时声明可选的`favorites_method`，`book_library`指定`library_method`与`read_method`。它们表达受支持的界面契约，不是任意HTML或脚本入口。当前宿主对已支持能力有明确页面实现和附加方法；新服务接入仍须完成对应前端与调度，不能仅靠清单自动获得完整页面。新增模板需要配套新宿主，旧客户端不能直接安装未知模板的包；在线目录本身不包含页面模板，本次不修改已发布目录或签名。

运行时包使用`kind: "runtime"`，没有页面。依赖固定精确包版本，`runtime`选择解释器依赖；宿主按运行时入口/参数、功能入口/参数组合启动。依赖目录通过`LAZYBUDDY_DEPENDENCY_<大写ID且连字符转下划线>_DIR`传入。代码与运行时留在各自不可变包目录，持久数据使用`LAZYBUDDY_DATA_DIR`。

宿主拥有服务进程树，通过私有stdin/stdout持久会话调用，不读取相邻工作区实现。关闭stdin请求服务正常退出，等待必要保存和已提交事务收尾，不立即强杀。服务必须保持stdout纯协议，日志不能混入回复。

## JSON Lines

每行一个UTF-8 JSON对象，请求与回复保留相同版本和字符串ID：

```json
{"version":1,"id":"health-1","method":"health","args":{}}
{"version":1,"id":"health-1","ok":true,"data":{"service":"news","api_version":1}}
{"version":1,"id":"call-2","ok":false,"error":{"code":"unknown_method"}}
```

宿主串行化私有传输请求。客户端要处理失败与未知错误码；字段边界、方法和错误语义以对应服务API为准。耗时事务应异步返回待完成状态，由当前可见调用方按所属服务API查询确认，不能占住低延迟控制操作。接受请求、官方确认、资源准备好和实际解码成功分别处理。

同一提供方的不同接入端分别核对能力、身份要求、访问权限及内容完整性；一个端的受限标记不能外推另一端，单项资源成功也不能代替完整能力验证。服务须声明实际使用且已验证的接入端，界面按该声明提供入口；搜索、目录与正文可按经验证的边界组合，但不能将试读、推荐或替代资源标作完整资源，也不能在失败后静默换端、换身份或降级试探权限。

公开CLI、状态及日志不输出Cookie、Token、原始账号标识、上游签名地址、媒体密钥或本地媒体能力URL。秘密资源只通过私有服务会话交给对应播放器。

## 生命周期

最多一个可见根页面派发业务请求。非当前页面及设置之外的内容停止刷新、轮询、封面、分页、重试和预加载；隐藏窗口暂停全部内容页，托盘、热键与应用更新独立运行。迟到结果必须通过当前激活/请求代数核对，A→B→A不能误接纳旧结果。

服务不拥有内容页面的刷新计时器；可见客户端最多派发四个实际在途聚合来源，未退出的旧请求仍计入。账号状态轮询仅在所属可见操作期间运行，不能每次返回集合视图就重新拉全量列表。已提交写入可能在离开后完成，不自动重发或声称已回滚；只允许当前事务完成必要核对。

Web宿主使用红果`playback.*`返回的受控压缩媒体资源，不消费原生GPU纹理。隐藏时暂停并卸载`video.src`、停止资源读取；恢复保持暂停。离开根页面、打开设置及退出关闭所属会话，旧会话不得关闭或写入新会话。具体资源、清晰度、checkpoint与错误字段见[浏览器播放契约](https://github.com/LazyBuddyDev/hongguo-service/blob/main/docs/browser-playback.md)。

## 安装与信任

可信目录记录包的HTTPS地址、大小、SHA-256和精确依赖；`catalog.json.sig`是目录原文字节的64字节Ed25519签名。客户端只信任随宿主携带的`module-catalog.json`公钥，不能从下载响应替换公钥。验签及ZIP路径、清单、平台和依赖校验通过后才执行包。

整组依赖准备成功后只原子切换一次registry；失败保留原状态。卸载回收无引用运行时，仍有引用时拒绝删除。运行中通过宿主管理，CLI不得直接修改包。`install-local`是显式开发入口，不是线上验签失败的回退。维护流程见[模块分发](module-distribution.md)。

## 宿主日志与服务诊断

服务不写日志、不接收宿主日志目录参数，也不依赖宿主工程。错误回复可包含`error.diagnostic`，异步失败通过`data.error`及`data.diagnostic`返回；诊断仅含稳定错误码、阶段、已脱敏上下文和有限文件/函数/行号。不得返回原始异常消息、局部变量、凭据、完整地址或媒体能力。

宿主在API调用失败、异步操作报错或浏览器报告异常时白名单投影诊断，由host-control::logs统一写`%LOCALAPPDATA%/LazyBuddy/logs/LB.log`。隔离实例使用自己的数据根目录；调用栈位置来自宿主调用点和服务诊断，文本格式包含本地时间/级别/模块/API/错误码，JSON只用于协议传输，不直接落盘。

每个宿主运行实例只有一份活动LB.log，不按大小拆日志；单条输出有界、过滤控制字符。正常退出完成服务收尾和设置保存后落关闭时间，下一次启动归档为`LB_backup_YYYYMMDDHHMM.log`，同分钟重启加序号避免覆盖；未正常关闭的日志用`LB_backup_unclean_<启动归档时间>.log`明确标记，不能当作准确关闭时间。归档只处理自有LB.log，不删除或迁移旧版日志。LB.session是宿主运行所有权锁文件，不是日志。

诊断写入失败不得阻塞或破坏功能。页面展示规则与提示归属见[UI规范](ui-guidelines.md)，本节只维护诊断传输及日志边界。

低频生命周期事件以INFO级别记录运行版本、事件序号、窗口状态、原生尺寸/DPI/能力位及切换耗时；前端在状态变化后的DOM更新完成时回报视口与控件尺寸，不依赖后台可能暂停的动画帧，可与原生状态对照。仅记录状态边界，不逐帧、逐鼠标移动或轮询写日志。事件经有界后台队列顺序写入，队列满时丢弃并报告数量；正常退出先冲刷队列再写关闭标记。服务失败保留脱敏诊断，并关联请求序号、代数、连接模式和耗时，不记录请求体、完整回复、凭据或资源地址。前端诊断仅接受固定布尔值和有界数值字段，未知字段拒绝。
