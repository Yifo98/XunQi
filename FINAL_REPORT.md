# 讯栖 XunQi · 栖｜QIDU 品牌刷新与双语发布素材报告

**状态：完成 macOS 实现与验收，以及中文、英文两套真实界面截图和 29 秒演示视频；Windows 原生启动和真实授权网络恢复仍待对应环境人工验收。已获小夫批准更新并推送 `brand/qidu-refresh`，不创建 Release、不发布 X。**

- 日期：2026-07-16
- 分支：`brand/qidu-refresh`
- 基准提交：`77185ba01d81b68d7629f17980ea78af6e76a568`
- 最终视觉基准：`assets/reference/xunqi-qidu-brand-board.png`
- 品牌题记：讯来有迹，文止于栖。
- 统一署名：A QIDU Utility

## 交付结果

1. 以小夫确认的最终品牌规范板为依据，生产蓝紫归栖轨道、暖黄色内容方块与单轨道圆点组成的正式位图母版。
2. 更新界面 Logo、macOS / Windows Tauri 配置图标、README、品牌说明、页头、About、空状态和侧栏署名。
3. 保留原有任务工作台、文章预览、PDF / Markdown 导出、公开视频下载与明确授权嗅探流程；生产抓取、解析、证书、代理和网络恢复代码未改动。
4. 以同一组真实本地预览状态生成中文、英文各 8 张 1920×1080 截图，全部使用合成公开测试数据。
5. 输出中文、英文各一支 29 秒 1920×1080 演示视频，H.264 视频、AAC 双声道静音轨、30 fps，并附独立 SRT。
6. 将最终公开素材、HyperFrames 源文件和可复现脚本整理到 `assets/publishing/` 与 `scripts/publishing/`。
7. 撤下 Windows 预编译 EXE 发布路径，改为只包含完整源码与 `Launch-XunQi.bat` 的候选包；BAT 不被描述为 Smart App Control 绕过方案。

## 主要修改

| 区域 | 结果 |
| --- | --- |
| `assets/brand/` | 新增生产母版，替换界面图标并补充 QIDU 品牌使用说明。 |
| `src-tauri/icons/` | 更新 Tauri 当前配置使用的 32 / 128 / 256 / ICNS / ICO 图标。 |
| `src/App.tsx`、`src/App.css`、`src/components/BrandAboutDialog.tsx` | 加入 `栖 · CHAPTER 01`、QIDU 页头与独立 About 对话框，收束蓝紫、暖黄、墨色与纸白视觉层级。 |
| `src/components/` | 在空状态与任务侧栏加入克制的品牌题记和本地优先署名。 |
| `src/lib/previewBackend.ts` | 仅为公开演示增加合成链接收取、公开视频和授权说明状态；生产后端未改。 |
| `README.md`、`AUDIT.md`、`design-qa.md` | 完成品牌说明、只读审计、真实界面对照和验收记录。 |
| `assets/publishing/` | 新增 ZH / EN 截图、29 秒视频、字幕、预览图和双语视频源文件。 |
| `scripts/publishing/`、`docs/PUBLISHING-ASSETS.md` | 新增英文界面语言层、截图流程和双语视频可复现渲染说明。 |
| `assets/portable/`、`scripts/package-windows-source.ps1`、`.github/workflows/windows-portable.yml` | Windows 发布候选改为源码 + BAT；工作流只沿用已登记路径，ZIP 验证会拒绝 EXE、安装器、DLL 和 CMD。 |
| `docs/SMART-APP-CONTROL.md` | 记录微软官方 Smart App Control 规则、BAT 边界和签名建议。 |

## 双语截图清单

