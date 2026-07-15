use std::sync::Arc;

use chrono::Utc;
use url::Url;

use crate::{
    content_inspector::{ContentInspector, PublicContentInspector},
    public_http::is_allowed_public_https_url,
    store::SqliteStore,
    AppError, CaptureKind, CaptureStatus, CaptureTaskDetail, DetectedVideo, DetectedVideoKind,
    VideoPageInspection,
};

#[derive(Clone)]
pub struct CaptureProcessor {
    store: SqliteStore,
    inspector: Arc<dyn ContentInspector>,
}

impl CaptureProcessor {
    pub(crate) fn new(store: SqliteStore) -> Self {
        Self::with_inspector(store, Arc::new(PublicContentInspector))
    }

    pub(crate) fn with_inspector(store: SqliteStore, inspector: Arc<dyn ContentInspector>) -> Self {
        Self { store, inspector }
    }

    pub fn process(&self, task_id: i64) -> Result<CaptureTaskDetail, AppError> {
        let task = self.store.get_detail(task_id)?.task;
        self.store.set_status(
            task_id,
            CaptureStatus::Processing,
            match task.kind {
                CaptureKind::Article => "正在读取公开文章正文…",
                CaptureKind::Video => "正在检查页面中的公开视频…",
            },
            &Utc::now().to_rfc3339(),
        )?;

        let result = match task.kind {
            CaptureKind::Article => self.process_article(task_id, &task.share_url),
            CaptureKind::Video => self.process_video(task_id, &task.share_url),
        };

        if let Err(error) = &result {
            let _ = self.store.set_status(
                task_id,
                CaptureStatus::Failed,
                &error.to_string(),
                &Utc::now().to_rfc3339(),
            );
        }
        result
    }

    fn process_article(&self, task_id: i64, url: &str) -> Result<CaptureTaskDetail, AppError> {
        let snapshot = self.inspector.inspect_article(url)?;
        self.store
            .save_article(task_id, &snapshot, &Utc::now().to_rfc3339())
    }

    fn process_video(&self, task_id: i64, url: &str) -> Result<CaptureTaskDetail, AppError> {
        let inspection = if is_direct_public_video_url(url) {
            VideoPageInspection {
                page_title: "公开视频".into(),
                page_url: url.into(),
                source_name: "视频号".into(),
                description: "用户提供的公开 HTTPS 视频文件".into(),
                published_at: None,
                cover_image_url: None,
                candidates: vec![DetectedVideo {
                    url: url.into(),
                    kind: DetectedVideoKind::DirectFile,
                    label: "公开视频文件".into(),
                    downloadable: true,
                }],
                limitation: "已识别到公开 HTTPS 视频文件，可由你手动下载。".into(),
            }
        } else {
            self.inspector.inspect_video_page(url)?
        };
        let downloadable = inspection
            .candidates
            .iter()
            .filter(|candidate| candidate.downloadable)
            .count();
        let (status, detail) = if downloadable > 0 {
            (
                CaptureStatus::Ready,
                format!("识别到 {downloadable} 个可下载的公开视频"),
            )
        } else {
            (CaptureStatus::NeedsAttention, inspection.limitation.clone())
        };
        self.store.save_video(
            task_id,
            &inspection,
            status,
            &detail,
            &Utc::now().to_rfc3339(),
        )
    }
}

fn is_direct_public_video_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if !is_allowed_public_https_url(&url) {
        return false;
    }
    url.path().rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "mp4" | "mov" | "m4v" | "webm"
        )
    })
}
