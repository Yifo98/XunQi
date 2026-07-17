# 讯栖 XunQi · 栖

> 讯来有迹，文止于栖。
> What arrives leaves a trace; what is kept finds its roost.

**A QIDU Utility**

讯者，消息之所至；栖者，文章之所安。讯栖是一款本地优先的微信链接捕获与导出工具：它在用户主动复制分享链接后识别公众号文章或视频号页面，让用户自行查看、选择并保存到本地。

当前正式版本为 **0.5.0**。macOS Apple Silicon 已完成本地运行、导出与授权下载流程验证；Windows 免开发环境便携候选包已通过原生 Windows Runner 的测试、构建与包内容校验，待 Windows 真机功能验收后再追加到 Release。仓库截图、测试数据和演示链接均为虚构内容，不包含真实公众号、视频号或用户任务。

![讯栖公开演示界面](assets/reference/xunqi-public-demo.png)

## 双语演示素材

| 中文｜微信公众号 | English｜X |
| --- | --- |
| [![中文 29 秒演示](assets/publishing/zh/video/xunqi-demo-zh-preview.png)](assets/publishing/zh/video/xunqi-demo-zh.mp4) | [![English 29-second demo](assets/publishing/en/video/xunqi-demo-en-preview.png)](assets/publishing/en/video/xunqi-demo-en.mp4) |

- 中文：[`8 张截图`](assets/publishing/zh/screenshots/) · [`29 秒 MP4`](assets/publishing/zh/video/xunqi-demo-zh.mp4) · [`SRT`](assets/publishing/zh/video/xunqi-demo-zh.srt)
- English: [`8 screenshots`](assets/publishing/en/screenshots/) · [`29-second MP4`](assets/publishing/en/video/xunqi-demo-en.mp4) · [`SRT`](assets/publishing/en/video/xunqi-demo-en.srt)

两套素材均来自真实运行的本地预览界面，并使用同一组虚构公开测试数据。生成方式、隐私边界和校验信息见 [双语发布素材说明](docs/PUBLISHING-ASSETS.md)。

## 下载正式版

| 平台 | 压缩包 | 当前状态 |
| --- | --- | --- |
| macOS Apple Silicon | `XunQi-0.5.0-macOS-arm64-source.zip` | 正式版；包含运行文件、源码和 `.command` 启动器，无 `.app` |
| Windows x64 | `XunQi-0.5.0-Windows-x64-portable-unsigned.zip` | 普通用户候选包；原生 Runner 已通过，免 Node.js / pnpm / Rust / MSVC，待真机验收后发布 |

