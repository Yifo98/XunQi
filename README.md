# 讯栖 XunQi · 栖

> 讯来有迹，文止于栖。
> What arrives leaves a trace; what is kept finds its roost.

**A QIDU Utility**

讯者，消息之所至；栖者，文章之所安。讯栖是一款本地优先的微信链接捕获与导出工具：它在用户主动复制分享链接后识别公众号文章或视频号页面，让用户自行查看、选择并保存到本地。

当前版本是公开预发布版，主要用于 Mac 与 Windows 双平台调试。仓库截图、测试数据和演示链接均为虚构内容，不包含真实公众号、视频号或用户任务。

![讯栖公开演示界面](assets/reference/xunqi-public-demo.png)

## 双语演示素材

| 中文｜微信公众号 | English｜X |
| --- | --- |
| [![中文 29 秒演示](assets/publishing/zh/video/xunqi-demo-zh-preview.png)](assets/publishing/zh/video/xunqi-demo-zh.mp4) | [![English 29-second demo](assets/publishing/en/video/xunqi-demo-en-preview.png)](assets/publishing/en/video/xunqi-demo-en.mp4) |

- 中文：[`8 张截图`](assets/publishing/zh/screenshots/) · [`29 秒 MP4`](assets/publishing/zh/video/xunqi-demo-zh.mp4) · [`SRT`](assets/publishing/zh/video/xunqi-demo-zh.srt)
- English: [`8 screenshots`](assets/publishing/en/screenshots/) · [`29-second MP4`](assets/publishing/en/video/xunqi-demo-en.mp4) · [`SRT`](assets/publishing/en/video/xunqi-demo-en.srt)

两套素材均来自真实运行的本地预览界面，并使用同一组虚构公开测试数据。生成方式、隐私边界和校验信息见 [双语发布素材说明](docs/PUBLISHING-ASSETS.md)。

## 下载预发布包

| 平台 | 压缩包 | 当前状态 |
| --- | --- | --- |
| macOS Apple Silicon | `XunQi-0.5.0-beta.1-macOS-arm64-source-preview.zip` | 可运行源码预览；无 `.app` |
| Windows x64 | `XunQi-0.5.0-beta.1-Windows-x64-source-preview.zip` | 原生 Windows 构建；待更多真机反馈 |

请从 [GitHub Releases](https://github.com/Yifo98/XunQi/releases) 下载。所有公开版本暂时都标记为 **Pre-release**。

两个 ZIP 都包含：

```text
XunQi-版本-平台-source-preview/
├── Launch-XunQi.command 或 Launch-XunQi.cmd
├── README-平台.txt
├── runtime/              # 运行所需文件
└── source/               # 与本次发布对应的完整源码
```

Mac 包不包含 `.app`。Windows 必须在 `runtime/` 中保留 `XunQi.exe`，这是 Windows 实际运行程序所需的文件格式；用户仍然只需解压 ZIP 并双击外层启动器，不需要安装。

## 使用方法

1. 完整解压 ZIP。
2. macOS 双击 `Launch-XunQi.command`；Windows 双击 `Launch-XunQi.cmd`。
3. 在微信中复制分享链接：
   - 公众号：打开文章，点击右上角三个点或四个点，选择“复制链接”。
   - 视频号：点击分享箭头，选择“复制链接”。
4. 微信位于前台时，讯栖会识别随后复制的新链接。也可以把链接直接粘贴到左侧搜索框。
5. 查看识别结果后，再手动导出文章或下载视频。

Windows 预发布版已经补上原生微信前台识别，支持 `WeChat.exe`、`Weixin.exe` 和视频号子进程 `WeChatAppEx.exe`。进入微信时程序只建立剪贴板基线，不会把旧链接误收取；随后复制的新链接才会进入任务列表。

## 当前功能

- 公众号公开文章读取与任务整理。
- 单篇或批量导出保留原文结构的 PDF。
- 导出 Markdown 与本地图片文件夹。
- 识别页面公开提供的 HTTPS 视频文件并手动下载。
- macOS 上可由用户明确授权临时启用视频号嗅探助手。
- 显示下载进度、速度、保存目录和已知媒体信息。
- 删除任务时同步清理讯栖内部正文、资源和识别缓存，不删除用户已经导出的文件。

## 平台差异与限制

| 能力 | macOS | Windows |
| --- | --- | --- |
| 微信前台复制链接监听 | 支持 | 支持，0.5.0-beta.1 起 |
| 直接粘贴链接收取任务 | 支持 | 支持 |
| 公众号 PDF / Markdown 导出 | 支持 | 支持；PDF 依赖 Microsoft Edge |
| 公开视频直链下载 | 支持 | 支持 |
| 授权嗅探下载 | 支持 Apple Silicon 预览 | 暂不支持 |
| 代码签名 / 公证 | 未公证 | 未签名 |

微信公众号和视频号没有向第三方提供“输入名称即可持续连接并读取全部更新”的公开接口。讯栖只处理用户主动复制或粘贴的分享链接，不读取聊天记录、微信本地数据库或密码。

视频号公开分享页经常不直接提供媒体地址。macOS 授权嗅探会临时修改系统代理并信任一张本地会话证书，存在明确风险与平台限制。启用前请阅读 [安全说明](docs/SECURITY.md) 与 [隐私说明](docs/PRIVACY.md)，关闭 VPN 和其他系统代理，使用完立即结束并恢复网络。讯栖不提供录屏保存，也不绕过 DRM 或内容访问权限。

## 本地开发

需要 Node.js 22、pnpm 11、Rust stable，以及 Tauri 对应平台的系统依赖。

```bash
pnpm install --frozen-lockfile
pnpm check
pnpm tauri dev
```

macOS 生成“源码＋运行文件＋启动器”预发布包：

```bash
pnpm package:portable:mac
```

Windows 预发布包由 [Windows source preview](.github/workflows/windows-portable.yml) 在原生 Windows Runner 构建。不要在 Mac 上把其他平台产物伪装成 Windows 包。

## 项目文档

- [发布与验收](docs/RELEASES.md)
- [路线图](docs/ROADMAP.md)
- [隐私说明](docs/PRIVACY.md)
- [安全说明](docs/SECURITY.md)
- [公众号评论能力研究](docs/COMMENT-RESEARCH.md)
- [双语发布素材](docs/PUBLISHING-ASSETS.md)

## 许可证

讯栖原创代码和资源采用 [MIT License](LICENSE)。可选的 `wx_channels_download` 助手属于第三方组件，采用其自己的许可证并包含 Commons Clause；详情见 [第三方许可](assets/portable/第三方许可-wx_channels_download.txt)。

请只处理你有权访问和保存的内容，并遵守微信平台规则、内容权利人的授权条件和所在地法律。
