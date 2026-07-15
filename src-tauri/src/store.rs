use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::Duration,
};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    AppError, ArticleSnapshot, CaptureKind, CaptureStatus, CaptureTask, CaptureTaskDetail,
    VideoPageInspection,
};

const SCHEMA_VERSION: i64 = 7;

#[derive(Clone)]
pub struct SqliteStore {
    path: Arc<PathBuf>,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let store = Self {
            path: Arc::new(path),
        };
        store.migrate()?;
        store.recover_interrupted_tasks()?;
        Ok(store)
    }

    fn connect(&self) -> Result<Connection, AppError> {
        let connection = Connection::open(self.path.as_ref())?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "secure_delete", true)?;
        Ok(connection)
    }

    fn migrate(&self) -> Result<(), AppError> {
        let mut connection = self.connect()?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(AppError::Validation(format!(
                "本地数据来自更新版本（{version}），当前讯栖无法安全打开"
            )));
        }
        if version == 6 {
            let legacy_outputs = {
                let mut statement = connection
                    .prepare("SELECT output_kind, temporary_path FROM temporary_outputs")?;
                let rows = statement
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            for (output_kind, temporary_path) in legacy_outputs {
                cleanup_tracked_output(&output_kind, Path::new(&temporary_path))?;
            }
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "DROP TABLE temporary_outputs;
                 CREATE TABLE temporary_outputs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    task_id INTEGER NOT NULL REFERENCES active_tasks(id) ON DELETE RESTRICT,
                    output_kind TEXT NOT NULL,
                    temporary_path TEXT NOT NULL UNIQUE,
                    created_at TEXT NOT NULL
                 );
                 PRAGMA user_version = 7;",
            )?;
            transaction.commit()?;
        }
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS active_tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                source_name TEXT NOT NULL,
                title TEXT NOT NULL,
                author TEXT NOT NULL DEFAULT '',
                published_at TEXT,
                share_url TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL,
                status_detail TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_path TEXT
            );

            CREATE TABLE IF NOT EXISTS article_documents (
                task_id INTEGER PRIMARY KEY REFERENCES active_tasks(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                author TEXT NOT NULL,
                published_at TEXT,
                canonical_url TEXT NOT NULL,
                body_markdown TEXT NOT NULL,
                body_html TEXT NOT NULL DEFAULT '',
                cover_image_url TEXT,
                image_urls_json TEXT NOT NULL,
                word_count INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS video_inspections (
                task_id INTEGER PRIMARY KEY REFERENCES active_tasks(id) ON DELETE CASCADE,
                inspection_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS temporary_outputs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL REFERENCES active_tasks(id) ON DELETE RESTRICT,
                output_kind TEXT NOT NULL,
                temporary_path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
            );

            ",
        )?;
        let has_body_html: i64 = connection.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('article_documents') WHERE name = 'body_html'",
            [],
            |row| row.get(0),
        )?;
        if has_body_html == 0 {
            connection.execute(
                "ALTER TABLE article_documents ADD COLUMN body_html TEXT NOT NULL DEFAULT ''",
                [],
            )?;
        }
        connection.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(())
    }

    fn recover_interrupted_tasks(&self) -> Result<(), AppError> {
        let connection = self.connect()?;
        let temporary_outputs = {
            let mut statement = connection
                .prepare("SELECT id, output_kind, temporary_path FROM temporary_outputs")?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for (output_id, output_kind, temporary_path) in temporary_outputs {
            let path = PathBuf::from(temporary_path);
            if cleanup_tracked_output(&output_kind, &path).is_ok() {
                connection.execute("DELETE FROM temporary_outputs WHERE id = ?1", [output_id])?;
            }
        }
        connection.execute(
            "UPDATE active_tasks
             SET status = 'queued', status_detail = '上次处理被中断，等待重试'
             WHERE status = 'processing'",
            [],
        )?;
        connection.execute(
            "UPDATE active_tasks
             SET status = 'ready', status_detail = '上次导出被中断，可重新导出'
             WHERE status = 'exporting'",
            [],
        )?;
        connection.execute(
            "UPDATE active_tasks
             SET status = 'ready', status_detail = '上次视频下载被中断，可重新下载'
             WHERE status = 'downloading'",
            [],
        )?;
        connection.execute(
            "UPDATE active_tasks
             SET status = 'needs_attention', status_detail = '旧版任务已停止，请使用授权嗅探下载'
             WHERE status = 'recording'",
            [],
        )?;
        connection.execute(
            "UPDATE active_tasks
             SET status = 'needs_attention',
                 status_detail = '旧版本地输出记录已移除，请使用授权嗅探下载',
                 completed_path = NULL
             WHERE kind = 'video'
               AND status_detail = '视频号窗口录制已保存到本地'",
            [],
        )?;
        Ok(())
    }

    pub fn track_temporary_output(
        &self,
        task_id: i64,
        output_kind: &str,
        path: &Path,
        now: &str,
    ) -> Result<(), AppError> {
        if !matches!(
            output_kind,
            "pdf_workspace" | "article_folder" | "video_download"
        ) {
            return Err(AppError::Validation("未知的临时输出类型".into()));
        }
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO temporary_outputs (
                task_id, output_kind, temporary_path, created_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![task_id, output_kind, path.to_string_lossy(), now],
        )?;
        Ok(())
    }

    pub fn untrack_temporary_output(&self, task_id: i64, path: &Path) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "DELETE FROM temporary_outputs WHERE task_id = ?1 AND temporary_path = ?2",
            params![task_id, path.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn insert_task(
        &self,
        kind: CaptureKind,
        source_name: &str,
        title: &str,
        share_url: &str,
        now: &str,
    ) -> Result<(CaptureTask, bool), AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "INSERT OR IGNORE INTO active_tasks (
                kind, source_name, title, share_url, status, status_detail, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'queued', '等待读取公开内容', ?5, ?5)",
            params![kind_name(kind), source_name, title, share_url, now],
        )?;
        let task = Self::get_task_with_connection(&connection, share_url)?
            .ok_or_else(|| AppError::Validation("任务写入后未能重新读取".into()))?;
        Ok((task, changed == 0))
    }

    pub fn list_summaries(&self) -> Result<Vec<CaptureTaskDetail>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, kind, source_name, title, author, published_at, share_url,
                    status, status_detail, created_at, updated_at, completed_path
             FROM active_tasks
             ORDER BY created_at DESC, id DESC",
        )?;
        let tasks = statement
            .query_map([], map_task)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(tasks
            .into_iter()
            .map(|task| CaptureTaskDetail {
                task,
                article: None,
                video: None,
            })
            .collect())
    }

    pub fn get_detail(&self, task_id: i64) -> Result<CaptureTaskDetail, AppError> {
        let connection = self.connect()?;
        let task = connection
            .query_row(
                "SELECT id, kind, source_name, title, author, published_at, share_url,
                        status, status_detail, created_at, updated_at, completed_path
                 FROM active_tasks WHERE id = ?1",
                [task_id],
                map_task,
            )
            .optional()?
            .ok_or(AppError::TaskNotFound(task_id))?;
        Self::detail_with_connection(&connection, task)
    }

    fn get_task_with_connection(
        connection: &Connection,
        share_url: &str,
    ) -> Result<Option<CaptureTask>, AppError> {
        Ok(connection
            .query_row(
                "SELECT id, kind, source_name, title, author, published_at, share_url,
                        status, status_detail, created_at, updated_at, completed_path
                 FROM active_tasks WHERE share_url = ?1",
                [share_url],
                map_task,
            )
            .optional()?)
    }

    fn detail_with_connection(
        connection: &Connection,
        task: CaptureTask,
    ) -> Result<CaptureTaskDetail, AppError> {
        let article = connection
            .query_row(
                "SELECT title, author, published_at, canonical_url, body_markdown, body_html,
                        cover_image_url, image_urls_json, word_count
                 FROM article_documents WHERE task_id = ?1",
                [task.id],
                |row| {
                    let image_urls_json: String = row.get(7)?;
                    let image_urls = serde_json::from_str(&image_urls_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok(ArticleSnapshot {
                        title: row.get(0)?,
                        author: row.get(1)?,
                        published_at: row.get(2)?,
                        canonical_url: row.get(3)?,
                        body_markdown: row.get(4)?,
                        body_html: row.get(5)?,
                        cover_image_url: row.get(6)?,
                        image_urls,
                        word_count: row.get::<_, i64>(8)? as usize,
                    })
                },
            )
            .optional()?;
        let video_json = connection
            .query_row(
                "SELECT inspection_json FROM video_inspections WHERE task_id = ?1",
                [task.id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let video = video_json
            .map(|value| serde_json::from_str(&value))
            .transpose()?;
        Ok(CaptureTaskDetail {
            task,
            article,
            video,
        })
    }

    pub fn set_status(
        &self,
        task_id: i64,
        status: CaptureStatus,
        detail: &str,
        now: &str,
    ) -> Result<CaptureTask, AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE active_tasks
             SET status = ?2, status_detail = ?3, updated_at = ?4
             WHERE id = ?1",
            params![task_id, status_name(status), detail, now],
        )?;
        if changed == 0 {
            return Err(AppError::TaskNotFound(task_id));
        }
        Ok(self.get_detail(task_id)?.task)
    }

    pub fn save_article(
        &self,
        task_id: i64,
        snapshot: &ArticleSnapshot,
        now: &str,
    ) -> Result<CaptureTaskDetail, AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE active_tasks
             SET source_name = ?2, title = ?3, author = ?4, published_at = ?5,
                 status = 'ready', status_detail = '内容读取完成，可导出', updated_at = ?6
             WHERE id = ?1",
            params![
                task_id,
                non_empty(&snapshot.author, "微信公众号"),
                snapshot.title,
                snapshot.author,
                snapshot.published_at,
                now,
            ],
        )?;
        transaction.execute(
            "INSERT INTO article_documents (
                task_id, title, author, published_at, canonical_url, body_markdown, body_html,
                cover_image_url, image_urls_json, word_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(task_id) DO UPDATE SET
                title = excluded.title,
                author = excluded.author,
                published_at = excluded.published_at,
                canonical_url = excluded.canonical_url,
                body_markdown = excluded.body_markdown,
                body_html = excluded.body_html,
                cover_image_url = excluded.cover_image_url,
                image_urls_json = excluded.image_urls_json,
                word_count = excluded.word_count",
            params![
                task_id,
                snapshot.title,
                snapshot.author,
                snapshot.published_at,
                snapshot.canonical_url,
                snapshot.body_markdown,
                snapshot.body_html,
                snapshot.cover_image_url,
                serde_json::to_string(&snapshot.image_urls)?,
                snapshot.word_count as i64,
            ],
        )?;
        transaction.execute(
            "DELETE FROM video_inspections WHERE task_id = ?1",
            [task_id],
        )?;
        transaction.commit()?;
        self.get_detail(task_id)
    }

    pub fn save_video(
        &self,
        task_id: i64,
        inspection: &VideoPageInspection,
        status: CaptureStatus,
        status_detail: &str,
        now: &str,
    ) -> Result<CaptureTaskDetail, AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE active_tasks
             SET source_name = ?2, title = ?3, author = ?2, published_at = ?4,
                 status = ?5, status_detail = ?6, updated_at = ?7
             WHERE id = ?1",
            params![
                task_id,
                non_empty(&inspection.source_name, "视频号"),
                inspection.page_title,
                inspection.published_at,
                status_name(status),
                status_detail,
                now,
            ],
        )?;
        transaction.execute(
            "INSERT INTO video_inspections (task_id, inspection_json)
             VALUES (?1, ?2)
             ON CONFLICT(task_id) DO UPDATE SET inspection_json = excluded.inspection_json",
            params![task_id, serde_json::to_string(inspection)?],
        )?;
        transaction.execute(
            "DELETE FROM article_documents WHERE task_id = ?1",
            [task_id],
        )?;
        transaction.commit()?;
        self.get_detail(task_id)
    }

    pub fn mark_completed(
        &self,
        task_id: i64,
        destination: &str,
        detail: &str,
        now: &str,
    ) -> Result<CaptureTask, AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE active_tasks
             SET status = 'completed', status_detail = ?2, completed_path = ?3, updated_at = ?4
             WHERE id = ?1",
            params![task_id, detail, destination, now],
        )?;
        if changed == 0 {
            return Err(AppError::TaskNotFound(task_id));
        }
        Ok(self.get_detail(task_id)?.task)
    }

    pub fn clear_tasks(&self, task_ids: &[i64]) -> Result<usize, AppError> {
        if task_ids.is_empty() {
            return Ok(0);
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        let mut deleted = 0;
        for task_id in task_ids {
            let temporary_outputs = {
                let mut statement = transaction.prepare(
                    "SELECT id, output_kind, temporary_path
                     FROM temporary_outputs WHERE task_id = ?1",
                )?;
                let rows = statement
                    .query_map([task_id], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                rows
            };
            for (output_id, output_kind, temporary_path) in temporary_outputs {
                cleanup_tracked_output(&output_kind, Path::new(&temporary_path))?;
                transaction.execute("DELETE FROM temporary_outputs WHERE id = ?1", [output_id])?;
            }
            deleted += transaction.execute("DELETE FROM active_tasks WHERE id = ?1", [task_id])?;
        }
        transaction.commit()?;
        self.truncate_wal(&connection)?;
        Ok(deleted)
    }

    fn truncate_wal(&self, connection: &Connection) -> Result<(), AppError> {
        for attempt in 0..10 {
            let (busy, _log_frames, _checkpointed_frames): (i64, i64, i64) =
                connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
            let wal_is_empty = fs::metadata(self.path.with_extension("db-wal"))
                .map(|metadata| metadata.len() == 0)
                .unwrap_or(true);
            if busy == 0 && wal_is_empty {
                return Ok(());
            }
            if attempt < 9 {
                thread::sleep(Duration::from_millis(50));
            }
        }
        Err(AppError::Validation(
            "任务已经移除，但本地缓存文件正被占用，暂时无法确认旧内容已彻底清理；请退出并重新打开讯栖后再试"
                .into(),
        ))
    }
}

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<CaptureTask> {
    let kind: String = row.get(1)?;
    let status: String = row.get(7)?;
    Ok(CaptureTask {
        id: row.get(0)?,
        kind: parse_kind(&kind)?,
        source_name: row.get(2)?,
        title: row.get(3)?,
        author: row.get(4)?,
        published_at: row.get(5)?,
        share_url: row.get(6)?,
        status: parse_status(&status)?,
        status_detail: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        completed_path: row.get(11)?,
    })
}

fn kind_name(kind: CaptureKind) -> &'static str {
    match kind {
        CaptureKind::Article => "article",
        CaptureKind::Video => "video",
    }
}

