use std::{fs, path::Path, sync::Arc};

use tempfile::tempdir;
use xunqi_lib::{
    AppError, Application, ArticleAsset, ArticleAssetFetcher, ArticleExportMode, ArticleSnapshot,
    CaptureKind, CaptureStatus, ContentInspector, DetectedVideo, DetectedVideoKind,
    DownloadedMedia, MediaDownloader, OutputAction, PdfRenderer, VideoPageInspection,
};

struct FixtureInspector;

impl ContentInspector for FixtureInspector {
    fn inspect_article(&self, _url: &str) -> Result<ArticleSnapshot, AppError> {
        Ok(ArticleSnapshot {
            title: "一篇值得保存的文章".into(),
            author: "示例科技周报".into(),
            published_at: Some("2026-07-13 12:30".into()),
            canonical_url: "https://mp.weixin.qq.com/s/canonical-article".into(),
            body_markdown: "第一段正文。\n\n第二段正文。".into(),
            body_html: "<p>第一段正文。</p><img data-xunqi-image=\"0\"><p>第二段正文。</p>".into(),
            cover_image_url: Some("https://mmbiz.qpic.cn/demo-cover.jpg".into()),
            image_urls: vec!["https://mmbiz.qpic.cn/demo-image.jpg".into()],
            word_count: 14,
        })
    }

    fn inspect_video_page(&self, url: &str) -> Result<VideoPageInspection, AppError> {
        Ok(VideoPageInspection {
            page_title: "测试视频：公开直链识别演示".into(),
            page_url: url.into(),
            source_name: "影像测试频道".into(),
            description: "公开视频测试".into(),
            published_at: None,
            cover_image_url: None,
            candidates: vec![DetectedVideo {
                url: "https://finder.video.qq.com/251/video/stodownload?token=public".into(),
                kind: DetectedVideoKind::DirectFile,
                label: "公开视频".into(),
                downloadable: true,
            }],
            limitation: "只下载公开直链".into(),
        })
    }
}

struct FixtureDownloader;

impl MediaDownloader for FixtureDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<DownloadedMedia, AppError> {
        assert_eq!(
            url,
            "https://finder.video.qq.com/251/video/stodownload?token=public"
        );
        fs::write(destination, b"public-video")?;
        Ok(DownloadedMedia {
            bytes_written: 12,
            extension: "webm".into(),
        })
    }
}

struct FixtureArticleAssetFetcher;

impl ArticleAssetFetcher for FixtureArticleAssetFetcher {
    fn fetch(&self, url: &str) -> Result<ArticleAsset, AppError> {
        assert_eq!(url, "https://mmbiz.qpic.cn/demo-image.jpg");
        Ok(ArticleAsset {
            extension: "jpg".into(),
            bytes: b"fixture-jpeg".to_vec(),
        })
    }
}

struct FixturePdfRenderer;

impl PdfRenderer for FixturePdfRenderer {
    fn render(&self, source_html: &Path, destination: &Path) -> Result<u64, AppError> {
        let html = fs::read_to_string(source_html)?;
        assert!(html.contains("<p>第一段正文。</p>"));
        assert!(html.contains("data:image/jpeg;base64,"));
        assert!(html.contains("default-src 'none'; img-src data:; style-src 'unsafe-inline'"));
        assert!(!html.contains("https://mmbiz.qpic.cn/demo-image.jpg"));
        let pdf = b"%PDF-1.4\n% fixture PDF\n%%EOF\n";
        fs::write(destination, pdf)?;
        Ok(pdf.len() as u64)
    }
}

