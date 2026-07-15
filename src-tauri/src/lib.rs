mod application;
mod article_assets;
mod authorized_sniffer;
mod content_inspector;
mod domain;
mod downloader;
mod export_manager;
mod intake;
mod pdf_renderer;
mod processor;
mod public_http;
mod store;
mod wechat_foreground;

pub use application::Application;
pub use article_assets::{ArticleAsset, ArticleAssetFetcher, PublicArticleAssetFetcher};
pub use authorized_sniffer::{
    AuthorizedSniffer, NativeSnifferRuntime, SniffAuthorizationPlan, SniffConflict, SniffOutput,
    SniffPhase, SniffRecoveryResult, SniffSessionSnapshot, SniffSystemChange,
};
pub use content_inspector::{ContentInspector, PublicContentInspector};
pub use domain::{
    AppError, ArticleExportMode, ArticleSnapshot, CaptureKind, CaptureStatus, CaptureTask,
    CaptureTaskDetail, DetectedVideo, DetectedVideoKind, OutputAction, OutputResult,
    SubmitLinksResult, VideoPageInspection, WechatChannelsNetworkRefreshResult,
    WechatForegroundStatus,
};
pub use downloader::{DownloadedMedia, MediaDownloader, PublicVideoDownloader};
pub use intake::IntakeModule;
pub use pdf_renderer::{NativePdfRenderer, PdfRenderer};
pub use processor::CaptureProcessor;
pub use public_http::is_allowed_public_https_url;

use tauri::{Manager, State};

#[tauri::command]
fn list_capture_tasks(
    application: State<'_, Application>,
) -> Result<Vec<CaptureTaskDetail>, String> {
    application.list_tasks().map_err(user_message)
}

#[tauri::command]
fn get_capture_task(
    application: State<'_, Application>,
    task_id: i64,
) -> Result<CaptureTaskDetail, String> {
    application.get_task_detail(task_id).map_err(user_message)
}

#[tauri::command]
fn submit_links(
    application: State<'_, Application>,
    raw_text: String,
) -> Result<SubmitLinksResult, String> {
    application.submit_links(&raw_text).map_err(user_message)
}

#[tauri::command]
async fn process_capture_task(
    application: State<'_, Application>,
    task_id: i64,
) -> Result<CaptureTaskDetail, String> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || application.process_task(task_id))
        .await
        .map_err(|error| format!("内容处理任务意外中断：{error}"))?
        .map_err(user_message)
}

#[tauri::command]
async fn export_article(
    application: State<'_, Application>,
    task_id: i64,
    destination_dir: String,
    mode: ArticleExportMode,
) -> Result<OutputResult, String> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        application.export_article(task_id, destination_dir, mode)
    })
    .await
    .map_err(|error| format!("文章导出任务意外中断：{error}"))?
    .map_err(user_message)
}

#[tauri::command]
async fn download_video(
    application: State<'_, Application>,
    task_id: i64,
    candidate_url: String,
    destination_dir: String,
) -> Result<OutputResult, String> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        application.download_video(task_id, &candidate_url, destination_dir)
    })
    .await
    .map_err(|error| format!("视频下载任务意外中断：{error}"))?
    .map_err(user_message)
}

#[tauri::command]
fn prepare_video_sniff(
    application: State<'_, Application>,
    task_id: i64,
) -> Result<SniffAuthorizationPlan, String> {
    application
        .prepare_video_sniff(task_id)
        .map_err(user_message)
}

#[tauri::command]
async fn start_video_sniff(
    application: State<'_, Application>,
    task_id: i64,
    plan_id: String,
    destination_dir: String,
) -> Result<SniffSessionSnapshot, String> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        application.start_video_sniff(task_id, &plan_id, destination_dir)
    })
    .await
    .map_err(|error| format!("授权嗅探助手意外中断：{error}"))?
    .map_err(user_message)
}

#[tauri::command]
async fn get_video_sniff_session(
    application: State<'_, Application>,
    session_id: String,
) -> Result<SniffSessionSnapshot, String> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || application.get_video_sniff_session(&session_id))
        .await
        .map_err(|error| format!("读取授权嗅探状态失败：{error}"))?
        .map_err(user_message)
}

#[tauri::command]
async fn stop_video_sniff(
    application: State<'_, Application>,
    session_id: String,
) -> Result<SniffSessionSnapshot, String> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || application.stop_video_sniff(&session_id))
        .await
        .map_err(|error| format!("停止授权嗅探助手失败：{error}"))?
        .map_err(user_message)
}

#[tauri::command]
async fn recover_video_sniffing(
    application: State<'_, Application>,
) -> Result<SniffRecoveryResult, String> {
    let application = application.inner().clone();
    tauri::async_runtime::spawn_blocking(move || application.recover_video_sniffing())
        .await
        .map_err(|error| format!("恢复网络设置失败：{error}"))?
        .map_err(user_message)
}

#[tauri::command]
fn clear_capture_tasks(
    application: State<'_, Application>,
    task_ids: Vec<i64>,
) -> Result<usize, String> {
    application.clear_tasks(&task_ids).map_err(user_message)
}

#[tauri::command]
fn detect_wechat_foreground() -> WechatForegroundStatus {
    wechat_foreground::detect_wechat_foreground()
}

fn user_message(error: AppError) -> String {
    error.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let application = Application::open(app_data_dir.join("v2").join("xunqi.db"))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            app.manage(application);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_capture_tasks,
            get_capture_task,
            submit_links,
            process_capture_task,
            export_article,
            download_video,
            prepare_video_sniff,
            start_video_sniff,
            get_video_sniff_session,
            stop_video_sniff,
            recover_video_sniffing,
            clear_capture_tasks,
            detect_wechat_foreground
        ])
        .run(tauri::generate_context!())
        .expect("failed to run 讯栖");
}