| 文件 | 场景 |
| --- | --- |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-01.png` | 收件箱与新品牌页头 / Inbox and brand header |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-02.png` | 复制链接后自动收取和分类 / Automatic intake and classification |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-03.png` | 公众号文章预览 / Article preview |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-04.png` | PDF / Markdown 导出选择 / Export choice |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-05.png` | 导出完成、路径与打开位置 / Local result |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-06.png` | 视频号公开直链状态 / Public direct-link video |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-07.png` | 明确授权、VPN 冲突和网络恢复说明 / Authorization boundary |
| `assets/publishing/{zh,en}/screenshots/xunqi-*-08.png` | About、品牌释义与 QIDU 署名 / About and QIDU signature |

以上均为真实渲染界面截图，不是概念图；测试名称、正文和链接均为虚构内容。

## 双语视频交付

| Audience | 文件 | 规格 | SHA-256 |
| --- | --- | --- | --- |
| 微信公众号 | `assets/publishing/zh/video/xunqi-demo-zh.mp4` | 29.000 秒、1920×1080、H.264、AAC、30 fps、约 2.7 MB | `8dd0fa2352b446ce7f1c7c3ad7d873bef974c37ac725ca13012f4af11b674d2c` |
| X | `assets/publishing/en/video/xunqi-demo-en.mp4` | 29.000 秒、1920×1080、H.264、AAC、30 fps、约 3.1 MB | `6d066db47413a9162210a817bf7f608b4e3d524f3a7f3f763953a7db21004f14` |

两支成片均演示公开视频状态和授权说明，不启动真实嗅探，不读取 Cookie，不展示真实账号或用户目录。

## 验证记录

| 检查 | 结果 |
| --- | --- |
| `pnpm check` | 通过：ESLint、TypeScript、Vitest 17/17、Rust 52 项有效测试；2 项需用户授权真实网址的测试按设计跳过。 |
| `pnpm build:native && pnpm build` | 通过：本机 PDF / 嗅探辅助程序与前端生产构建成功。 |
| `PATH="$HOME/.cargo/bin:$PATH" pnpm tauri build --no-bundle` | 通过：macOS release 二进制构建成功。 |
| macOS 原生启动 smoke | 通过：release 二进制成功启动并保持运行，3 秒后由验收脚本正常结束。 |
| 用户授权公开文章导出 smoke | 通过：真实读取后生成 1 份 PDF、1 份 Markdown 与 19 个本地图片资源；验收产物位于临时目录并已清理。 |
| Windows 源码包 | 待原生工作流验收：只允许完整源码、BAT 和说明文件，不携带 EXE / MSI / MSIX / APPX / DLL / CMD。 |
| Windows 原生启动 | 待验收：BAT 在本地编译后仍会生成未签名程序，必须在 Windows 真机与 Smart App Control 强制模式复验。 |
| 真实授权网络恢复 | 待人工验收：本轮未启动真实嗅探；代理、证书与临时状态恢复相关 Rust 自动测试已通过。 |
| HyperFrames `check` | 中文通过：0 errors、0 warnings、23/23 对比度；英文通过：0 errors、0 warnings、21/21 对比度。 |
| 视频关键帧检查 | 两支成片均抽取 8 个脚本节点复核，字幕、界面和授权边界可读。 |
| `ffprobe` / 完整解码 | 两支视频均通过：H.264、AAC、1920×1080、30 fps、29.000 秒。 |
| `git diff --check` | 通过：无空白错误。 |

## 隐私与安全边界

- 公开截图和视频未使用真实公众号、视频号、微信账号、Cookie、Token、登录态或私有分享链接。
- 真实导出 smoke 仅使用小夫此前明确提供的公开文章链接，产物写入临时目录并在验证后清理；链接和文章内容未进入交付素材或仓库文档。
- 演示中的链接收取、文章、导出和视频状态全部来自浏览器预览层的本地合成数据。
- 英文画面只替换界面语言和虚构测试封面，与中文画面使用同一套预览后端状态。
- 未启动真实嗅探助手，未修改系统代理、证书或网络设置。
- 未修改 `src-tauri/src/` 中成熟的解析、下载、授权和恢复实现。
- Windows ZIP 不再直接分发 XunQi.exe；BAT 只编排开发工具和本地源码构建，不能绕过系统应用控制。
- 品牌改动已按小夫本轮授权提交并推送至 GitHub 分支；未创建 Release，也未发布执行包内的 X 文案。

## 剩余风险

- 本轮在 macOS 原生构建环境完成；Windows 源码 + BAT 包仍应由 Windows runner 生成，并在真机复验首次安装、编译、启动和 Smart App Control 行为。
- 仅删除预编译 EXE 不能从根本上解决 Smart App Control：BAT 启动的工具和本地生成的程序仍受系统策略约束，正式分发最终需要可信签名通道。
- 本轮没有启动真实授权嗅探，因此真实系统代理、会话证书与临时状态的端到端恢复仍是人工验收门槛；自动恢复测试已通过。
- 公众号 PDF / Markdown 真实导出已使用小夫此前明确提供的公开文章完成；另一项纯文本微信页面测试仍需单独授权网址，默认测试中保持跳过。
- 当前 Logo 是高分辨率位图生产稿；正式商用前仍需做商标近似检索。若需要印刷或无限缩放，应在不改变轮廓的前提下补人工校正矢量稿。