#[test]
fn captured_tasks_can_be_processed_and_written_only_after_user_selected_output() {
    let directory = tempdir().expect("temporary directory");
    let app = Application::open_with_services(
        directory.path().join("xunqi.db"),
        Arc::new(FixtureInspector),
        Arc::new(FixtureDownloader),
        Arc::new(FixtureArticleAssetFetcher),
        Arc::new(FixturePdfRenderer),
    )
    .expect("open application");
    let submitted = app
        .submit_links("https://mp.weixin.qq.com/s/article-one https://weixin.qq.com/sph/video-one")
        .expect("submit links");
    let article_id = submitted
        .tasks
        .iter()
        .find(|task| task.kind == CaptureKind::Article)
        .expect("article task")
        .id;
    let video_id = submitted
        .tasks
        .iter()
        .find(|task| task.kind == CaptureKind::Video)
        .expect("video task")
        .id;

    let article = app.process_task(article_id).expect("process article");
    assert_eq!(article.task.source_name, "示例科技周报");
    assert_eq!(article.task.status, CaptureStatus::Ready);
    assert_eq!(article.article.expect("article snapshot").word_count, 14);

    let video = app.process_task(video_id).expect("process video");
    assert_eq!(video.task.source_name, "影像测试频道");
    assert_eq!(video.task.status, CaptureStatus::Ready);
    assert!(video.video.as_ref().expect("video inspection").candidates[0].downloadable);

    let summaries = app.list_tasks().expect("list compact task summaries");
    assert!(summaries
        .iter()
        .all(|detail| detail.article.is_none() && detail.video.is_none()));
    assert!(app
        .get_task_detail(article_id)
        .expect("load article detail on demand")
        .article
        .is_some());

    let output_dir = directory.path().join("outputs");
    fs::create_dir(&output_dir).expect("create output directory");
    let pdf = app
        .export_article(article_id, &output_dir, ArticleExportMode::Pdf)
        .expect("export original-layout PDF");
    assert_eq!(pdf.action, OutputAction::ArticlePdf);
    assert_eq!(
        Path::new(&pdf.destination)
            .extension()
            .and_then(|value| value.to_str()),
        Some("pdf")
    );
    assert!(fs::read(&pdf.destination)
        .expect("PDF bytes")
        .starts_with(b"%PDF-"));

    let markdown = app
        .export_article(article_id, &output_dir, ArticleExportMode::Markdown)
        .expect("export markdown");
    assert_eq!(markdown.action, OutputAction::ArticleMarkdown);
    let markdown_folder = Path::new(&markdown.destination);
    let markdown_path = markdown_folder.join("文章.md");
    let markdown_image = markdown_folder.join("images/001.jpg");
    let markdown_text = fs::read_to_string(&markdown_path).expect("read markdown");
    assert!(markdown_text.contains("# 一篇值得保存的文章"));
    assert!(markdown_text.contains("![原文图片 1](images/001.jpg)"));
    assert!(!markdown_text.contains("https://mmbiz.qpic.cn/demo-image.jpg"));
    assert_eq!(
        fs::read(&markdown_image).expect("local image"),
        b"fixture-jpeg"
    );

    let downloaded = app
        .download_video(
            video_id,
            "https://finder.video.qq.com/251/video/stodownload?token=public",
            &output_dir,
        )
        .expect("download video");
    assert_eq!(downloaded.action, OutputAction::VideoDownload);
    assert_eq!(
        Path::new(&downloaded.destination)
            .extension()
            .and_then(|value| value.to_str()),
        Some("webm")
    );
    assert_eq!(
        fs::read(&downloaded.destination).expect("video bytes"),
        b"public-video"
    );

    assert_eq!(
        app.clear_tasks(&[article_id, video_id])
            .expect("clear tasks"),
        2
    );
    assert!(app.list_tasks().expect("empty queue").is_empty());
    assert!(markdown_path.is_file());
    assert!(markdown_image.is_file());
    assert!(Path::new(&downloaded.destination).is_file());
}

#[test]
fn an_uninspected_article_cannot_be_exported() {
    let directory = tempdir().expect("temporary directory");
    let app = Application::open_with_services(
        directory.path().join("xunqi.db"),
        Arc::new(FixtureInspector),
        Arc::new(FixtureDownloader),
        Arc::new(FixtureArticleAssetFetcher),
        Arc::new(FixturePdfRenderer),
    )
    .expect("open application");
    let task = app
        .submit_links("https://mp.weixin.qq.com/s/unread")
        .expect("submit article")
        .tasks
        .remove(0);
    assert!(matches!(
        app.export_article(task.id, directory.path(), ArticleExportMode::Markdown),
        Err(AppError::Validation(message)) if message.contains("先读取文章")
    ));
}
