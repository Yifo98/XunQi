import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const css = await readFile(path.join(root, "src", "App.css"), "utf8");
const app = await readFile(path.join(root, "src", "App.tsx"), "utf8");

const failures = [];
const ruleFor = (selector) => {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`, "s"))?.[1] ?? "";
};

const topActions = ruleFor(".listening-status-top");
const aboutButton = css.match(/\.about-button,\s*\.link-guide-button\s*\{([^}]*)\}/s)?.[1] ?? "";
const compactHeaderStart = css.indexOf("@media (max-width: 1180px)");
const compactHeaderEnd = css.indexOf("@media (max-width: 1050px)");
const compactHeader = compactHeaderStart >= 0 && compactHeaderEnd > compactHeaderStart
  ? css.slice(compactHeaderStart, compactHeaderEnd)
  : "";

if (!/flex-wrap\s*:\s*nowrap/.test(topActions)) {
  failures.push("header actions must remain on one line");
}
if (!/white-space\s*:\s*nowrap/.test(aboutButton)) {
  failures.push("header action labels must not wrap");
}
if (!/flex\s*:\s*0\s+0\s+auto/.test(aboutButton)) {
  failures.push("header action buttons must not be squeezed narrower than their labels");
}
if (!app.includes('className="language-toggle"')) {
  failures.push("language toggle is missing from the shared app header");
}
if (!/\.link-guide-button\s*\{[\s\S]*?font-size\s*:\s*0/.test(compactHeader)) {
  failures.push("copy-link guidance must collapse to an icon before the 1180px header becomes crowded");
}
if (!/\.sniff-active-pill\s*\{[\s\S]*?text-overflow\s*:\s*ellipsis/.test(compactHeader)) {
  failures.push("active sniff status must be width-limited in the compact header");
}

if (failures.length > 0) {
  console.error(`Header layout contract failed:\n- ${failures.join("\n- ")}`);
  process.exit(1);
}

console.log("Header layout contract passed: language switch present and action labels stay on one line.");
