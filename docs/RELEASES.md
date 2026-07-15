# 讯栖发布记录

## 0.5.0-beta.1 · 2026-07-15

- 首个公开预发布版本；Mac 与 Windows ZIP 均包含完整源码、运行文件和平台启动器。
- macOS 包不再使用 `.app`，由 `Launch-XunQi.command` 启动 `runtime/xunqi`。
- Windows 包由 `Launch-XunQi.cmd` 启动 `runtime/XunQi.exe`，不需要安装器。
- 修复 Windows 无法自动收取复制链接的问题：新增对 `WeChat.exe`、`Weixin.exe` 和 `WeChatAppEx.exe` 的原生前台识别。
- 仓库演示界面、测试数据与链接全部改为虚构内容，不收录真实公众号、视频号或用户任务。
- 所有公开 GitHub Release 暂时标记为 Pre-release。

### 继承自 0.4.9 的能力

- 兼容微信新版 `appmsg_type = 10002` 文字内容页，可读取标题、作者、发布时间和公开正文。
- 把完整微信链接粘贴到搜索框时，改为直接收取任务，不再将任务列表过滤为空。
- 支持多篇公众号文章批量导出原版 PDF。
- 支持视频号授权嗅探的连续会话、单并发队列、进度与保存位置展示。
- 公众号公开文章读取、单篇与批量 PDF、Markdown 和本地图片导出。
- 视频号公开直链下载，以及 macOS 用户明确授权后的本地嗅探辅助流程。

### 已知限制

- macOS 运行文件使用 ad-hoc 签名，未进行 Apple Developer ID 公证。
- Windows 未签名，SmartScreen 可能显示来源提示；不包含 macOS 专用的视频号授权嗅探助手。
- 预发布包用于调试与验收，不承诺稳定版兼容性。
