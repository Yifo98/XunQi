use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::AppError;

use super::{
    RuntimePreflight, RuntimeSnapshot, RuntimeStartRequest, SniffConflict, SniffOutput, SniffPhase,
    SniffProgress, SniffQualityMode, SniffRecoveryResult, SnifferRuntime,
};

#[cfg(target_os = "windows")]
#[path = "native_windows.rs"]
mod windows;

#[cfg(target_os = "windows")]
use windows::{
    apply_network_state, certificate_fingerprint, default_user_keychain,
    generate_session_certificate, install_certificate, read_network, remove_certificate,
    set_private_directory, set_private_file, snapshot_network, terminate_recovered_helper,
    validate_platform_network_conflicts,
};

#[cfg(target_os = "macos")]
const MACOS_HELPER_SHA256: &str =
    "ee777e9f07a4784163d16c6dc0288bb9d8c1ea9db3ae4a7275f098d1a1a56aca";
#[cfg(target_os = "macos")]
const MACOS_PACKAGED_HELPER_SHA256: &str =
    "2ef57ef05513466fac48be21d8c0a1bf8de9f26f86edf337fd6772f9c2fcd631";
const SESSION_CAPTURE_TIMEOUT: Duration = Duration::from_secs(120);
const OUTPUT_FINALIZATION_TIMEOUT: Duration = Duration::from_secs(90);
const HELPER_GUIDE_SCRIPT: &str = r#"
(function () {
  var originalError = WXU.error;
  WXU.error = function (payload) {
    var message = String((payload && payload.msg) || "");
    if (message.indexOf("检测不到视频") !== -1 || message.indexOf("没有获取到视频详情") !== -1) {
      WXU.toast("讯栖会按已复制的分享链接自动处理；无需刷新，也不用点击本页下载按钮。");
      return;
    }
    return originalError.call(WXU, payload);
  };
})();
"#;

pub struct NativeSnifferRuntime {
    root: PathBuf,
    session: Mutex<Option<NativeSession>>,
}

struct NativeSession {
    id: String,
    child: Child,
    api_port: u16,
    destination_dir: PathBuf,
    staging_dir: PathBuf,
    expected_title: String,
    share_url: String,
    bound_task_id: Option<String>,
    last_share_submit_attempt: Option<Instant>,
    completion_observed_at: Option<Instant>,
    api_failure_count: u8,
    share_submit_failure_count: u8,
    started_at: Instant,
    quality_mode: SniffQualityMode,
    journal: RecoveryJournal,
}

#[derive(Debug, PartialEq, Eq)]
enum ShareLinkSubmission {
    Created(String),
    Waiting,
}

#[derive(Debug)]
enum CompletedOutputError {
    Pending(String),
    Security(String),
    Export(String),
}

