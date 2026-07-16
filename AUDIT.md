# 讯栖 XunQi · 栖｜QIDU 品牌刷新审计

审计基线：`77185ba`（`Initial public preview`）

实施分支：`brand/qidu-refresh`
最终设计真值：`assets/reference/xunqi-qidu-brand-board.png`
本地规格：`.scratch/qidu-refresh/spec.md`（按仓库约定不进入公开提交）

## 审计结论

- 现有产品已经具备稳定的三栏工作台、微信公众号文章预览与 PDF / Markdown 导出、视频号公开直链下载及 macOS 明确授权嗅探流程。
- 本轮定位为品牌刷新，不重写捕获、解析、导出、下载、代理恢复、证书清理或任务存储链路。
- 执行包原先提到的 `Logo/生产草案/` 未随包附带；小夫已明确授权以最终品牌规范板为参考生产正式草案。
- 新标志必须保留蓝紫归栖轨道与暖黄色内容方块，并避免鸟、巢、雷达、Wi-Fi、爬虫和下载器意象。
- 公开截图和演示只使用本地合成数据，不出现真实账号、私有链接、Cookie、Token 或用户目录。

## 文件级变更地图

### 将修改

| 文件 / 区域 | 计划 |
| --- | --- |
| `assets/brand/` | 增加 QIDU 新标志母版、界面版和品牌说明；保留方向板作为来源记录。 |
| `src-tauri/icons/` | 从同一母版生成 macOS、Windows 与小尺寸图标，保持轮廓一致。 |
| `src/App.tsx` | 更新品牌锁定区、帮助入口与 About/QIDU 信息层级，不改变捕获和下载事件。 |
| `src/components/TaskSidebar.tsx` | 轻量更新底部品牌署名与本地优先提示。 |
| `src/components/TaskDetail.tsx` | 仅调整空状态和品牌说明的展示，不改导出/下载调用。 |
| `src/App.css` | 引入 QIDU 蓝紫、暖黄和纸白色调，收紧排版、阴影与状态层级。 |
| `src/App.test.tsx` | 补充新品牌文案、About/Help 与既有核心操作未回退的断言。 |
| `README.md`、`assets/brand/README.md` | 融入“讯来有迹，文止于栖。”、`讯栖 XunQi · 栖` 与 `A QIDU Utility`。 |
| `design-qa.md` | 记录方向板与真实运行界面的对照、修复循环和最终结论。 |
| `FINAL_REPORT.md` | 汇总变更、验证、截图/视频与剩余风险。 |

### 绝不修改

- `src-tauri/src/` 内的微信前台识别、链接接收、文章解析、资源下载、PDF 渲染、视频下载和授权嗅探实现。
- 代理冲突检测、会话证书移除、网络恢复警告与退出恢复逻辑。
- 已发布的 `v0.5.0-beta.1` 标签、Release 资产、仓库可见性和远端历史。
- 用户已经导出的本地文件，以及任何真实微信数据。
- 第三方 `wx_channels_download` 固定版本、校验值和许可文本。

## 验收门槛

1. 新标志在 16–32 px 仍能识别“轨道 + 内容方块”的核心轮廓。
2. 公众号 PDF / Markdown 导出和视频号授权入口行为保持不变。
3. `pnpm check` 全部通过。
4. 真实运行界面完成 1920×1080 截图；公开素材仅含合成数据。
5. macOS 本机启动通过；Windows 只在原生 Windows 构建通道验证，不从 macOS 伪造。
6. 未经小夫批准，不 Push、不 Merge、不创建 Release、不发布 X。
