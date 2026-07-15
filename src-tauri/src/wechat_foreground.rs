use crate::{AppError, WechatChannelsNetworkRefreshResult, WechatForegroundStatus};

#[cfg(target_os = "macos")]
use std::{process::Command, thread, time::Duration};

const LIMITATION: &str =
    "只能识别微信是否位于前台；微信不会把当前文章链接或视频源直接交给第三方软件。";

#[cfg(target_os = "macos")]
pub fn detect_wechat_foreground() -> WechatForegroundStatus {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let (application_name, bundle_identifier) = match workspace.frontmostApplication() {
        Some(application) => (
            application.localizedName().map(|value| value.to_string()),
            application
                .bundleIdentifier()
                .map(|value| value.to_string()),
        ),
        None => (None, None),
    };
    let is_wechat_frontmost = bundle_identifier
        .as_deref()
        .is_some_and(is_wechat_bundle_identifier);

    WechatForegroundStatus {
        is_wechat_frontmost,
        application_name,
        bundle_identifier,
        limitation: LIMITATION.to_owned(),
    }
}

#[cfg(target_os = "macos")]
pub fn refresh_wechat_channels_network() -> Result<WechatChannelsNetworkRefreshResult, AppError> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "pid=,ppid=,command="])
        .output()
        .map_err(|error| AppError::Content(format!("无法检查微信视频号窗口：{error}")))?;
    if !output.status.success() {
        return Err(AppError::Content("无法检查微信视频号窗口".into()));
    }

    let process_list = String::from_utf8_lossy(&output.stdout);
    let process_ids = wechat_channels_view_process_ids(&process_list);
    if process_ids.is_empty() {
        return Ok(WechatChannelsNetworkRefreshResult {
            refreshed: false,
            message:
                "视频号子窗口当前没有打开。请从微信左侧重新进入“视频号”；不用刷新，也不用退出微信。"
                    .into(),
        });
    }

    for process_id in &process_ids {
        unsafe {
            libc::kill(*process_id as i32, libc::SIGTERM);
        }
    }
    thread::sleep(Duration::from_millis(900));

    Ok(WechatChannelsNetworkRefreshResult {
        refreshed: true,
        message: "已重新加载视频号子窗口，微信聊天主程序保持打开。请从微信左侧重新进入“视频号”；讯栖会按已复制的分享链接自动下载。".into(),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn refresh_wechat_channels_network() -> Result<WechatChannelsNetworkRefreshResult, AppError> {
    Ok(WechatChannelsNetworkRefreshResult {
        refreshed: false,
        message: "当前首版只支持在 macOS 上重新加载微信视频号子窗口".into(),
    })
}

fn wechat_channels_view_process_ids(process_list: &str) -> Vec<u32> {
    const WECHAT_CHANNELS_VIEW: &str =
        "/Applications/WeChat.app/Contents/MacOS/WeChatAppEx.app/Contents/MacOS/WeChatAppEx ";

    process_list
        .lines()
        .filter_map(|line| {
            let (process_id, command) = parse_process_line(line)?;
            (command.starts_with(WECHAT_CHANNELS_VIEW)
                && command
                    .split_whitespace()
                    .any(|part| part == "--product-id=1002"))
            .then_some(process_id)
        })
        .collect()
}

fn parse_process_line(line: &str) -> Option<(u32, &str)> {
    let line = line.trim_start();
    let pid_end = line.find(char::is_whitespace)?;
    let process_id = line[..pid_end].parse::<u32>().ok()?;
    let after_pid = line[pid_end..].trim_start();
    let parent_end = after_pid.find(char::is_whitespace)?;
    let command = after_pid[parent_end..].trim_start();
    Some((process_id, command))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn detect_wechat_foreground() -> WechatForegroundStatus {
    WechatForegroundStatus {
        is_wechat_frontmost: false,
        application_name: None,
        bundle_identifier: None,
        limitation: "当前首版只在 macOS 上识别微信前台状态。".to_owned(),
    }
}

fn is_wechat_bundle_identifier(value: &str) -> bool {
    matches!(
        value,
        "com.tencent.xinWeChat" | "com.tencent.WeChat" | "com.tencent.wechat"
    )
}

#[cfg(target_os = "windows")]
pub fn detect_wechat_foreground() -> WechatForegroundStatus {
    let executable_path = windows_foreground_executable();
    let application_name = executable_path.as_deref().and_then(|value| {
        value
            .rsplit(['\\', '/'])
            .next()
            .map(ToOwned::to_owned)
    });
    let is_wechat_frontmost = executable_path
        .as_deref()
        .is_some_and(is_windows_wechat_executable);

    WechatForegroundStatus {
        is_wechat_frontmost,
        application_name,
        bundle_identifier: None,
        limitation: LIMITATION.to_owned(),
    }
}

#[cfg(target_os = "windows")]
fn windows_foreground_executable() -> Option<String> {
    use std::ptr::null_mut;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    };

    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return None;
        }

        let mut process_id = 0_u32;
        if GetWindowThreadProcessId(window, &mut process_id) == 0 || process_id == 0 {
            return None;
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return None;
        }

        let mut buffer = vec![0_u16; 32_768];
        let mut length = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length);
        let _ = CloseHandle(process);
        if queried == 0 || length == 0 {
            return None;
        }

        String::from_utf16(&buffer[..length as usize]).ok()
    }
}

