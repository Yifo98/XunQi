use std::path::Path;

use crate::AppError;

use super::{
    RuntimePreflight, RuntimeSnapshot, RuntimeStartRequest, SniffConflict, SniffRecoveryResult,
    SnifferRuntime,
};

pub struct NativeSnifferRuntime;

impl NativeSnifferRuntime {
    pub fn new(_state_root: impl AsRef<Path>) -> Self {
        Self
    }

    fn unavailable_error() -> AppError {
        AppError::Validation(
            "当前系统暂未提供授权嗅探下载。公众号导出和公开视频直链下载仍可正常使用。".into(),
        )
    }
}

impl SnifferRuntime for NativeSnifferRuntime {
    fn preflight(&self) -> Result<RuntimePreflight, SniffConflict> {
        Err(SniffConflict {
            code: "platform_sniffer_unavailable".into(),
            message: "当前系统暂未提供授权嗅探下载。公众号导出和公开视频直链下载仍可正常使用。"
                .into(),
        })
    }

    fn start(&self, _request: &RuntimeStartRequest) -> Result<RuntimeSnapshot, AppError> {
        Err(Self::unavailable_error())
    }

    fn observe(&self, _session_id: &str) -> Result<RuntimeSnapshot, AppError> {
        Err(Self::unavailable_error())
    }

    fn stop(&self, _session_id: &str) -> Result<RuntimeSnapshot, AppError> {
        Err(Self::unavailable_error())
    }

    fn recover(&self) -> Result<SniffRecoveryResult, AppError> {
        Ok(SniffRecoveryResult {
            recovered: true,
            message: "当前系统没有需要恢复的授权嗅探会话".into(),
        })
    }
}
