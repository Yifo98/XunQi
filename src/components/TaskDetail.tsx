import { useState } from "react";
import {
  ArchiveBoxIcon,
  ArrowClockwiseIcon,
  ArrowSquareOutIcon,
  DownloadSimpleIcon,
  FileTextIcon,
  FolderOpenIcon,
  LinkSimpleIcon,
  SpinnerGapIcon,
  VideoCameraIcon,
  WarningCircleIcon,
} from "@phosphor-icons/react";
import logoUrl from "../../assets/brand/xunqi-ui.png";
import type {
  ArticleExportMode,
  CaptureStatus,
  CaptureTaskDetail,
  DetectedVideo,
  SniffQualityMode,
  SniffSessionSnapshot,
} from "../lib/backend";
import { useI18n, type AppLanguage } from "../i18n";

type TaskDetailProps = {
  detail: CaptureTaskDetail | null;
  busy: boolean;
  onProcess: (taskId: number) => void;
  onExport: (taskId: number, mode: ArticleExportMode) => void;
  onDownload: (taskId: number, candidate: DetectedVideo) => void;
  onSniff: (taskId: number) => void;
  onStopSniff: (sessionId: string) => void;
  onRecoverSniff: () => void;
  sniffSession: SniffSessionSnapshot | null;
  authorizedSniffSupported: boolean;
  videoQualityMode: SniffQualityMode;
  onVideoQualityModeChange: (mode: SniffQualityMode) => void;
  onOpenOriginal: (url: string) => void;
  onRevealOutput: (path: string) => void;
  articleBatch: {
    articleCount: number;
    videoCount: number;
    busy: boolean;
    progressLabel: string | null;
    onExport: () => void;
    onClearSelection: () => void;
  };
  videoQueue: {
    selectedCount: number;
    skippedCount: number;
    running: boolean;
    progressLabel: string | null;
    onStart: () => void;
    onClearSelection: () => void;
    qualityMode: SniffQualityMode;
    onQualityModeChange: (mode: SniffQualityMode) => void;
    qualityLocked: boolean;
  };
};