#[cfg(any(target_os = "windows", test))]
fn is_windows_wechat_executable(value: &str) -> bool {
    let executable = value.rsplit(['\\', '/']).next().unwrap_or(value);
    matches!(
        executable.to_ascii_lowercase().as_str(),
        "wechat.exe" | "weixin.exe" | "wechatappex.exe"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        is_wechat_bundle_identifier, is_windows_wechat_executable,
        wechat_channels_view_process_ids,
    };

    #[test]
    fn recognizes_known_macos_wechat_bundle_identifier() {
        assert!(is_wechat_bundle_identifier("com.tencent.xinWeChat"));
        assert!(!is_wechat_bundle_identifier("com.xiaofu.xunqi"));
    }

    #[test]
    fn recognizes_windows_wechat_foreground_processes() {
        assert!(is_windows_wechat_executable(
            r"C:\Program Files\Tencent\WeChat\WeChat.exe"
        ));
        assert!(is_windows_wechat_executable(
            r"C:\Program Files\Tencent\Weixin\Weixin.exe"
        ));
        assert!(is_windows_wechat_executable(
            r"C:\Program Files\Tencent\WeChat\WeChatAppEx.exe"
        ));
        assert!(!is_windows_wechat_executable(
            r"C:\Tools\XunQi\XunQi.exe"
        ));
    }

    #[test]
    fn restarts_only_the_video_channels_view_process() {
        let processes = r#"
95387     1 /Applications/WeChat.app/Contents/MacOS/WeChat --client_version=4066646838
95391 95387 /Applications/WeChat.app/Contents/MacOS/WeChatAppEx.app/Contents/MacOS/WeChatAppEx --product-id=1002
95392 95391 /Applications/WeChat.app/Contents/MacOS/WeChatAppEx.app/Contents/Frameworks/WeChatAppEx Framework.framework/Versions/C/Helpers/WeChatAppEx Helper.app/Contents/MacOS/WeChatAppEx Helper --type=utility --utility-sub-type=network.mojom.NetworkService --product-id=1002
95393 95391 /Applications/WeChat.app/Contents/MacOS/WeChatAppEx.app/Contents/Frameworks/WeChatAppEx Framework.framework/Versions/C/Helpers/WeChatAppEx Helper.app/Contents/MacOS/WeChatAppEx Helper --type=gpu-process --product-id=1002
95400 95387 /Applications/WeChat.app/Contents/MacOS/WeChatAppEx.app/Contents/MacOS/WeChatAppEx --product-id=1001
95401 95387 /bin/zsh -lc rg 'WeChatAppEx Helper.app/Contents/MacOS/WeChatAppEx Helper --type=utility --utility-sub-type=network.mojom.NetworkService --product-id=1002'
"#;

        assert_eq!(wechat_channels_view_process_ids(processes), vec![95391]);
    }
}
