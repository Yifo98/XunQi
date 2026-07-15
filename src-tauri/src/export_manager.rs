use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;

use crate::{
    article_assets::{ArticleAsset, ArticleAssetFetcher, PublicArticleAssetFetcher},
    downloader::{MediaDownloader, PublicVideoDownloader},
    pdf_renderer::{NativePdfRenderer, PdfRenderer},
    store::SqliteStore,
    AppError, ArticleExportMode, ArticleSnapshot, CaptureKind, CaptureStatus, OutputAction,
    OutputResult,
};

#[derive(Clone)]
pub struct ExportManager {
    store: SqliteStore,
    media_downloader: Arc<dyn MediaDownloader>,
    article_asset_fetcher: Arc<dyn ArticleAssetFetcher>,
    pdf_renderer: Arc<dyn PdfRenderer>,
    output_lock: Arc<Mutex<()>>,
}

const MAX_ARTICLE_IMAGES: usize = 100;
const MAX_ARTICLE_IMAGES_BYTES: u64 = 200 * 1024 * 1024;

impl ExportManager {
    pub(crate) fn new(store: SqliteStore) -> Self {
        Self::with_services(
            store,
            Arc::new(PublicVideoDownloader),
            Arc::new(PublicArticleAssetFetcher),
            Arc::new(NativePdfRenderer),
        )
    }