请从 [GitHub Releases](https://github.com/Yifo98/XunQi/releases/tag/v0.5.0) 下载。`v0.5.0` 当前只提供已经验证的 macOS 包和对应 SHA-256；Windows 便携候选包已由 GitHub 原生 Windows Runner 构建并校验，在真机验收后再追加到同一版本，不会在 Mac 上伪造 Windows 产物。

macOS 包包含源码、运行文件和 `.command` 启动器。面向普通用户的 Windows 候选包调整为：

```text
XunQi-版本-Windows-x64-portable-unsigned/
├── Launch-XunQi.bat
├── Open-XunQi-Logs.bat
├── README-Windows.txt
├── THIRD-PARTY-wx_channels_download.txt
└── runtime/
    ├── xunqi.exe                       # 原生 Windows Runner 从公开源码构建；当前未商业签名
    └── xunqi-authorized-sniffer.exe    # 固定版本与校验值的本地识别助手
```

普通用户 BAT 使用纯 ASCII 和 Windows CRLF，只启动包内已编译的 `runtime/xunqi.exe`，不联网安装依赖，也不要求 Node.js、pnpm、Rust 或 MSVC。`Open-XunQi-Logs.bat` 只打开本机启动日志目录；讯栖顶部的“导出日志”（“关于”页也有同一入口）可导出不含 Cookie、聊天记录、正文、分享链接、密码或令牌的诊断 TXT。开发者仍可单独生成“源码 + BAT”包，但它不再作为普通用户下载项。**BAT 不能绕过 Smart App Control**，Windows 仍可能检查 BAT 和未签名的运行文件。详见 [Smart App Control 评估](docs/SMART-APP-CONTROL.md)。

当前 Windows 便携候选包没有商业代码签名。签名不是讯栖的功能依赖，在系统没有拦截时不影响功能使用；它主要用于让 Windows 验证发布者身份和文件完整性。“未签名”不等于恶意软件，也不会让讯栖获得额外权限。Windows 运行文件由公开源码在 GitHub 原生 Windows Runner 构建，不读取微信聊天记录、通讯录、本地数据库或密码，不提供云端账号、远程数据库或遥测上报。若遇拦截，请按 [Windows 拦截处理步骤](docs/SMART-APP-CONTROL.md#用户遇到拦截时) 先核对来源、校验值和提示类型；讯栖不会自动关闭系统安全功能。

## 使用方法

1. 完整解压 ZIP。
2. macOS 双击 `Launch-XunQi.command`；Windows 普通用户双击 `Launch-XunQi.bat`，不需要安装 Node.js、pnpm、Rust 或 MSVC。Windows 仍需要系统 WebView2（Windows 11 与多数已更新的 Windows 10 已包含）。
3. 在微信中复制分享链接：
   - 公众号：打开文章，点击右上角三个点或四个点，选择“复制链接”。
   - 视频号：点击分享箭头，选择“复制链接”。
4. 微信位于前台时，讯栖会识别随后复制的新链接。也可以把链接直接粘贴到左侧搜索框。
5. 查看识别结果后，再手动导出文章或下载视频。

Windows 已补上原生微信前台识别，支持 `WeChat.exe`、`Weixin.exe` 和视频号子进程 `WeChatAppEx.exe`。进入微信时程序只建立剪贴板基线，不会把旧链接误收取；随后复制的新链接才会进入任务列表。便携候选包会携带由原生 Windows Runner 构建的讯栖运行文件，仍需真机验收后才进入公开 Release。

## 当前功能

- 公众号公开文章读取与任务整理。
- 界面支持中文 / English 一键切换；捕获到的标题、正文和视频内容保持原文。
- 按当前全部/公众号/视频号筛选结果一键全选，并在切换分类时保留已选任务。
- 单篇或批量导出保留原文结构的 PDF。
- 导出 Markdown 与本地图片文件夹。
- 识别页面公开提供的 HTTPS 视频文件并手动下载。
- macOS 上可由用户明确授权临时启用视频号嗅探助手；Windows 试验适配已通过原生 Runner，尚须真机验收。
- 视频号支持“原始画质”与“节省空间（微信默认）”两种真实下载策略；连续授权期间保持同一策略。
- 多条视频可进入单任务校验队列，完成一条后自动接续下一条；当前不同时运行多个授权嗅探任务。
- 显示下载进度、速度、保存目录和已知媒体信息。
- 删除任务时同步清理讯栖内部正文、资源和识别缓存，不删除用户已经导出的文件。
- 顶部“导出日志”（“关于”页同样可用）可随时导出本地运行诊断；只含时间、版本、平台、受控事件和错误分类，不含用户内容或凭据。

## 平台差异与限制

| 能力 | macOS | Windows |
| --- | --- | --- |
| 微信前台复制链接监听 | 支持 | 便携候选已支持，正式包待真机验收 |
| 直接粘贴链接收取任务 | 支持 | 支持 |
| 公众号 PDF / Markdown 导出 | 支持 | 支持；PDF 依赖 Microsoft Edge |
| 公开视频直链下载 | 支持 | 支持 |
| 授权嗅探下载 | 支持 Apple Silicon | 试验支持；原生 Runner 已通过，待真机验收 |
| 代码签名 / 公证 | 未公证 | 便携候选包含未商业签名的 `xunqi.exe`；BAT 不会绕过 Windows 安全检查 |

微信公众号和视频号没有向第三方提供“输入名称即可持续连接并读取全部更新”的公开接口。讯栖只处理用户主动复制或粘贴的分享链接，不读取聊天记录、微信本地数据库或密码。

视频号公开分享页经常不直接提供媒体地址。macOS 与 Windows 试验版授权嗅探会临时修改系统代理并信任一张本地会话证书，存在明确风险与平台限制。Windows 仅使用当前用户证书库，启用前还会检查 VPN/TUN、PAC 和系统代理；不安全时直接拒绝覆盖。请阅读 [安全说明](docs/SECURITY.md) 与 [隐私说明](docs/PRIVACY.md)，关闭 VPN 和其他系统代理，使用完立即结束并恢复网络。讯栖不提供录屏保存，也不绕过 DRM 或内容访问权限。

## 本地开发

需要 Node.js 22、pnpm 11、Rust stable，以及 Tauri 对应平台的系统依赖。

```bash
pnpm install --frozen-lockfile
pnpm check
pnpm tauri dev
```

macOS 生成“源码＋运行文件＋启动器”正式包：

```bash
pnpm package:portable:mac
```

面向普通用户的 Windows 免开发环境候选包由 [Windows packages](.github/workflows/windows-portable.yml) 工作流默认选择 `portable_runtime`，并在原生 Windows Runner 测试、编译和生成：

```powershell
pnpm package:runtime:win
```

便携包只允许两个预期运行文件：`runtime/xunqi.exe` 和 `runtime/xunqi-authorized-sniffer.exe`；拒绝 MSI、MSIX、APPX、CMD、源码开发依赖和其他可执行文件。同一工作流可手动选择 `developer_source` 生成开发者调试包；普通用户不要下载该源码包。不要把 BAT 描述成 Smart App Control 绕过方案，也不要在 Mac 上伪造 Windows 原生验收结果。

## 项目文档

- [0.5.0 正式版说明](docs/release-notes-0.5.0.md)
- [发布与验收](docs/RELEASES.md)
- [路线图](docs/ROADMAP.md)
- [隐私说明](docs/PRIVACY.md)
- [安全说明](docs/SECURITY.md)
- [公众号评论能力研究](docs/COMMENT-RESEARCH.md)
- [双语发布素材](docs/PUBLISHING-ASSETS.md)
- [Windows Smart App Control 评估](docs/SMART-APP-CONTROL.md)

## 许可证

讯栖原创代码和资源采用 [MIT License](LICENSE)。可选的 `wx_channels_download` 助手属于第三方组件，采用其自己的许可证并包含 Commons Clause；详情见 [第三方许可](assets/portable/第三方许可-wx_channels_download.txt)。

请只处理你有权访问和保存的内容，并遵守微信平台规则、内容权利人的授权条件和所在地法律。
