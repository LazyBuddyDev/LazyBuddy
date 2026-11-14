# 宿主模块契约

API v1，目标平台Windows x64。本文只定义宿主的包、传输与生命周期边界；[新闻API](https://github.com/LazyBuddyDev/news-service/blob/main/docs/module-api.md)和[红果API](https://github.com/LazyBuddyDev/hongguo-service/blob/main/docs/module-api.md)由所属服务维护。源码边界见[架构](architecture.md)，文件归位见[规范](file-layout.md)。

## 清单与进程

ZIP根目录包含`module.json`，使用`schema: 1`、`service_api: 1`、三段式不可变包版本与`target: "windows-x86_64"`。ID以字母开头，仅含小写字母、数字及连字符。入口必须是包内相对路径。完整字段与校验由宿主`crates/module-contract`定义，通用样例见`contracts/fixtures`；各服务维护自己的实际清单。

功能包声明页面模板；`news_grid`指定`catalog_method`与`fetch_method`，`media_library`指定`search_method`与`favorites_method`。它们表达受支持的界面契约，不是任意HTML或脚本入口。当前宿主对新闻、红果有明确页面实现和附加设置/账号方法；新服务接入仍须完成对应前端与调度，不能仅靠清单自动获得完整页面。

运行时包使用`kind: "runtime"`，没有页面。依赖固定精确包版本，`runtime`选择解释器依赖；宿主按运行时入口/参数、功能入口/参数组合启动。依赖目录通过`LAZYBUDDY_DEPENDENCY_<大写ID且连字符转下划线>_DIR`传入。代码与运行时留在各自不可变包目录，持久数据使用`LAZYBUDDY_DATA_DIR`。

宿主拥有服务进程树，通过私有stdin/stdout持久会话调用，不读取相邻工作区实现。关闭stdin请求服务正常退出，等待必要保存和已提交事务收尾，不立即强杀。服务必须保持stdout纯协议，日志不能混入回复。

## JSON Lines

每行一个UTF-8 JSON对象，请求与回复保留相同版本和字符串ID：

```json
{"version":1,"id":"health-1","method":"health","args":{}}
{"version":1,"id":"health-1","ok":true,"data":{"service":"news","api_version":1}}
{"version":1,"id":"call-2","ok":false,"error":{"code":"unknown_method"}}
```

宿主串行化私有传输请求。客户端要处理失败与未知错误码；字段边界、方法和错误语义以对应服务API为准。红果耗时账号事务返回`operation: "pending"`，由可见账号操作查询`account.status`确认，不能占住媒体控制。接受请求、官方确认、资源准备好和实际解码成功分别处理。

公开CLI、状态及日志不输出Cookie、Token、原始账号标识、上游签名地址、媒体密钥或本地媒体能力URL。秘密资源只通过私有服务会话交给对应播放器。

## 生命周期

最多一个可见根页面派发业务请求。非当前页面及设置之外的内容停止刷新、轮询、封面、分页、重试和预加载；隐藏窗口暂停全部内容页，托盘、热键与应用更新独立运行。迟到结果必须通过当前激活/请求代数核对，A→B→A不能误接纳旧结果。

新闻服务没有刷新计时器；可见客户端最多派发四个实际在途来源，未退出的旧请求仍计入。红果账号轮询仅在所属可见操作期间运行，不能每次返回收藏页就重新拉全量列表。已提交写入可能在离开后完成，不自动重发或声称已回滚；只允许当前事务完成必要核对。

Web宿主使用红果`playback.*`返回的受控压缩媒体资源，不消费原生GPU纹理。隐藏时暂停并卸载`video.src`、停止资源读取；恢复保持暂停。离开根页面、打开设置及退出关闭所属会话，旧会话不得关闭或写入新会话。具体资源、清晰度、checkpoint与错误字段见[浏览器播放契约](https://github.com/LazyBuddyDev/hongguo-service/blob/main/docs/browser-playback.md)。

## 安装与信任

可信目录记录包的HTTPS地址、大小、SHA-256和精确依赖；`catalog.json.sig`是目录原文字节的64字节Ed25519签名。客户端只信任随宿主携带的`module-catalog.json`公钥，不能从下载响应替换公钥。验签及ZIP路径、清单、平台和依赖校验通过后才执行包。

整组依赖准备成功后只原子切换一次registry；失败保留原状态。卸载回收无引用运行时，仍有引用时拒绝删除。运行中通过宿主管理，CLI不得直接修改包。`install-local`是显式开发入口，不是线上验签失败的回退。维护流程见[模块分发](module-distribution.md)。
