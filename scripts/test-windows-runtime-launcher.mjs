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
if (!Array.isArray(windowsConfig.bundle?.externalBin) || windowsConfig.bundle.externalBin.length !== 0) {
  failures.push("Windows runtime build must not bundle macOS-only sidecars");
}

if (failures.length > 0) {
  console.error(`Windows runtime launcher validation failed:\n- ${failures.join("\n- ")}`);
  process.exit(1);
}

console.log("Windows runtime launchers passed: ASCII, CRLF, prebuilt runtime only, local log access.");