export function TaskDetail({
  detail,
  busy,
  onProcess,
  onExport,
  onDownload,
  onSniff,
  onStopSniff,
  onRecoverSniff,
  sniffSession,
  authorizedSniffSupported,
  videoQualityMode,
  onVideoQualityModeChange,
  onOpenOriginal,
  onRevealOutput,
  articleBatch,
  videoQueue,
}: TaskDetailProps) {
  const { language, text } = useI18n();
  const [exportMode, setExportMode] = useState<ArticleExportMode>("pdf");

  if (!detail) {
    return (
      <main className="task-detail task-detail-empty">
        <img src={logoUrl} alt="讯栖" />
        <h2>{text("等待微信分享链接", "Waiting for a WeChat Share Link")}</h2>
        <p>{text("在微信中打开文章或视频号，点“分享 → 复制链接”，任务会自动出现在左侧。", "Open an article or Channels video in WeChat, choose Share → Copy Link, and the task will appear on the left.")}</p>
        <span className="empty-brand-note">{text("讯来有迹，文止于栖。只接住你主动选择的内容。", "Messages traced, stories at rest. Only content you choose is received.")}</span>
      </main>
    );
  }

  const { task, article, video } = detail;
  const taskSniffSession = sniffSession?.taskId === task.id ? sniffSession : null;
  const downloadable = video?.candidates.find((candidate) => candidate.downloadable) ?? null;
  const canProcess = task.status === "queued" || task.status === "failed" || task.status === "needs_attention";

  return (
    <main className="task-detail">
      <div className="detail-scroll">
        <header className="detail-header">
          <h1>{task.title}</h1>
          <div className="detail-byline">
            <strong>{task.sourceName}</strong>
            <StatusBadge kind={task.kind} status={task.status} />
          </div>
          <time dateTime={task.publishedAt ?? task.createdAt}>
            {formatLongDate(task.publishedAt ?? task.createdAt, language)}
          </time>
        </header>

        {task.kind === "article" ? (
          <ArticleDetail detail={detail} onRevealOutput={onRevealOutput} />
        ) : (
          <VideoDetail
            detail={detail}
            onDownload={onDownload}
            onRevealOutput={onRevealOutput}
            busy={busy}
            sniffSession={taskSniffSession}
            authorizationReusable={sniffSession?.authorizationReusable === true}
            authorizedSniffSupported={authorizedSniffSupported}
            qualityMode={videoQualityMode}
            onStopSniff={onStopSniff}
            onRecoverSniff={onRecoverSniff}
          />
        )}

        {(task.status === "failed" || task.status === "needs_attention") && (
          <div className="attention-note" role="status">
            <WarningCircleIcon size={22} weight="fill" />
            <div>
              <strong>{task.status === "failed"
                ? text("这次没有处理成功", "This task could not be processed")
                : text("当前页面需要你确认", "This page needs your review")}</strong>
              <p>{localizedTaskStatusDetail(task, language)}</p>
            </div>
          </div>
        )}
      </div>

      {articleBatch.articleCount > 0 ? (
        <footer className="detail-actions article-batch-actions" aria-label={text("公众号批量导出", "Batch article export")}>
          <div className="article-batch-summary">
            <span className="article-batch-icon"><FileTextIcon size={21} weight="duotone" /></span>
            <div>
              <strong>{text(`已选 ${articleBatch.articleCount} 篇公众号文章`, `${articleBatch.articleCount} Articles Selected`)}</strong>
              <p>
                {articleBatch.videoCount > 0
                  ? text(`另选 ${articleBatch.videoCount} 个视频号，不参与本次 PDF 导出`, `${articleBatch.videoCount} selected Channels items will not be included in this PDF export`)
                  : text("未读取的文章会先自动处理，全部保存到同一文件夹", "Unread articles are processed first, then all PDFs are saved in one folder")}
              </p>
            </div>
          </div>
          <div className="detail-action-buttons">
            <button
              type="button"
              className="secondary-action"
              onClick={articleBatch.onClearSelection}
              disabled={articleBatch.busy}
            >
              {text("取消选择", "Clear Selection")}
            </button>
            <button
              type="button"
              className="primary-action article-batch-button"
              onClick={articleBatch.onExport}
              disabled={articleBatch.busy}
            >
              {articleBatch.busy ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              {articleBatch.busy && articleBatch.progressLabel
                ? text(`正在批量导出 ${articleBatch.progressLabel}`, `Exporting ${articleBatch.progressLabel}`)
                : text(`批量处理并导出 ${articleBatch.articleCount} 篇 PDF`, `Process and Export ${articleBatch.articleCount} PDFs`)}
            </button>
          </div>
        </footer>
      ) : videoQueue.selectedCount > 0 ? (
        <footer className="detail-actions video-queue-actions" aria-label={text("视频号连续下载队列", "Channels download queue")}>
          <div className="article-batch-summary">
            <span className="article-batch-icon"><VideoCameraIcon size={21} weight="duotone" /></span>
            <div>
              <strong>
                {videoQueue.progressLabel
                  ? text(`连续下载 ${videoQueue.progressLabel}`, `Download Queue ${videoQueue.progressLabel}`)
                  : text(`已选 ${videoQueue.selectedCount} 条视频号`, `${videoQueue.selectedCount} Channels Videos Selected`)}
              </strong>
              <p>
                {text("每条独立校验，完成后自动接下一条；同一队列只需首次授权一次。", "Each video is verified independently, then the queue advances automatically. One authorization covers the queue.")}
                {videoQueue.skippedCount > 0
                  ? text(` 已跳过 ${videoQueue.skippedCount} 条已保存或不需要嗅探的视频。`, ` ${videoQueue.skippedCount} saved or direct-download items were skipped.`)
                  : ""}
              </p>
            </div>
          </div>
          <div className="detail-action-buttons">
            <QualitySelect
              value={videoQueue.qualityMode}
              onChange={videoQueue.onQualityModeChange}
              disabled={videoQueue.running || videoQueue.qualityLocked}
            />
            <span className="queue-concurrency-badge">{text("单任务校验 · 自动续接", "One at a Time · Auto-Continue")}</span>
            <button
              type="button"
              className="secondary-action"
              onClick={videoQueue.onClearSelection}
              disabled={videoQueue.running}
            >
              {text("取消选择", "Clear Selection")}
            </button>
            <button
              type="button"
              className="primary-action"
              onClick={videoQueue.onStart}
              disabled={videoQueue.running}
            >
              {videoQueue.running ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              {videoQueue.running
                ? text("队列处理中", "Queue in Progress")
                : text(`开始连续下载 ${videoQueue.selectedCount} 条`, `Download ${videoQueue.selectedCount} Videos`)}
            </button>
          </div>
        </footer>
      ) : (
      <footer className="detail-actions">
        {task.kind === "article" && article ? (
          <label className="format-select">
            <FileTextIcon size={18} />
            <select value={exportMode} onChange={(event) => setExportMode(event.target.value as ArticleExportMode)}>
              <option value="pdf">{text("PDF（保留原文结构）", "PDF (Original Layout)")}</option>
              <option value="markdown">{text("Markdown + 本地图片", "Markdown + Local Images")}</option>
            </select>
          </label>
        ) : task.kind === "video" && video && !downloadable && authorizedSniffSupported ? (
          <QualitySelect
            value={videoQualityMode}
            onChange={onVideoQualityModeChange}
            disabled={busy || isActiveSniff(sniffSession) || sniffSession?.authorizationReusable === true}
          />
        ) : (
          <span className="detail-capability">
            {task.kind === "video" ? <VideoCameraIcon size={18} /> : <FileTextIcon size={18} />}
            {task.kind === "video" ? text("公开视频直链", "Public Video Link") : text("公开文章", "Public Article")}
          </span>
        )}

        <div className="detail-action-buttons">
          {canProcess && (
            <button type="button" className="secondary-action" onClick={() => onProcess(task.id)} disabled={busy}>
              <ArrowClockwiseIcon size={19} />
              {busy ? text("正在处理", "Processing") : text("重新识别", "Retry")}
            </button>
          )}
          {task.kind === "article" && article && (
            <button type="button" className="primary-action" onClick={() => onExport(task.id, exportMode)} disabled={busy}>
              {busy ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              {exportMode === "pdf" ? text("导出原版 PDF", "Export Original PDF") : text("导出到本地", "Export Locally")}
            </button>
          )}
          {task.kind === "video" && downloadable && (
            <button type="button" className="primary-action" onClick={() => onDownload(task.id, downloadable)} disabled={busy}>
              {busy ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              {text("下载到本地", "Download Locally")}
            </button>
          )}
          {task.kind === "video" && video && !downloadable && authorizedSniffSupported && (
            <button type="button" className="primary-action" onClick={() => onSniff(task.id)} disabled={busy || isActiveSniff(sniffSession)}>
              {busy ? <SpinnerGapIcon className="spin" size={20} /> : <LinkSimpleIcon size={20} />}
              {sniffSession?.authorizationReusable
                ? text("嗅探下载", "Detection Download")
                : text("授权嗅探下载", "Authorize Detection")}
            </button>
          )}
          <button type="button" className="secondary-action" onClick={() => onOpenOriginal(task.shareUrl)} disabled={busy}>
            <ArrowSquareOutIcon size={19} />
            {text("打开微信原文", "Open in WeChat")}
          </button>
        </div>
      </footer>
      )}
    </main>
  );
}

function ArticleDetail({
  detail,
  onRevealOutput,
}: {
  detail: CaptureTaskDetail;
  onRevealOutput: (path: string) => void;
}) {
  const { language, text } = useI18n();
  const { task, article } = detail;
  if (!article) {
    return (
      <section className="pending-detail">
        <SpinnerGapIcon className={task.status === "processing" ? "spin" : ""} size={34} />
        <h2>{task.status === "processing"
          ? text("正在读取公开文章", "Reading Public Article")
          : text("文章尚未读取", "Article Not Read Yet")}</h2>
        <p>{text("讯栖会读取公开页面的标题、作者、发布时间和正文，不使用微信 Cookie。", "XunQi reads the title, author, date, and body from the public page without using WeChat cookies.")}</p>
      </section>
    );
  }

  const paragraphs = article.bodyMarkdown
    .split(/\n{2,}/)
    .map((paragraph) => paragraph.replace(/^#{1,6}\s+|^>\s+|^-\s+/u, "").trim())
    .filter(Boolean);

  const previewCoverUrl = localPreviewAsset(article.coverImageUrl);

  return (
    <article className="article-preview">
      <div className="article-body">
        {paragraphs.slice(0, 8).map((paragraph, index) => (
          <p key={`${index}-${paragraph.slice(0, 18)}`}>{paragraph}</p>
        ))}
      </div>
      {previewCoverUrl && (
        <img className="article-cover" src={previewCoverUrl} alt={`${article.title}封面`} />
      )}
      <dl className="detail-metadata">
        <Metadata label={text("类型", "Type")} value={text("公众号文章", "Official Account Article")} />
        <Metadata label={text("作者", "Author")} value={article.author || task.sourceName} />
        <Metadata label={text("原文链接", "Original Link")} value={article.canonicalUrl} link />
        <Metadata
          label={text("字数统计", "Word Count")}
          value={language === "zh"
            ? `约 ${article.wordCount.toLocaleString("zh-CN")} 字`
            : `About ${article.wordCount.toLocaleString("en-US")} words`}
        />
        <Metadata
          label={text("资源统计", "Assets")}
          value={language === "zh"
            ? `图片 ${article.imageUrls.length} 张`
            : `${article.imageUrls.length} ${article.imageUrls.length === 1 ? "image" : "images"}`}
        />
      </dl>
      {task.completedPath && (
        <div className="completed-output">
          <ArchiveBoxIcon size={20} />
          <span>{text("已保存到", "Saved to")} {task.completedPath}</span>
          <button type="button" onClick={() => onRevealOutput(task.completedPath!)}>
            <FolderOpenIcon size={17} />
            {text("在 Finder 中显示", "Show in Folder")}
          </button>
        </div>
      )}
    </article>
  );
}

function localPreviewAsset(value: string | null) {
  if (!value) return null;
  try {
    const parsed = new URL(value, window.location.href);
    return parsed.origin === window.location.origin ? parsed.toString() : null;
  } catch {
    return null;
  }
}

function VideoDetail({
  detail,
  onDownload,
  onRevealOutput,
  busy,
  sniffSession,
  authorizationReusable,
  authorizedSniffSupported,
  qualityMode,
  onStopSniff,
  onRecoverSniff,
}: {
  detail: CaptureTaskDetail;
  onDownload: (taskId: number, candidate: DetectedVideo) => void;
  onRevealOutput: (path: string) => void;
  busy: boolean;
  sniffSession: SniffSessionSnapshot | null;
  authorizationReusable: boolean;
  authorizedSniffSupported: boolean;
  qualityMode: SniffQualityMode;
  onStopSniff: (sessionId: string) => void;
  onRecoverSniff: () => void;
}) {
  const { language, text } = useI18n();
  const { task, video } = detail;
  if (!video) {
    return (
      <section className="pending-detail">
        <SpinnerGapIcon className={task.status === "processing" ? "spin" : ""} size={34} />
        <h2>{task.status === "processing"
          ? text("正在识别公开视频", "Detecting Public Video")
          : text("等待识别视频页面", "Waiting to Inspect Video Page")}</h2>
        <p>{text("只检查页面公开声明的媒体地址，不抓包、不安装证书、不读取登录态。", "Only public media addresses declared by the page are checked. No packet capture, certificates, or login state are used.")}</p>
      </section>
    );
  }

  return (
    <section className="video-detail">
      <div className="video-summary">
        <VideoCameraIcon size={36} weight="duotone" />
        <div>
          <h2>{video.pageTitle}</h2>
          <p>{localizedVideoLimitation(video, language)}</p>
        </div>
      </div>
      <div className="candidate-list">
        {video.candidates.length === 0 ? (
          <div className="candidate-empty">
            <WarningCircleIcon size={22} />
            {localizedVideoLimitation(video, language)}
          </div>
        ) : (
          video.candidates.map((candidate) => (
            <article className="candidate-card" key={candidate.url}>
              <span className="candidate-icon"><LinkSimpleIcon size={20} /></span>
              <div>
                <strong>{localizedCandidateLabel(candidate.label, language)}</strong>
                <p>{candidate.url}</p>
                <span className={candidate.downloadable ? "candidate-ready" : "candidate-blocked"}>
                  {candidate.downloadable ? text("可下载", "Downloadable") : candidateKindLabel(candidate.kind, language)}
                </span>
              </div>
              {candidate.downloadable && (
                <button type="button" onClick={() => onDownload(task.id, candidate)} disabled={busy}>
                  <DownloadSimpleIcon size={19} />
                  {text("下载", "Download")}
                </button>
              )}
            </article>
          ))
        )}
      </div>
      {video.candidates.every((candidate) => !candidate.downloadable) && !sniffSession && authorizedSniffSupported && (
        <div className="authorized-sniff-intro">
          <LinkSimpleIcon size={24} weight="duotone" />
          <div>
            <strong>{authorizationReusable
              ? text("连续授权可用", "Continuous Authorization Available")
              : text("可改用授权嗅探助手", "Authorized Detection Available")}</strong>
            <p>{authorizationReusable
              ? text("下一条不再要求指纹；处理完成前请保持 VPN 关闭。", "The next item will not request Touch ID again. Keep your VPN off until processing is complete.")
              : text("请先关闭 VPN。首次确认后只在本机临时启用代理和会话证书。", "Turn off your VPN first. After confirmation, a local proxy and session certificate are enabled temporarily on this Mac.")}</p>
          </div>
        </div>
      )}
      {video.candidates.every((candidate) => !candidate.downloadable) && !authorizedSniffSupported && (
        <div className="authorized-sniff-intro authorized-sniff-unavailable" role="note">
          <WarningCircleIcon size={24} weight="duotone" />
          <div>
            <strong>{text("Windows 测试版暂不支持授权嗅探", "Authorized Detection Is Not Available on Windows Yet")}</strong>
            <p>{text(
              "这不是 WebView2 或安装包缺失。公众号导出和公开视频直链下载仍可使用；授权嗅探需等 Windows 网络恢复适配完成后再开放。",
              "This is not a missing WebView2 component or an incomplete package. Article export and public direct-video downloads still work; authorized detection will be enabled only after Windows network recovery is safely supported.",
            )}</p>
          </div>
        </div>
      )}
      {sniffSession && (
        <div className={`sniff-session sniff-session-${sniffSession.phase}`} role="status">
          <SpinnerGapIcon className={isActiveSniff(sniffSession) ? "spin" : ""} size={24} />
          <div>
            <strong>{sniffPhaseTitle(sniffSession.phase, language)}</strong>
            <p>{localizedSniffSessionMessage(sniffSession, language)}</p>
            {sniffSession.progress && (
              <div className="sniff-progress" aria-label={text("下载进度", "Download progress")}>
                <div className="sniff-progress-heading">
                  <strong>{sniffSession.progress.percent === null ? text("正在下载", "Downloading") : `${sniffSession.progress.percent}%`}</strong>
                  <span>{formatBytes(sniffSession.progress.downloadedBytes)}{sniffSession.progress.totalBytes ? ` / ${formatBytes(sniffSession.progress.totalBytes)}` : ""}</span>
                  <span>{formatSpeed(sniffSession.progress.bytesPerSecond, language)}</span>
                </div>
                <div className="sniff-progress-track">
                  <span style={{ width: `${sniffSession.progress.percent ?? 4}%` }} />
                </div>
              </div>
            )}
            <div className="sniff-media-summary" aria-label={text("视频基础信息", "Video details")}>
              <div>
                <span>{text("画质", "Quality")}</span>
                <strong>{sniffSession.output
                  ? localizedOutputQualityLabel(sniffSession.output.qualityLabel, qualityMode, language)
                  : qualityModeLabel(qualityMode, language)}</strong>
              </div>
              <div>
                <span>{sniffSession.output ? text("文件大小", "File Size") : text("预计大小", "Estimated Size")}</span>
                <strong>{formatOptionalBytes(
                  sniffSession.output?.bytesWritten
                    ?? sniffSession.progress?.totalBytes
                    ?? sniffSession.progress?.downloadedBytes
                    ?? null,
                  language,
                )}</strong>
              </div>
              <div>
                <span>{text("分辨率", "Resolution")}</span>
                <strong>{sniffSession.output?.width && sniffSession.output?.height
                  ? `${sniffSession.output.width} × ${sniffSession.output.height}`
                  : text("完成后读取", "Read after download")}</strong>
              </div>
            </div>
            <p className="sniff-media-note">{qualityMode === "original"
              ? text("本次按原始视频保存；实际分辨率和大小会在下载后读取。", "The original stream will be saved. Resolution and size are read after download.")
              : text("本次使用微信返回的默认规格以节省空间；实际分辨率和大小会在下载后读取。", "WeChat's default stream is used to save space. Resolution and size are read after download.")}</p>
            {sniffSession.destinationDirectory && (
              <p className="sniff-destination">{text("保存到", "Save to")} {sniffSession.destinationDirectory}</p>
            )}
            {isActiveSniff(sniffSession) && (
              <p className="sniff-session-tip">
                {text("不要退出微信，也不要在浏览器扫码。请从微信左侧重新进入一次“视频号”；无需刷新，也不用寻找页面下载按钮。讯栖只会下载当前任务里已经复制的分享链接。", "Do not quit WeChat or scan a browser QR code. Re-enter Channels from WeChat's sidebar once; do not refresh or look for an in-page download button. XunQi downloads only the copied link bound to this task.")}
              </p>
            )}
            <div className="sniff-session-actions">
              {sniffSession.destinationDirectory && (
                <button type="button" onClick={() => onRevealOutput(sniffSession.destinationDirectory)}>
                  <FolderOpenIcon size={17} />
                  {text("打开保存位置", "Open Save Location")}
                </button>
              )}
              {(isActiveSniff(sniffSession) || sniffSession.authorizationReusable) && (
                <button type="button" onClick={() => onStopSniff(sniffSession.sessionId)}>
                  {text("结束并恢复网络", "End and Restore Network")}
                </button>
              )}
              {sniffSession.phase === "restoration_required" && (
                <button type="button" onClick={onRecoverSniff}>
                  {text("立即恢复网络设置", "Restore Network Settings")}
                </button>
              )}
            </div>
          </div>
        </div>
      )}
      <dl className="detail-metadata">
        <Metadata label={text("类型", "Type")} value={text("视频号内容", "WeChat Channels Video")} />
        <Metadata label={text("来源", "Source")} value={video.sourceName || task.sourceName} />
        {video.publishedAt && <Metadata label={text("发布时间", "Published")} value={formatLongDate(video.publishedAt, language)} />}
        <Metadata label={text("分享链接", "Share Link")} value={video.pageUrl} link />
        <Metadata
          label={text("识别结果", "Detection Result")}
          value={language === "zh"
            ? `${video.candidates.filter((candidate) => candidate.downloadable).length} 个可下载视频`
            : `${video.candidates.filter((candidate) => candidate.downloadable).length} downloadable videos`}
        />
      </dl>
      {task.completedPath && (
        <div className="completed-output">
          <ArchiveBoxIcon size={20} />
          <span>{text("已保存到", "Saved to")} {task.completedPath}</span>
          <button type="button" onClick={() => onRevealOutput(task.completedPath!)}>
            <FolderOpenIcon size={17} />
            {text("在 Finder 中显示", "Show in Folder")}
          </button>
        </div>
      )}
    </section>
  );
}

function isActiveSniff(session: SniffSessionSnapshot | null) {
  return session !== null && ["starting", "awaiting_playback", "capturing", "saving", "restoring"].includes(session.phase);
}

function localizedTaskStatusDetail(
  task: CaptureTaskDetail["task"],
  language: AppLanguage,
) {
  if (language === "zh") return task.statusDetail;
  const byStatus: Record<CaptureStatus, string> = {
    queued: "Waiting to be processed.",
    processing: task.kind === "video" ? "Inspecting the public video page…" : "Reading the public article…",
    ready: task.kind === "video" ? "A public downloadable video is available." : "The article is ready to export.",
    needs_attention: task.kind === "video"
      ? "The public page did not expose a downloadable media address. Review the available options below."
      : "The public article needs review before it can be exported.",
    exporting: "Exporting the article…",
    downloading: "Downloading the video…",
    completed: task.completedPath ? `Saved to ${task.completedPath}` : "Processing completed.",
    failed: "Processing failed. Export the diagnostic log if you need help troubleshooting.",
  };
  return byStatus[task.status];
}

function localizedVideoLimitation(
  video: NonNullable<CaptureTaskDetail["video"]>,
  language: AppLanguage,
) {
  if (language === "zh") return video.limitation;
  if (video.candidates.some((candidate) => candidate.downloadable)) {
    return "A public downloadable media address was found.";
  }
  return "The public page did not expose a downloadable media address. Script-loaded, session-bound, or protected streams may not be available.";
}

function localizedSniffSessionMessage(
  session: SniffSessionSnapshot,
  language: AppLanguage,
) {
  if (language === "zh") return session.message;
  const messages: Record<SniffSessionSnapshot["phase"], string> = {
    starting: "Starting the local assistant and preparing the temporary network session…",
    awaiting_playback: "Return to WeChat and enter Channels once so the copied video can load.",
    capturing: "The matching video was detected and is being verified.",
    saving: "Saving and validating the video file…",
    restoring: "Restoring the original network settings…",
    completed: "The video was saved and verified.",
    failed_reusable: "This video was not completed. The authorization session can still be used for the next item.",
    failed_restored: "The download failed and the original network settings were restored.",
    cancelled_restored: "The assistant stopped and the original network settings were restored.",
    restoration_required: "Automatic recovery did not finish. Restore the network settings before starting another task.",
  };
  return messages[session.phase];
}

function QualitySelect({
  value,
  onChange,
  disabled,
}: {
  value: SniffQualityMode;
  onChange: (mode: SniffQualityMode) => void;
  disabled: boolean;
}) {
  const { language } = useI18n();
  return (
    <label className="format-select quality-select">
      <VideoCameraIcon size={18} />
      <select
        aria-label={language === "zh" ? "视频下载画质" : "Video download quality"}
        value={value}
        onChange={(event) => onChange(event.target.value as SniffQualityMode)}
        disabled={disabled}
      >
        <option value="original">{language === "zh" ? "原始画质（文件较大）" : "Original Quality (Larger File)"}</option>
        <option value="space_saver">{language === "zh" ? "节省空间（微信默认）" : "Save Space (WeChat Default)"}</option>
      </select>
    </label>
  );
}

function qualityModeLabel(mode: SniffQualityMode, language: AppLanguage) {
  if (language === "en") return mode === "original" ? "Original Quality" : "Save Space (WeChat Default)";
  return mode === "original" ? "原始画质" : "节省空间（微信默认）";
}

function localizedOutputQualityLabel(
  label: string,
  fallbackMode: SniffQualityMode,
  language: AppLanguage,
) {
  if (language === "zh" || !containsChinese(label)) return label;
  if (label === "原始画质") return "Original Quality";
  const wechatSpec = label.match(/^节省空间（微信默认规格 (.+)）$/u);
  if (wechatSpec) return `Save Space (WeChat Default ${wechatSpec[1]})`;
  return qualityModeLabel(fallbackMode, language);
}

function sniffPhaseTitle(phase: SniffSessionSnapshot["phase"], language: AppLanguage) {
  return (language === "zh" ? {
    starting: "正在启用授权助手",
    awaiting_playback: "请回微信播放视频",
    capturing: "已经识别到视频",
    saving: "正在保存视频",
    restoring: "正在恢复网络",
    completed: "视频已保存",
    failed_reusable: "本条未完成，可继续下一条",
    failed_restored: "未能下载，网络已恢复",
    cancelled_restored: "已停止，网络已恢复",
    restoration_required: "网络设置需要恢复",
  } : {
    starting: "Enabling Authorized Assistant",
    awaiting_playback: "Play the Video in WeChat",
    capturing: "Video Detected",
    saving: "Saving Video",
    restoring: "Restoring Network",
    completed: "Video Saved",
    failed_reusable: "This Item Failed; Session Still Available",
    failed_restored: "Download Failed; Network Restored",
    cancelled_restored: "Stopped; Network Restored",
    restoration_required: "Network Settings Need Recovery",
  })[phase];
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

function formatOptionalBytes(bytes: number | null, language: AppLanguage) {
  return bytes && bytes > 0 ? formatBytes(bytes) : language === "zh" ? "等待识别" : "Waiting";
}

function formatSpeed(bytesPerSecond: number, language: AppLanguage) {
  return bytesPerSecond > 0 ? `${formatBytes(bytesPerSecond)}/s` : language === "zh" ? "正在校验文件" : "Verifying file";
}

function Metadata({ label, value, link = false }: { label: string; value: string; link?: boolean }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd className={link ? "metadata-link" : ""}>{value}</dd>
    </div>
  );
}

function StatusBadge({ kind, status }: { kind: CaptureTaskDetail["task"]["kind"]; status: CaptureStatus }) {
  const { language } = useI18n();
  const labels: Record<CaptureStatus, string> = language === "zh" ? {
    queued: "等待处理",
    processing: "正在读取",
    ready: kind === "video" ? "视频可下载" : "内容读取完成",
    needs_attention: "需要查看",
    exporting: "正在导出",
    downloading: "正在下载",
    completed: "处理完成",
    failed: "处理失败",
  } : {
    queued: "Queued",
    processing: "Reading",
    ready: kind === "video" ? "Video Ready" : "Content Ready",
    needs_attention: "Review Needed",
    exporting: "Exporting",
    downloading: "Downloading",
    completed: "Completed",
    failed: "Failed",
  };
  return <span className={`detail-status detail-status-${status}`}>{labels[status]}</span>;
}

function candidateKindLabel(kind: DetectedVideo["kind"], language: AppLanguage) {
  return (language === "zh" ? {
    direct_file: "当前不可下载",
    hls_playlist: "HLS 暂不下载",
    dash_manifest: "DASH 暂不下载",
    unsupported: "受保护或动态媒体",
  } : {
    direct_file: "Unavailable",
    hls_playlist: "HLS Not Supported Yet",
    dash_manifest: "DASH Not Supported Yet",
    unsupported: "Protected or Dynamic Media",
  })[kind];
}

function localizedCandidateLabel(label: string, language: AppLanguage) {
  if (language === "zh" || !containsChinese(label)) return label;
  const labels: Record<string, string> = {
    "公开视频文件": "Public Video File",
    "微信公开视频（H.264）": "WeChat Public Video (H.264)",
    "微信公开视频（H.265）": "WeChat Public Video (H.265)",
    "微信原始视频": "WeChat Original Video",
    "微信公开视频": "WeChat Public Video",
  };
  return labels[label] ?? "Detected Video";
}

function containsChinese(value: string) {
  return /[\u3400-\u9fff]/u.test(value);
}

function formatLongDate(value: string, language: AppLanguage) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en-US", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}
