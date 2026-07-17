use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{SecondsFormat, Utc};

use crate::{AppError, DiagnosticExportResult};

const RUNTIME_LOG_NAME: &str = "runtime.log";
const LAUNCHER_LOG_NAME: &str = "launcher.log";
const PREVIOUS_LOG_NAME: &str = "runtime.previous.log";
const MAX_LOG_BYTES: u64 = 1024 * 1024;
const ALLOWED_LAUNCHER_EVENTS: &[&str] = &[
    "launcher_start",
    "launcher_exit_ok",
    "error_runtime_missing",
    "error_runtime_exit",
];

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticEvent {
    AppStart,
    AppStorageOpen,
    CaptureList,
    CaptureGet,
    CaptureSubmit,
    CaptureProcess,
    CaptureClear,
    ArticleExport,
    VideoDownload,
    SnifferPrepare,
    SnifferStart,
    SnifferStatus,
    SnifferStop,
    SnifferRecover,
    DiagnosticsExport,
}

impl DiagnosticEvent {
    const ALL: &'static [Self] = &[
        Self::AppStart,
        Self::AppStorageOpen,
        Self::CaptureList,
        Self::CaptureGet,
        Self::CaptureSubmit,
        Self::CaptureProcess,
        Self::CaptureClear,
        Self::ArticleExport,
        Self::VideoDownload,
        Self::SnifferPrepare,
        Self::SnifferStart,
        Self::SnifferStatus,
        Self::SnifferStop,
        Self::SnifferRecover,
        Self::DiagnosticsExport,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::AppStart => "app.start",
            Self::AppStorageOpen => "app.storage_open",
            Self::CaptureList => "capture.list",
            Self::CaptureGet => "capture.get",
            Self::CaptureSubmit => "capture.submit",
            Self::CaptureProcess => "capture.process",
            Self::CaptureClear => "capture.clear",
            Self::ArticleExport => "article.export",
            Self::VideoDownload => "video.download",
            Self::SnifferPrepare => "sniffer.prepare",
            Self::SnifferStart => "sniffer.start",
            Self::SnifferStatus => "sniffer.status",
            Self::SnifferStop => "sniffer.stop",
            Self::SnifferRecover => "sniffer.recover",
            Self::DiagnosticsExport => "diagnostics.export",
        }
    }

    fn recognizes(value: &str) -> bool {
        Self::ALL.iter().any(|event| event.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticOutcome {
    Ok,
    StorageError,
    ValidationError,
    ContentError,
    DownloadError,
    TaskNotFound,
    IoError,
    SerializationError,
    RuntimeError,
}

impl DiagnosticOutcome {
    const ALL: &'static [Self] = &[
        Self::Ok,
        Self::StorageError,
        Self::ValidationError,
        Self::ContentError,
        Self::DownloadError,
        Self::TaskNotFound,
        Self::IoError,
        Self::SerializationError,
        Self::RuntimeError,
    ];

    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::StorageError => "error.storage",
            Self::ValidationError => "error.validation",
            Self::ContentError => "error.content",
            Self::DownloadError => "error.download",
            Self::TaskNotFound => "error.task_not_found",
            Self::IoError => "error.io",
            Self::SerializationError => "error.serialization",
            Self::RuntimeError => "error.runtime",
        }
    }

    fn recognizes(value: &str) -> bool {
        Self::ALL.iter().any(|outcome| outcome.as_str() == value)
    }
}

#[derive(Clone)]
pub struct Diagnostics {
    log_path: Arc<PathBuf>,
    launcher_log_path: Arc<PathBuf>,
    write_lock: Arc<Mutex<()>>,
}

impl Diagnostics {
    pub fn open(directory: impl AsRef<Path>) -> Self {
        let directory = directory.as_ref();
        let _ = fs::create_dir_all(directory);
        let log_path = directory.join(RUNTIME_LOG_NAME);
        let launcher_log_path = directory.join(LAUNCHER_LOG_NAME);
        rotate_log_if_needed(&log_path);
        let diagnostics = Self {
            log_path: Arc::new(log_path),
            launcher_log_path: Arc::new(launcher_log_path),
            write_lock: Arc::new(Mutex::new(())),
        };
        diagnostics.record(DiagnosticEvent::AppStart, DiagnosticOutcome::Ok);
        diagnostics
    }

