# 文档导航

第一次使用从[项目README](../README.md)开始；开发前阅读[维护规范](../AGENTS.md)。文件归位统一按[文件放置规范](file-layout.md)执行，功能进度和未完成验收查看[TASKS](../TASKS.md)。

| 要做什么 | 说明 |
| --- | --- |
| 判断一个文件应该放哪里 | [文件放置规范](file-layout.md) |
| 理解宿主、模块和独立服务的边界 | [架构](architecture.md) |
| 本地构建、启动、管理模块或调用CLI | [构建与CLI](cli.md) |
| 实现可接入的模块包与私有传输 | [宿主模块契约](module-api.md) |
| 选择局部验证入口 | [定向测试](testing.md) |
| 维护模块包与可信下载目录 | [模块分发](module-distribution.md) |
| 维护主程序更新与替换 | [应用更新](app-updates.md) |
| 卸载主程序及可选功能资源 | [Windows卸载](uninstall.md) |

业务接口分别由[新闻服务](https://github.com/LazyBuddyDev/news-service/blob/main/docs/module-api.md)和[红果服务](https://github.com/LazyBuddyDev/hongguo-service/blob/main/docs/module-api.md)维护，宿主不保留整份副本。

`index.html`和`icon.svg`用于公开站点；`modules/`和`updates/`用于已发布机器端点。签名文件按原字节维护，不作为普通文档排版或清理。