impl std::fmt::Display for CompletedOutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(message) | Self::Security(message) | Self::Export(message) => {
                formatter.write_str(message)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionDisposition {
    Wait,
    KeepAuthorization,
    Restore,
}

fn completion_disposition(
    error: &CompletedOutputError,
    elapsed: Duration,
) -> CompletionDisposition {
    match error {
        CompletedOutputError::Security(_) => CompletionDisposition::Restore,
        CompletedOutputError::Pending(_) if elapsed < OUTPUT_FINALIZATION_TIMEOUT => {
            CompletionDisposition::Wait
        }
        CompletedOutputError::Pending(_) | CompletedOutputError::Export(_) => {
            CompletionDisposition::KeepAuthorization
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryJournal {
    session_id: String,
    task_id: i64,
    helper_pid: Option<u32>,
    session_dir: PathBuf,
    certificate_name: String,
    certificate_fingerprint: String,
    keychain: PathBuf,
    proxy_port: u16,
    #[serde(default)]
    applied_pac_url: String,
    network: NetworkSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct NetworkSnapshot {
    service: String,
    web: ProxyState,
    secure_web: ProxyState,
    auto_proxy: AutoProxyState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    windows: Option<WindowsProxySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WindowsProxySnapshot {
    proxy_enable: Option<i64>,
    proxy_server: Option<String>,
    auto_config_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProxyState {
    enabled: bool,
    server: String,
    port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct AutoProxyState {
    enabled: bool,
    url: String,
}

#[cfg(target_os = "macos")]
fn helper_source() -> &'static str {
    "ltaoo/wx_channels_download v260706"
}

#[cfg(target_os = "windows")]
fn helper_source() -> &'static str {
    windows::helper_source()
}

#[cfg(target_os = "macos")]
fn helper_candidates(executable_dir: &Path) -> Vec<PathBuf> {
    vec![
        executable_dir.join("xunqi-authorized-sniffer"),
        executable_dir.join("../Resources/xunqi-authorized-sniffer"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "bin/xunqi-authorized-sniffer-{}-apple-darwin",
            std::env::consts::ARCH
        )),
    ]
}

#[cfg(target_os = "windows")]
fn helper_candidates(executable_dir: &Path) -> Vec<PathBuf> {
    windows::helper_candidates(executable_dir)
}

#[cfg(target_os = "macos")]
fn helper_hash_matches(actual: &str) -> bool {
    actual == MACOS_HELPER_SHA256 || actual == MACOS_PACKAGED_HELPER_SHA256
}

#[cfg(target_os = "windows")]
fn helper_hash_matches(actual: &str) -> bool {
    windows::helper_hash_matches(actual)
}

#[cfg(target_os = "macos")]
fn validate_platform_prerequisites() -> Result<(), SniffConflict> {
    for required in [
        "/usr/bin/openssl",
        "/usr/bin/security",
        "/usr/sbin/networksetup",
    ] {
        if !Path::new(required).is_file() {
            return Err(SniffConflict {
                code: "system_tool_missing".into(),
                message: format!("系统缺少授权助手需要的组件：{required}"),
            });
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_platform_network_conflicts() -> Result<(), SniffConflict> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn validate_platform_prerequisites() -> Result<(), SniffConflict> {
    windows::validate_platform_prerequisites()
}

impl NativeSnifferRuntime {
    pub fn new(state_root: impl AsRef<Path>) -> Self {
        Self {
            root: state_root.as_ref().to_path_buf(),
            session: Mutex::new(None),
        }
    }

    fn helper_path(&self) -> Result<PathBuf, SniffConflict> {
        if let Some(value) = std::env::var_os("XUNQI_AUTHORIZED_SNIFFER") {
            let candidate = PathBuf::from(value);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        let current_exe = std::env::current_exe().map_err(|error| SniffConflict {
            code: "helper_missing".into(),
            message: format!("无法确定讯栖程序位置：{error}"),
        })?;
        let executable_dir = current_exe.parent().ok_or_else(|| SniffConflict {
            code: "helper_missing".into(),
            message: "无法确定讯栖程序目录".into(),
        })?;
        helper_candidates(executable_dir)
            .into_iter()
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| SniffConflict {
                code: "helper_missing".into(),
                message: "授权嗅探组件缺失，请重新解压完整的讯栖安装包".into(),
            })
    }

    fn verify_helper(&self, path: &Path) -> Result<(), SniffConflict> {
        let mut file = fs::File::open(path).map_err(|error| SniffConflict {
            code: "helper_unreadable".into(),
            message: format!("授权嗅探组件无法读取：{error}"),
        })?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(|error| SniffConflict {
                code: "helper_unreadable".into(),
                message: format!("授权嗅探组件校验失败：{error}"),
            })?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        let actual = format!("{:x}", digest.finalize());
        if !helper_hash_matches(&actual) {
            return Err(SniffConflict {
                code: "helper_checksum_mismatch".into(),
                message: "授权嗅探组件校验值不符，为保护你的网络设置已拒绝启动".into(),
            });
        }
        Ok(())
    }

    fn journal_path(&self) -> PathBuf {
        self.root.join("recovery.json")
    }

    fn start_inner(&self, request: &RuntimeStartRequest) -> Result<RuntimeSnapshot, AppError> {
        {
            let mut guard = self
                .session
                .lock()
                .map_err(|_| AppError::Validation("授权嗅探运行状态已损坏".into()))?;
            if let Some(session) = guard.as_mut() {
                if session.child.try_wait()?.is_some() || !self.journal_path().is_file() {
                    return Err(AppError::Validation(
                        "上一次连续下载会话已失效，请先恢复网络后重试".into(),
                    ));
                }
                if session.quality_mode != request.quality_mode {
                    return Err(AppError::Validation(
                        "连续授权期间不能切换画质；请先结束并恢复网络，再用新画质重新启用".into(),
                    ));
                }
                session.id = request.session_id.clone();
                session.destination_dir = request.destination_dir.clone();
                session.expected_title = request.expected_title.clone();
                session.share_url = request.share_url.clone();
                session.bound_task_id = None;
                session.last_share_submit_attempt = None;
                session.completion_observed_at = None;
                session.api_failure_count = 0;
                session.share_submit_failure_count = 0;
                session.started_at = Instant::now();
                session.journal.session_id = request.session_id.clone();
                session.journal.task_id = request.task_id;
                write_journal(&self.journal_path(), &session.journal)?;
                return Ok(RuntimeSnapshot {
                    phase: SniffPhase::AwaitingPlayback,
                    message: "已复用授权助手，无需再次确认，也不会重新加载当前视频号窗口。".into(),
                    helper_page_url: Some(format!(
                        "http://127.0.0.1:{}/download",
                        session.api_port
                    )),
                    destination_directory: request.destination_dir.to_string_lossy().into_owned(),
                    authorization_reusable: true,
                    progress: None,
                    output: None,
                    error_code: None,
                });
            }
        }
        let helper = self
            .helper_path()
            .map_err(|conflict| AppError::Validation(conflict.message))?;
        self.verify_helper(&helper)
            .map_err(|conflict| AppError::Validation(conflict.message))?;
        validate_platform_prerequisites()
            .map_err(|conflict| AppError::Validation(conflict.message))?;
        let network = snapshot_network().map_err(AppError::Content)?;
        validate_network(&network).map_err(|conflict| AppError::Validation(conflict.message))?;
        let api_port = free_loopback_port()?;
        let proxy_port = free_loopback_port()?;
        let session_dir = self.root.join(&request.session_id);
        fs::create_dir_all(&session_dir)?;
        set_private_directory(&session_dir)?;
        let staging_dir = session_dir.join("downloads");
        fs::create_dir_all(&staging_dir)?;
        set_private_directory(&staging_dir)?;
        let certificate_name = format!("XunQi-{}", request.session_id.replace('-', ""));
        let certificate_path = session_dir.join("session-ca.pem");
        let key_path = session_dir.join("session-ca-key.pem");
        generate_session_certificate(&certificate_name, &certificate_path, &key_path)?;
        let certificate_fingerprint = certificate_fingerprint(&certificate_path)?;
        let keychain = default_user_keychain()?;
        let upstream_proxy = compatible_upstream_proxy(&network);
        let mut journal = RecoveryJournal {
            session_id: request.session_id.clone(),
            task_id: request.task_id,
            helper_pid: None,
            session_dir: session_dir.clone(),
            certificate_name: certificate_name.clone(),
            certificate_fingerprint,
            keychain,
            proxy_port,
            applied_pac_url: String::new(),
            network,
        };
        fs::create_dir_all(&self.root)?;
        set_private_directory(&self.root)?;
        let journal_path = self.journal_path();
        write_journal(&journal_path, &journal)?;
        let guide_script_path = session_dir.join("xunqi-guide.js");
        let config_path = session_dir.join("config.yaml");
        let mut child: Option<Child> = None;
        let mut reload_message = String::new();
        let startup = (|| -> Result<(), AppError> {
            install_certificate(&journal.keychain, &certificate_path)?;
            fs::write(&guide_script_path, HELPER_GUIDE_SCRIPT)?;
            fs::write(
                &config_path,
                helper_config(
                    &staging_dir,
                    &certificate_path,
                    &key_path,
                    &certificate_name,
                    api_port,
                    proxy_port,
                    upstream_proxy.as_deref(),
                    &guide_script_path,
                    request.quality_mode,
                ),
            )?;
            discard_upstream_log(&session_dir)?;
            child = Some(
                Command::new(&helper)
                    .arg("--config")
                    .arg(&config_path)
                    .current_dir(&session_dir)
                    .env("HOME", &session_dir)
                    .env("XDG_CONFIG_HOME", session_dir.join("config"))
                    .env("XDG_CACHE_HOME", session_dir.join("cache"))
                    .env("XDG_DATA_HOME", session_dir.join("data"))
                    .env("APPDATA", session_dir.join("appdata"))
                    .env("LOCALAPPDATA", session_dir.join("local-appdata"))
                    .env("USERPROFILE", &session_dir)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|error| AppError::Content(format!("无法启动授权嗅探组件：{error}")))?,
            );
            journal.helper_pid = child.as_ref().map(Child::id);
            write_journal(&journal_path, &journal)?;
            wait_for_helper(
                api_port,
                child.as_mut().expect("helper child was just created"),
            )?;
            apply_network_state(&session_network_state(&journal.network, journal.proxy_port))?;
            reload_message = crate::wechat_foreground::refresh_wechat_channels_network()?.message;
            Ok(())
        })();
        if let Err(error) = startup {
            let recovery = rollback_startup(&journal_path, &journal, child.as_mut());
            return Err(combine_startup_and_recovery_error(error, recovery));
        }
        let mut native = NativeSession {
            id: request.session_id.clone(),
            child: child.expect("successful startup always owns a helper process"),
            api_port,
            destination_dir: request.destination_dir.clone(),
            staging_dir,
            expected_title: request.expected_title.clone(),
            share_url: request.share_url.clone(),
            bound_task_id: None,
            last_share_submit_attempt: None,
            completion_observed_at: None,
            api_failure_count: 0,
            share_submit_failure_count: 0,
            started_at: Instant::now(),
            quality_mode: request.quality_mode,
            journal,
        };
        let helper_page_url = format!("http://127.0.0.1:{api_port}/download");
        let mut session_guard = match self.session.lock() {
            Ok(guard) => guard,
            Err(_) => {
                let recovery = self.cleanup(&mut native);
                return Err(combine_startup_and_recovery_error(
                    AppError::Validation("授权嗅探运行状态已损坏".into()),
                    recovery,
                ));
            }
        };
        *session_guard = Some(native);
        Ok(RuntimeSnapshot {
            phase: SniffPhase::AwaitingPlayback,
            message: format!("授权助手已启用。{reload_message} 无需刷新，也不用寻找页面下载按钮；讯栖会按已复制的分享链接自动创建下载。"),
            helper_page_url: Some(helper_page_url),
            destination_directory: request.destination_dir.to_string_lossy().into_owned(),
            authorization_reusable: true,
            progress: None,
            output: None,
            error_code: None,
        })
    }

    fn observe_inner(&self, session_id: &str) -> Result<RuntimeSnapshot, AppError> {
        let mut guard = self
            .session
            .lock()
            .map_err(|_| AppError::Validation("授权嗅探运行状态已损坏".into()))?;
        let session = guard
            .as_mut()
            .filter(|session| session.id == session_id)
            .ok_or_else(|| AppError::Validation("授权嗅探助手没有运行".into()))?;
        if let Some(status) = session.child.try_wait()? {
            let destination_directory = session.destination_dir.to_string_lossy().into_owned();
            let recovery = self.cleanup(session);
            *guard = None;
            return Ok(match recovery {
                Ok(()) => RuntimeSnapshot {
                    phase: SniffPhase::FailedRestored,
                    message: format!(
                        "授权助手已退出（{status}），网络设置已恢复。可以稍后重试或打开微信原文。"
                    ),
                    helper_page_url: None,
                    destination_directory,
                    authorization_reusable: false,
                    progress: None,
                    output: None,
                    error_code: Some("helper_exited".into()),
                },
                Err(error) => restoration_required(error),
            });
        }
        if session.bound_task_id.is_none()
            && session.started_at.elapsed() >= SESSION_CAPTURE_TIMEOUT
        {
            let recovery = self.cleanup(session);
            *guard = None;
            return Ok(failed_restored_or_required(
                recovery,
                "等待视频号连接超时，授权助手已停止，网络设置已恢复。请重新授权后再进入一次视频号。",
                "capture_timeout",
            ));
        }
        let mut tasks = match fetch_tasks(session.api_port) {
            Ok(tasks) => {
                session.api_failure_count = 0;
                tasks
            }
            Err(error) => {
                session.api_failure_count = session.api_failure_count.saturating_add(1);
                if session.api_failure_count < 3 {
                    return Ok(RuntimeSnapshot {
                        phase: SniffPhase::Capturing,
                        message: format!(
                            "本地助手短暂无响应，讯栖会自动重试（{}/3）：{error}",
                            session.api_failure_count
                        ),
                        helper_page_url: Some(format!(
                            "http://127.0.0.1:{}/download",
                            session.api_port
                        )),
                        destination_directory: session
                            .destination_dir
                            .to_string_lossy()
                            .into_owned(),
                        authorization_reusable: true,
                        progress: None,
                        output: None,
                        error_code: Some("helper_api_retrying".into()),
                    });
                }
                let recovery = self.cleanup(session);
                *guard = None;
                return Ok(failed_restored_or_required(
                    recovery,
                    &format!("本地助手连接中断：{error}。授权会话已停止。"),
                    "helper_api_unavailable",
                ));
            }
        };
        let should_submit_share_link = session.bound_task_id.is_none()
            && session
                .last_share_submit_attempt
                .is_none_or(|attempt| attempt.elapsed() >= Duration::from_secs(3));
        if should_submit_share_link {
            session.last_share_submit_attempt = Some(Instant::now());
            match submit_share_link_task(session.api_port, &session.share_url) {
                Ok(ShareLinkSubmission::Created(task_id)) => {
                    session.share_submit_failure_count = 0;
                    session.bound_task_id = Some(task_id);
                    if let Ok(refreshed) = fetch_tasks(session.api_port) {
                        tasks = refreshed;
                    }
                }
                Ok(ShareLinkSubmission::Waiting) => {
                    session.share_submit_failure_count = 0;
                }
                Err(error) => {
                    session.share_submit_failure_count =
                        session.share_submit_failure_count.saturating_add(1);
                    if session.share_submit_failure_count < 3 {
                        return Ok(RuntimeSnapshot {
                            phase: SniffPhase::AwaitingPlayback,
                            message: format!(
                                "自动提交分享链接暂时失败，讯栖会重试（{}/3）：{error}",
                                session.share_submit_failure_count
                            ),
                            helper_page_url: Some(format!(
                                "http://127.0.0.1:{}/download",
                                session.api_port
                            )),
                            destination_directory: session
                                .destination_dir
                                .to_string_lossy()
                                .into_owned(),
                            authorization_reusable: true,
                            progress: None,
                            output: None,
                            error_code: Some("share_submit_retrying".into()),
                        });
                    }
                    let recovery = self.cleanup(session);
                    *guard = None;
                    return Ok(failed_restored_or_required(
                        recovery,
                        &format!("连续 3 次自动提交分享链接失败：{error}。授权会话已停止。"),
                        "share_submit_failed",
                    ));
                }
            }
        }
        if let Some(task) = select_bound_task(&tasks, session.bound_task_id.as_deref()) {
            let status = task["status"].as_str().unwrap_or_default();
            if status == "done" {
                let completed_since = *session
                    .completion_observed_at
                    .get_or_insert_with(Instant::now);
                let completed = completed_output(task, session);
                return Ok(match completed {
                    Ok(output) => RuntimeSnapshot {
                        phase: SniffPhase::Completed,
                        message: format!(
                            "视频已保存到 {}。连续下载仍已启用，下一条无需再次确认。",
                            output.destination
                        ),
                        helper_page_url: Some(format!(
                            "http://127.0.0.1:{}/download",
                            session.api_port
                        )),
                        destination_directory: session
                            .destination_dir
                            .to_string_lossy()
                            .into_owned(),
                        authorization_reusable: true,
                        progress: Some(SniffProgress {
                            downloaded_bytes: output.bytes_written,
                            total_bytes: Some(output.bytes_written),
                            bytes_per_second: 0,
                            percent: Some(100),
                        }),
                        output: Some(output),
                        error_code: None,
                    },
                    Err(error)
                        if completion_disposition(&error, completed_since.elapsed())
                            == CompletionDisposition::Wait =>
                    {
                        RuntimeSnapshot {
                            phase: SniffPhase::Saving,
                            message: "视频数据已下载，正在等待助手完成解密和封装…".into(),
                            helper_page_url: Some(format!(
                                "http://127.0.0.1:{}/download",
                                session.api_port
                            )),
                            destination_directory: session
                                .destination_dir
                                .to_string_lossy()
                                .into_owned(),
                            authorization_reusable: true,
                            progress: sniff_progress(task).or(Some(SniffProgress {
                                downloaded_bytes: 0,
                                total_bytes: None,
                                bytes_per_second: 0,
                                percent: Some(100),
                            })),
                            output: None,
                            error_code: None,
                        }
                    }
                    Err(error)
                        if completion_disposition(&error, completed_since.elapsed())
                            == CompletionDisposition::KeepAuthorization =>
                    {
                        remove_incomplete_staging_output(task, &session.staging_dir);
                        RuntimeSnapshot {
                            phase: SniffPhase::FailedReusable,
                            message: format!("当前视频没有完成保存：{error}。本条临时文件已清理，连续授权仍然保留；可以直接处理下一条，或结束并恢复网络。"),
                            helper_page_url: Some(format!(
                                "http://127.0.0.1:{}/download",
                                session.api_port
                            )),
                            destination_directory: session
                                .destination_dir
                                .to_string_lossy()
                                .into_owned(),
                            authorization_reusable: true,
                            progress: None,
                            output: None,
                            error_code: Some("invalid_video_output".into()),
                        }
                    }
                    Err(error) => {
                        let destination_directory =
                            session.destination_dir.to_string_lossy().into_owned();
                        let recovery = self.cleanup(session);
                        *guard = None;
                        match recovery {
                            Ok(()) => RuntimeSnapshot {
                                phase: SniffPhase::FailedRestored,
                                message: format!("授权助手返回了不安全的文件位置：{error}。讯栖已拒绝接收，并恢复网络设置。"),
                                helper_page_url: None,
                                destination_directory,
                                authorization_reusable: false,
                                progress: None,
                                output: None,
                                error_code: Some("unsafe_video_output".into()),
                            },
                            Err(error) => restoration_required(error),
                        }
                    }
                });
            }
            if matches!(status, "error" | "failed") {
                remove_incomplete_staging_output(task, &session.staging_dir);
                return Ok(RuntimeSnapshot {
                    phase: SniffPhase::FailedReusable,
                    message:
                        "当前视频下载失败，未完整文件已清理；连续授权仍然保留，可以直接处理下一条。"
                            .into(),
                    helper_page_url: Some(format!(
                        "http://127.0.0.1:{}/download",
                        session.api_port
                    )),
                    destination_directory: session.destination_dir.to_string_lossy().into_owned(),
                    authorization_reusable: true,
                    progress: None,
                    output: None,
                    error_code: Some("helper_task_failed".into()),
                });
            }
            session.completion_observed_at = None;
        }
        let matching_task = select_bound_task(&tasks, session.bound_task_id.as_deref()).is_some();
        let has_other_task = !tasks.is_empty() && !matching_task;
        Ok(RuntimeSnapshot {
            phase: if matching_task {
                SniffPhase::Saving
            } else if has_other_task {
                SniffPhase::Capturing
            } else {
                SniffPhase::AwaitingPlayback
            },
            message: if matching_task {
                "已匹配当前分享的视频，正在私有临时目录中下载和校验…".into()
            } else if has_other_task {
                format!(
                    "页面里出现了其他视频，讯栖没有把它当作当前任务，仍会继续按已复制的分享链接处理：{}",
                    session.share_url,
                )
            } else if session.started_at.elapsed() < Duration::from_secs(12) {
                "正在等待视频号环境就绪。请从微信左侧重新进入一次“视频号”；无需刷新，也不用寻找下载按钮。".into()
            } else {
                "还没有连上视频号页面。请保持讯栖打开，从微信左侧退出“视频号”后再进入一次；讯栖会自动处理已复制的链接。".into()
            },
            helper_page_url: Some(format!("http://127.0.0.1:{}/download", session.api_port)),
            destination_directory: session.destination_dir.to_string_lossy().into_owned(),
            authorization_reusable: true,
            progress: select_bound_task(&tasks, session.bound_task_id.as_deref())
                .and_then(sniff_progress),
            output: None,
            error_code: None,
        })
    }

    fn stop_inner(&self, session_id: &str) -> Result<RuntimeSnapshot, AppError> {
        let mut guard = self
            .session
            .lock()
            .map_err(|_| AppError::Validation("授权嗅探运行状态已损坏".into()))?;
        let session = guard
            .as_mut()
            .filter(|session| session.id == session_id)
            .ok_or_else(|| AppError::Validation("授权嗅探助手没有运行".into()))?;
        let destination_directory = session.destination_dir.to_string_lossy().into_owned();
        let recovery = self.cleanup(session);
        *guard = None;
        Ok(match recovery {
            Ok(()) => RuntimeSnapshot {
                phase: SniffPhase::CancelledRestored,
                message: "已停止授权助手，恢复原有代理并移除本次会话证书。".into(),
                helper_page_url: None,
                destination_directory,
                authorization_reusable: false,
                progress: None,
                output: None,
                error_code: None,
            },
            Err(error) => restoration_required(error),
        })
    }

    fn cleanup(&self, session: &mut NativeSession) -> Result<(), AppError> {
        let termination = terminate_child(&mut session.child);
        let restoration = restore_from_journal(&session.journal);
        let result = combine_recovery_steps(termination, restoration);
        if result.is_ok() {
            let _ = fs::remove_file(self.journal_path());
            let _ = fs::remove_dir_all(&session.journal.session_dir);
        }
        result
    }
}

fn platform_sniffer_unavailability(target_os: &str) -> Option<SniffConflict> {
    (!matches!(target_os, "macos" | "windows")).then(|| SniffConflict {
        code: "platform_sniffer_unavailable".into(),
        message: "当前系统暂未提供授权嗅探下载。公众号导出和公开视频直链下载仍可正常使用。".into(),
    })
}

impl SnifferRuntime for NativeSnifferRuntime {
    fn preflight(&self) -> Result<RuntimePreflight, SniffConflict> {
        if let Ok(mut guard) = self.session.lock() {
            if let Some(session) = guard.as_mut() {
                if session.child.try_wait().ok().flatten().is_some() {
                    return Err(SniffConflict {
                        code: "recovery_required".into(),
                        message: "连续下载助手已退出，请先恢复网络设置".into(),
                    });
                }
                validate_platform_network_conflicts()?;
                let current =
                    read_network(&session.journal.network.service).map_err(|message| {
                        SniffConflict {
                            code: "network_preflight_failed".into(),
                            message,
                        }
                    })?;
                if current != applied_network_state(&session.journal) {
                    return Err(SniffConflict {
                        code: "network_changed".into(),
                        message:
                            "连续下载期间的网络设置已变化。请先关闭 VPN，再停止并恢复授权助手。"
                                .into(),
                    });
                }
                return Ok(RuntimePreflight {
                    helper_source: helper_source().into(),
                    authorization_reusable: true,
                });
            }
        }
        if self.journal_path().exists() {
            return Err(SniffConflict {
                code: "recovery_required".into(),
                message: "发现上次未完成的授权会话，请先点击恢复网络设置".into(),
            });
        }
        if let Some(conflict) = platform_sniffer_unavailability(std::env::consts::OS) {
            return Err(conflict);
        }
        let helper = self.helper_path()?;
        self.verify_helper(&helper)?;
        validate_platform_prerequisites()?;
        let network = snapshot_network().map_err(|message| SniffConflict {
            code: "network_preflight_failed".into(),
            message,
        })?;
        validate_network(&network)?;
        Ok(RuntimePreflight {
            helper_source: helper_source().into(),
            authorization_reusable: false,
        })
    }

    fn start(&self, request: &RuntimeStartRequest) -> Result<RuntimeSnapshot, AppError> {
        self.start_inner(request)
    }

    fn observe(&self, session_id: &str) -> Result<RuntimeSnapshot, AppError> {
        self.observe_inner(session_id)
    }

    fn stop(&self, session_id: &str) -> Result<RuntimeSnapshot, AppError> {
        self.stop_inner(session_id)
    }

    fn recover(&self) -> Result<SniffRecoveryResult, AppError> {
        if let Ok(mut guard) = self.session.lock() {
            if let Some(session) = guard.as_mut() {
                self.cleanup(session)?;
                *guard = None;
                return Ok(SniffRecoveryResult {
                    recovered: true,
                    message: "已停止授权助手并恢复网络设置".into(),
                });
            }
        }
        let path = self.journal_path();
        if !path.exists() {
            return Ok(SniffRecoveryResult {
                recovered: true,
                message: "没有发现需要恢复的授权嗅探会话".into(),
            });
        }
        let journal: RecoveryJournal = serde_json::from_slice(&fs::read(&path)?)?;
        let termination = terminate_recovered_helper(&journal);
        let restoration = restore_from_journal(&journal);
        combine_recovery_steps(termination, restoration)?;
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(journal.session_dir);
        Ok(SniffRecoveryResult {
            recovered: true,
            message: "已恢复原有代理并移除上次会话证书".into(),
        })
    }
}

impl Drop for NativeSnifferRuntime {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.session.lock() {
            if let Some(session) = guard.as_mut() {
                let _ = self.cleanup(session);
            }
        }
    }
}

fn rollback_startup(
    journal_path: &Path,
    journal: &RecoveryJournal,
    child: Option<&mut Child>,
) -> Result<(), AppError> {
    let termination = child.map_or(Ok(()), terminate_child);
    let restoration = restore_from_journal(journal);
    let result = combine_recovery_steps(termination, restoration);
    if result.is_ok() {
        let _ = fs::remove_file(journal_path);
        let _ = fs::remove_dir_all(&journal.session_dir);
    }
    result
}

fn terminate_child(child: &mut Child) -> Result<(), AppError> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    if let Err(error) = child.kill() {
        if child.try_wait()?.is_none() {
            return Err(AppError::Content(format!("授权嗅探助手没有停止：{error}")));
        }
        return Ok(());
    }
    child.wait()?;
    Ok(())
}

fn combine_recovery_steps(
    termination: Result<(), AppError>,
    restoration: Result<(), AppError>,
) -> Result<(), AppError> {
    match (termination, restoration) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(termination), Ok(())) => Err(termination),
        (Ok(()), Err(restoration)) => Err(restoration),
        (Err(termination), Err(restoration)) => Err(AppError::Content(format!(
            "授权助手没有停止：{termination}；同时网络和证书恢复失败：{restoration}"
        ))),
    }
}

fn combine_startup_and_recovery_error(error: AppError, recovery: Result<(), AppError>) -> AppError {
    match recovery {
        Ok(()) => error,
        Err(recovery_error) => AppError::Content(format!(
            "{error}；同时未能确认网络和证书恢复：{recovery_error}"
        )),
    }
}

fn helper_config(
    destination: &Path,
    certificate: &Path,
    key: &Path,
    certificate_name: &str,
    api_port: u16,
    proxy_port: u16,
    upstream_proxy: Option<&str>,
    guide_script: &Path,
    quality_mode: SniffQualityMode,
) -> String {
    format!(
        "debug:\n  error: false\n  echolog: false\npagespy:\n  enabled: false\ninject:\n  globalScript: {}\ndownload:\n  defaultHighest: {}\n  dir: {}\n  playDoneAudio: false\n  frontend: false\n  remoteServer:\n    enabled: false\napi:\n  protocol: http\n  hostname: 127.0.0.1\n  port: {api_port}\nupdate:\n  proxy: \"\"\n  mirror: \"\"\nproxy:\n  system: false\n  hostname: 127.0.0.1\n  port: {proxy_port}\n  tcpRelay:\n    enabled: false\n  tun: false\n  skipInstallRootCert: true\n  upstreamProxy: {}\ncert:\n  file: {}\n  key: {}\n  name: {}\nmp:\n  enabled: false\ncloudflare:\n  accountId: \"\"\n  apiToken: \"\"\n  sphCookie: \"\"\n",
        yaml_string(guide_script),
        matches!(quality_mode, SniffQualityMode::Original),
        yaml_string(destination),
        yaml_string(Path::new(upstream_proxy.unwrap_or(""))),
        yaml_string(certificate),
        yaml_string(key),
        yaml_scalar(certificate_name),
    )
}

fn yaml_string(path: &Path) -> String {
    yaml_scalar(&path.to_string_lossy())
}

fn yaml_scalar(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn free_loopback_port() -> Result<u16, AppError> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

#[cfg(unix)]
fn discard_upstream_log(session_dir: &Path) -> Result<(), AppError> {
    std::os::unix::fs::symlink("/dev/null", session_dir.join("app.log"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn discard_upstream_log(session_dir: &Path) -> Result<(), AppError> {
    let log = session_dir.join("app.log");
    fs::write(&log, [])?;
    set_private_file(&log)?;
    Ok(())
}

#[cfg(all(not(unix), not(target_os = "windows")))]
fn discard_upstream_log(_session_dir: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn generate_session_certificate(name: &str, cert: &Path, key: &Path) -> Result<(), AppError> {
    let status = Command::new("/usr/bin/openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:2048", "-sha256", "-nodes", "-days", "1",
        ])
        .arg("-subj")
        .arg(format!("/CN={name}"))
        .arg("-keyout")
        .arg(key)
        .arg("-out")
        .arg(cert)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(AppError::Content("无法生成本次会话的临时证书".into()));
    }
    set_private_file(key)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn certificate_fingerprint(cert: &Path) -> Result<String, AppError> {
    let output = Command::new("/usr/bin/openssl")
        .args(["x509", "-in"])
        .arg(cert)
        .args(["-noout", "-fingerprint", "-sha1"])
        .output()?;
    if !output.status.success() {
        return Err(AppError::Content("无法读取临时证书指纹".into()));
    }
    String::from_utf8_lossy(&output.stdout)
        .split_once('=')
        .map(|(_, value)| value.trim().replace(':', ""))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Content("临时证书指纹格式无效".into()))
}

#[cfg(target_os = "macos")]
fn default_user_keychain() -> Result<PathBuf, AppError> {
    let output = Command::new("/usr/bin/security")
        .args(["default-keychain", "-d", "user"])
        .output()?;
    if !output.status.success() {
        return Err(AppError::Content("无法读取当前用户钥匙串".into()));
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('"')
        .to_string();
    if value.is_empty() {
        return Err(AppError::Content("当前用户钥匙串路径为空".into()));
    }
    Ok(PathBuf::from(value))
}

#[cfg(target_os = "macos")]
fn install_certificate(keychain: &Path, cert: &Path) -> Result<(), AppError> {
    let output = Command::new("/usr/bin/security")
        .args(["add-trusted-cert", "-r", "trustRoot", "-k"])
        .arg(keychain)
        .arg(cert)
        .output()?;
    if !output.status.success() {
        return Err(AppError::Content(format!(
            "临时证书没有获得系统授权：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_certificate(journal: &RecoveryJournal) -> Result<(), AppError> {
    let found = Command::new("/usr/bin/security")
        .args(["find-certificate", "-Z", "-c", &journal.certificate_name])
        .arg(&journal.keychain)
        .output()?;
    if !found.status.success() {
        let stderr = String::from_utf8_lossy(&found.stderr);
        let not_found = found.status.code() == Some(44)
            || stderr.contains("could not be found in the keychain");
        if not_found {
            return Ok(());
        }
        return Err(AppError::Content(format!(
            "无法确认本次授权会话证书是否已移除：{}",
            stderr.trim()
        )));
    }
    if !String::from_utf8_lossy(&found.stdout).contains(&journal.certificate_fingerprint) {
        return Ok(());
    }
    let output = Command::new("/usr/bin/security")
        .args(["delete-certificate", "-Z", &journal.certificate_fingerprint])
        .arg(&journal.keychain)
        .output()?;
    if !output.status.success() {
        return Err(AppError::Content("未能移除本次授权会话证书".into()));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn snapshot_network() -> Result<NetworkSnapshot, String> {
    let service = active_network_service()?;
    read_network(&service)
}

#[cfg(target_os = "macos")]
fn read_network(service: &str) -> Result<NetworkSnapshot, String> {
    Ok(NetworkSnapshot {
        web: read_proxy(service, false)?,
        secure_web: read_proxy(service, true)?,
        auto_proxy: read_auto_proxy(service)?,
        service: service.to_string(),
        windows: None,
    })
}

fn validate_network(snapshot: &NetworkSnapshot) -> Result<(), SniffConflict> {
    if snapshot.auto_proxy.enabled {
        return Err(SniffConflict {
            code: "active_pac".into(),
            message: "当前网络正在使用自动代理脚本，首版无法保证安全恢复；请稍后重试或打开微信原文"
                .into(),
        });
    }
    if snapshot.web.enabled
        && snapshot.secure_web.enabled
        && (snapshot.web.server != snapshot.secure_web.server
            || snapshot.web.port != snapshot.secure_web.port)
    {
        return Err(SniffConflict {
            code: "different_proxies".into(),
            message: "当前 HTTP 与 HTTPS 代理不同，首版无法安全串接；请稍后重试或打开微信原文"
                .into(),
        });
    }
    if snapshot.web.enabled || snapshot.secure_web.enabled {
        return Err(SniffConflict {
            code: "vpn_or_proxy_enabled".into(),
            message: "检测到 VPN 或系统代理正在使用。请先关闭 VPN/代理，再重新点击“授权嗅探下载”；讯栖不会覆盖现有网络设置。"
                .into(),
        });
    }
    Ok(())
}

fn compatible_upstream_proxy(snapshot: &NetworkSnapshot) -> Option<String> {
    let proxy = if snapshot.secure_web.enabled {
        &snapshot.secure_web
    } else if snapshot.web.enabled {
        &snapshot.web
    } else {
        return None;
    };
    Some(format!("http://{}:{}", proxy.server, proxy.port))
}

#[cfg(target_os = "macos")]
fn active_network_service() -> Result<String, String> {
    let nwi = Command::new("/usr/sbin/scutil")
        .arg("--nwi")
        .output()
        .map_err(|error| format!("无法读取活动网络接口：{error}"))?;
    let text = String::from_utf8_lossy(&nwi.stdout);
    let interface = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("Network interfaces:"))
        .and_then(|value| value.split_whitespace().next())
        .ok_or_else(|| "没有找到当前活动网络接口".to_string())?;
    let ports = Command::new("/usr/sbin/networksetup")
        .arg("-listallhardwareports")
        .output()
        .map_err(|error| format!("无法读取网络服务：{error}"))?;
    let mut current_port = None::<String>;
    for line in String::from_utf8_lossy(&ports.stdout).lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("Hardware Port: ") {
            current_port = Some(value.to_string());
        } else if line.strip_prefix("Device: ") == Some(interface) {
            if let Some(port) = current_port.take() {
                return Ok(port);
            }
        }
    }
    Err("没有找到活动接口对应的网络服务".into())
}

#[cfg(target_os = "macos")]
fn read_proxy(service: &str, secure: bool) -> Result<ProxyState, String> {
    let command = if secure {
        "-getsecurewebproxy"
    } else {
        "-getwebproxy"
    };
    let output = Command::new("/usr/sbin/networksetup")
        .args([command, service])
        .output()
        .map_err(|error| format!("无法读取系统代理：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "无法读取系统代理：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut state = ProxyState {
        enabled: false,
        server: String::new(),
        port: 0,
    };
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some((key, value)) = line.split_once(':') {
            match key.trim() {
                "Enabled" => state.enabled = value.trim().eq_ignore_ascii_case("Yes"),
                "Server" => state.server = value.trim().to_string(),
                "Port" => state.port = value.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    Ok(state)
}

#[cfg(target_os = "macos")]
fn read_auto_proxy(service: &str) -> Result<AutoProxyState, String> {
    let output = Command::new("/usr/sbin/networksetup")
        .args(["-getautoproxyurl", service])
        .output()
        .map_err(|error| format!("无法读取自动代理设置：{error}"))?;
    let mut state = AutoProxyState {
        enabled: false,
        url: String::new(),
    };
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(value) = line.trim().strip_prefix("URL: ") {
            state.url = value.to_string();
        } else if line.trim().eq_ignore_ascii_case("Enabled: Yes") {
            state.enabled = true;
        }
    }
    Ok(state)
}

fn restore_from_journal(journal: &RecoveryJournal) -> Result<(), AppError> {
    let current = read_network(&journal.network.service).map_err(AppError::Content)?;
    let applied = applied_network_state(journal);
    if !network_state_is_owned(&current, &journal.network, &applied) {
        return Err(AppError::Content(
            "代理设置已被其他程序修改，为避免覆盖新设置，请先退出代理软件后点“立即恢复”".into(),
        ));
    }
    if current != journal.network {
        apply_network_state(&journal.network)?;
    }
    remove_certificate(journal)?;
    Ok(())
}

fn applied_network_state(journal: &RecoveryJournal) -> NetworkSnapshot {
    if journal.applied_pac_url.is_empty() {
        session_network_state(&journal.network, journal.proxy_port)
    } else {
        let mut applied = journal.network.clone();
        applied.auto_proxy = AutoProxyState {
            enabled: true,
            url: journal.applied_pac_url.clone(),
        };
        if let Some(windows) = applied.windows.as_mut() {
            windows.auto_config_url = Some(journal.applied_pac_url.clone());
        }
        applied
    }
}

fn session_network_state(original: &NetworkSnapshot, proxy_port: u16) -> NetworkSnapshot {
    let helper = session_proxy_state(proxy_port);
    NetworkSnapshot {
        service: original.service.clone(),
        web: helper.clone(),
        secure_web: helper,
        auto_proxy: AutoProxyState {
            enabled: false,
            url: original.auto_proxy.url.clone(),
        },
        windows: session_windows_proxy_snapshot(proxy_port),
    }
}

#[cfg(target_os = "windows")]
fn session_windows_proxy_snapshot(proxy_port: u16) -> Option<WindowsProxySnapshot> {
    Some(WindowsProxySnapshot {
        proxy_enable: Some(1),
        proxy_server: Some(format!("127.0.0.1:{proxy_port}")),
        auto_config_url: None,
    })
}

#[cfg(not(target_os = "windows"))]
fn session_windows_proxy_snapshot(_proxy_port: u16) -> Option<WindowsProxySnapshot> {
    None
}

#[cfg(target_os = "macos")]
fn session_proxy_state(proxy_port: u16) -> ProxyState {
    ProxyState {
        enabled: true,
        server: "127.0.0.1".into(),
        port: proxy_port,
    }
}

#[cfg(target_os = "windows")]
fn session_proxy_state(proxy_port: u16) -> ProxyState {
    ProxyState {
        enabled: true,
        server: format!("127.0.0.1:{proxy_port}"),
        port: proxy_port,
    }
}

fn network_state_is_owned(
    current: &NetworkSnapshot,
    original: &NetworkSnapshot,
    applied: &NetworkSnapshot,
) -> bool {
    if current.service != original.service {
        return false;
    }
    match (
        current.windows.as_ref(),
        original.windows.as_ref(),
        applied.windows.as_ref(),
    ) {
        (Some(current), Some(original), Some(applied)) => {
            optional_value_is_owned(
                &current.proxy_enable,
                &original.proxy_enable,
                &applied.proxy_enable,
            ) && optional_value_is_owned(
                &current.proxy_server,
                &original.proxy_server,
                &applied.proxy_server,
            ) && optional_value_is_owned(
                &current.auto_config_url,
                &original.auto_config_url,
                &applied.auto_config_url,
            )
        }
        (None, None, None) => {
            (current.web == original.web || current.web == applied.web)
                && (current.secure_web == original.secure_web
                    || current.secure_web == applied.secure_web)
                && (current.auto_proxy == original.auto_proxy
                    || current.auto_proxy == applied.auto_proxy)
        }
        _ => false,
    }
}

fn optional_value_is_owned<T: PartialEq>(current: &T, original: &T, applied: &T) -> bool {
    current == original || current == applied
}

#[cfg(target_os = "macos")]
fn apply_network_state(state: &NetworkSnapshot) -> Result<(), AppError> {
    apply_auto_proxy(&state.service, &state.auto_proxy)?;
    apply_proxy(&state.service, false, &state.web)?;
    apply_proxy(&state.service, true, &state.secure_web)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_proxy(service: &str, secure: bool, state: &ProxyState) -> Result<(), AppError> {
    let set_proxy = if secure {
        "-setsecurewebproxy"
    } else {
        "-setwebproxy"
    };
    let set_state = if secure {
        "-setsecurewebproxystate"
    } else {
        "-setwebproxystate"
    };
    if !state.server.is_empty() && state.port != 0 {
        let status = Command::new("/usr/sbin/networksetup")
            .args([set_proxy, service, &state.server, &state.port.to_string()])
            .status()?;
        if !status.success() {
            return Err(AppError::Content("未能设置授权助手的网页代理".into()));
        }
    }
    let status = Command::new("/usr/sbin/networksetup")
        .args([set_state, service, if state.enabled { "on" } else { "off" }])
        .status()?;
    if !status.success() {
        return Err(AppError::Content("未能恢复网页代理开关".into()));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_auto_proxy(service: &str, state: &AutoProxyState) -> Result<(), AppError> {
    if !state.url.is_empty() {
        let status = Command::new("/usr/sbin/networksetup")
            .args(["-setautoproxyurl", service, &state.url])
            .status()?;
        if !status.success() {
            return Err(AppError::Content("未能设置授权助手的域名路由".into()));
        }
    }
    let status = Command::new("/usr/sbin/networksetup")
        .args([
            "-setautoproxystate",
            service,
            if state.enabled { "on" } else { "off" },
        ])
        .status()?;
    if !status.success() {
        return Err(AppError::Content("未能恢复自动代理开关".into()));
    }
    Ok(())
}

fn write_journal(path: &Path, journal: &RecoveryJournal) -> Result<(), AppError> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec(journal)?)?;
    set_private_file(&temporary)?;
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(unix)]
fn set_private_file(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(unix)]
fn set_private_directory(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn wait_for_helper(api_port: u16, child: &mut Child) -> Result<(), AppError> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(1))
        .no_proxy()
        .build()
        .map_err(|error| AppError::Content(format!("无法创建本地助手连接：{error}")))?;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Err(AppError::Content(format!(
                "授权嗅探组件在启动时退出：{status}"
            )));
        }
        if client
            .get(format!("http://127.0.0.1:{api_port}/api/status"))
            .send()
            .is_ok_and(|response| response.status().is_success())
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(200));
    }
    Err(AppError::Content("授权嗅探组件启动超时".into()))
}

fn fetch_tasks(api_port: u16) -> Result<Vec<Value>, AppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
        .map_err(|error| AppError::Content(format!("无法创建本地助手连接：{error}")))?;
    let response = client
        .get(format!(
            "http://127.0.0.1:{api_port}/api/task/list?page=1&page_size=20"
        ))
        .send()
        .map_err(|error| AppError::Content(format!("无法读取授权助手状态：{error}")))?;
    let value: Value = serde_json::from_str(
        &response
            .text()
            .map_err(|error| AppError::Content(format!("无法读取授权助手响应：{error}")))?,
    )?;
    Ok(value["data"]["list"]
        .as_array()
        .cloned()
        .unwrap_or_default())
}

fn submit_share_link_task(api_port: u16, share_url: &str) -> Result<ShareLinkSubmission, AppError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(12))
        .no_proxy()
        .build()
        .map_err(|error| AppError::Content(format!("无法创建本地助手连接：{error}")))?;
    let payload = serde_json::to_vec(&serde_json::json!({
        "url": share_url,
        "spec": "",
        "mp3": false,
        "cover": false,
    }))?;
    let response = client
        .post(format!(
            "http://127.0.0.1:{api_port}/api/task/create_channels"
        ))
        .header("content-type", "application/json")
        .body(payload)
        .send()
        .map_err(|error| AppError::Content(format!("本地助手尚未连接视频号页面：{error}")))?;
    let value: Value = serde_json::from_str(
        &response
            .text()
            .map_err(|error| AppError::Content(format!("无法读取本地助手响应：{error}")))?,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Ok(ShareLinkSubmission::Waiting);
    }
    let task_id = value["data"]["id"]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Content("本地助手没有返回下载任务编号".into()))?;
    Ok(ShareLinkSubmission::Created(task_id.to_owned()))
}

fn select_bound_task<'a>(tasks: &'a [Value], bound_task_id: Option<&str>) -> Option<&'a Value> {
    let bound_task_id = bound_task_id?;
    tasks
        .iter()
        .find(|task| task["id"].as_str() == Some(bound_task_id))
}

fn sniff_progress(task: &Value) -> Option<SniffProgress> {
    let downloaded_bytes = task["progress"]["downloaded"].as_u64()?;
    let bytes_per_second = task["progress"]["speed"].as_u64().unwrap_or_default();
    let total_bytes = task["meta"]["res"]["size"]
        .as_u64()
        .filter(|value| *value > 0)
        .or_else(|| {
            let total = task["meta"]["res"]["files"]
                .as_array()?
                .iter()
                .filter_map(|file| file["size"].as_u64())
                .sum::<u64>();
            (total > 0).then_some(total)
        });
    let percent = total_bytes
        .map(|total| ((downloaded_bytes.saturating_mul(100) / total.max(1)).min(100)) as u8);
    Some(SniffProgress {
        downloaded_bytes,
        total_bytes,
        bytes_per_second,
        percent,
    })
}

fn normalized_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn completed_output(
    task: &Value,
    session: &NativeSession,
) -> Result<SniffOutput, CompletedOutputError> {
    completed_output_from_paths(
        task,
        &session.staging_dir,
        &session.destination_dir,
        &session.expected_title,
        &session.id,
    )
}

fn completed_output_from_paths(
    task: &Value,
    staging_dir: &Path,
    destination_dir: &Path,
    expected_title: &str,
    session_id: &str,
) -> Result<SniffOutput, CompletedOutputError> {
    let path = task["meta"]["opts"]["path"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| CompletedOutputError::Pending("助手没有返回视频临时路径".into()))?;
    let name = task["name"].as_str().unwrap_or_default();
    if name.is_empty() {
        return Err(CompletedOutputError::Pending(
            "助手没有返回视频文件名".into(),
        ));
    }
    let candidate = path.join(name);
    let staging_root = staging_dir.canonicalize().map_err(|error| {
        CompletedOutputError::Export(format!("无法读取讯栖私有临时目录：{error}"))
    })?;
    let canonical = candidate.canonicalize().map_err(|_| {
        CompletedOutputError::Pending("助手报告完成，但临时视频文件还没有就绪".into())
    })?;
    if !canonical.starts_with(&staging_root) {
        return Err(CompletedOutputError::Security(
            "助手返回的文件不在讯栖的私有临时目录中，已拒绝接收".into(),
        ));
    }
    let metadata = fs::metadata(&canonical).map_err(|error| {
        CompletedOutputError::Pending(format!("助手生成的视频文件还不能读取：{error}"))
    })?;
    if metadata.len() < 16 {
        return Err(CompletedOutputError::Pending(
            "助手生成的视频文件为空或仍在写入".into(),
        ));
    }
    if let Some(expected_bytes) = task["meta"]["res"]["size"]
        .as_u64()
        .filter(|value| *value > 0)
    {
        if metadata.len() < expected_bytes {
            return Err(CompletedOutputError::Pending(format!(
                "助手仍在写入视频文件（{} / {} 字节）",
                metadata.len(),
                expected_bytes
            )));
        }
    }
    validate_video_container(&canonical)
        .map_err(|error| CompletedOutputError::Pending(error.to_string()))?;
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("mp4");
    let output_title = preferred_output_title(task, expected_title);
    let filename = safe_output_filename(&output_title, extension);
    let destination = available_destination(destination_dir, &filename);
    copy_verified_output(&canonical, &destination, session_id)
        .map_err(|error| CompletedOutputError::Export(error.to_string()))?;
    let dimensions = iso_media_dimensions(&destination).unwrap_or(None);
    let bytes_written = fs::metadata(&destination)
        .map_err(|error| CompletedOutputError::Export(error.to_string()))?
        .len();
    let _ = fs::remove_file(&canonical);
    let spec = task["meta"]["req"]["labels"]["spec"]
        .as_str()
        .unwrap_or_default()
        .trim();
    Ok(SniffOutput {
        destination: destination.to_string_lossy().into_owned(),
        bytes_written,
        quality_label: if spec.is_empty() {
            "原始画质".into()
        } else {
            format!("节省空间（微信默认规格 {spec}）")
        },
        width: dimensions.map(|value| value.0),
        height: dimensions.map(|value| value.1),
    })
}

fn remove_incomplete_staging_output(task: &Value, staging_dir: &Path) {
    let Some(path) = task["meta"]["opts"]["path"].as_str().map(PathBuf::from) else {
        return;
    };
    let Some(name) = task["name"].as_str().filter(|value| !value.is_empty()) else {
        return;
    };
    let Ok(staging_root) = staging_dir.canonicalize() else {
        return;
    };
    let Ok(candidate) = path.join(name).canonicalize() else {
        return;
    };
    if candidate.starts_with(staging_root) {
        let _ = fs::remove_file(candidate);
    }
}

fn validate_video_container(path: &Path) -> Result<(), AppError> {
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() < 16 {
        return Err(AppError::Content("视频文件头不完整".into()));
    }
    let mut header = vec![0_u8; metadata.len().min(64 * 1024) as usize];
    file.read_exact(&mut header)
        .map_err(|_| AppError::Content("视频文件头不完整".into()))?;
    let iso_media = contains_iso_media_box(&header);
    let webm = header.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]);
    if !iso_media && !webm {
        return Err(AppError::Content(
            "文件不是可识别的 MP4/MOV/WebM 容器，可能仍在解密或已中断".into(),
        ));
    }
    Ok(())
}

fn contains_iso_media_box(header: &[u8]) -> bool {
    let mut offset = 0_usize;
    while offset + 8 <= header.len() {
        let size = u32::from_be_bytes(header[offset..offset + 4].try_into().unwrap()) as u64;
        let kind = &header[offset + 4..offset + 8];
        if matches!(kind, b"ftyp" | b"styp" | b"moov" | b"mdat") {
            return true;
        }
        let (box_size, box_header_size) = if size == 1 {
            if offset + 16 > header.len() {
                return false;
            }
            (
                u64::from_be_bytes(header[offset + 8..offset + 16].try_into().unwrap()),
                16_u64,
            )
        } else {
            (size, 8_u64)
        };
        if box_size == 0 || box_size < box_header_size {
            return false;
        }
        let Ok(next_offset) = usize::try_from(offset as u64 + box_size) else {
            return false;
        };
        if next_offset <= offset || next_offset > header.len() {
            return false;
        }
        offset = next_offset;
    }
    false
}

fn iso_media_dimensions(path: &Path) -> Result<Option<(u32, u32)>, AppError> {
    let mut file = fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    let Some((moov_start, moov_end)) = find_iso_box(&mut file, 0, file_len, b"moov")? else {
        return Ok(None);
    };
    let mut offset = moov_start;
    while let Some((trak_start, trak_end)) = find_iso_box(&mut file, offset, moov_end, b"trak")? {
        if let Some((tkhd_start, tkhd_end)) =
            find_iso_box(&mut file, trak_start, trak_end, b"tkhd")?
        {
            let payload_len = tkhd_end.saturating_sub(tkhd_start);
            if payload_len >= 8 {
                file.seek(SeekFrom::Start(tkhd_end - 8))?;
                let mut fixed = [0_u8; 8];
                file.read_exact(&mut fixed)?;
                let width = u32::from_be_bytes(fixed[..4].try_into().unwrap()) >> 16;
                let height = u32::from_be_bytes(fixed[4..].try_into().unwrap()) >> 16;
                if width > 0 && height > 0 {
                    return Ok(Some((width, height)));
                }
            }
        }
        offset = trak_end;
    }
    Ok(None)
}

fn find_iso_box(
    file: &mut fs::File,
    mut offset: u64,
    end: u64,
    target: &[u8; 4],
) -> Result<Option<(u64, u64)>, AppError> {
    while offset + 8 <= end {
        file.seek(SeekFrom::Start(offset))?;
        let mut header = [0_u8; 16];
        file.read_exact(&mut header[..8])?;
        let size32 = u32::from_be_bytes(header[..4].try_into().unwrap()) as u64;
        let kind: [u8; 4] = header[4..8].try_into().unwrap();
        let (size, header_size) = if size32 == 1 {
            file.read_exact(&mut header[8..16])?;
            (
                u64::from_be_bytes(header[8..16].try_into().unwrap()),
                16_u64,
            )
        } else if size32 == 0 {
            (end.saturating_sub(offset), 8_u64)
        } else {
            (size32, 8_u64)
        };
        if size < header_size || offset.saturating_add(size) > end {
            return Ok(None);
        }
        let content_start = offset + header_size;
        let box_end = offset + size;
        if &kind == target {
            return Ok(Some((content_start, box_end)));
        }
        offset = box_end;
    }
    Ok(None)
}

fn safe_output_filename(title: &str, extension: &str) -> String {
    let mut base = title.trim().to_string();
    loop {
        let lower = base.to_ascii_lowercase();
        let Some(suffix) = [".mp4", ".mov", ".m4v", ".webm"]
            .into_iter()
            .find(|suffix| lower.ends_with(suffix))
        else {
            break;
        };
        base.truncate(base.len() - suffix.len());
        base = base.trim_end().to_string();
    }
    let mut cleaned = String::new();
    let mut previous_space = false;
    for character in base.chars() {
        let replacement = if character.is_control()
            || matches!(
                character,
                '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'
            ) {
            ' '
        } else {
            character
        };
        if replacement.is_whitespace() {
            if !previous_space {
                cleaned.push(' ');
            }
            previous_space = true;
        } else {
            cleaned.push(replacement);
            previous_space = false;
        }
        if cleaned.chars().count() >= 120 {
            break;
        }
    }
    let cleaned = cleaned.trim_matches([' ', '.']);
    let stem = if cleaned.is_empty() {
        "微信视频"
    } else {
        cleaned
    };
    let extension = extension.trim_start_matches('.').to_ascii_lowercase();
    format!(
        "{stem}.{}",
        if extension.is_empty() {
            "mp4"
        } else {
            &extension
        }
    )
}

fn preferred_output_title(task: &Value, expected_title: &str) -> String {
    let expected_is_placeholder = normalized_title(expected_title).chars().count() < 8;
    if expected_is_placeholder {
        if let Some(page_title) = task["meta"]["req"]["labels"]["title"]
            .as_str()
            .filter(|title| !title.trim().is_empty())
        {
            return page_title.to_string();
        }
    }
    expected_title.to_string()
}

fn available_destination(directory: &Path, filename: &str) -> PathBuf {
    let candidate = directory.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("微信视频");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("mp4");
    for sequence in 2..10_000 {
        let candidate = directory.join(format!("{stem} ({sequence}).{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!(
        "{stem}-{}.{extension}",
        chrono::Utc::now().timestamp_millis()
    ))
}

fn copy_verified_output(
    source: &Path,
    destination: &Path,
    session_id: &str,
) -> Result<(), AppError> {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("video.mp4");
    let temporary = destination.with_file_name(format!(".{file_name}.{session_id}.part"));
    let result = (|| -> Result<(), AppError> {
        fs::copy(source, &temporary)?;
        fs::File::open(&temporary)?.sync_all()?;
        fs::rename(&temporary, destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn restoration_required(error: AppError) -> RuntimeSnapshot {
    RuntimeSnapshot {
        phase: SniffPhase::RestorationRequired,
        message: format!("授权助手已停止，但网络恢复还没完成：{error}"),
        helper_page_url: None,
        destination_directory: String::new(),
        authorization_reusable: false,
        progress: None,
        output: None,
        error_code: Some("restore_failed".into()),
    }
}

fn failed_restored_or_required(
    recovery: Result<(), AppError>,
    message: &str,
    error_code: &str,
) -> RuntimeSnapshot {
    match recovery {
        Ok(()) => RuntimeSnapshot {
            phase: SniffPhase::FailedRestored,
            message: message.into(),
            helper_page_url: None,
            destination_directory: String::new(),
            authorization_reusable: false,
            progress: None,
            output: None,
            error_code: Some(error_code.into()),
        },
        Err(error) => restoration_required(error),
    }
}

#[cfg(target_os = "macos")]
fn terminate_recovered_helper(journal: &RecoveryJournal) -> Result<(), AppError> {
    let Some(pid) = journal.helper_pid else {
        return Ok(());
    };
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()?;
    if !output.status.success() {
        return Ok(());
    }
    let command = String::from_utf8_lossy(&output.stdout);
    if !is_sniffer_process_command(&command, &journal.session_dir.join("config.yaml")) {
        return Ok(());
    }
    let terminated = unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    if terminated != 0 {
        return Err(AppError::Io(std::io::Error::last_os_error()));
    }
    thread::sleep(Duration::from_millis(250));
    if Command::new("/bin/ps")
        .args(["-p", &pid.to_string()])
        .status()
        .is_ok_and(|status| status.success())
    {
        let killed = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        if killed != 0 {
            return Err(AppError::Io(std::io::Error::last_os_error()));
        }
        thread::sleep(Duration::from_millis(100));
        if Command::new("/bin/ps")
            .args(["-p", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
        {
            return Err(AppError::Content("授权嗅探助手进程仍在运行".into()));
        }
    }
    Ok(())
}

fn is_sniffer_process_command(command: &str, expected_config: &Path) -> bool {
    let is_known_helper =
        command.contains("xunqi-authorized-sniffer") || command.contains("/wx_video_download ");
    let expected_config = expected_config.to_string_lossy();
    is_known_helper && command.contains("--config") && command.contains(expected_config.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        sync::mpsc,
    };
    use tempfile::tempdir;

    #[test]
    fn enables_supported_desktop_sniffer_platforms() {
        assert!(platform_sniffer_unavailability("windows").is_none());
        assert!(platform_sniffer_unavailability("macos").is_none());
        let conflict = platform_sniffer_unavailability("linux").unwrap();
        assert_eq!(conflict.code, "platform_sniffer_unavailable");
        assert!(conflict.message.contains("公众号导出"));
    }

    #[test]
    fn submits_the_saved_share_url_and_binds_the_exact_helper_task() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (request_tx, request_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 2048];
            loop {
                let read = stream.read(&mut chunk).unwrap();
                request.extend_from_slice(&chunk[..read]);
                let Some(headers_end) = request.windows(4).position(|value| value == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers_end = headers_end + 4;
                let headers = String::from_utf8_lossy(&request[..headers_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or_default();
                if request.len() >= headers_end + content_length {
                    break;
                }
            }
            request_tx
                .send(String::from_utf8(request).unwrap())
                .unwrap();
            let body = r#"{"code":0,"msg":"ok","data":{"id":"exact-video-task","path":"/tmp","name":"video.mp4"}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });

        let submission =
            submit_share_link_task(port, "https://weixin.qq.com/sph/the-exact-shared-video")
                .unwrap();

        assert_eq!(
            submission,
            ShareLinkSubmission::Created("exact-video-task".into())
        );
        let request = request_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(request.starts_with("POST /api/task/create_channels HTTP/1.1"));
        assert!(request.contains("\"url\":\"https://weixin.qq.com/sph/the-exact-shared-video\""));
        assert!(request.contains("\"mp3\":false"));
        assert!(request.contains("\"cover\":false"));
        server.join().unwrap();
    }

    #[test]
    fn helper_configuration_never_contains_credentials_and_disables_optional_services() {
        let config = helper_config(
            Path::new("/tmp/output"),
            Path::new("/tmp/cert.pem"),
            Path::new("/tmp/key.pem"),
            "XunQi-session",
            22022,
            22023,
            Some("http://127.0.0.1:7890"),
            Path::new("/tmp/xunqi-guide.js"),
            SniffQualityMode::Original,
        );
        assert!(config.contains("skipInstallRootCert: true"));
        assert!(config.contains("pagespy:\n  enabled: false"));
        assert!(config.contains("mp:\n  enabled: false"));
        assert!(config.contains("remoteServer:\n    enabled: false"));
        assert!(config.contains("tcpRelay:\n    enabled: false"));
        assert!(config.contains("sphCookie: \"\""));
        assert!(config.contains("upstreamProxy: \"http://127.0.0.1:7890\""));
        assert!(config.contains("globalScript: \"/tmp/xunqi-guide.js\""));
        assert!(!config.lines().any(|line| {
            line.trim_start().starts_with("sphCookie:") && line.trim() != "sphCookie: \"\""
        }));
        assert!(!config.contains("Authorization"));
        assert!(HELPER_GUIDE_SCRIPT.contains("按已复制的分享链接自动处理"));
        assert!(!HELPER_GUIDE_SCRIPT.contains("提交 issue"));
    }

    #[test]
    fn quality_mode_controls_the_helpers_real_download_preference() {
        let original = helper_config(
            Path::new("/tmp/output"),
            Path::new("/tmp/cert.pem"),
            Path::new("/tmp/key.pem"),
            "XunQi-session",
            22022,
            22023,
            None,
            Path::new("/tmp/xunqi-guide.js"),
            SniffQualityMode::Original,
        );
        let space_saver = helper_config(
            Path::new("/tmp/output"),
            Path::new("/tmp/cert.pem"),
            Path::new("/tmp/key.pem"),
            "XunQi-session",
            22022,
            22023,
            None,
            Path::new("/tmp/xunqi-guide.js"),
            SniffQualityMode::SpaceSaver,
        );

        assert!(original.contains("defaultHighest: true"));
        assert!(space_saver.contains("defaultHighest: false"));
    }

    #[test]
    fn refuses_to_bind_any_page_task_before_the_exact_helper_id_is_returned() {
        let tasks = vec![serde_json::json!({
            "id": "current-video",
            "status": "running",
            "meta": {
                "req": {
                    "labels": {
                        "title": "终于明白为啥小姐姐扎堆这个馆了"
                    }
                }
            }
        })];

        assert!(select_bound_task(&tasks, None).is_none());
    }

    #[test]
    fn exposes_helper_download_progress_for_the_xunqi_ui() {
        let task = serde_json::json!({
            "progress": {
                "downloaded": 38_797_312,
                "speed": 1_572_864
            },
            "meta": {
                "res": {
                    "size": 104_857_600
                }
            }
        });

        assert_eq!(
            sniff_progress(&task),
            Some(SniffProgress {
                downloaded_bytes: 38_797_312,
                total_bytes: Some(104_857_600),
                bytes_per_second: 1_572_864,
                percent: Some(37),
            })
        );
    }

    #[test]
    fn uses_the_page_title_for_output_when_public_metadata_is_a_placeholder() {
        let task = serde_json::json!({
            "meta": {
                "req": {
                    "labels": {
                        "title": "终于明白为啥小姐姐扎堆这个馆了"
                    }
                }
            }
        });

        assert_eq!(
            preferred_output_title(&task, "TY12"),
            "终于明白为啥小姐姐扎堆这个馆了"
        );
    }

    #[test]
    fn rejects_an_encrypted_or_incomplete_file_before_export() {
        let directory = tempdir().unwrap();
        let invalid = directory.path().join("broken.mp4");
        fs::write(&invalid, [0xd3, 0x80, 0xbb, 0x6f, 0x16, 0x33, 0x8a, 0x09]).unwrap();

        assert!(validate_video_container(&invalid).is_err());
    }

    #[test]
    fn accepts_an_iso_media_container_and_normalizes_duplicate_extensions() {
        let directory = tempdir().unwrap();
        let valid = directory.path().join("video.mp4");
        fs::write(
            &valid,
            [
                0x00, 0x00, 0x00, 0x18, b'f', b't', b'y', b'p', b'i', b's', b'o', b'm', 0x00, 0x00,
                0x02, 0x00,
            ],
        )
        .unwrap();

        validate_video_container(&valid).unwrap();
        assert_eq!(
            safe_output_filename("测试视频.mp4.mp4", "mp4"),
            "测试视频.mp4"
        );
    }

    #[test]
    fn accepts_iso_media_when_a_valid_leading_box_precedes_ftyp() {
        let directory = tempdir().unwrap();
        let valid = directory.path().join("video-with-leading-free-box.mp4");
        fs::write(
            &valid,
            [
                0x00, 0x00, 0x00, 0x08, b'f', b'r', b'e', b'e', 0x00, 0x00, 0x00, 0x18, b'f', b't',
                b'y', b'p', b'i', b's', b'o', b'm', 0x00, 0x00, 0x02, 0x00,
            ],
        )
        .unwrap();

        validate_video_container(&valid).unwrap();
    }

    #[test]
    fn completed_helper_task_waits_for_async_decryption_without_losing_authorization() {
        let directory = tempdir().unwrap();
        let staging = directory.path().join("staging");
        let destination = directory.path().join("output");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(staging.join("encrypted.mp4"), [0xd3; 32]).unwrap();
        let task = serde_json::json!({
            "name": "encrypted.mp4",
            "meta": {
                "opts": { "path": staging },
                "req": { "labels": { "title": "等待解密的视频" } }
            }
        });

        let failure = completed_output_from_paths(
            &task,
            &staging,
            &destination,
            "等待解密的视频",
            "session-test",
        )
        .unwrap_err();

        assert!(matches!(failure, CompletedOutputError::Pending(_)));
        assert_eq!(
            completion_disposition(&failure, Duration::from_secs(1)),
            CompletionDisposition::Wait
        );
        assert_eq!(
            completion_disposition(&failure, OUTPUT_FINALIZATION_TIMEOUT),
            CompletionDisposition::KeepAuthorization
        );
    }

    #[test]
    fn reads_video_dimensions_from_the_completed_iso_media_file() {
        let directory = tempdir().unwrap();
        let video = directory.path().join("dimensions.mp4");
        let mut tkhd_payload = vec![0_u8; 84];
        tkhd_payload[76..80].copy_from_slice(&(1920_u32 << 16).to_be_bytes());
        tkhd_payload[80..84].copy_from_slice(&(1080_u32 << 16).to_be_bytes());
        let tkhd = iso_box(b"tkhd", &tkhd_payload);
        let trak = iso_box(b"trak", &tkhd);
        let moov = iso_box(b"moov", &trak);
        let mut bytes = iso_box(b"ftyp", b"isom\0\0\x02\0");
        bytes.extend_from_slice(&moov);
        fs::write(&video, bytes).unwrap();

        assert_eq!(iso_media_dimensions(&video).unwrap(), Some((1920, 1080)));
    }

    fn iso_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut value = Vec::with_capacity(payload.len() + 8);
        value.extend_from_slice(&((payload.len() + 8) as u32).to_be_bytes());
        value.extend_from_slice(kind);
        value.extend_from_slice(payload);
        value
    }

    #[test]
    fn different_existing_http_and_https_proxies_are_rejected() {
        let conflict = validate_network(&NetworkSnapshot {
            service: "Wi-Fi".into(),
            web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7890,
            },
            secure_web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7891,
            },
            auto_proxy: AutoProxyState {
                enabled: false,
                url: "http://127.0.0.1:33331/commands/pac".into(),
            },
            windows: None,
        })
        .unwrap_err();
        assert_eq!(conflict.code, "different_proxies");
    }

    #[test]
    fn an_existing_vpn_or_proxy_is_rejected_before_authorization() {
        let conflict = validate_network(&NetworkSnapshot {
            service: "Wi-Fi".into(),
            web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            secure_web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            auto_proxy: AutoProxyState {
                enabled: false,
                url: String::new(),
            },
            windows: None,
        })
        .unwrap_err();

        assert_eq!(conflict.code, "vpn_or_proxy_enabled");
        assert!(conflict.message.contains("关闭 VPN"));
    }

    #[test]
    fn authorized_route_matches_the_helpers_required_manual_proxy_shape() {
        let original = NetworkSnapshot {
            service: "Wi-Fi".into(),
            web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            secure_web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            auto_proxy: AutoProxyState {
                enabled: false,
                url: "http://127.0.0.1:33331/commands/pac".into(),
            },
            windows: None,
        };

        let routed = session_network_state(&original, 22023);

        assert_eq!(routed.web, session_proxy_state(22023));
        assert_eq!(routed.secure_web, routed.web);
        assert_eq!(
            routed.auto_proxy,
            AutoProxyState {
                enabled: false,
                url: original.auto_proxy.url.clone(),
            }
        );
        assert_eq!(
            compatible_upstream_proxy(&original).as_deref(),
            Some("http://127.0.0.1:7897")
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn recovery_accepts_only_original_or_partially_applied_network_components() {
        let original = NetworkSnapshot {
            service: "Wi-Fi".into(),
            web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            secure_web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            auto_proxy: AutoProxyState {
                enabled: false,
                url: "http://127.0.0.1:33331/commands/pac".into(),
            },
            windows: None,
        };
        let applied = session_network_state(&original, 22023);
        let mut partial = original.clone();
        partial.web = applied.web.clone();
        assert!(network_state_is_owned(&partial, &original, &applied));

        partial.secure_web.port = 9999;
        assert!(!network_state_is_owned(&partial, &original, &applied));
    }

    #[test]
    fn windows_recovery_accepts_each_partially_written_registry_value() {
        let original = NetworkSnapshot {
            service: r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings".into(),
            web: ProxyState {
                enabled: false,
                server: "existing-proxy".into(),
                port: 0,
            },
            secure_web: ProxyState {
                enabled: false,
                server: "existing-proxy".into(),
                port: 0,
            },
            auto_proxy: AutoProxyState {
                enabled: false,
                url: String::new(),
            },
            windows: Some(WindowsProxySnapshot {
                proxy_enable: Some(0),
                proxy_server: Some("existing-proxy".into()),
                auto_config_url: None,
            }),
        };
        let applied = NetworkSnapshot {
            service: original.service.clone(),
            web: ProxyState {
                enabled: true,
                server: "127.0.0.1:22023".into(),
                port: 22023,
            },
            secure_web: ProxyState {
                enabled: true,
                server: "127.0.0.1:22023".into(),
                port: 22023,
            },
            auto_proxy: AutoProxyState {
                enabled: false,
                url: String::new(),
            },
            windows: Some(WindowsProxySnapshot {
                proxy_enable: Some(1),
                proxy_server: Some("127.0.0.1:22023".into()),
                auto_config_url: None,
            }),
        };
        let mut partial = original.clone();
        partial.windows.as_mut().unwrap().proxy_enable = Some(1);
        partial.web.enabled = true;
        partial.secure_web.enabled = true;
        assert!(network_state_is_owned(&partial, &original, &applied));

        partial.windows.as_mut().unwrap().proxy_server = Some("foreign-proxy".into());
        assert!(!network_state_is_owned(&partial, &original, &applied));
    }

    #[test]
    fn recovery_still_recognizes_legacy_pac_journals() {
        let original = NetworkSnapshot {
            service: "Wi-Fi".into(),
            web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            secure_web: ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 7897,
            },
            auto_proxy: AutoProxyState {
                enabled: false,
                url: "http://127.0.0.1:33331/commands/pac".into(),
            },
            windows: None,
        };
        let journal = RecoveryJournal {
            session_id: "legacy".into(),
            task_id: 1,
            helper_pid: None,
            session_dir: PathBuf::from("/tmp/legacy"),
            certificate_name: "legacy".into(),
            certificate_fingerprint: "legacy".into(),
            keychain: PathBuf::from("/tmp/keychain"),
            proxy_port: 22023,
            applied_pac_url: "http://127.0.0.1:22024/proxy.pac".into(),
            network: original.clone(),
        };

        let applied = applied_network_state(&journal);

        assert_eq!(applied.web, original.web);
        assert_eq!(applied.secure_web, original.secure_web);
        assert_eq!(applied.auto_proxy.url, journal.applied_pac_url);
        assert!(applied.auto_proxy.enabled);
    }

    #[test]
    fn crash_recovery_only_targets_the_known_helper_process() {
        let config = Path::new("/tmp/xunqi-session/config.yaml");
        assert!(is_sniffer_process_command(
            "/Applications/讯栖.app/Contents/MacOS/xunqi-authorized-sniffer --config /tmp/xunqi-session/config.yaml",
            config,
        ));
        assert!(is_sniffer_process_command(
            "/tmp/wx_video_download --config /tmp/xunqi-session/config.yaml",
            config,
        ));
        assert!(!is_sniffer_process_command(
            "/Applications/讯栖.app/Contents/MacOS/xunqi-authorized-sniffer --config /tmp/another-session/config.yaml",
            config,
        ));
        assert!(!is_sniffer_process_command(
            "/Applications/微信.app/Contents/MacOS/WeChat",
            config,
        ));
    }
}
