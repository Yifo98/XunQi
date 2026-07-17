import articleCoverUrl from "../../assets/preview/article-cover.svg?no-inline";
import type {
  ArticleExportMode,
  Backend,
  CaptureTaskDetail,
  OutputResult,
  SubmitLinksResult,
} from "./backend";

const now = "2026-07-13T15:42:00+08:00";

const articleBody = [
  "这是一份由讯栖项目生成的公开测试文档。",
  "文中的名称、标题和链接均为虚构内容，只用于演示任务识别、预览和导出流程。",
  "你可以勾选多篇测试文章，批量导出为 PDF，也可以选择 Markdown 和本地图片。",
  "所有操作都需要用户明确选择，讯栖不会在后台自动下载或完整备份。",
].join("\n\n");

function task(
  id: number,
  sourceName: string,
  title: string,
  kind: "article" | "video",
  status: CaptureTaskDetail["task"]["status"],
  statusDetail: string,
  minute: number,
): CaptureTaskDetail {
  const isPrimary = id === 1;
  const isReadyVideo = kind === "video" && status === "ready";
  const showsSniffAuthorizationState = kind === "video" && status === "failed";
  return {
    task: {
      id,
      kind,
      sourceName,
      title,
      author: kind === "article" ? sourceName : "",
      publishedAt: isPrimary ? "2026-07-13 15:42" : null,
      shareUrl:
        kind === "article"
          ? `https://mp.weixin.qq.com/s/preview-${id}`
          : `https://weixin.qq.com/sph/preview-${id}`,
      status,
      statusDetail,
      createdAt: `2026-07-13T${String(Math.max(10, 15 - Math.floor(minute / 60))).padStart(2, "0")}:${String(minute % 60).padStart(2, "0")}:00+08:00`,
      updatedAt: now,
      completedPath: status === "completed" ? "讯栖导出/已完成" : null,
    },
    article:
      kind === "article" && (status === "ready" || status === "completed")
        ? {
            title,
            author: sourceName,
            publishedAt: isPrimary ? "2026-07-13 15:42" : null,
            canonicalUrl: `https://mp.weixin.qq.com/s/preview-${id}`,
            bodyMarkdown: articleBody,
            bodyHtml: articleBody.split("\n\n").map((paragraph) => `<p>${paragraph}</p>`).join(""),
            coverImageUrl: isPrimary ? articleCoverUrl : null,
            imageUrls: isPrimary ? [articleCoverUrl] : [],
            wordCount: isPrimary ? 3852 : 1240,
          }
        : null,
    video: isReadyVideo || showsSniffAuthorizationState
      ? {
          pageTitle: title,
          pageUrl: `https://weixin.qq.com/sph/preview-${id}`,
          sourceName,
          description: title,
          publishedAt: null,
          coverImageUrl: null,
          candidates: [
            {
              url: isReadyVideo ? `https://cdn.example.com/preview-${id}.mp4` : "",
              kind: isReadyVideo ? "direct_file" : "unsupported",
              label: isReadyVideo ? "公开视频" : "页面未公开视频地址",
              downloadable: isReadyVideo,
            },
          ],
          limitation: isReadyVideo
            ? "只下载页面公开声明的 HTTPS 视频文件。"
            : "公开页面没有提供视频直链；如需继续，必须由用户明确授权临时嗅探，并在结束后恢复网络。",
        }
      : null,
  };
}

const fixtures: CaptureTaskDetail[] = [
  task(1, "示例科技周报", "测试文档：本地内容整理工作流", "article", "ready", "内容读取完成，可导出", 42),
  task(2, "示例科技周报", "测试文档：三步完成文章导出", "article", "ready", "内容读取完成，可导出", 28),
  task(3, "示例科技周报", "测试文档：正在读取的公开内容", "article", "processing", "正在读取公开文章正文…", 14),
  task(4, "示例科技周报", "测试文档：已完成的归档示例", "article", "completed", "文章已导出到本地", 5),
  task(5, "示例科技周报", "测试文档：等待处理的文章", "article", "queued", "等待读取公开内容", 1),
  task(6, "影像测试频道", "测试视频：公开直链识别演示", "video", "ready", "识别到 1 个可下载的公开视频", 55),
  task(7, "影像测试频道", "测试视频：下载进度演示", "video", "downloading", "正在下载公开视频…", 41),
  task(8, "城市观察实验室", "测试文档：批量导出示例", "article", "ready", "内容读取完成，可导出", 22),
  task(9, "影像测试频道", "测试视频：页面识别演示", "video", "processing", "正在检查页面中的公开视频…", 32),
  task(10, "影像测试频道", "测试视频：无公开直链示例", "video", "failed", "页面没有公开直接视频地址", 18),
  task(11, "影像测试频道", "测试视频：可下载内容示例", "video", "ready", "识别到 1 个可下载的公开视频", 8),
  task(12, "影像测试频道", "测试视频：已完成任务示例", "video", "completed", "视频已下载到本地", 2),
];

