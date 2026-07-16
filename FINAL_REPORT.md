# 讯栖 XunQi · 栖｜QIDU 品牌刷新最终报告

**状态：完成 macOS 实现与验收；Windows 原生启动和真实授权网络恢复仍待对应环境人工验收。已获小夫批准提交并推送 `brand/qidu-refresh`，不创建 Release、不发布 X。**

- 日期：2026-07-15
- 分支：`brand/qidu-refresh`
- 基准提交：`77185ba01d81b68d7629f17980ea78af6e76a568`
- 最终视觉基准：`assets/reference/xunqi-qidu-brand-board.png`
- 品牌题记：讯来有迹，文止于栖。
- 统一署名：A QIDU Utility

## 交付结果

1. 以小夫确认的最终品牌规范板为依据，生产蓝紫归栖轨道、暖黄色内容方块与单轨道圆点组成的正式位图母版。
2. 更新界面 Logo、macOS / Windows Tauri 配置图标、README、品牌说明、页头、About、空状态和侧栏署名。
3. 保留原有任务工作台、文章预览、PDF / Markdown 导出、公开视频下载与明确授权嗅探流程；生产抓取、解析、证书、代理和网络恢复代码未改动。
4. 以真实运行的本地预览界面生成 8 张 1920×1080 截图，全部使用合成公开测试数据。
5. 输出 29 秒 1920×1080 演示视频，H.264 视频、AAC 双声道静音轨、30 fps；8 个场景完整保留，中文字幕已烧录，并附独立 SRT。

## 主要修改

| 区域 | 结果 |
| --- | --- |
| `assets/brand/` | 新增生产母版，替换界面图标并补充 QIDU 品牌使用说明。 |
| `src-tauri/icons/` | 更新 Tauri 当前配置使用的 32 / 128 / 256 / ICNS / ICO 图标。 |
| `src/App.tsx`、`src/App.css`、`src/components/BrandAboutDialog.tsx` | 加入 `栖 · CHAPTER 01`、QIDU 页头与独立 About 对话框，收束蓝紫、暖黄、墨色与纸白视觉层级。 |
| `src/components/` | 在空状态与任务侧栏加入克制的品牌题记和本地优先署名。 |
| `src/lib/previewBackend.ts` | 仅为公开演示增加合成链接收取、公开视频和授权说明状态；生产后端未改。 |
| `README.md`、`AUDIT.md`、`design-qa.md` | 完成品牌说明、只读审计、真实界面对照和验收记录。 |

## 截图清单

| 文件 | 场景 |
| --- | --- |
| `OUTPUT/Screenshots/xunqi-01.png` | 收件箱与新品牌页头 |
| `OUTPUT/Screenshots/xunqi-02.png` | 复制链接后自动收取和分类 |
| `OUTPUT/Screenshots/xunqi-03.png` | 公众号文章预览 |
| `OUTPUT/Screenshots/xunqi-04.png` | PDF / Markdown 导出选择 |
| `OUTPUT/Screenshots/xunqi-05.png` | 导出完成、路径与打开位置 |
| `OUTPUT/Screenshots/xunqi-06.png` | 视频号公开直链状态 |
| `OUTPUT/Screenshots/xunqi-07.png` | 明确授权、VPN 冲突和网络恢复说明 |
| `OUTPUT/Screenshots/xunqi-08.png` | About、品牌释义与 QIDU 署名 |

以上均为真实渲染界面截图，不是概念图；测试名称、正文和链接均为虚构内容。

## 视频交付

- 主文件：`OUTPUT/Videos/xunqi-demo.mp4`
- 字幕文件：`OUTPUT/Videos/xunqi-demo.srt`
- 时长：29.000 秒（较初版整体提升至约 1.72×）
- 画面：1920×1080，H.264 High，30 fps
- 音轨：AAC LC，48 kHz，双声道静音
- 文件大小：11,164,284 字节
- 内容边界：演示公开视频状态和授权说明，不启动真实嗅探，不读取 Cookie，不展示真实账号或用户目录。

## 验证记录

| 检查 | 结果 |
| --- | --- |
| `pnpm check` | 通过：ESLint、TypeScript、Vitest 17/17、Rust 52 项有效测试；2 项需用户授权真实网址的测试按设计跳过。 |
| `pnpm build:native && pnpm build` | 通过：本机 PDF / 嗅探辅助程序与前端生产构建成功。 |
| `PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build --no-bundle` | 通过：macOS release 二进制构建成功。 |
| macOS 原生启动 smoke | 通过：release 二进制成功启动并保持运行，3 秒后由验收脚本正常结束。 |
| 用户授权公开文章导出 smoke | 通过：真实读取后生成 1 份 PDF、1 份 Markdown 与 19 个本地图片资源；验收产物位于临时目录并已清理。 |
| Windows 原生启动 | 待验收：必须在 Windows runner 或真机执行，不从 macOS 伪造结果。 |
| 真实授权网络恢复 | 待人工验收：本轮未启动真实嗅探；代理、证书与临时状态恢复相关 Rust 自动测试已通过。 |
| HyperFrames `check` | 通过：0 errors、0 warnings；23/23 文本对比度检查达到 WCAG AA。 |
| 视频关键帧检查 | 通过：8 个脚本节点均已逐帧检查，字幕、界面和授权边界可读。 |
| `ffprobe` | 通过：H.264、AAC、1920×1080、30 fps、29.000 秒。 |
| `git diff --check` | 通过：无空白错误。 |

## 隐私与安全边界

- 公开截图和视频未使用真实公众号、视频号、微信账号、Cookie、Token、登录态或私有分享链接。
- 真实导出 smoke 仅使用小夫此前明确提供的公开文章链接，产物写入临时目录并在验证后清理；链接和文章内容未进入交付素材或仓库文档。
- 演示中的链接收取、文章、导出和视频状态全部来自浏览器预览层的本地合成数据。
- 未启动真实嗅探助手，未修改系统代理、证书或网络设置。
- 未修改 `src-tauri/src/` 中成熟的解析、下载、授权和恢复实现。
- 品牌改动已按小夫本轮授权提交并推送至 GitHub 分支；未创建 Release，也未发布执行包内的 X 文案。

## 剩余风险

- 本轮在 macOS 原生构建环境完成；Windows 图标资源已更新，但 Windows 原生运行仍应由 Windows runner / 真机复验。
- 本轮没有启动真实授权嗅探，因此真实系统代理、会话证书与临时状态的端到端恢复仍是人工验收门槛；自动恢复测试已通过。
- 公众号 PDF / Markdown 真实导出已使用小夫此前明确提供的公开文章完成；另一项纯文本微信页面测试仍需单独授权网址，默认测试中保持跳过。
- 当前 Logo 是高分辨率位图生产稿；正式商用前仍需做商标近似检索。若需要印刷或无限缩放，应在不改变轮廓的前提下补人工校正矢量稿。
