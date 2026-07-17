mod application;
mod article_assets;
mod authorized_sniffer;
mod content_inspector;
mod diagnostics;
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
    SniffPhase, SniffQualityMode, SniffRecoveryResult, SniffSessionSnapshot, SniffSystemChange,
};
pub use content_inspector::{ContentInspector, PublicContentInspector};
pub use diagnostics::{DiagnosticEvent, DiagnosticOutcome, Diagnostics};
pub use domain::{
    AppError, ArticleExportMode, ArticleSnapshot, CaptureKind, CaptureStatus, CaptureTask,
    CaptureTaskDetail, DetectedVideo, DetectedVideoKind, DiagnosticExportResult, OutputAction,
    OutputResult, SubmitLinksResult, VideoPageInspection, WechatChannelsNetworkRefreshResult,
    WechatForegroundStatus,
};
pub use downloader::{DownloadedMedia, MediaDownloader, PublicVideoDownloader};
pub use intake::IntakeModule;
pub use pdf_renderer::{NativePdfRenderer, PdfRenderer};
pub use processor::CaptureProcessor;
pub use public_http::is_allowed_public_https_url;

use tauri::{Manager, State};

#[cfg(target_os = "macos")]
fn configure_macos_runtime_icon() -> Result<(), String> {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let main_thread =
        MainThreadMarker::new().ok_or_else(|| "讯栖只能在 macOS 主线程设置运行图标".to_string())?;
    let icon_data = NSData::with_bytes(include_bytes!("../icons/icon.png"));
    let icon = NSImage::initWithData(NSImage::alloc(), &icon_data)
        .ok_or_else(|| "无法读取讯栖运行图标".to_string())?;

    unsafe {
        NSApplication::sharedApplication(main_thread).setApplicationIconImage(Some(&icon));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn configure_macos_runtime_icon() -> Result<(), String> {
    Ok(())
}

fn diagnostics_directory(app_data_dir: &std::path::Path) -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return std::path::PathBuf::from(local_app_data)
            .join("XunQi")
            .join("logs");
    }
    app_data_dir.join("v2").join("diagnostics")
}

