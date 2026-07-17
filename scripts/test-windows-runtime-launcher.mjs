import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const launcherPath = path.join(root, "assets", "portable", "Launch-XunQi-Windows-Runtime.bat");
const logLauncherPath = path.join(root, "assets", "portable", "Open-XunQi-Logs.bat");
const packageJson = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
const windowsConfig = JSON.parse(
  await readFile(path.join(root, "src-tauri", "tauri.windows.conf.json"), "utf8"),
);
const helperManifest = JSON.parse(
  await readFile(path.join(root, "scripts", "authorized-sniffer-helper-windows.json"), "utf8"),
);
const packager = await readFile(path.join(root, "scripts", "package-windows-runtime.ps1"), "utf8");
const helperBuilder = await readFile(
  path.join(root, "scripts", "build-authorized-sniffer-helper-windows.ps1"),
  "utf8",
);
const snifferRuntime = await readFile(
  path.join(root, "src-tauri", "src", "authorized_sniffer", "native.rs"),
  "utf8",
);
const windowsSnifferRuntime = await readFile(
  path.join(root, "src-tauri", "src", "authorized_sniffer", "native_windows.rs"),
  "utf8",
);
const launcher = await readFile(launcherPath);
const logLauncher = await readFile(logLauncherPath);
const text = launcher.toString("ascii");
const logText = logLauncher.toString("ascii");

const failures = [];
for (const [label, buffer, source] of [
  ["runtime launcher", launcher, text],
  ["log launcher", logLauncher, logText],
]) {
  if ([...buffer].some((byte) => byte > 0x7f)) failures.push(`${label} contains non-ASCII bytes`);
  if ((source.match(/(?<!\r)\n/g) ?? []).length > 0) failures.push(`${label} is not strict CRLF`);
  if (!source.startsWith("@echo off\r\n")) failures.push(`${label} does not start with @echo off`);
}
if (!text.includes("runtime\\xunqi.exe")) failures.push("runtime executable path is missing");
if (!text.includes("XUNQI_RUNTIME_LAUNCHER_SELF_TEST_OK")) failures.push("runtime self-test is missing");
if (!text.includes("launcher.log")) failures.push("launcher log is missing");
if (!text.includes(`version=${packageJson.version}`)) failures.push("launcher version does not match package.json");
if (/xunqi\.exe[^\r\n]*(?:>>|2>&1)/i.test(text)) {
  failures.push("runtime stdout/stderr must not be copied into the privacy-safe launcher log");
}
if (/\b(node|pnpm|corepack|npx|cargo|rustup)\b/i.test(text)) {
  failures.push("ordinary-user launcher must not require developer tools");
}
if (!logText.includes("explorer.exe")) failures.push("log folder launcher does not open Explorer");
if (windowsConfig.build?.beforeBuildCommand !== "pnpm build") {
  failures.push("Windows runtime build must not invoke macOS native-helper scripts");
}
if (
  !Array.isArray(windowsConfig.bundle?.externalBin)
  || !windowsConfig.bundle.externalBin.includes("bin/xunqi-authorized-sniffer")
) {
  failures.push("Windows runtime does not declare the authorized-detection sidecar");
}
if (snifferRuntime.includes('code: "windows_sniffer_unavailable"')) {
  failures.push("Windows authorized detection is still blocked by the old platform gate");
}
for (const required of ["APPDATA", "LOCALAPPDATA", "USERPROFILE"]) {
  if (!snifferRuntime.includes(`.env("${required}"`)) {
    failures.push(`Windows helper isolation is missing ${required}`);
  }
}
for (const required of [
  String.raw`CurrentUser\Root`,
  "XUNQI_CERT_FINGERPRINT",
  "ProxyEnable",
  "ProxyServer",
  "AutoConfigURL",
  "XUNQI_PROXY_ENABLE_PRESENT",
  "InternetSetOptionW",
  "Get-VpnConnection",
  "-AllUserConnection",
  "Win32_NetworkAdapter",
  "NetConnectionStatus -eq 2",
  "Windows VPN state could not be confirmed safely",
  "Windows VpnClient command is unavailable",
  "certificate still exists",
  "Assert-OptionalValue",
  "recovered_process_command",
  "恢复记录已保留",
]) {
  if (!windowsSnifferRuntime.includes(required)) {
    failures.push(`Windows recovery adapter is missing: ${required}`);
  }
}
if (/Set-ItemProperty[^\n]+-(?:Type|PropertyType)\b/u.test(windowsSnifferRuntime)) {
  failures.push("Windows proxy restoration uses an unsupported Set-ItemProperty type parameter");
}
if (/Remove-ItemProperty[^\n]+ErrorAction SilentlyContinue/u.test(windowsSnifferRuntime)) {
  failures.push("Windows proxy restoration silently ignores registry removal failures");
}
if (!windowsSnifferRuntime.includes("New-ItemProperty -LiteralPath $path -Name ProxyEnable -PropertyType DWord")) {
  failures.push("Windows proxy restoration does not preserve the ProxyEnable registry type");
}
if (windowsSnifferRuntime.includes(String.raw`LocalMachine\Root`)) {
  failures.push("Windows session certificates must not be installed machine-wide");
}
if (!snifferRuntime.includes('fs::write(&log, [])') || !snifferRuntime.includes("set_private_file(&log)")) {
  failures.push("Windows upstream log is not pre-created with a private session ACL");
}
for (const required of [
  "xunqi-authorized-sniffer.exe",
  "THIRD-PARTY-wx_channels_download.txt",
]) {
  if (!packager.includes(required)) failures.push(`Windows portable package is missing ${required}`);
}
if (!helperBuilder.includes("authorized-sniffer-helper-windows.json")) {
  failures.push("Windows helper builder is not reading the shared pin manifest");
}
if (!windowsSnifferRuntime.includes("authorized-sniffer-helper-windows.json")) {
  failures.push("Windows runtime is not reading the shared helper pin manifest");
}
for (const [label, value] of [
  ["release tag", helperManifest.releaseTag],
  ["asset name", helperManifest.assetName],
  ["download URL", helperManifest.downloadUrl],
]) {
  if (typeof value !== "string" || value.length === 0) failures.push(`Windows helper ${label} is missing`);
}
for (const [label, value] of [
  ["archive SHA-256", helperManifest.archiveSha256],
  ["binary SHA-256", helperManifest.binarySha256],
]) {
  if (!/^[a-f0-9]{64}$/u.test(value)) failures.push(`Windows helper ${label} is invalid`);
}

if (failures.length > 0) {
  console.error(`Windows runtime launcher validation failed:\n- ${failures.join("\n- ")}`);
  process.exit(1);
}

console.log("Windows runtime launchers passed: prebuilt app, local logs, and a pinned authorized-detection helper.");