    pub(crate) fn with_services(
        store: SqliteStore,
        media_downloader: Arc<dyn MediaDownloader>,
        article_asset_fetcher: Arc<dyn ArticleAssetFetcher>,
        pdf_renderer: Arc<dyn PdfRenderer>,
    ) -> Self {
        Self {
            store,
            media_downloader,
            article_asset_fetcher,
            pdf_renderer,
            output_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn export_article(
        &self,
        task_id: i64,
        directory: impl AsRef<Path>,
        mode: ArticleExportMode,
    ) -> Result<OutputResult, AppError> {
        let directory = checked_directory(directory.as_ref())?;
        let _output_guard = self
            .output_lock
            .lock()
            .map_err(|_| AppError::Validation("输出队列已损坏，请重启讯栖".into()))?;
        let detail = self.store.get_detail(task_id)?;
        if detail.task.kind != CaptureKind::Article {
            return Err(AppError::Validation("只有公众号文章可以导出文档".into()));
        }
        let snapshot = detail
            .article
            .ok_or_else(|| AppError::Validation("请先读取文章并确认正文，再导出到本地".into()))?;
        self.store.set_status(
            task_id,
            CaptureStatus::Exporting,
            "正在写入本地文档…",
            &Utc::now().to_rfc3339(),
        )?;

        let result = match mode {
            ArticleExportMode::Pdf => self.write_article_pdf(task_id, &snapshot, &directory),
            ArticleExportMode::Markdown => {
                self.write_article_folder(task_id, &snapshot, &directory)
            }
        };
        if let Err(error) = &result {
            let _ = self.store.set_status(
                task_id,
                CaptureStatus::Failed,
                &format!("导出失败：{error}"),
                &Utc::now().to_rfc3339(),
            );
        }
        result
    }

    fn write_article_pdf(
        &self,
        task_id: i64,
        snapshot: &ArticleSnapshot,
        directory: &Path,
    ) -> Result<OutputResult, AppError> {
        let stem = safe_stem(&snapshot.title, "讯栖文章");
        let destination = unique_file(directory, &stem, "pdf");
        let workspace = temporary_path_for(&directory.join(format!("{stem}-pdf")));
        self.store.track_temporary_output(
            task_id,
            "pdf_workspace",
            &workspace,
            &Utc::now().to_rfc3339(),
        )?;
        if let Err(error) = fs::create_dir(&workspace) {
            let _ = self.store.untrack_temporary_output(task_id, &workspace);
            return Err(error.into());
        }
        let result = (|| {
            let assets = self.fetch_article_assets(snapshot)?;
            let (html, warning) = article_pdf_html(snapshot, &assets);
            let html_path = workspace.join("article.html");
            write_atomic(&html_path, html.as_bytes())?;
            let temporary_pdf = workspace.join("article.pdf");
            let bytes_written = self.pdf_renderer.render(&html_path, &temporary_pdf)?;
            commit_temporary_file(&temporary_pdf, &destination)?;
            Ok::<_, AppError>(OutputResult {
                task_id,
                action: OutputAction::ArticlePdf,
                destination: destination.to_string_lossy().into_owned(),
                bytes_written,
                warning,
            })
        })();
        let cleanup_error = fs::remove_dir_all(&workspace).err();
        if cleanup_error.is_none() || !workspace.exists() {
            let _ = self.store.untrack_temporary_output(task_id, &workspace);
        }
        let mut output = result?;
        if cleanup_error.is_some() {
            let warning = "PDF 已保存，但本次临时缓存未能立即清理；讯栖会在下次启动时重试。";
            output.warning = Some(match output.warning {
                Some(existing) => format!("{existing} {warning}"),
                None => warning.into(),
            });
        }
        Ok(self.finish_output(output, "原版长页 PDF 已保存到本地"))
    }

    pub fn download_video(
        &self,
        task_id: i64,
        candidate_url: &str,
        directory: impl AsRef<Path>,
    ) -> Result<OutputResult, AppError> {
        let directory = checked_directory(directory.as_ref())?;
        let _output_guard = self
            .output_lock
            .lock()
            .map_err(|_| AppError::Validation("输出队列已损坏，请重启讯栖".into()))?;
        let detail = self.store.get_detail(task_id)?;
        if detail.task.kind != CaptureKind::Video {
            return Err(AppError::Validation("只有视频号任务可以下载视频".into()));
        }
        let inspection = detail
            .video
            .ok_or_else(|| AppError::Validation("请先识别视频页面中的公开媒体".into()))?;
        let candidate = inspection
            .candidates
            .iter()
            .find(|candidate| candidate.url == candidate_url && candidate.downloadable)
            .ok_or_else(|| AppError::Validation("所选媒体不是本次识别到的可下载公开视频".into()))?;
        let stem = safe_stem(&detail.task.title, "讯栖视频");
        let temporary = temporary_path_for(&directory.join(&stem));
        self.store.track_temporary_output(
            task_id,
            "video_download",
            &temporary,
            &Utc::now().to_rfc3339(),
        )?;
        if let Err(error) = self.store.set_status(
            task_id,
            CaptureStatus::Downloading,
            "正在下载公开视频…",
            &Utc::now().to_rfc3339(),
        ) {
            let _ = self.store.untrack_temporary_output(task_id, &temporary);
            return Err(error);
        }
        let result = self
            .media_downloader
            .download(&candidate.url, &temporary)
            .and_then(|downloaded| {
                let extension = match downloaded.extension.as_str() {
                    "mp4" | "mov" | "m4v" | "webm" => downloaded.extension,
                    _ => return Err(AppError::Download("下载器返回了不支持的视频格式".into())),
                };
                let destination = unique_file(&directory, &stem, &extension);
                commit_temporary_file(&temporary, &destination)?;
                Ok(OutputResult {
                    task_id,
                    action: OutputAction::VideoDownload,
                    destination: destination.to_string_lossy().into_owned(),
                    bytes_written: downloaded.bytes_written,
                    warning: None,
                })
            });
        if !temporary.exists() || fs::remove_file(&temporary).is_ok() {
            let _ = self.store.untrack_temporary_output(task_id, &temporary);
        }
        match result {
            Ok(output) => Ok(self.finish_output(output, "视频已下载到本地")),
            Err(error) => {
                let _ = self.store.set_status(
                    task_id,
                    CaptureStatus::Failed,
                    &format!("下载失败：{error}"),
                    &Utc::now().to_rfc3339(),
                );
                Err(error)
            }
        }
    }

    fn write_article_folder(
        &self,
        task_id: i64,
        snapshot: &ArticleSnapshot,
        directory: &Path,
    ) -> Result<OutputResult, AppError> {
        let base_name = safe_stem(&snapshot.title, "讯栖文章");
        let folder = unique_directory(directory, &base_name);
        let temporary_folder = temporary_path_for(&folder);
        self.store.track_temporary_output(
            task_id,
            "article_folder",
            &temporary_folder,
            &Utc::now().to_rfc3339(),
        )?;
        if let Err(error) = fs::create_dir(&temporary_folder) {
            let _ = self
                .store
                .untrack_temporary_output(task_id, &temporary_folder);
            return Err(error.into());
        }
        let write_result = (|| {
            let (image_paths, image_bytes) =
                self.download_article_images(snapshot, &temporary_folder)?;
            let markdown = article_markdown(snapshot, &image_paths);
            write_atomic(&temporary_folder.join("文章.md"), markdown.as_bytes())?;
            commit_temporary_directory(&temporary_folder, &folder)?;
            Ok::<_, AppError>((markdown.len() as u64) + image_bytes)
        })();
        if write_result.is_err() {
            let _ = fs::remove_dir_all(&temporary_folder);
        }
        if !temporary_folder.exists() {
            let _ = self
                .store
                .untrack_temporary_output(task_id, &temporary_folder);
        }
        let bytes_written = write_result?;
        let output = OutputResult {
            task_id,
            action: OutputAction::ArticleMarkdown,
            destination: folder.to_string_lossy().into_owned(),
            bytes_written,
            warning: None,
        };
        Ok(self.finish_output(output, "文章和本地图片已导出"))
    }

    fn download_article_images(
        &self,
        snapshot: &ArticleSnapshot,
        output_folder: &Path,
    ) -> Result<(Vec<String>, u64), AppError> {
        let assets = self.fetch_article_assets(snapshot)?;
        if assets.is_empty() {
            return Ok((Vec::new(), 0));
        }
        let images_folder = output_folder.join("images");
        fs::create_dir(&images_folder)?;
        let mut relative_paths = Vec::with_capacity(assets.len());
        let mut total_bytes = 0_u64;
        for (index, asset) in assets.into_iter().enumerate() {
            total_bytes += asset.bytes.len() as u64;
            let file_name = format!("{:03}.{}", index + 1, asset.extension);
            write_atomic(&images_folder.join(&file_name), &asset.bytes)?;
            relative_paths.push(format!("images/{file_name}"));
        }
        Ok((relative_paths, total_bytes))
    }

    fn fetch_article_assets(
        &self,
        snapshot: &ArticleSnapshot,
    ) -> Result<Vec<ArticleAsset>, AppError> {
        if snapshot.image_urls.len() > MAX_ARTICLE_IMAGES {
            return Err(AppError::Validation(format!(
                "文章包含 {} 张图片，超过单篇 {} 张安全上限",
                snapshot.image_urls.len(),
                MAX_ARTICLE_IMAGES
            )));
        }
        let mut assets = Vec::with_capacity(snapshot.image_urls.len());
        let mut total_bytes = 0_u64;
        for (index, url) in snapshot.image_urls.iter().enumerate() {
            let asset = self.article_asset_fetcher.fetch(url).map_err(|error| {
                AppError::Content(format!("第 {} 张图片下载失败：{error}", index + 1))
            })?;
            let extension = match asset.extension.as_str() {
                "jpg" | "png" | "gif" | "webp" => asset.extension,
                _ => {
                    return Err(AppError::Content(format!(
                        "第 {} 张图片返回了不支持的格式",
                        index + 1
                    )))
                }
            };
            total_bytes = total_bytes.saturating_add(asset.bytes.len() as u64);
            if total_bytes > MAX_ARTICLE_IMAGES_BYTES {
                return Err(AppError::Content(
                    "文章图片合计超过 200 MiB 安全上限".into(),
                ));
            }
            assets.push(ArticleAsset {
                extension,
                bytes: asset.bytes,
            });
        }
        Ok(assets)
    }

    fn finish_output(&self, mut output: OutputResult, detail: &str) -> OutputResult {
        let mut last_error = None;
        for attempt in 0..3 {
            match self.store.mark_completed(
                output.task_id,
                &output.destination,
                detail,
                &Utc::now().to_rfc3339(),
            ) {
                Ok(_) => return output,
                Err(error) => last_error = Some(error),
            }
            if attempt < 2 {
                thread::sleep(Duration::from_millis(75));
            }
        }
        output.warning = Some(format!(
            "文件已经保存，但任务状态未能同步：{}。保存位置：{}",
            last_error
                .map(|error| error.to_string())
                .unwrap_or_else(|| "未知原因".into()),
            output.destination
        ));
        output
    }
}

fn article_pdf_html(
    snapshot: &ArticleSnapshot,
    assets: &[ArticleAsset],
) -> (String, Option<String>) {
    let (mut body_html, warning) = if snapshot.body_html.trim().is_empty() {
        (
            legacy_markdown_html(&snapshot.body_markdown, assets.len()),
            Some(
                "这条任务来自旧版缓存，PDF 已生成；重新识别后可恢复图片在原文中的精确位置。".into(),
            ),
        )
    } else {
        (snapshot.body_html.clone(), None)
    };
    for (index, asset) in assets.iter().enumerate() {
        let mime = match asset.extension.as_str() {
            "jpg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "application/octet-stream",
        };
        let source = format!("src=\"data:{mime};base64,{}\"", BASE64.encode(&asset.bytes));
        body_html = body_html.replace(&format!("data-xunqi-image=\"{index}\""), &source);
    }
    let title = escape_html(&snapshot.title);
    let author = escape_html(&snapshot.author);
    let published = snapshot
        .published_at
        .as_deref()
        .map(escape_html)
        .unwrap_or_default();
    let meta = [author, published]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let html = format!(
        r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'"><meta name="viewport" content="width=device-width, initial-scale=1">
<style>
html,body{{margin:0;padding:0;background:#fff;color:#222}}body{{font-family:-apple-system,BlinkMacSystemFont,"PingFang SC","Hiragino Sans GB","Microsoft YaHei",sans-serif;font-size:18px;line-height:1.8;overflow-wrap:anywhere}}main{{width:678px;margin:0 auto;padding:72px 58px 88px}}h1.article-title{{font-size:34px;line-height:1.35;margin:0 0 16px;font-weight:700;letter-spacing:.01em}}.article-meta{{font-size:15px;color:#777;margin:0 0 46px;padding-bottom:20px;border-bottom:1px solid #eee}}.article-content h1,.article-content h2,.article-content h3,.article-content h4,.article-content h5,.article-content h6{{line-height:1.45;margin:1.5em 0 .65em;color:#181818}}.article-content h1{{font-size:29px}}.article-content h2{{font-size:26px}}.article-content h3{{font-size:23px}}.article-content p{{margin:1em 0}}.article-content blockquote{{margin:1.25em 0;padding:12px 18px;border-left:4px solid #8b7cf6;background:#f7f5ff;color:#555}}.article-content ul,.article-content ol{{padding-left:1.6em;margin:1em 0}}.article-content li{{margin:.45em 0}}.article-content img{{display:block;max-width:100%;height:auto;margin:26px auto}}.article-content a{{color:#5d4fc7;text-decoration:none}}.article-content pre{{white-space:pre-wrap;background:#f6f6f6;padding:16px;border-radius:8px}}.article-source{{margin-top:54px;padding-top:18px;border-top:1px solid #eee;color:#999;font-size:13px}}
</style></head><body><main><h1 class="article-title">{title}</h1><div class="article-meta">{meta}</div><article class="article-content">{body_html}</article><div class="article-source">由讯栖从公开公众号页面离线导出 · 原文链接：{source}</div></main></body></html>"#,
        source = escape_html(&snapshot.canonical_url)
    );
    (html, warning)
}

fn legacy_markdown_html(markdown: &str, image_count: usize) -> String {
    let mut output = markdown
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .map(|block| format!("<p>{}</p>", escape_html(block.trim())))
        .collect::<String>();
    for index in 0..image_count {
        output.push_str(&format!(
            "<img data-xunqi-image=\"{index}\" alt=\"原文图片 {}\">",
            index + 1
        ));
    }
    output
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn checked_directory(directory: &Path) -> Result<PathBuf, AppError> {
    if !directory.is_dir() {
        return Err(AppError::Validation(
            "请选择一个已经存在且可写入的本地文件夹".into(),
        ));
    }
    Ok(directory.to_path_buf())
}

fn article_markdown(snapshot: &ArticleSnapshot, image_paths: &[String]) -> String {
    let mut output = format!("# {}\n\n", snapshot.title);
    if !snapshot.author.trim().is_empty() {
        output.push_str(&format!("- 作者：{}\n", snapshot.author));
    }
    if let Some(published_at) = &snapshot.published_at {
        output.push_str(&format!("- 发布时间：{published_at}\n"));
    }
    output.push_str(&format!(
        "- 原始链接：{}\n\n---\n\n{}\n",
        snapshot.canonical_url, snapshot.body_markdown
    ));
    if !image_paths.is_empty() {
        output.push_str("\n## 原文图片\n");
        for (index, image_path) in image_paths.iter().enumerate() {
            output.push_str(&format!("\n![原文图片 {}]({image_path})\n", index + 1));
        }
    }
    output
}

fn safe_stem(value: &str, fallback: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            ) {
                '-'
            } else {
                character
            }
        })
        .collect::<String>();
    let cleaned = cleaned.trim().trim_matches('.');
    if cleaned.is_empty() {
        fallback.into()
    } else {
        cleaned.chars().take(80).collect()
    }
}

fn unique_file(directory: &Path, stem: &str, extension: &str) -> PathBuf {
    let direct = directory.join(format!("{stem}.{extension}"));
    if !direct.exists() {
        return direct;
    }
    (2..=999)
        .map(|index| directory.join(format!("{stem} ({index}).{extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| {
            directory.join(format!("{stem}-{}.{}", Utc::now().timestamp(), extension))
        })
}

fn unique_directory(directory: &Path, stem: &str) -> PathBuf {
    let direct = directory.join(stem);
    if !direct.exists() {
        return direct;
    }
    (2..=999)
        .map(|index| directory.join(format!("{stem} ({index})")))
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| directory.join(format!("{stem}-{}", Utc::now().timestamp())))
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let part_path = temporary_path_for(destination);
    let write_result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&part_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        commit_temporary_file(&part_path, destination)?;
        Ok::<_, AppError>(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&part_path);
    }
    write_result
}

fn temporary_path_for(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("讯栖输出");
    let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    destination.with_file_name(format!(".{file_name}.{timestamp}.xunqi-part"))
}

fn commit_temporary_file(temporary: &Path, destination: &Path) -> Result<(), AppError> {
    commit_temporary_path(temporary, destination)
}

fn commit_temporary_directory(temporary: &Path, destination: &Path) -> Result<(), AppError> {
    commit_temporary_path(temporary, destination)
}

fn commit_temporary_path(temporary: &Path, destination: &Path) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};

        let source = CString::new(temporary.as_os_str().as_bytes())
            .map_err(|_| AppError::Validation("临时输出路径包含无效字符".into()))?;
        let target = CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| AppError::Validation("目标输出路径包含无效字符".into()))?;
        // macOS 的 RENAME_EXCL 在同卷内原子提交且不覆盖既有文件，
        // 不依赖目标文件系统支持硬链接。
        let result =
            unsafe { libc::renamex_np(source.as_ptr(), target.as_ptr(), libc::RENAME_EXCL) };
        if result == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOTSUP) {
            return Err(AppError::Validation(
                "所选磁盘或网络目录不支持安全无覆盖写入；请改选 Mac 本地文件夹".into(),
            ));
        }
        Err(error.into())
    }

    #[cfg(not(target_os = "macos"))]
    {
        if temporary.is_dir() {
            if destination.exists() {
                return Err(AppError::Validation("目标输出目录已经存在".into()));
            }
            fs::rename(temporary, destination)?;
            Ok(())
        } else {
            fs::hard_link(temporary, destination)?;
            let _ = fs::remove_file(temporary);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{commit_temporary_directory, commit_temporary_file};

    #[test]
    fn temporary_output_is_committed_without_overwriting_an_existing_file() {
        let directory = tempdir().expect("temporary directory");
        let destination = directory.path().join("文章.md");
        let first_temporary = directory.path().join(".first.xunqi-part");
        fs::write(&first_temporary, b"first").expect("write first temporary file");

        commit_temporary_file(&first_temporary, &destination).expect("commit first file");
        assert_eq!(fs::read(&destination).expect("read destination"), b"first");
        assert!(!first_temporary.exists());

        let second_temporary = directory.path().join(".second.xunqi-part");
        fs::write(&second_temporary, b"second").expect("write second temporary file");
        assert!(commit_temporary_file(&second_temporary, &destination).is_err());
        assert_eq!(
            fs::read(&destination).expect("read original destination"),
            b"first"
        );
        assert!(second_temporary.exists());
    }

    #[test]
    fn temporary_folder_is_committed_without_replacing_an_existing_folder() {
        let directory = tempdir().expect("temporary directory");
        let destination = directory.path().join("文章归档");
        fs::create_dir(&destination).expect("create competing empty folder");
        let temporary = directory.path().join(".folder.xunqi-part");
        fs::create_dir(&temporary).expect("create temporary folder");
        fs::write(temporary.join("文章.md"), b"article").expect("write temporary article");

        assert!(commit_temporary_directory(&temporary, &destination).is_err());
        assert!(destination.is_dir());
        assert_eq!(
            fs::read_dir(&destination)
                .expect("read competing folder")
                .count(),
            0
        );
        assert_eq!(
            fs::read(temporary.join("文章.md")).expect("temporary article remains"),
            b"article"
        );
    }
}
