use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("本地存储操作失败：{0}")]
    Storage(#[from] rusqlite::Error),
    #[error("{0}")]
    Validation(String),
    #[error("读取公开内容失败：{0}")]
    Content(String),
    #[error("下载失败：{0}")]
    Download(String),
    #[error("没有找到任务 #{0}")]
    TaskNotFound(i64),
    #[error("本地文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("数据序列化失败：{0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureKind {
    Article,
    Video,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Queued,
    Processing,
    Ready,
    NeedsAttention,
    Exporting,
    Downloading,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTask {
    pub id: i64,
    pub kind: CaptureKind,
    pub source_name: String,
    pub title: String,
    pub author: String,
    pub published_at: Option<String>,
    pub share_url: String,
    pub status: CaptureStatus,
    pub status_detail: String,
    pub created_at: String,
    pub updated_at: String,
    pub completed_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubmitLinksResult {
    pub tasks: Vec<CaptureTask>,
    pub duplicate_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArticleSnapshot {
    pub title: String,
    pub author: String,
    pub published_at: Option<String>,
    pub canonical_url: String,
    pub body_markdown: String,
    #[serde(default)]
    pub body_html: String,
    pub cover_image_url: Option<String>,
    pub image_urls: Vec<String>,
    pub word_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectedVideoKind {
    DirectFile,
    HlsPlaylist,
    DashManifest,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DetectedVideo {
    pub url: String,
    pub kind: DetectedVideoKind,
    pub label: String,
    pub downloadable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VideoPageInspection {
    pub page_title: String,
    pub page_url: String,
    pub source_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub cover_image_url: Option<String>,
    pub candidates: Vec<DetectedVideo>,
    pub limitation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTaskDetail {
    pub task: CaptureTask,
    pub article: Option<ArticleSnapshot>,
    pub video: Option<VideoPageInspection>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArticleExportMode {
    Pdf,
    Markdown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputAction {
    ArticlePdf,
    ArticleMarkdown,
    VideoDownload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutputResult {
    pub task_id: i64,
    pub action: OutputAction,
    pub destination: String,
    pub bytes_written: u64,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticExportResult {
    pub destination: String,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WechatForegroundStatus {
    pub is_wechat_frontmost: bool,
    pub application_name: Option<String>,
    pub bundle_identifier: Option<String>,
    pub limitation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WechatChannelsNetworkRefreshResult {
    pub refreshed: bool,
    pub message: String,
}
