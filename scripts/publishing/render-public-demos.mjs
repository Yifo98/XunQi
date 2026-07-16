import { cpSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = join(scriptDir, "..", "..");
const sourceRoot = join(root, "assets", "publishing", "video-source");
const workRoot = join(root, "output", "publishing-render-work");
const outputRoot = process.env.XUNQI_PUBLISHING_OUTPUT
  ? resolve(process.cwd(), process.env.XUNQI_PUBLISHING_OUTPUT)
  : join(root, "output", "publishing-render");
const hyperframesVersion = "0.7.58";
const npx = process.platform === "win32" ? "npx.cmd" : "npx";

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    ...options,
  });

  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status}`);
  }
}

function prepareProject(locale) {
  const workDir = join(workRoot, locale);
  cpSync(join(sourceRoot, locale), workDir, { recursive: true });
  mkdirSync(join(workDir, "assets", "screens"), { recursive: true });
  mkdirSync(join(workDir, "assets", "brand"), { recursive: true });
  cpSync(
    join(root, "assets", "brand", "xunqi-qidu-master.png"),
    join(workDir, "assets", "brand", "xunqi-qidu-master.png"),
  );

  for (const index of ["01", "02", "03", "04", "05", "06", "07", "08"]) {
    cpSync(
      join(root, "assets", "publishing", locale, "screenshots", `xunqi-${locale}-${index}.png`),
      join(workDir, "assets", "screens", `xunqi-${index}.png`),
    );
  }
}

function renderLocale(locale) {
  const workDir = join(workRoot, locale);
  const videoOnly = join(workRoot, `xunqi-demo-${locale}-video-only.mp4`);
  const output = join(outputRoot, `xunqi-demo-${locale}.mp4`);

  run(npx, ["--yes", `hyperframes@${hyperframesVersion}`, "check", "--snapshots"], {
    cwd: workDir,
  });
  run(
    npx,
    [
      "--yes",
      `hyperframes@${hyperframesVersion}`,
      "render",
      "--quality",
      "high",
      "--fps",
      "30",
      "--output",
      videoOnly,
    ],
    { cwd: workDir },
  );

  run("ffmpeg", [
    "-y",
    "-loglevel",
    "error",
    "-i",
    videoOnly,
    "-f",
    "lavfi",
    "-t",
    "29",
    "-i",
    "anullsrc=channel_layout=stereo:sample_rate=48000",
    "-map",
    "0:v:0",
    "-map",
    "1:a:0",
    "-t",
    "29",
    "-r",
    "30",
    "-c:v",
    "libx264",
    "-preset",
    "slow",
    "-crf",
    "27",
    "-tune",
    "animation",
    "-pix_fmt",
    "yuv420p",
    "-c:a",
    "aac",
    "-b:a",
    "128k",
    "-movflags",
    "+faststart",
    "-shortest",
    output,
  ]);

  run("ffprobe", [
    "-v",
    "error",
    "-show_entries",
    "format=duration,size",
    "-show_entries",
    "stream=index,codec_name,codec_type,width,height,r_frame_rate",
    "-of",
    "default=noprint_wrappers=1",
    output,
  ]);
}

run(process.execPath, ["--version"]);
run("ffmpeg", ["-version"], { stdio: "ignore" });
run("ffprobe", ["-version"], { stdio: "ignore" });

rmSync(workRoot, { force: true, recursive: true });
mkdirSync(workRoot, { recursive: true });
mkdirSync(outputRoot, { recursive: true });

for (const locale of ["zh", "en"]) prepareProject(locale);
for (const locale of ["zh", "en"]) renderLocale(locale);

console.log(`Rendered bilingual demos to ${outputRoot}`);
