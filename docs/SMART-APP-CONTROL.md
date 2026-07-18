# Windows Smart App Control 边界

本文讨论讯栖 Windows “免开发环境便携包”和“完整源码 + BAT 开发者包”与 Smart App Control 的关系。结论依据均为微软官方资料。

## 结论

1. **未知、未签名的二进制仍有被拦截的可能。** Smart App Control 先使用微软云端安全服务判断应用是否安全；当服务无法做出有信心的安全判断时，会再检查有效签名。此时未签名或签名无效的应用会被视为不可信并拦截。这不等于“所有未签名程序在所有设备上都必然被拦截”，但对新生成、缺少声誉的未签名程序必须按可拦截处理。见 [Smart App Control Frequently Asked Questions](https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions) 和 [Application Control for Windows](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/appcontrol#app-control-and-smart-app-control)。

2. **BAT/CMD 不是绕过方案。** 微软说明 Smart App Control 建立在 App Control for Business 之上；App Control 不直接控制由 Windows Command Processor（`cmd.exe`）执行的代码，包括 `.bat` / `.cmd` 文件，但批处理文件尝试启动的任何程序仍受 App Control 控制。因此，把启动逻辑放进 BAT 不会让它后续启动的 EXE、DLL 或其他二进制获得豁免。见 [Understand App Control script enforcement](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/design/script-enforcement#scripts-that-arent-directly-controlled-by-app-control)。

3. **便携包和源码包都只是在改变交付边界。** 面向普通用户的便携候选包由 GitHub 原生 Windows Runner 从公开源码预编译，用户不需要 Node.js、pnpm、Rust 或 MSVC；它仍包含一个新的、未商业签名的 `xunqi.exe`，可能被 Smart App Control 检查。开发者包在本机调用 Node.js、pnpm、Rust/Cargo 与 Tauri，从源码生成的程序同样可能被检查。两种 BAT 都不能表述为“解决”或“绕过” Smart App Control。

4. **正式 Windows 分发仍应使用可信签名，并在强制模式覆盖全部二进制路径。** 微软说明 Smart App Control 接受基于 RSA 的数字证书，并且只把可信提供者签发的证书视为可信；微软将 Trusted Signing 列为首选签名方式。见 [Sign your app for Smart App Control compliance](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control)。

   发布前应在 Smart App Control **On（Enforcement）** 下完成真实 Windows 验收，遍历应用的所有代码路径与功能，包括安装、卸载、主程序、辅助程序、子进程、动态加载的二进制以及会加载讯栖二进制的第三方集成。微软要求在分发已签名应用前如此测试，并支持审计策略或直接在强制模式中验证。见 [Test your app's signature with Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/test-your-app-with-smart-app-control)。

## 没有签名是否影响使用

当前讯栖 Windows 候选版没有商业代码签名。代码签名解决的是“发布者是谁、文件发布后是否被篡改”的身份与完整性问题，并不是应用运行所必需的功能，也不会给软件增加读取隐私数据的权限。在 Smart App Control、SmartScreen 或组织策略没有拦截时，便携运行文件和本地源码构建都可以正常使用。

因此，准确表述是：**未签名不影响讯栖功能逻辑，但可能影响 Windows 是否允许本地生成的程序启动。** “未签名”也不自动等于恶意软件；Windows 在缺少签名和云端声誉时，只是无法建立足够的信任判断。

## 用户遇到拦截时

1. **先核对来源。** 只从项目 GitHub Releases 下载，核对同名 `.sha256`，并用 Microsoft Defender 扫描。
2. **只有下载来源标记时。** 若文件属性显示“此文件来自其他计算机，可能被阻止”，可按微软 [Attachment Manager 说明](https://support.microsoft.com/en-us/windows/security/information-about-the-attachment-manager-in-microsoft-windows) 在 ZIP 的“属性 → 常规”中选择“解除锁定”，再重新解压。该操作不关闭安全功能。
3. **Smart App Control 明确拦截时。** 微软说明目前不能只允许某一个应用。优先改用 Windows Sandbox、虚拟机或专门开发环境，或等待正式签名版本。
4. **用户自主关闭时。** 若用户理解风险并决定关闭，可进入“Windows 安全中心 → 应用和浏览器控制 → 智能应用控制设置”。微软当前 FAQ 说明近期 Windows 更新允许之后重新开启；关闭期间系统保护会降低，讯栖不会自动修改该设置。
5. **组织设备。** 企业或学校的 App Control 策略应由管理员处理允许列表。
6. **恶意软件告警。** 若 Defender 明确报告木马、恶意软件或潜在有害程序，应停止运行，不要强行放行，并提交脱敏截图与校验值供项目核查。

## 隐私与安全解释

- 普通用户便携包交付 BAT、说明文件和由原生 Windows Runner 从公开源码构建的单一讯栖运行文件；不含安装器、后台服务或开发工具链。开发者包仍交付公开源码。
- 讯栖只处理用户主动复制或粘贴的微信分享链接；不读取聊天记录、通讯录、微信数据库或密码。
- 公众号公开读取默认不使用微信 Cookie；任务、正文和导出内容保存在本机，不提供云端账号、远程数据库或遥测上报。
- 首次安装依赖、读取公开页面和下载用户选择的内容需要联网，但讯栖不会把本地任务或导出内容上传到自有服务器。
- 代码签名是一项发布信任机制，不是隐私审计结论；讯栖的隐私边界由公开源码、最小权限设计和可验证的数据流共同保证。

## 讯栖发布准则

- Windows 普通用户便携包只允许预期的 `runtime/xunqi.exe`，不包含 MSI、MSIX、APPX、后台服务、源码开发依赖或额外可执行文件。
- 普通用户 `Launch-XunQi.bat` 只启动包内运行文件；开发者 BAT 负责环境检查、依赖安装和本地构建。文档、UI 和发布说明不得将任何 BAT 宣称为安全控制绕过方案。
- 本地构建失败或生成程序被拦截时，先指导用户核对来源、校验值和拦截类型；不得自动关闭系统保护，也不得把普通下载标记、SmartScreen、Smart App Control 与组织 App Control 混为一谈。
- 在可信签名与 Smart App Control 强制模式验收完成前，Windows 产物保持 Preview 标识，不承诺在所有已启用 Smart App Control 的设备上可运行。
