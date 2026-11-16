# 文档导航

第一次使用从[项目README](../README.md)开始；开发前阅读[维护规范](../AGENTS.md)。文件归位统一按[文件放置规范](file-layout.md)执行，当前未完成任务及接手信息查看[TASKS](../TASKS.md)。

| 要做什么 | 说明 |
| --- | --- |
| 判断一个文件应该放哪里 | [文件放置规范](file-layout.md) |
| 理解宿主、模块和独立服务的边界 | [架构](architecture.md) |
| 开发一致的外观、控件和公共交互 | [UI规范](ui-guidelines.md) |
| 本地构建、启动、管理模块或调用CLI | [构建与CLI](cli.md) |
| 实现可接入的模块包与私有传输 | [宿主模块契约](module-api.md) |
| 选择局部验证入口 | [定向测试](testing.md) |
| 维护模块包与可信下载目录 | [模块分发](module-distribution.md) |
| 维护主程序更新与替换 | [应用更新](app-updates.md) |
| 构建安装向导、安装及卸载主程序 | [Windows安装与卸载](uninstall.md) |

业务接口分别由[新闻服务](https://github.com/LazyBuddyDev/news-service/blob/main/docs/module-api.md)和[红果服务](https://github.com/LazyBuddyDev/hongguo-service/blob/main/docs/module-api.md)维护，宿主不保留整份副本。

`index.html`和`icon.svg`用于公开站点；`modules/`和`updates/`用于已发布机器端点。签名文件按原字节维护，不作为普通文档排版或清理。

## 下载站点

下载页源文件为`index.html`，通过[GitHub Pages](https://lazybuddydev.github.io/LazyBuddy/)提供。页面为纯静态HTML/CSS，无构建依赖；`.nojekyll`关闭Jekyll处理。下载按钮指向GitHub Release的EXE安装程序，版本、包大小与发布说明应在发版时一起核对。

站点已在上述地址托管。若需重新配置，在仓库Settings → Pages选择“Deploy from a branch”、`main`分支和`/docs`目录，参见[GitHub Pages发布源说明](https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site)。后续提交推送后由Pages发布；仅本地修改不会更新在线站点。必须发布完整`docs`目录，保留`modules/`和`updates/`及其签名的精确字节，不能只上传下载页覆盖现有机器端点。
