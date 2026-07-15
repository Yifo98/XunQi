use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::{AppError, CaptureKind, CaptureTaskDetail};

mod native;

pub use native::NativeSnifferRuntime;

const NOTICE_VERSION: u16 = 4;
const PLAN_TTL_SECONDS: i64 = 5 * 60;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SniffAuthorizationPlan {
    pub plan_id: String,
    pub task_id: i64,
    pub expires_at: String,
    pub can_start: bool,
    pub changes: Vec<SniffSystemChange>,
    pub reuses_authorization: bool,
    pub helper_source: String,
    pub conflict: Option<SniffConflict>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SniffSystemChange {
    TemporaryProxy,
    TemporaryCertificate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SniffConflict {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SniffPhase {
    Starting,
    AwaitingPlayback,
    Capturing,
    Saving,
    Restoring,
    Completed,
    FailedReusable,
    FailedRestored,
    CancelledRestored,
    RestorationRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SniffOutput {
    pub destination: String,
    pub bytes_written: u64,
    pub quality_label: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SniffProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub bytes_per_second: u64,
    pub percent: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SniffSessionSnapshot {
    pub session_id: String,
    pub task_id: i64,
    pub phase: SniffPhase,
    pub message: String,
    pub helper_page_url: Option<String>,
    pub destination_directory: String,
    pub authorization_reusable: bool,
    pub progress: Option<SniffProgress>,
    pub output: Option<SniffOutput>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SniffRecoveryResult {
    pub recovered: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
struct PendingPlan {
    task_id: i64,
    expires_at_epoch: i64,
    notice_version: u16,
    reuses_authorization: bool,
}

#[derive(Default)]
struct CoordinatorState {
    plans: HashMap<String, PendingPlan>,
    active: Option<SniffSessionSnapshot>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePreflight {
    pub helper_source: String,
    pub authorization_reusable: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeStartRequest {
    pub session_id: String,
    pub task_id: i64,
    pub expected_title: String,
    pub share_url: String,
    pub destination_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeSnapshot {
    pub phase: SniffPhase,
    pub message: String,
    pub helper_page_url: Option<String>,
    pub destination_directory: String,
    pub authorization_reusable: bool,
    pub progress: Option<SniffProgress>,
    pub output: Option<SniffOutput>,
    pub error_code: Option<String>,
}

pub(crate) trait SnifferRuntime: Send + Sync {
    fn preflight(&self) -> Result<RuntimePreflight, SniffConflict>;
    fn start(&self, request: &RuntimeStartRequest) -> Result<RuntimeSnapshot, AppError>;
    fn observe(&self, session_id: &str) -> Result<RuntimeSnapshot, AppError>;
    fn stop(&self, session_id: &str) -> Result<RuntimeSnapshot, AppError>;
    fn recover(&self) -> Result<SniffRecoveryResult, AppError>;
}

#[derive(Clone)]
pub struct AuthorizedSniffer {
    runtime: Arc<dyn SnifferRuntime>,
    state: Arc<Mutex<CoordinatorState>>,
}

impl AuthorizedSniffer {
    pub fn native(state_root: impl AsRef<Path>) -> Self {
        Self::with_runtime(Arc::new(NativeSnifferRuntime::new(state_root)))
    }

    fn with_runtime(runtime: Arc<dyn SnifferRuntime>) -> Self {
        Self {
            runtime,
            state: Arc::new(Mutex::new(CoordinatorState::default())),
        }
    }

    pub fn prepare(&self, detail: &CaptureTaskDetail) -> Result<SniffAuthorizationPlan, AppError> {
        validate_eligibility(detail)?;
        let preflight = self.runtime.preflight();
        let helper_source = preflight
            .as_ref()
            .map(|value| value.helper_source.clone())
            .unwrap_or_else(|_| "ltaoo/wx_channels_download v260706".into());
        let authorization_reusable = preflight
            .as_ref()
            .is_ok_and(|value| value.authorization_reusable);
        let changes = if authorization_reusable {
            Vec::new()
        } else {
            system_changes()
        };
        let mut state = self.lock_state()?;
        if let Some(active) = state.active.as_ref() {
            if is_active_phase(active.phase) {
                return Ok(SniffAuthorizationPlan {
                    plan_id: String::new(),
                    task_id: detail.task.id,
                    expires_at: Utc::now().to_rfc3339(),
                    can_start: false,
                    changes: changes.clone(),
                    reuses_authorization: authorization_reusable,
                    helper_source,
                    conflict: Some(SniffConflict {
                        code: "active_session".into(),
                        message: "已有一个授权嗅探任务正在运行，请先停止并恢复网络".into(),
                    }),
                });
            }
        }
        if let Err(conflict) = preflight {
            return Ok(SniffAuthorizationPlan {
                plan_id: String::new(),
                task_id: detail.task.id,
                expires_at: Utc::now().to_rfc3339(),
                can_start: false,
                changes,
                reuses_authorization: authorization_reusable,
                helper_source,
                conflict: Some(conflict),
            });
        }
        let expires_at = Utc::now() + Duration::seconds(PLAN_TTL_SECONDS);
        let plan_id = opaque_id("plan");
        state
            .plans
            .retain(|_, plan| plan.expires_at_epoch > Utc::now().timestamp());
        state.plans.insert(
            plan_id.clone(),
            PendingPlan {
                task_id: detail.task.id,
                expires_at_epoch: expires_at.timestamp(),
                notice_version: NOTICE_VERSION,
                reuses_authorization: authorization_reusable,
            },
        );
        Ok(SniffAuthorizationPlan {
            plan_id,
            task_id: detail.task.id,
            expires_at: expires_at.to_rfc3339(),
            can_start: true,
            changes,
            reuses_authorization: authorization_reusable,
            helper_source,
            conflict: None,
        })
    }

    pub fn start(
        &self,
        detail: &CaptureTaskDetail,
        plan_id: &str,
        destination_dir: impl AsRef<Path>,
    ) -> Result<SniffSessionSnapshot, AppError> {
        validate_eligibility(detail)?;
        let destination_dir = destination_dir.as_ref();
        if !destination_dir.is_dir() {
            return Err(AppError::Validation(
                "请选择一个已经存在的保存文件夹".into(),
            ));
        }
        let session_id = opaque_id("session");
        {
            let mut state = self.lock_state()?;
            if state
                .active
                .as_ref()
                .is_some_and(|session| is_active_phase(session.phase))
            {
                return Err(AppError::Validation(
                    "已有一个授权嗅探任务正在运行，请先停止它".into(),
                ));
            }
            let plan = state
                .plans
                .remove(plan_id)
                .ok_or_else(|| AppError::Validation("授权计划已失效，请重新打开授权说明".into()))?;
            if plan.task_id != detail.task.id
                || plan.expires_at_epoch <= Utc::now().timestamp()
                || plan.notice_version != NOTICE_VERSION
            {
                return Err(AppError::Validation(
                    "授权计划已过期或不属于当前任务".into(),
                ));
            }
            state.active = Some(SniffSessionSnapshot {
                session_id: session_id.clone(),
                task_id: detail.task.id,
                phase: SniffPhase::Starting,
                message: "正在启动隔离助手并准备临时网络设置…".into(),
                helper_page_url: None,
                destination_directory: destination_dir.to_string_lossy().into_owned(),
                authorization_reusable: plan.reuses_authorization,
                progress: None,
                output: None,
                error_code: None,
            });
        }
        let request = RuntimeStartRequest {
            session_id: session_id.clone(),
            task_id: detail.task.id,
            expected_title: detail.task.title.clone(),
            share_url: detail.task.share_url.clone(),
            destination_dir: destination_dir.to_path_buf(),
        };
        let runtime = match self.runtime.start(&request) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let recovered = self.runtime.recover().unwrap_or(SniffRecoveryResult {
                    recovered: false,
                    message: "助手启动失败，且网络恢复状态无法确认".into(),
                });
                RuntimeSnapshot {
                    phase: if recovered.recovered {
                        SniffPhase::FailedRestored
                    } else {
                        SniffPhase::RestorationRequired
                    },
                    message: if recovered.recovered {
                        format!("授权助手没有启动成功：{error}。网络设置已恢复，可稍后重试或打开微信原文。")
                    } else {
                        format!("授权助手没有启动成功：{error}。{}", recovered.message)
                    },
                    helper_page_url: None,
                    destination_directory: destination_dir.to_string_lossy().into_owned(),
                    authorization_reusable: false,
                    progress: None,
                    output: None,
                    error_code: Some("helper_start_failed".into()),
                }
            }
        };
        self.update_active(&session_id, runtime)
    }

    pub fn snapshot(&self, session_id: &str) -> Result<SniffSessionSnapshot, AppError> {
        let current = self.active_session(session_id)?;
        if !is_active_phase(current.phase) {
            return Ok(current);
        }
        let runtime = self.runtime.observe(session_id)?;
        self.update_active(session_id, runtime)
    }

    pub fn stop(&self, session_id: &str) -> Result<SniffSessionSnapshot, AppError> {
        let current = self.active_session(session_id)?;
        let mut runtime = self.runtime.stop(session_id)?;
        if current.phase == SniffPhase::Completed && runtime.output.is_none() {
            runtime.output = current.output;
        }
        self.update_active(session_id, runtime)
    }

    pub fn recover(&self) -> Result<SniffRecoveryResult, AppError> {
        let result = self.runtime.recover()?;
        if result.recovered {
            let mut state = self.lock_state()?;
            if state
                .active
                .as_ref()
                .is_some_and(|session| session.phase == SniffPhase::RestorationRequired)
            {
                state.active = None;
            }
        }
        Ok(result)
    }

    pub fn has_active_task(&self, task_id: i64) -> Result<bool, AppError> {
        Ok(self
            .lock_state()?
            .active
            .as_ref()
            .is_some_and(|session| session.task_id == task_id && is_active_phase(session.phase)))
    }

    fn update_active(
        &self,
        session_id: &str,
        runtime: RuntimeSnapshot,
    ) -> Result<SniffSessionSnapshot, AppError> {
        let mut state = self.lock_state()?;
        let active = state
            .active
            .as_mut()
            .filter(|session| session.session_id == session_id)
            .ok_or_else(|| AppError::Validation("没有找到这次授权嗅探会话".into()))?;
        active.phase = runtime.phase;
        active.message = runtime.message;
        active.helper_page_url = runtime.helper_page_url;
        active.destination_directory = runtime.destination_directory;
        active.authorization_reusable = runtime.authorization_reusable;
        active.progress = runtime.progress;
        active.output = runtime.output;
        active.error_code = runtime.error_code;
        Ok(active.clone())
    }

    fn active_session(&self, session_id: &str) -> Result<SniffSessionSnapshot, AppError> {
        self.lock_state()?
            .active
            .as_ref()
            .filter(|session| session.session_id == session_id)
            .cloned()
            .ok_or_else(|| AppError::Validation("没有找到这次授权嗅探会话".into()))
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, CoordinatorState>, AppError> {
        self.state
            .lock()
            .map_err(|_| AppError::Validation("授权嗅探状态已损坏，请重启讯栖".into()))
    }
}

fn validate_eligibility(detail: &CaptureTaskDetail) -> Result<(), AppError> {
    if detail.task.kind != CaptureKind::Video {
        return Err(AppError::Validation(
            "只有视频号任务可以使用授权嗅探助手".into(),
        ));
    }
    let video = detail
        .video
        .as_ref()
        .ok_or_else(|| AppError::Validation("请先完成公开视频页面识别".into()))?;
    if video
        .candidates
        .iter()
        .any(|candidate| candidate.downloadable)
    {
        return Err(AppError::Validation(
            "当前任务已有公开直链，请直接使用普通下载".into(),
        ));
    }
    Ok(())
}

fn system_changes() -> Vec<SniffSystemChange> {
    vec![
        SniffSystemChange::TemporaryProxy,
        SniffSystemChange::TemporaryCertificate,
    ]
}

fn is_active_phase(phase: SniffPhase) -> bool {
    matches!(
        phase,
        SniffPhase::Starting
            | SniffPhase::AwaitingPlayback
            | SniffPhase::Capturing
            | SniffPhase::Saving
            | SniffPhase::Restoring
    )
}

fn opaque_id(prefix: &str) -> String {
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}-{}-{sequence}", Utc::now().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
    };

    use tempfile::tempdir;

    use super::*;
    use crate::{
        CaptureStatus, CaptureTask, DetectedVideo, DetectedVideoKind, VideoPageInspection,
    };

    #[derive(Default)]
    struct FakeRuntime {
        calls: Mutex<Vec<&'static str>>,
        authorization_reusable: AtomicBool,
    }

    impl FakeRuntime {
        fn calls(&self) -> Vec<&'static str> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl SnifferRuntime for FakeRuntime {
        fn preflight(&self) -> Result<RuntimePreflight, SniffConflict> {
            self.calls.lock().unwrap().push("preflight");
            Ok(RuntimePreflight {
                helper_source: "test-helper".into(),
                authorization_reusable: self.authorization_reusable.load(Ordering::Relaxed),
            })
        }

        fn start(&self, _request: &RuntimeStartRequest) -> Result<RuntimeSnapshot, AppError> {
            self.calls.lock().unwrap().push("start");
            self.authorization_reusable.store(true, Ordering::Relaxed);
            Ok(RuntimeSnapshot {
                phase: SniffPhase::AwaitingPlayback,
                message: "play".into(),
                helper_page_url: Some("http://127.0.0.1:2022/download".into()),
                destination_directory: "/tmp".into(),
                authorization_reusable: true,
                progress: None,
                output: None,
                error_code: None,
            })
        }

        fn observe(&self, _session_id: &str) -> Result<RuntimeSnapshot, AppError> {
            self.calls.lock().unwrap().push("observe");
            Ok(RuntimeSnapshot {
                phase: SniffPhase::Capturing,
                message: "captured".into(),
                helper_page_url: None,
                destination_directory: "/tmp".into(),
                authorization_reusable: true,
                progress: None,
                output: None,
                error_code: None,
            })
        }

        fn stop(&self, _session_id: &str) -> Result<RuntimeSnapshot, AppError> {
            self.calls.lock().unwrap().push("stop_and_restore");
            self.authorization_reusable.store(false, Ordering::Relaxed);
            Ok(RuntimeSnapshot {
                phase: SniffPhase::CancelledRestored,
                message: "restored".into(),
                helper_page_url: None,
                destination_directory: "/tmp".into(),
                authorization_reusable: false,
                progress: None,
                output: None,
                error_code: None,
            })
        }

        fn recover(&self) -> Result<SniffRecoveryResult, AppError> {
            self.calls.lock().unwrap().push("recover");
            self.authorization_reusable.store(false, Ordering::Relaxed);
            Ok(SniffRecoveryResult {
                recovered: true,
                message: "restored".into(),
            })
        }
    }

    #[test]
    fn prepare_is_read_only_and_plan_is_single_use() {
        let runtime = Arc::new(FakeRuntime::default());
        let sniffer = AuthorizedSniffer::with_runtime(runtime.clone());
        let detail = eligible_video(false);
        let directory = tempdir().unwrap();

        let plan = sniffer.prepare(&detail).unwrap();
        assert!(plan.can_start);
        assert_eq!(runtime.calls(), vec!["preflight"]);

        let session = sniffer
            .start(&detail, &plan.plan_id, directory.path())
            .unwrap();
        assert_eq!(session.phase, SniffPhase::AwaitingPlayback);
        assert_eq!(runtime.calls(), vec!["preflight", "start"]);
        let second = sniffer.start(&detail, &plan.plan_id, directory.path());
        assert!(second.is_err());
    }

    #[test]
    fn completed_session_reuses_authorization_for_the_next_video() {
        let runtime = Arc::new(FakeRuntime::default());
        let sniffer = AuthorizedSniffer::with_runtime(runtime);
        let first = eligible_video(false);
        let directory = tempdir().unwrap();
        let first_plan = sniffer.prepare(&first).unwrap();
        let first_session = sniffer
            .start(&first, &first_plan.plan_id, directory.path())
            .unwrap();
        sniffer
            .update_active(
                &first_session.session_id,
                RuntimeSnapshot {
                    phase: SniffPhase::Completed,
                    message: "saved".into(),
                    helper_page_url: None,
                    destination_directory: directory.path().display().to_string(),
                    authorization_reusable: true,
                    progress: None,
                    output: Some(SniffOutput {
                        destination: directory.path().join("first.mp4").display().to_string(),
                        bytes_written: 1024,
                        quality_label: "原始画质".into(),
                        width: Some(1920),
                        height: Some(1080),
                    }),
                    error_code: None,
                },
            )
            .unwrap();

        let mut next = eligible_video(false);
        next.task.id = 10;
        next.task.title = "第二条视频".into();
        next.task.share_url = "https://weixin.qq.com/sph/next".into();
        let next_plan = sniffer.prepare(&next).unwrap();

        assert!(next_plan.can_start);
        assert!(
            next_plan.changes.is_empty(),
            "已授权的连续下载不应再触发证书和代理授权"
        );
    }

    #[test]
    fn public_direct_link_rejects_sniffer_without_touching_runtime() {
        let runtime = Arc::new(FakeRuntime::default());
        let sniffer = AuthorizedSniffer::with_runtime(runtime.clone());
        let error = sniffer.prepare(&eligible_video(true)).unwrap_err();

        assert!(error.to_string().contains("已有公开直链"));
        assert!(runtime.calls().is_empty());
    }

    #[test]
    fn stop_goes_through_runtime_recovery_boundary() {
        let runtime = Arc::new(FakeRuntime::default());
        let sniffer = AuthorizedSniffer::with_runtime(runtime.clone());
        let detail = eligible_video(false);
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path()).unwrap();
        let plan = sniffer.prepare(&detail).unwrap();
        let session = sniffer
            .start(&detail, &plan.plan_id, directory.path())
            .unwrap();

        let stopped = sniffer.stop(&session.session_id).unwrap();

        assert_eq!(stopped.phase, SniffPhase::CancelledRestored);
        assert_eq!(
            runtime.calls(),
            vec!["preflight", "start", "stop_and_restore"]
        );
    }

    #[test]
    fn stopping_reusable_authorization_preserves_the_completed_output() {
        let runtime = Arc::new(FakeRuntime::default());
        let sniffer = AuthorizedSniffer::with_runtime(runtime);
        let detail = eligible_video(false);
        let directory = tempdir().unwrap();
        let plan = sniffer.prepare(&detail).unwrap();
        let session = sniffer
            .start(&detail, &plan.plan_id, directory.path())
            .unwrap();
        let destination = directory.path().join("saved.mp4").display().to_string();
        sniffer
            .update_active(
                &session.session_id,
                RuntimeSnapshot {
                    phase: SniffPhase::Completed,
                    message: "saved".into(),
                    helper_page_url: None,
                    destination_directory: directory.path().display().to_string(),
                    authorization_reusable: true,
                    progress: None,
                    output: Some(SniffOutput {
                        destination: destination.clone(),
                        bytes_written: 1024,
                        quality_label: "原始画质".into(),
                        width: Some(1920),
                        height: Some(1080),
                    }),
                    error_code: None,
                },
            )
            .unwrap();

        let stopped = sniffer.stop(&session.session_id).unwrap();

        assert_eq!(stopped.phase, SniffPhase::CancelledRestored);
        assert_eq!(stopped.output.unwrap().destination, destination);
    }

    fn eligible_video(has_direct_link: bool) -> CaptureTaskDetail {
        CaptureTaskDetail {
            task: CaptureTask {
                id: 9,
                kind: CaptureKind::Video,
                source_name: "测试视频号".into(),
                title: "测试视频".into(),
                author: "".into(),
                published_at: None,
                share_url: "https://weixin.qq.com/sph/test".into(),
                status: CaptureStatus::NeedsAttention,
                status_detail: "no direct media".into(),
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
                completed_path: None,
            },
            article: None,
            video: Some(VideoPageInspection {
                page_title: "测试视频".into(),
                page_url: "https://weixin.qq.com/sph/test".into(),
                source_name: "测试视频号".into(),
                description: String::new(),
                published_at: None,
                cover_image_url: None,
                candidates: if has_direct_link {
                    vec![DetectedVideo {
                        url: "https://example.com/video.mp4".into(),
                        kind: DetectedVideoKind::DirectFile,
                        label: "video".into(),
                        downloadable: true,
                    }]
                } else {
                    Vec::new()
                },
                limitation: "no direct media".into(),
            }),
        }
    }
}