export function createPreviewBackend(): Backend {
  let records = structuredClone(fixtures);
  let nextTaskId = 13;

  return {
    listTasks: async () => structuredClone(records),
    getTaskDetail: async (taskId) => {
      const record = records.find((item) => item.task.id === taskId);
      if (!record) throw new Error("没有找到任务");
      return structuredClone(record);
    },
    submitLinks: async (rawText): Promise<SubmitLinksResult> => {
      const shareUrl = rawText.trim();
      const existing = records.find((item) => item.task.shareUrl === shareUrl);
      if (existing) {
        return {
          tasks: [structuredClone(existing.task)],
          duplicateCount: 1,
        };
      }

      const isVideo = previewShareKind(shareUrl) === "video";
      const created = task(
        nextTaskId,
        isVideo ? "公开影像测试源" : "公开文章测试源",
        isVideo ? "测试视频：自动收取与分类演示" : "测试文档：自动收取与分类演示",
        isVideo ? "video" : "article",
        "ready",
        isVideo ? "识别到 1 个可下载的公开视频" : "内容读取完成，可导出",
        59,
      );
      created.task.shareUrl = shareUrl;
      if (created.article) created.article.canonicalUrl = shareUrl;
      if (created.video) created.video.pageUrl = shareUrl;
      nextTaskId += 1;
      records = [created, ...records];
      return {
        tasks: [structuredClone(created.task)],
        duplicateCount: 0,
      };
    },
    processTask: async (taskId) => {
      const record = records.find((item) => item.task.id === taskId);
      if (!record) throw new Error("没有找到任务");
      return structuredClone(record);
    },
    exportArticle: async (taskId, _destinationDir, mode: ArticleExportMode) => {
      const result: OutputResult = {
        taskId,
        action: mode === "pdf" ? "article_pdf" : "article_markdown",
        destination: mode === "pdf" ? "讯栖导出/文章.pdf" : "讯栖导出/文章.md",
        bytesWritten: 4096,
      };
      records = records.map((record) =>
        record.task.id === taskId
          ? {
              ...record,
              task: {
                ...record.task,
                status: "completed",
                statusDetail: mode === "pdf" ? "原版长页 PDF 已保存" : "文章已导出到本地",
                completedPath: result.destination,
              },
            }
          : record,
      );
      return result;
    },
    downloadVideo: async (taskId) => ({
      taskId,
      action: "video_download",
      destination: "讯栖导出/视频.mp4",
      bytesWritten: 1024 * 1024,
    }),
    prepareVideoSniff: async (taskId) => ({
      planId: `preview-plan-${taskId}`,
      taskId,
      expiresAt: "2099-01-01T00:00:00Z",
      canStart: true,
      changes: ["temporary_proxy", "temporary_certificate"],
      reusesAuthorization: false,
      helperSource: "ltaoo/wx_channels_download v260706",
      conflict: null,
    }),
    startVideoSniff: async (taskId) => ({
      sessionId: `preview-session-${taskId}`,
      taskId,
      phase: "awaiting_playback",
      message: "助手已启用，请回到微信播放当前视频。",
      helperPageUrl: "http://127.0.0.1:2022/download",
      destinationDirectory: "/tmp/讯栖预览输出",
      authorizationReusable: true,
      progress: null,
      output: null,
      errorCode: null,
    }),
    getVideoSniffSession: async (sessionId) => ({
      sessionId,
      taskId: Number(sessionId.split("-").slice(-1)[0]) || 0,
      phase: "awaiting_playback",
      message: "助手已启用，请回到微信播放当前视频。",
      helperPageUrl: "http://127.0.0.1:2022/download",
      destinationDirectory: "/tmp/讯栖预览输出",
      authorizationReusable: true,
      progress: null,
      output: null,
      errorCode: null,
    }),
    stopVideoSniff: async (sessionId) => ({
      sessionId,
      taskId: Number(sessionId.split("-").slice(-1)[0]) || 0,
      phase: "cancelled_restored",
      message: "已停止助手并恢复网络设置。",
      helperPageUrl: null,
      destinationDirectory: "/tmp/讯栖预览输出",
      authorizationReusable: false,
      progress: null,
      output: null,
      errorCode: null,
    }),
    recoverVideoSniffing: async () => ({
      recovered: true,
      message: "系统网络设置正常。",
    }),
    clearTasks: async (taskIds) => {
      const before = records.length;
      records = records.filter((record) => !taskIds.includes(record.task.id));
      return before - records.length;
    },
    chooseOutputDirectory: async () => "/tmp/讯栖预览输出",
    chooseDiagnosticDestination: async () => "/tmp/XunQi-Diagnostics-preview.txt",
    exportDiagnostics: async (destinationPath) => ({
      destination: destinationPath,
      bytesWritten: 2048,
    }),
    detectWechatForeground: async () => ({
      isWechatFrontmost: false,
      applicationName: null,
      bundleIdentifier: null,
      limitation: "预览模式不读取本机微信状态。",
    }),
    readClipboardText: async () => "",
    openExternal: async () => undefined,
    revealOutput: async () => undefined,
  };
}

function previewShareKind(shareUrl: string): "article" | "video" {
  const url = new URL(shareUrl);
  return url.hostname === "mp.weixin.qq.com" ? "article" : "video";
}