    pub fn record(&self, event: DiagnosticEvent, outcome: DiagnosticOutcome) {
        let Ok(_guard) = self.write_lock.lock() else {
            return;
        };
        rotate_log_if_needed(self.log_path.as_ref());
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path.as_ref())
        else {
            return;
        };
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let _ = writeln!(
            file,
            "{timestamp} event={} outcome={}",
            event.as_str(),
            outcome.as_str()
        );
    }

    pub fn record_error(&self, event: DiagnosticEvent, error: &AppError) {
        self.record(event, error_category(error));
    }

    pub fn export(
        &self,
        destination: impl AsRef<Path>,
    ) -> Result<DiagnosticExportResult, AppError> {
        let destination = destination.as_ref();
        if destination.as_os_str().is_empty() || destination.is_dir() {
            return Err(AppError::Validation(
                "请选择一个诊断日志文件保存位置".into(),
            ));
        }
        if let Some(parent) = destination.parent() {
            if !parent.is_dir() {
                return Err(AppError::Validation("诊断日志的目标文件夹不存在".into()));
            }
        }
        reject_internal_destination(
            destination,
            &[
                self.log_path.as_ref(),
                self.launcher_log_path.as_ref(),
                &self.log_path.with_file_name(PREVIOUS_LOG_NAME),
            ],
        )?;

        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| AppError::Validation("诊断日志状态锁已损坏，请重启讯栖".into()))?;
        let runtime_log = sanitized_runtime_events(self.log_path.as_ref());
        let launcher_log = sanitized_launcher_events(self.launcher_log_path.as_ref());
        let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        let report = format!(
            "XunQi Diagnostic Report\n\
             =======================\n\
             Version: {}\n\
             Generated: {generated_at}\n\
             Platform: {} / {}\n\
             Privacy: This report contains controlled event codes and result categories only.\n\
             It does not contain Cookies, chat history, article bodies, share links, passwords, or tokens.\n\
             \n\
             Runtime events\n\
             --------------\n\
             {runtime_log}\n\
             Launcher events\n\
             ---------------\n\
             {launcher_log}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        fs::write(destination, report.as_bytes())?;
        let bytes_written = fs::metadata(destination)?.len();
        Ok(DiagnosticExportResult {
            destination: destination.to_string_lossy().into_owned(),
            bytes_written,
        })
    }
}

fn sanitized_runtime_events(path: &Path) -> String {
    let Ok(contents) = fs::read_to_string(path) else {
        return "No runtime events were available. The application log may not have been writable.\n"
            .into();
    };
    let events: String = contents
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let timestamp = parts.next()?;
            let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
            let event = parts.next()?.strip_prefix("event=")?;
            let outcome = parts.next()?.strip_prefix("outcome=")?;
            if !DiagnosticEvent::recognizes(event) || !DiagnosticOutcome::recognizes(outcome) {
                return None;
            }
            Some(format!(
                "{} event={event} outcome={outcome}\n",
                timestamp
                    .with_timezone(&Utc)
                    .to_rfc3339_opts(SecondsFormat::Secs, true)
            ))
        })
        .collect();
    if events.is_empty() {
        "No recognized runtime events were available.\n".into()
    } else {
        events
    }
}

fn sanitized_launcher_events(path: &Path) -> String {
    let Ok(contents) = fs::read_to_string(path) else {
        return "No launcher events were available.\n".into();
    };
    let events: String = contents
        .lines()
        .filter_map(|line| {
            ALLOWED_LAUNCHER_EVENTS
                .iter()
                .find(|event| line.contains(&format!("event={event}")))
        })
        .map(|event| format!("event={event}\n"))
        .collect();
    if events.is_empty() {
        "No recognized launcher events were available.\n".into()
    } else {
        events
    }
}

fn rotate_log_if_needed(log_path: &Path) {
    let Ok(metadata) = fs::metadata(log_path) else {
        return;
    };
    if metadata.len() <= MAX_LOG_BYTES {
        return;
    }
    let previous = log_path.with_file_name(PREVIOUS_LOG_NAME);
    let _ = fs::remove_file(&previous);
    let _ = fs::rename(log_path, previous);
}