#[tauri::command]
fn list_capture_tasks(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<Vec<CaptureTaskDetail>, String> {
    logged_errors_only(
        diagnostics.inner(),
        DiagnosticEvent::CaptureList,
        application.list_tasks(),
    )
}

#[tauri::command]
fn get_capture_task(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    task_id: i64,
) -> Result<CaptureTaskDetail, String> {
    logged_errors_only(
        diagnostics.inner(),
        DiagnosticEvent::CaptureGet,
        application.get_task_detail(task_id),
    )
}

#[tauri::command]
fn submit_links(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    raw_text: String,
) -> Result<SubmitLinksResult, String> {
    logged_result(
        diagnostics.inner(),
        DiagnosticEvent::CaptureSubmit,
        application.submit_links(&raw_text),
    )
}

#[tauri::command]
async fn process_capture_task(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    task_id: i64,
) -> Result<CaptureTaskDetail, String> {
    let application = application.inner().clone();
    run_blocking_logged(
        diagnostics.inner().clone(),
        DiagnosticEvent::CaptureProcess,
        "内容处理任务意外中断",
        move || application.process_task(task_id),
    )
    .await
}

#[tauri::command]
async fn export_article(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    task_id: i64,
    destination_dir: String,
    mode: ArticleExportMode,
) -> Result<OutputResult, String> {
    let application = application.inner().clone();
    run_blocking_logged(
        diagnostics.inner().clone(),
        DiagnosticEvent::ArticleExport,
        "文章导出任务意外中断",
        move || application.export_article(task_id, destination_dir, mode),
    )
    .await
}

#[tauri::command]
async fn download_video(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    task_id: i64,
    candidate_url: String,
    destination_dir: String,
) -> Result<OutputResult, String> {
    let application = application.inner().clone();
    run_blocking_logged(
        diagnostics.inner().clone(),
        DiagnosticEvent::VideoDownload,
        "视频下载任务意外中断",
        move || application.download_video(task_id, &candidate_url, destination_dir),
    )
    .await
}

#[tauri::command]
fn prepare_video_sniff(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    task_id: i64,
) -> Result<SniffAuthorizationPlan, String> {
    logged_result(
        diagnostics.inner(),
        DiagnosticEvent::SnifferPrepare,
        application.prepare_video_sniff(task_id),
    )
}

#[tauri::command]
async fn start_video_sniff(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    task_id: i64,
    plan_id: String,
    destination_dir: String,
    quality_mode: SniffQualityMode,
) -> Result<SniffSessionSnapshot, String> {
    let application = application.inner().clone();
    run_blocking_logged(
        diagnostics.inner().clone(),
        DiagnosticEvent::SnifferStart,
        "授权嗅探助手意外中断",
        move || application.start_video_sniff(task_id, &plan_id, destination_dir, quality_mode),
    )
    .await
}

#[tauri::command]
async fn get_video_sniff_session(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    session_id: String,
) -> Result<SniffSessionSnapshot, String> {
    let application = application.inner().clone();
    run_blocking_errors_only(
        diagnostics.inner().clone(),
        DiagnosticEvent::SnifferStatus,
        "读取授权嗅探状态失败",
        move || application.get_video_sniff_session(&session_id),
    )
    .await
}

#[tauri::command]
async fn stop_video_sniff(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    session_id: String,
) -> Result<SniffSessionSnapshot, String> {
    let application = application.inner().clone();
    run_blocking_logged(
        diagnostics.inner().clone(),
        DiagnosticEvent::SnifferStop,
        "停止授权嗅探助手失败",
        move || application.stop_video_sniff(&session_id),
    )
    .await
}

#[tauri::command]
async fn recover_video_sniffing(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<SniffRecoveryResult, String> {
    let application = application.inner().clone();
    run_blocking_logged(
        diagnostics.inner().clone(),
        DiagnosticEvent::SnifferRecover,
        "恢复网络设置失败",
        move || application.recover_video_sniffing(),
    )
    .await
}

#[tauri::command]
fn clear_capture_tasks(
    application: State<'_, Application>,
    diagnostics: State<'_, Diagnostics>,
    task_ids: Vec<i64>,
) -> Result<usize, String> {
    logged_result(
        diagnostics.inner(),
        DiagnosticEvent::CaptureClear,
        application.clear_tasks(&task_ids),
    )
}

#[tauri::command]
fn export_diagnostics(
    diagnostics: State<'_, Diagnostics>,
    destination_path: String,
) -> Result<DiagnosticExportResult, String> {
    let result = diagnostics.export(destination_path);
    logged_result(
        diagnostics.inner(),
        DiagnosticEvent::DiagnosticsExport,
        result,
    )
}

#[tauri::command]
fn detect_wechat_foreground() -> WechatForegroundStatus {
    wechat_foreground::detect_wechat_foreground()
}

fn user_message(error: AppError) -> String {
    error.to_string()
}

fn logged_result<T>(
    diagnostics: &Diagnostics,
    event: DiagnosticEvent,
    result: Result<T, AppError>,
) -> Result<T, String> {
    match result {
        Ok(value) => {
            diagnostics.record(event, DiagnosticOutcome::Ok);
            Ok(value)
        }
        Err(error) => {
            diagnostics.record_error(event, &error);
            Err(user_message(error))
        }
    }
}

fn logged_errors_only<T>(
    diagnostics: &Diagnostics,
    event: DiagnosticEvent,
    result: Result<T, AppError>,
) -> Result<T, String> {
    result.map_err(|error| {
        diagnostics.record_error(event, &error);
        user_message(error)
    })
}

async fn run_blocking_logged<T, F>(
    diagnostics: Diagnostics,
    event: DiagnosticEvent,
    interrupted_message: &'static str,
    operation: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(operation).await {
        Ok(result) => logged_result(&diagnostics, event, result),
        Err(error) => {
            diagnostics.record(event, DiagnosticOutcome::RuntimeError);
            Err(format!("{interrupted_message}：{error}"))
        }
    }
}

async fn run_blocking_errors_only<T, F>(
    diagnostics: Diagnostics,
    event: DiagnosticEvent,
    interrupted_message: &'static str,
    operation: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(operation).await {
        Ok(result) => logged_errors_only(&diagnostics, event, result),
        Err(error) => {
            diagnostics.record(event, DiagnosticOutcome::RuntimeError);
            Err(format!("{interrupted_message}：{error}"))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            configure_macos_runtime_icon().map_err(std::io::Error::other)?;
            let app_data_dir = app.path().app_data_dir()?;
            let diagnostics = Diagnostics::open(diagnostics_directory(&app_data_dir));
            let application =
                Application::open(app_data_dir.join("v2").join("xunqi.db")).map_err(|error| {
                    diagnostics.record_error(DiagnosticEvent::AppStorageOpen, &error);
                    std::io::Error::other(error.to_string())
                })?;
            diagnostics.record(DiagnosticEvent::AppStorageOpen, DiagnosticOutcome::Ok);
            app.manage(diagnostics);
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
            detect_wechat_foreground,
            export_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("failed to run 讯栖");
}
