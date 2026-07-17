import { invoke } from "@tauri-apps/api/core";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { open } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";

export type CaptureKind = "article" | "video";
export type CaptureStatus =
  | "queued"
  | "processing"
  | "ready"
  | "needs_attention"
  | "exporting"
  | "downloading"
  | "completed"
  | "failed";

export type CaptureTask = {
  id: number;
  kind: CaptureKind;
  sourceName: string;
  title: string;
  author: string;
  publishedAt: string | null;
  shareUrl: string;
  status: CaptureStatus;
  statusDetail: string;
  createdAt: string;
  updatedAt: string;
  completedPath: string | null;
};

export type ArticleSnapshot = {
  title: string;
  author: string;
  publishedAt: string | null;
  canonicalUrl: string;
  bodyMarkdown: string;
  bodyHtml: string;
  coverImageUrl: string | null;
  imageUrls: string[];
  wordCount: number;
};

export type DetectedVideoKind =
  | "direct_file"
  | "hls_playlist"
  | "dash_manifest"
  | "unsupported";

export type DetectedVideo = {
  url: string;
  kind: DetectedVideoKind;
  label: string;
  downloadable: boolean;
};

export type VideoPageInspection = {
  pageTitle: string;
  pageUrl: string;
  sourceName: string;
  description: string;
  publishedAt: string | null;
  coverImageUrl: string | null;
  candidates: DetectedVideo[];
  limitation: string;
};

export type CaptureTaskDetail = {
  task: CaptureTask;
  article: ArticleSnapshot | null;
  video: VideoPageInspection | null;
};

export type SubmitLinksResult = {
  tasks: CaptureTask[];
  duplicateCount: number;
};

export type ArticleExportMode = "pdf" | "markdown";
export type OutputAction = "article_pdf" | "article_markdown" | "video_download";

export type OutputResult = {
  taskId: number;
  action: OutputAction;
  destination: string;
  bytesWritten: number;
  warning?: string | null;
};

export type WechatForegroundStatus = {
  isWechatFrontmost: boolean;
  applicationName: string | null;
  bundleIdentifier: string | null;
  limitation: string;
};

export type SniffAuthorizationPlan = {
  planId: string;
  taskId: number;
  expiresAt: string;
  canStart: boolean;
  changes: readonly ("temporary_proxy" | "temporary_certificate")[];
  reusesAuthorization: boolean;
  helperSource: string;
  conflict: null | {
    code: string;
    message: string;
  };
};

export type SniffQualityMode = "original" | "space_saver";

export type SniffPhase =
  | "starting"
  | "awaiting_playback"
  | "capturing"
  | "saving"
  | "restoring"
  | "completed"
  | "failed_reusable"
  | "failed_restored"
  | "cancelled_restored"
  | "restoration_required";

export type SniffSessionSnapshot = {
  sessionId: string;
  taskId: number;
  phase: SniffPhase;
  message: string;
  helperPageUrl: string | null;
  destinationDirectory: string;
  authorizationReusable: boolean;
  progress: null | {
    downloadedBytes: number;
    totalBytes: number | null;
    bytesPerSecond: number;
    percent: number | null;
  };
  output: null | {
    destination: string;
    bytesWritten: number;
    qualityLabel: string;
    width: number | null;
    height: number | null;
  };
  errorCode: string | null;
};

export type SniffRecoveryResult = {
  recovered: boolean;
  message: string;
};

export interface Backend {
  listTasks(): Promise<CaptureTaskDetail[]>;
  getTaskDetail(taskId: number): Promise<CaptureTaskDetail>;
  submitLinks(rawText: string): Promise<SubmitLinksResult>;
  processTask(taskId: number): Promise<CaptureTaskDetail>;
  exportArticle(
    taskId: number,
    destinationDir: string,
    mode: ArticleExportMode,
  ): Promise<OutputResult>;
  downloadVideo(
    taskId: number,
    candidateUrl: string,
    destinationDir: string,
  ): Promise<OutputResult>;
  prepareVideoSniff(taskId: number): Promise<SniffAuthorizationPlan>;
  startVideoSniff(
    taskId: number,
    planId: string,
    destinationDir: string,
    qualityMode: SniffQualityMode,
  ): Promise<SniffSessionSnapshot>;
  getVideoSniffSession(sessionId: string): Promise<SniffSessionSnapshot>;
  stopVideoSniff(sessionId: string): Promise<SniffSessionSnapshot>;
  recoverVideoSniffing(): Promise<SniffRecoveryResult>;
  clearTasks(taskIds: number[]): Promise<number>;
  chooseOutputDirectory(): Promise<string | null>;
  detectWechatForeground(): Promise<WechatForegroundStatus>;
  readClipboardText(): Promise<string>;
  openExternal(url: string): Promise<void>;
  revealOutput(path: string): Promise<void>;
}

export const tauriBackend: Backend = {
  listTasks: () => invoke<CaptureTaskDetail[]>("list_capture_tasks"),
  getTaskDetail: (taskId) => invoke<CaptureTaskDetail>("get_capture_task", { taskId }),
  submitLinks: (rawText) => invoke<SubmitLinksResult>("submit_links", { rawText }),
  processTask: (taskId) => invoke<CaptureTaskDetail>("process_capture_task", { taskId }),
  exportArticle: (taskId, destinationDir, mode) =>
    invoke<OutputResult>("export_article", { taskId, destinationDir, mode }),
  downloadVideo: (taskId, candidateUrl, destinationDir) =>
    invoke<OutputResult>("download_video", {
      taskId,
      candidateUrl,
      destinationDir,
    }),
  prepareVideoSniff: (taskId) =>
    invoke<SniffAuthorizationPlan>("prepare_video_sniff", { taskId }),
  startVideoSniff: (taskId, planId, destinationDir, qualityMode) =>
    invoke<SniffSessionSnapshot>("start_video_sniff", {
      taskId,
      planId,
      destinationDir,
      qualityMode,
    }),
  getVideoSniffSession: (sessionId) =>
    invoke<SniffSessionSnapshot>("get_video_sniff_session", { sessionId }),
  stopVideoSniff: (sessionId) =>
    invoke<SniffSessionSnapshot>("stop_video_sniff", { sessionId }),
  recoverVideoSniffing: () =>
    invoke<SniffRecoveryResult>("recover_video_sniffing"),
  clearTasks: (taskIds) => invoke<number>("clear_capture_tasks", { taskIds }),
  chooseOutputDirectory: async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择讯栖输出文件夹",
    });
    return typeof selected === "string" ? selected : null;
  },
  detectWechatForeground: () =>
    invoke<WechatForegroundStatus>("detect_wechat_foreground"),
  readClipboardText: async () => {
    try {
      return await readText();
    } catch (reason) {
      if (/clipboard.*empty|剪贴板.*空|requested format/i.test(String(reason))) {
        return "";
      }
      throw reason;
    }
  },
  openExternal: (url) => openUrl(url),
  revealOutput: (path) => revealItemInDir(path),
};

export function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}
