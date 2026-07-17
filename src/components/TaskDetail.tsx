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
  videoQualityMode,
  onVideoQualityModeChange,
  onOpenOriginal,
  onRevealOutput,
  articleBatch,
  videoQueue,
}: TaskDetailProps) {
  const [exportMode, setExportMode] = useState<ArticleExportMode>("pdf");

  if (!detail) {
    return (
      <main className="task-detail task-detail-empty">
        <img src={logoUrl} alt="讯栖" />
        <h2>等待微信分享链接</h2>
        <p>在微信中打开文章或视频号，点“分享 → 复制链接”，任务会自动出现在左侧。</p>
        <span className="empty-brand-note">讯来有迹，文止于栖。只接住你主动选择的内容。</span>
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
            <StatusBadge status={task.status} detail={task.statusDetail} />
          </div>
          <time dateTime={task.publishedAt ?? task.createdAt}>
            {formatLongDate(task.publishedAt ?? task.createdAt)}
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
            qualityMode={videoQualityMode}
            onStopSniff={onStopSniff}
            onRecoverSniff={onRecoverSniff}
          />
        )}

        {(task.status === "failed" || task.status === "needs_attention") && (
          <div className="attention-note" role="status">
            <WarningCircleIcon size={22} weight="fill" />
            <div>
              <strong>{task.status === "failed" ? "这次没有处理成功" : "当前页面需要你确认"}</strong>
              <p>{task.statusDetail}</p>
            </div>
          </div>
        )}
      </div>

      {articleBatch.articleCount > 0 ? (
        <footer className="detail-actions article-batch-actions" aria-label="公众号批量导出">
          <div className="article-batch-summary">
            <span className="article-batch-icon"><FileTextIcon size={21} weight="duotone" /></span>
            <div>
              <strong>已选 {articleBatch.articleCount} 篇公众号文章</strong>
              <p>
                {articleBatch.videoCount > 0
                  ? `另选 ${articleBatch.videoCount} 个视频号，不参与本次 PDF 导出`
                  : "未读取的文章会先自动处理，全部保存到同一文件夹"}
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
              取消选择
            </button>
            <button
              type="button"
              className="primary-action article-batch-button"
              onClick={articleBatch.onExport}
              disabled={articleBatch.busy}
            >
              {articleBatch.busy ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              {articleBatch.busy && articleBatch.progressLabel
                ? `正在批量导出 ${articleBatch.progressLabel}`
                : `批量处理并导出 ${articleBatch.articleCount} 篇 PDF`}
            </button>
          </div>
        </footer>
      ) : videoQueue.selectedCount > 0 ? (
        <footer className="detail-actions video-queue-actions" aria-label="视频号连续下载队列">
          <div className="article-batch-summary">
            <span className="article-batch-icon"><VideoCameraIcon size={21} weight="duotone" /></span>
            <div>
              <strong>
                {videoQueue.progressLabel
                  ? `连续下载 ${videoQueue.progressLabel}`
                  : `已选 ${videoQueue.selectedCount} 条视频号`}
              </strong>
              <p>
                每条独立校验，完成后自动接下一条；同一队列只需首次授权一次。
                {videoQueue.skippedCount > 0 ? ` 已跳过 ${videoQueue.skippedCount} 条已保存或不需要嗅探的视频。` : ""}
              </p>
            </div>
          </div>
          <div className="detail-action-buttons">
            <QualitySelect
              value={videoQueue.qualityMode}
              onChange={videoQueue.onQualityModeChange}
              disabled={videoQueue.running || videoQueue.qualityLocked}
            />
            <span className="queue-concurrency-badge">单任务校验 · 自动续接</span>
            <button
              type="button"
              className="secondary-action"
              onClick={videoQueue.onClearSelection}
              disabled={videoQueue.running}
            >
              取消选择
            </button>
            <button
              type="button"
              className="primary-action"
              onClick={videoQueue.onStart}
              disabled={videoQueue.running}
            >
              {videoQueue.running ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              {videoQueue.running ? "队列处理中" : `开始连续下载 ${videoQueue.selectedCount} 条`}
            </button>
          </div>
        </footer>
      ) : (
      <footer className="detail-actions">
        {task.kind === "article" && article ? (
          <label className="format-select">
            <FileTextIcon size={18} />
            <select value={exportMode} onChange={(event) => setExportMode(event.target.value as ArticleExportMode)}>
              <option value="pdf">PDF（保留原文结构）</option>
              <option value="markdown">Markdown + 本地图片</option>
            </select>
          </label>
        ) : task.kind === "video" && video && !downloadable ? (
          <QualitySelect
            value={videoQualityMode}
            onChange={onVideoQualityModeChange}
            disabled={busy || isActiveSniff(sniffSession) || sniffSession?.authorizationReusable === true}
          />
        ) : (
          <span className="detail-capability">
            {task.kind === "video" ? <VideoCameraIcon size={18} /> : <FileTextIcon size={18} />}
            {task.kind === "video" ? "公开视频直链" : "公开文章"}
          </span>
        )}

        <div className="detail-action-buttons">
          {canProcess && (
            <button type="button" className="secondary-action" onClick={() => onProcess(task.id)} disabled={busy}>
              <ArrowClockwiseIcon size={19} />
              {busy ? "正在处理" : "重新识别"}
            </button>
          )}
          {task.kind === "article" && article && (
            <button type="button" className="primary-action" onClick={() => onExport(task.id, exportMode)} disabled={busy}>
              {busy ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              {exportMode === "pdf" ? "导出原版 PDF" : "导出到本地"}
            </button>
          )}
          {task.kind === "video" && downloadable && (
            <button type="button" className="primary-action" onClick={() => onDownload(task.id, downloadable)} disabled={busy}>
              {busy ? <SpinnerGapIcon className="spin" size={20} /> : <DownloadSimpleIcon size={20} />}
              下载到本地
            </button>
          )}
          {task.kind === "video" && video && !downloadable && (
            <button type="button" className="primary-action" onClick={() => onSniff(task.id)} disabled={busy || isActiveSniff(sniffSession)}>
              {busy ? <SpinnerGapIcon className="spin" size={20} /> : <LinkSimpleIcon size={20} />}
              {sniffSession?.authorizationReusable ? "嗅探下载" : "授权嗅探下载"}
            </button>
          )}
          <button type="button" className="secondary-action" onClick={() => onOpenOriginal(task.shareUrl)} disabled={busy}>
            <ArrowSquareOutIcon size={19} />
            打开微信原文
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
  const { task, article } = detail;
  if (!article) {
    return (
      <section className="pending-detail">
        <SpinnerGapIcon className={task.status === "processing" ? "spin" : ""} size={34} />
        <h2>{task.status === "processing" ? "正在读取公开文章" : "文章尚未读取"}</h2>
        <p>讯栖会读取公开页面的标题、作者、发布时间和正文，不使用微信 Cookie。</p>
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
        <Metadata label="类型" value="公众号文章" />
        <Metadata label="作者" value={article.author || task.sourceName} />
        <Metadata label="原文链接" value={article.canonicalUrl} link />
        <Metadata label="字数统计" value={`约 ${article.wordCount.toLocaleString("zh-CN")} 字`} />
        <Metadata label="资源统计" value={`图片 ${article.imageUrls.length} 张`} />
      </dl>
      {task.completedPath && (
        <div className="completed-output">
          <ArchiveBoxIcon size={20} />
          <span>已保存到 {task.completedPath}</span>
          <button type="button" onClick={() => onRevealOutput(task.completedPath!)}>
            <FolderOpenIcon size={17} />
            在 Finder 中显示
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
  qualityMode: SniffQualityMode;
  onStopSniff: (sessionId: string) => void;
  onRecoverSniff: () => void;
}) {
  const { task, video } = detail;
  if (!video) {
    return (
      <section className="pending-detail">
        <SpinnerGapIcon className={task.status === "processing" ? "spin" : ""} size={34} />
        <h2>{task.status === "processing" ? "正在识别公开视频" : "等待识别视频页面"}</h2>
        <p>只检查页面公开声明的媒体地址，不抓包、不安装证书、不读取登录态。</p>
      </section>
    );
  }

  return (
    <section className="video-detail">
      <div className="video-summary">
        <VideoCameraIcon size={36} weight="duotone" />
        <div>
          <h2>{video.pageTitle}</h2>
          <p>{video.limitation}</p>
        </div>
      </div>
      <div className="candidate-list">
        {video.candidates.length === 0 ? (
          <div className="candidate-empty">
            <WarningCircleIcon size={22} />
            {video.limitation}
          </div>
        ) : (
          video.candidates.map((candidate) => (
            <article className="candidate-card" key={candidate.url}>
              <span className="candidate-icon"><LinkSimpleIcon size={20} /></span>
              <div>
                <strong>{candidate.label}</strong>
                <p>{candidate.url}</p>
                <span className={candidate.downloadable ? "candidate-ready" : "candidate-blocked"}>
                  {candidate.downloadable ? "可下载" : candidateKindLabel(candidate.kind)}
                </span>
              </div>
              {candidate.downloadable && (
                <button type="button" onClick={() => onDownload(task.id, candidate)} disabled={busy}>
                  <DownloadSimpleIcon size={19} />
                  下载
                </button>
              )}
            </article>
          ))
        )}
      </div>
      {video.candidates.every((candidate) => !candidate.downloadable) && !sniffSession && (
        <div className="authorized-sniff-intro">
          <LinkSimpleIcon size={24} weight="duotone" />
          <div>
            <strong>{authorizationReusable ? "连续授权可用" : "可改用授权嗅探助手"}</strong>
            <p>{authorizationReusable
              ? "下一条不再要求指纹；处理完成前请保持 VPN 关闭。"
              : "请先关闭 VPN。首次确认后只在本机临时启用代理和会话证书。"}</p>
          </div>
        </div>
      )}
      {sniffSession && (
        <div className={`sniff-session sniff-session-${sniffSession.phase}`} role="status">
          <SpinnerGapIcon className={isActiveSniff(sniffSession) ? "spin" : ""} size={24} />
          <div>
            <strong>{sniffPhaseTitle(sniffSession.phase)}</strong>
            <p>{sniffSession.message}</p>
            {sniffSession.progress && (
              <div className="sniff-progress" aria-label="下载进度">
                <div className="sniff-progress-heading">
                  <strong>{sniffSession.progress.percent === null ? "正在下载" : `${sniffSession.progress.percent}%`}</strong>
                  <span>{formatBytes(sniffSession.progress.downloadedBytes)}{sniffSession.progress.totalBytes ? ` / ${formatBytes(sniffSession.progress.totalBytes)}` : ""}</span>
                  <span>{formatSpeed(sniffSession.progress.bytesPerSecond)}</span>
                </div>
                <div className="sniff-progress-track">
                  <span style={{ width: `${sniffSession.progress.percent ?? 4}%` }} />
                </div>
              </div>
            )}
            <div className="sniff-media-summary" aria-label="视频基础信息">
              <div>
                <span>画质</span>
                <strong>{sniffSession.output?.qualityLabel ?? qualityModeLabel(qualityMode)}</strong>
              </div>
              <div>
                <span>{sniffSession.output ? "文件大小" : "预计大小"}</span>
                <strong>{formatOptionalBytes(
                  sniffSession.output?.bytesWritten
                    ?? sniffSession.progress?.totalBytes
                    ?? sniffSession.progress?.downloadedBytes
                    ?? null,
                )}</strong>
              </div>
              <div>
                <span>分辨率</span>
                <strong>{sniffSession.output?.width && sniffSession.output?.height
                  ? `${sniffSession.output.width} × ${sniffSession.output.height}`
                  : "完成后读取"}</strong>
              </div>
            </div>
            <p className="sniff-media-note">{qualityMode === "original"
              ? "本次按原始视频保存；实际分辨率和大小会在下载后读取。"
              : "本次使用微信返回的默认规格以节省空间；实际分辨率和大小会在下载后读取。"}</p>
            {sniffSession.destinationDirectory && (
              <p className="sniff-destination">保存到 {sniffSession.destinationDirectory}</p>
            )}
            {isActiveSniff(sniffSession) && (
              <p className="sniff-session-tip">
                不要退出微信，也不要在浏览器扫码。请从微信左侧重新进入一次“视频号”；无需刷新，也不用寻找页面下载按钮。讯栖只会下载当前任务里已经复制的分享链接。
              </p>
            )}
            <div className="sniff-session-actions">
              {sniffSession.destinationDirectory && (
                <button type="button" onClick={() => onRevealOutput(sniffSession.destinationDirectory)}>
                  <FolderOpenIcon size={17} />
                  打开保存位置
                </button>
              )}
              {(isActiveSniff(sniffSession) || sniffSession.authorizationReusable) && (
                <button type="button" onClick={() => onStopSniff(sniffSession.sessionId)}>
                  结束并恢复网络
                </button>
              )}
              {sniffSession.phase === "restoration_required" && (
                <button type="button" onClick={onRecoverSniff}>
                  立即恢复网络设置
                </button>
              )}
            </div>
          </div>
        </div>
      )}
      <dl className="detail-metadata">
        <Metadata label="类型" value="视频号内容" />
        <Metadata label="来源" value={video.sourceName || task.sourceName} />
        {video.publishedAt && <Metadata label="发布时间" value={formatLongDate(video.publishedAt)} />}
        <Metadata label="分享链接" value={video.pageUrl} link />
        <Metadata label="识别结果" value={`${video.candidates.filter((candidate) => candidate.downloadable).length} 个可下载视频`} />
      </dl>
      {task.completedPath && (
        <div className="completed-output">
          <ArchiveBoxIcon size={20} />
          <span>已保存到 {task.completedPath}</span>
          <button type="button" onClick={() => onRevealOutput(task.completedPath!)}>
            <FolderOpenIcon size={17} />
            在 Finder 中显示
          </button>
        </div>
      )}
    </section>
  );
}

function isActiveSniff(session: SniffSessionSnapshot | null) {
  return session !== null && ["starting", "awaiting_playback", "capturing", "saving", "restoring"].includes(session.phase);
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
  return (
    <label className="format-select quality-select">
      <VideoCameraIcon size={18} />
      <select
        aria-label="视频下载画质"
        value={value}
        onChange={(event) => onChange(event.target.value as SniffQualityMode)}
        disabled={disabled}
      >
        <option value="original">原始画质（文件较大）</option>
        <option value="space_saver">节省空间（微信默认）</option>
      </select>
    </label>
  );
}

function qualityModeLabel(mode: SniffQualityMode) {
  return mode === "original" ? "原始画质" : "节省空间（微信默认）";
}

function sniffPhaseTitle(phase: SniffSessionSnapshot["phase"]) {
  return {
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
  }[phase];
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

function formatOptionalBytes(bytes: number | null) {
  return bytes && bytes > 0 ? formatBytes(bytes) : "等待识别";
}

function formatSpeed(bytesPerSecond: number) {
  return bytesPerSecond > 0 ? `${formatBytes(bytesPerSecond)}/s` : "正在校验文件";
}

function Metadata({ label, value, link = false }: { label: string; value: string; link?: boolean }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd className={link ? "metadata-link" : ""}>{value}</dd>
    </div>
  );
}

function StatusBadge({ status, detail }: { status: CaptureStatus; detail: string }) {
  const labels: Record<CaptureStatus, string> = {
    queued: "等待处理",
    processing: "正在读取",
    ready: detail.includes("视频") ? "视频可下载" : "内容读取完成",
    needs_attention: "需要查看",
    exporting: "正在导出",
    downloading: "正在下载",
    completed: "处理完成",
    failed: "处理失败",
  };
  return <span className={`detail-status detail-status-${status}`}>{labels[status]}</span>;
}

function candidateKindLabel(kind: DetectedVideo["kind"]) {
  return {
    direct_file: "当前不可下载",
    hls_playlist: "HLS 暂不下载",
    dash_manifest: "DASH 暂不下载",
    unsupported: "受保护或动态媒体",
  }[kind];
}

function formatLongDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}
