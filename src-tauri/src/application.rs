use std::{
    collections::HashSet,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{
    article_assets::ArticleAssetFetcher, content_inspector::ContentInspector,
    downloader::MediaDownloader, export_manager::ExportManager, intake::IntakeModule,
    pdf_renderer::PdfRenderer, processor::CaptureProcessor, store::SqliteStore, AppError,
    ArticleExportMode, AuthorizedSniffer, CaptureStatus, CaptureTaskDetail, OutputResult,
    SniffAuthorizationPlan, SniffPhase, SniffRecoveryResult, SniffSessionSnapshot,
    SubmitLinksResult,
};

#[derive(Clone)]
pub struct Application {
    store: SqliteStore,
    intake: IntakeModule,
    processor: CaptureProcessor,
    exports: ExportManager,
    sniffer: AuthorizedSniffer,
    in_flight_tasks: Arc<Mutex<HashSet<i64>>>,
}

impl Application {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let database_path = path.as_ref();
        let store = SqliteStore::open(database_path)?;
        let sniffer_root = database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("authorized-sniffer");
        Ok(Self {
            intake: IntakeModule::new(store.clone()),
            processor: CaptureProcessor::new(store.clone()),
            exports: ExportManager::new(store.clone()),
            sniffer: AuthorizedSniffer::native(sniffer_root),
            in_flight_tasks: Arc::new(Mutex::new(HashSet::new())),
            store,
        })
    }

    pub fn open_with_services(
        path: impl AsRef<Path>,
        inspector: Arc<dyn ContentInspector>,
        media_downloader: Arc<dyn MediaDownloader>,
        article_asset_fetcher: Arc<dyn ArticleAssetFetcher>,
        pdf_renderer: Arc<dyn PdfRenderer>,
    ) -> Result<Self, AppError> {
        let database_path = path.as_ref();
        let store = SqliteStore::open(database_path)?;
        let sniffer_root = database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("authorized-sniffer");
        Ok(Self {
            intake: IntakeModule::new(store.clone()),
            processor: CaptureProcessor::with_inspector(store.clone(), inspector),
            exports: ExportManager::with_services(
                store.clone(),
                media_downloader,
                article_asset_fetcher,
                pdf_renderer,
            ),
            sniffer: AuthorizedSniffer::native(sniffer_root),
            in_flight_tasks: Arc::new(Mutex::new(HashSet::new())),
            store,
        })
    }

    pub fn submit_links(&self, raw_text: &str) -> Result<SubmitLinksResult, AppError> {
        self.intake.submit(raw_text)
    }

    pub fn list_tasks(&self) -> Result<Vec<CaptureTaskDetail>, AppError> {
        self.store.list_summaries()
    }

    pub fn get_task_detail(&self, task_id: i64) -> Result<CaptureTaskDetail, AppError> {
        self.store.get_detail(task_id)
    }

    pub fn process_task(&self, task_id: i64) -> Result<CaptureTaskDetail, AppError> {
        let _guard = self.begin_task_operation(task_id)?;
        self.processor.process(task_id)
    }

    pub fn export_article(
        &self,
        task_id: i64,
        directory: impl AsRef<Path>,
        mode: ArticleExportMode,
    ) -> Result<OutputResult, AppError> {
        let _guard = self.begin_task_operation(task_id)?;
        self.exports.export_article(task_id, directory, mode)
    }

    pub fn download_video(
        &self,
        task_id: i64,
        candidate_url: &str,
        directory: impl AsRef<Path>,
    ) -> Result<OutputResult, AppError> {
        let _guard = self.begin_task_operation(task_id)?;
        self.exports
            .download_video(task_id, candidate_url, directory)
    }

    pub fn prepare_video_sniff(&self, task_id: i64) -> Result<SniffAuthorizationPlan, AppError> {
        let detail = self.store.get_detail(task_id)?;
        self.sniffer.prepare(&detail)
    }

    pub fn start_video_sniff(
        &self,
        task_id: i64,
        plan_id: &str,
        directory: impl AsRef<Path>,
    ) -> Result<SniffSessionSnapshot, AppError> {
        let detail = self.store.get_detail(task_id)?;
        self.sniffer.start(&detail, plan_id, directory)
    }

    pub fn get_video_sniff_session(
        &self,
        session_id: &str,
    ) -> Result<SniffSessionSnapshot, AppError> {
        let snapshot = self.sniffer.snapshot(session_id)?;
        self.sync_sniff_task(&snapshot)?;
        Ok(snapshot)
    }

    pub fn stop_video_sniff(&self, session_id: &str) -> Result<SniffSessionSnapshot, AppError> {
        let snapshot = self.sniffer.stop(session_id)?;
        self.sync_sniff_task(&snapshot)?;
        Ok(snapshot)
    }

    pub fn recover_video_sniffing(&self) -> Result<SniffRecoveryResult, AppError> {
        self.sniffer.recover()
    }

    pub fn clear_tasks(&self, task_ids: &[i64]) -> Result<usize, AppError> {
        for task_id in task_ids {
            if self.sniffer.has_active_task(*task_id)? {
                return Err(AppError::Validation(
                    "这个任务的授权嗅探助手仍在运行，请先停止并恢复网络".into(),
                ));
            }
        }
        let _guards = task_ids
            .iter()
            .copied()
            .map(|task_id| self.begin_task_operation(task_id))
            .collect::<Result<Vec<_>, _>>()?;
        self.store.clear_tasks(task_ids)
    }

    fn sync_sniff_task(&self, snapshot: &SniffSessionSnapshot) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        match snapshot.phase {
            SniffPhase::Completed => {
                if let Some(output) = snapshot.output.as_ref() {
                    self.store.mark_completed(
                        snapshot.task_id,
                        &output.destination,
                        "授权嗅探视频已保存到本地",
                        &now,
                    )?;
                }
            }
            SniffPhase::FailedReusable | SniffPhase::FailedRestored => {
                self.store.set_status(
                    snapshot.task_id,
                    CaptureStatus::NeedsAttention,
                    &snapshot.message,
                    &now,
                )?;
            }
            SniffPhase::CancelledRestored if snapshot.output.is_none() => {
                self.store.set_status(
                    snapshot.task_id,
                    CaptureStatus::NeedsAttention,
                    &snapshot.message,
                    &now,
                )?;
            }
            SniffPhase::RestorationRequired => {
                self.store.set_status(
                    snapshot.task_id,
                    CaptureStatus::Failed,
                    &snapshot.message,
                    &now,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    fn begin_task_operation(&self, task_id: i64) -> Result<TaskOperationGuard, AppError> {
        let mut in_flight = self
            .in_flight_tasks
            .lock()
            .map_err(|_| AppError::Validation("任务状态锁已损坏，请重启讯栖".into()))?;
        if !in_flight.insert(task_id) {
            return Err(AppError::Validation(
                "该任务正在处理，请等待当前操作完成".into(),
            ));
        }
        Ok(TaskOperationGuard {
            task_id,
            in_flight_tasks: self.in_flight_tasks.clone(),
        })
    }
}

struct TaskOperationGuard {
    task_id: i64,
    in_flight_tasks: Arc<Mutex<HashSet<i64>>>,
}

impl Drop for TaskOperationGuard {
    fn drop(&mut self) {
        if let Ok(mut in_flight) = self.in_flight_tasks.lock() {
            in_flight.remove(&self.task_id);
        }
    }
}