fn reject_internal_destination(destination: &Path, internal: &[&Path]) -> Result<(), AppError> {
    let destination = path_identity(destination)?;
    for internal_path in internal {
        let internal_path = path_identity(internal_path)?;
        #[cfg(target_os = "windows")]
        let matches = destination
            .to_string_lossy()
            .eq_ignore_ascii_case(&internal_path.to_string_lossy());
        #[cfg(not(target_os = "windows"))]
        let matches = destination == internal_path;
        if matches {
            return Err(AppError::Validation(
                "诊断日志不能覆盖讯栖正在使用的内部日志，请选择其他位置".into(),
            ));
        }
    }
    Ok(())
}

fn path_identity(path: &Path) -> Result<PathBuf, AppError> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| AppError::Validation("诊断日志文件名无效".into()))?;
    Ok(fs::canonicalize(parent)?.join(file_name))
}

fn error_category(error: &AppError) -> DiagnosticOutcome {
    match error {
        AppError::Storage(_) => DiagnosticOutcome::StorageError,
        AppError::Validation(_) => DiagnosticOutcome::ValidationError,
        AppError::Content(_) => DiagnosticOutcome::ContentError,
        AppError::Download(_) => DiagnosticOutcome::DownloadError,
        AppError::TaskNotFound(_) => DiagnosticOutcome::TaskNotFound,
        AppError::Io(_) => DiagnosticOutcome::IoError,
        AppError::Serialization(_) => DiagnosticOutcome::SerializationError,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{DiagnosticEvent, DiagnosticOutcome, Diagnostics};

    #[test]
    fn diagnostic_export_contains_system_context_and_controlled_events() {
        let root = tempdir().unwrap();
        let diagnostics = Diagnostics::open(root.path().join("logs"));
        diagnostics.record(DiagnosticEvent::ArticleExport, DiagnosticOutcome::Ok);
        let destination = root.path().join("XunQi-Diagnostics.txt");

        let result = diagnostics.export(&destination).unwrap();
        let report = fs::read_to_string(destination).unwrap();

        assert!(result.bytes_written > 0);
        assert!(report.contains("XunQi Diagnostic Report"));
        assert!(report.contains("event=app.start outcome=ok"));
        assert!(report.contains("event=article.export outcome=ok"));
        assert!(report.contains("does not contain Cookies"));
    }

    #[test]
    fn diagnostic_export_rebuilds_tampered_runtime_lines_from_the_allowlist() {
        let root = tempdir().unwrap();
        let log_root = root.path().join("logs");
        let diagnostics = Diagnostics::open(&log_root);
        fs::write(
            log_root.join("runtime.log"),
            "2026-07-17T00:00:00Z event=article.export outcome=ok\nhttps://mp.weixin.qq.com/s/private-link Cookie=session-secret article body\n",
        )
        .unwrap();
        let destination = root.path().join("diagnostics.txt");

        diagnostics.export(&destination).unwrap();
        let report = fs::read_to_string(destination).unwrap();

        assert!(!report.contains("mp.weixin.qq.com"));
        assert!(!report.contains("session-secret"));
        assert!(!report.contains("article body"));
        assert!(report.contains("event=article.export outcome=ok"));
    }

    #[test]
    fn diagnostic_export_only_accepts_known_launcher_events() {
        let root = tempdir().unwrap();
        let log_root = root.path().join("logs");
        let diagnostics = Diagnostics::open(&log_root);
        fs::write(
            log_root.join("launcher.log"),
            "event=launcher_start version=0.5.0\nCookie=session-secret\nevent=made_up\n",
        )
        .unwrap();
        let destination = root.path().join("diagnostics.txt");

        diagnostics.export(&destination).unwrap();
        let report = fs::read_to_string(destination).unwrap();

        assert!(report.contains("event=launcher_start"));
        assert!(!report.contains("session-secret"));
        assert!(!report.contains("made_up"));
    }

    #[test]
    fn diagnostic_export_cannot_overwrite_internal_logs() {
        let root = tempdir().unwrap();
        let log_root = root.path().join("logs");
        let diagnostics = Diagnostics::open(&log_root);

        let runtime_error = diagnostics
            .export(log_root.join("runtime.log"))
            .unwrap_err();
        let launcher_error = diagnostics
            .export(log_root.join("launcher.log"))
            .unwrap_err();
        let previous_error = diagnostics
            .export(log_root.join("runtime.previous.log"))
            .unwrap_err();

        assert!(runtime_error.to_string().contains("不能覆盖"));
        assert!(launcher_error.to_string().contains("不能覆盖"));
        assert!(previous_error.to_string().contains("不能覆盖"));
    }
}