fn parse_kind(value: &str) -> rusqlite::Result<CaptureKind> {
    match value {
        "article" => Ok(CaptureKind::Article),
        "video" => Ok(CaptureKind::Video),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown kind {other}"
        ))),
    }
}

fn status_name(status: CaptureStatus) -> &'static str {
    match status {
        CaptureStatus::Queued => "queued",
        CaptureStatus::Processing => "processing",
        CaptureStatus::Ready => "ready",
        CaptureStatus::NeedsAttention => "needs_attention",
        CaptureStatus::Exporting => "exporting",
        CaptureStatus::Downloading => "downloading",
        CaptureStatus::Completed => "completed",
        CaptureStatus::Failed => "failed",
    }
}

fn parse_status(value: &str) -> rusqlite::Result<CaptureStatus> {
    match value {
        "queued" => Ok(CaptureStatus::Queued),
        "processing" => Ok(CaptureStatus::Processing),
        "ready" => Ok(CaptureStatus::Ready),
        "needs_attention" => Ok(CaptureStatus::NeedsAttention),
        "exporting" => Ok(CaptureStatus::Exporting),
        "downloading" => Ok(CaptureStatus::Downloading),
        "completed" => Ok(CaptureStatus::Completed),
        "failed" => Ok(CaptureStatus::Failed),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown status {other}"
        ))),
    }
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn cleanup_tracked_output(output_kind: &str, path: &Path) -> Result<(), std::io::Error> {
    if !path.exists() {
        return Ok(());
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let metadata = fs::symlink_metadata(path)?;
    match output_kind {
        "pdf_workspace" | "article_folder"
            if file_name.starts_with('.')
                && file_name.ends_with(".xunqi-part")
                && metadata.is_dir()
                && !metadata.file_type().is_symlink() =>
        {
            fs::remove_dir_all(path)
        }
        "video_download"
            if file_name.starts_with('.')
                && file_name.ends_with(".xunqi-part")
                && metadata.is_file()
                && !metadata.file_type().is_symlink() =>
        {
            fs::remove_file(path)
        }
        "video_recording"
            if file_name.starts_with('.')
                && file_name.ends_with(".xunqi-part.mp4")
                && metadata.is_file()
                && !metadata.file_type().is_symlink() =>
        {
            fs::remove_file(path)
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use rusqlite::{params, Connection};
    use tempfile::tempdir;

    use super::SqliteStore;
    use crate::{AppError, ArticleSnapshot, CaptureKind, CaptureStatus};

    #[test]
    fn clearing_a_task_erases_its_cached_content_from_sqlite_files() -> Result<(), AppError> {
        let directory = tempdir().expect("temporary directory");
        let database = directory.path().join("xunqi.db");
        let store = SqliteStore::open(&database)?;
        let (task, _) = store.insert_task(
            CaptureKind::Article,
            "微信公众号",
            "等待读取公众号文章",
            "https://mp.weixin.qq.com/s/secure-delete",
            "2026-07-13T21:00:00+08:00",
        )?;
        let sentinel = "XUNQI_PRIVATE_CACHE_SENTINEL_8F42A1";
        store.save_article(
            task.id,
            &ArticleSnapshot {
                title: sentinel.into(),
                author: "测试作者".into(),
                published_at: None,
                canonical_url: "https://mp.weixin.qq.com/s/secure-delete".into(),
                body_markdown: format!("正文 {sentinel}"),
                body_html: format!("<p>正文 {sentinel}</p>"),
                cover_image_url: None,
                image_urls: vec![],
                word_count: sentinel.len(),
            },
            "2026-07-13T21:01:00+08:00",
        )?;

        assert!(database_bytes(&database)
            .windows(sentinel.len())
            .any(|bytes| bytes == sentinel.as_bytes()));
        assert_eq!(store.clear_tasks(&[task.id])?, 1);
        assert!(matches!(
            store.get_detail(task.id),
            Err(AppError::TaskNotFound(_))
        ));
        assert!(!database_bytes(&database)
            .windows(sentinel.len())
            .any(|bytes| bytes == sentinel.as_bytes()));
        Ok(())
    }

    #[test]
    fn reopening_cleans_current_and_legacy_temporary_outputs() -> Result<(), AppError> {
        let directory = tempdir().expect("temporary directory");
        let database = directory.path().join("xunqi.db");
        let pdf_workspace = directory.path().join(".article.123.xunqi-part");
        let article_folder = directory.path().join(".article-folder.234.xunqi-part");
        let download = directory.path().join(".video.345.xunqi-part");
        let legacy_capture = directory.path().join(".video.456.xunqi-part.mp4");
        fs::create_dir(&pdf_workspace)?;
        fs::create_dir(&article_folder)?;
        fs::write(pdf_workspace.join("article.html"), b"private article")?;
        fs::write(article_folder.join("文章.md"), b"private markdown")?;
        fs::write(&download, b"partial download")?;
        fs::write(&legacy_capture, b"legacy partial capture")?;
        {
            let store = SqliteStore::open(&database)?;
            let (article, _) = store.insert_task(
                CaptureKind::Article,
                "微信公众号",
                "文章",
                "https://mp.weixin.qq.com/s/recover-pdf",
                "2026-07-13T21:00:00+08:00",
            )?;
            let (video, _) = store.insert_task(
                CaptureKind::Video,
                "视频号",
                "视频",
                "https://weixin.qq.com/sph/recover-video",
                "2026-07-13T21:00:00+08:00",
            )?;
            store.track_temporary_output(
                article.id,
                "pdf_workspace",
                &pdf_workspace,
                "2026-07-13T21:01:00+08:00",
            )?;
            store.track_temporary_output(
                article.id,
                "article_folder",
                &article_folder,
                "2026-07-13T21:02:00+08:00",
            )?;
            store.track_temporary_output(
                video.id,
                "video_download",
                &download,
                "2026-07-13T21:02:00+08:00",
            )?;
            Connection::open(&database)?.execute(
                "INSERT INTO temporary_outputs (task_id, output_kind, temporary_path, created_at)
                 VALUES (?1, 'video_recording', ?2, ?3)",
                params![
                    video.id,
                    legacy_capture.to_string_lossy(),
                    "2026-07-13T21:01:00+08:00"
                ],
            )?;
        }

        let _reopened = SqliteStore::open(&database)?;
        assert!(!pdf_workspace.exists());
        assert!(!article_folder.exists());
        assert!(!download.exists());
        assert!(!legacy_capture.exists());
        Ok(())
    }

    #[test]
    fn reopening_removes_legacy_capture_metadata_without_deleting_user_output(
    ) -> Result<(), AppError> {
        let directory = tempdir().expect("temporary directory");
        let database = directory.path().join("xunqi.db");
        let user_output = directory.path().join("旧版本地输出.mp4");
        fs::write(&user_output, b"user-owned-output")?;
        let task_id = {
            let store = SqliteStore::open(&database)?;
            let (task, _) = store.insert_task(
                CaptureKind::Video,
                "视频号",
                "旧版任务",
                "https://weixin.qq.com/sph/legacy-output",
                "2026-07-13T21:00:00+08:00",
            )?;
            Connection::open(&database)?.execute(
                "UPDATE active_tasks
                 SET status = 'completed',
                     status_detail = '视频号窗口录制已保存到本地',
                     completed_path = ?1
                 WHERE id = ?2",
                params![user_output.to_string_lossy(), task.id],
            )?;
            task.id
        };

        let reopened = SqliteStore::open(&database)?;
        let detail = reopened.get_detail(task_id)?;
        assert_eq!(detail.task.status, CaptureStatus::NeedsAttention);
        assert_eq!(
            detail.task.status_detail,
            "旧版本地输出记录已移除，请使用授权嗅探下载"
        );
        assert_eq!(detail.task.completed_path, None);
        assert_eq!(fs::read(user_output)?, b"user-owned-output");
        Ok(())
    }

    #[test]
    fn clearing_a_task_cleans_all_of_its_tracked_temporary_outputs() -> Result<(), AppError> {
        let directory = tempdir().expect("temporary directory");
        let store = SqliteStore::open(directory.path().join("xunqi.db"))?;
        let (task, _) = store.insert_task(
            CaptureKind::Article,
            "微信公众号",
            "文章",
            "https://mp.weixin.qq.com/s/clear-temporary",
            "2026-07-13T21:00:00+08:00",
        )?;
        let first = directory.path().join(".first.123.xunqi-part");
        let second = directory.path().join(".second.456.xunqi-part");
        fs::create_dir(&first)?;
        fs::create_dir(&second)?;
        store.track_temporary_output(
            task.id,
            "pdf_workspace",
            &first,
            "2026-07-13T21:01:00+08:00",
        )?;
        store.track_temporary_output(
            task.id,
            "article_folder",
            &second,
            "2026-07-13T21:02:00+08:00",
        )?;

        assert_eq!(store.clear_tasks(&[task.id])?, 1);
        assert!(!first.exists());
        assert!(!second.exists());
        Ok(())
    }

    #[test]
    fn schema_six_temporary_output_migrates_atomically_to_multi_row_schema() -> Result<(), AppError>
    {
        let directory = tempdir().expect("temporary directory");
        let database = directory.path().join("xunqi.db");
        let store = SqliteStore::open(&database)?;
        let (task, _) = store.insert_task(
            CaptureKind::Article,
            "微信公众号",
            "文章",
            "https://mp.weixin.qq.com/s/schema-six",
            "2026-07-13T21:00:00+08:00",
        )?;
        drop(store);
        let legacy_path = directory.path().join(".legacy.123.xunqi-part");
        fs::create_dir(&legacy_path)?;
        {
            let connection = Connection::open(&database)?;
            connection.execute_batch(
                "PRAGMA foreign_keys = OFF;
                 DROP TABLE temporary_outputs;
                 CREATE TABLE temporary_outputs (
                    task_id INTEGER PRIMARY KEY REFERENCES active_tasks(id) ON DELETE CASCADE,
                    output_kind TEXT NOT NULL,
                    temporary_path TEXT NOT NULL,
                    created_at TEXT NOT NULL
                 );
                 PRAGMA user_version = 6;",
            )?;
            connection.execute(
                "INSERT INTO temporary_outputs (
                    task_id, output_kind, temporary_path, created_at
                 ) VALUES (?1, 'pdf_workspace', ?2, '2026-07-13T21:01:00+08:00')",
                params![task.id, legacy_path.to_string_lossy()],
            )?;
        }

        let migrated = SqliteStore::open(&database)?;
        assert!(!legacy_path.exists());
        migrated.track_temporary_output(
            task.id,
            "pdf_workspace",
            &directory.path().join(".first.456.xunqi-part"),
            "2026-07-13T21:02:00+08:00",
        )?;
        migrated.track_temporary_output(
            task.id,
            "article_folder",
            &directory.path().join(".second.789.xunqi-part"),
            "2026-07-13T21:03:00+08:00",
        )?;
        let connection = Connection::open(&database)?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let rows: i64 = connection.query_row(
            "SELECT COUNT(*) FROM temporary_outputs WHERE task_id = ?1",
            [task.id],
            |row| row.get(0),
        )?;
        assert_eq!(version, 7);
        assert_eq!(rows, 2);
        Ok(())
    }

    fn database_bytes(database: &Path) -> Vec<u8> {
        [database.to_path_buf(), database.with_extension("db-wal")]
            .into_iter()
            .filter_map(|path| fs::read(path).ok())
            .flatten()
            .collect()
    }
}
