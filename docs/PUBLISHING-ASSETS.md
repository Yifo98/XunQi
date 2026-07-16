# Bilingual publishing assets

讯栖的公开展示素材分成中文和英文两套，并严格对应同一组本地合成测试状态：

| Audience | Screenshots | Video | Subtitle |
| --- | --- | --- | --- |
| 微信公众号 | `assets/publishing/zh/screenshots/` | `assets/publishing/zh/video/xunqi-demo-zh.mp4` | `xunqi-demo-zh.srt` |
| X | `assets/publishing/en/screenshots/` | `assets/publishing/en/video/xunqi-demo-en.mp4` | `xunqi-demo-en.srt` |

两支成片均为 29 秒、1920×1080、30 fps、H.264 和 AAC。中文成片用于微信公众号；英文成片和英文截图用于 X。

## Privacy boundary

- 所有名称、标题、正文和链接均为虚构测试内容。
- 不包含真实微信账号、公众号、视频号、Cookie、Token、登录态、用户目录或私人文件。
- 授权页只展示说明，不启动真实嗅探，不修改系统代理或证书。
- 英文截图来自同一真实本地预览状态，仅使用 `scripts/publishing/translate-demo-en.js` 替换界面语言和虚构测试封面。

## Reproduce the video

视频源文件位于 `assets/publishing/video-source/zh/` 和 `assets/publishing/video-source/en/`。运行：

```bash
pnpm publishing:render
```

跨平台 Node 脚本会把已提交的截图和品牌母版复制到忽略目录 `output/publishing-render-work/`，运行 HyperFrames 完整检查并生成两支本地视频。它可在 macOS、Windows 和 Linux 使用，不会修改已提交的发布素材。

依赖：Node.js 22+、FFmpeg、FFprobe；HyperFrames 固定为 `0.7.58`。中文和英文源时间线本身均为 29 秒，成片再以 H.264 CRF 27 压缩，避免公开仓库累积大体积视频。

## Reproduce the English screenshots

`scripts/publishing/capture-demo-en.js` 导出一个接收 Playwright `page` 的复用函数；`translate-demo-en.js` 是仅用于公开演示的语言层。它们不是独立命令，也不会让项目额外安装浏览器。已提交的 8 张英文截图是本次发布的规范版本；如已有 Playwright 驱动，可导入该函数并传入隔离预览页。

启动本地预览：

```bash
pnpm dev --host 127.0.0.1 --port 4173
```

再由现有浏览器自动化驱动调用 `captureDemoEn(page)`。函数会把 1920×1080 截图写入 `XUNQI_CAPTURE_DIR`；未设置时写入 `output/playwright/`。它只使用预览后端的合成任务，不访问真实微信内容。

## Checksums

- Chinese MP4 SHA-256: `8dd0fa2352b446ce7f1c7c3ad7d873bef974c37ac725ca13012f4af11b674d2c`
- English MP4 SHA-256: `6d066db47413a9162210a817bf7f608b4e3d524f3a7f3f763953a7db21004f14`
