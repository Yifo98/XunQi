import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const launcherPath = path.join(root, "assets", "portable", "启动讯栖.bat");
const packageJson = JSON.parse(await readFile(path.join(root, "package.json"), "utf8"));
const launcher = await readFile(launcherPath);
const text = launcher.toString("ascii");

const failures = [];
const nonAsciiBytes = [...launcher].filter((byte) => byte > 0x7f).length;
const bareLineFeeds = (text.match(/(?<!\r)\n/g) ?? []).length;
const loneCarriageReturns = (text.match(/\r(?!\n)/g) ?? []).length;
const pnpmVersion = packageJson.packageManager?.match(/^pnpm@(.+)$/)?.[1];

if (nonAsciiBytes > 0) failures.push(`launcher contains ${nonAsciiBytes} non-ASCII bytes`);
if (bareLineFeeds > 0 || loneCarriageReturns > 0) {
  failures.push(`launcher is not strict CRLF: bare LF=${bareLineFeeds}, lone CR=${loneCarriageReturns}`);
}
if (!text.startsWith("@echo off\r\n")) failures.push("launcher does not start with @echo off");
if (!text.includes("XUNQI_LAUNCHER_SELF_TEST_OK")) failures.push("self-test marker is missing");
if (!text.includes("set \"COREPACK_ENABLE_DOWNLOAD_PROMPT=0\"")) {
  failures.push("Corepack download prompt is not disabled for unattended launcher use");
}
if (!/where corepack/i.test(text) || !/corepack pnpm --version/i.test(text)) {
  failures.push("Corepack pnpm fallback is missing");
}
if (!/where pnpm/i.test(text) || !/call pnpm --version/i.test(text)) {
  failures.push("global pnpm fallback is missing");
}
if (!pnpmVersion || !text.includes(`npx --yes pnpm@${pnpmVersion}`)) {
  failures.push("npx fallback does not match packageManager");
}
if (!text.includes("call %PNPM_RUN% install --frozen-lockfile")) {
  failures.push("dependency installation does not use the selected pnpm runner");
}
if (!text.includes("call %PNPM_RUN% tauri dev")) {
  failures.push("Tauri launch does not use the selected pnpm runner");
}

if (failures.length > 0) {
  console.error(`Windows launcher validation failed:\n- ${failures.join("\n- ")}`);
  process.exit(1);
}

console.log(
  `Windows launcher validation passed: ASCII-only, CRLF, self-test, pnpm/Corepack/npx fallbacks (${pnpmVersion}).`,
);
