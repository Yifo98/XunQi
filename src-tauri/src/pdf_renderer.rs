use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppError;

pub trait PdfRenderer: Send + Sync {
    fn render(&self, source_html: &Path, destination: &Path) -> Result<u64, AppError>;
}

pub struct NativePdfRenderer;

impl PdfRenderer for NativePdfRenderer {
    fn render(&self, source_html: &Path, destination: &Path) -> Result<u64, AppError> {
        if !source_html.is_file() {
            return Err(AppError::Validation("没有找到待转换的离线文章页面".into()));
        }

        #[cfg(target_os = "windows")]
        {
            return render_with_edge(source_html, destination);
        }

        #[cfg(not(target_os = "windows"))]
        {
            let helper = helper_path()?;
            let mut child = Command::new(&helper)
                .arg(source_html)
                .arg(destination)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|error| {
                    AppError::Content(format!(
                        "无法启动系统 PDF 渲染器（{}）：{error}",
                        helper.display()
                    ))
                })?;
            let deadline = Instant::now() + Duration::from_secs(60);
            loop {
                if let Some(status) = child.try_wait()? {
                    if !status.success() {
                        let mut stderr = String::new();
                        if let Some(mut pipe) = child.stderr.take() {
                            let _ = pipe.read_to_string(&mut stderr);
                        }
                        let detail = stderr.trim();
                        return Err(AppError::Content(if detail.is_empty() {
                            "系统 PDF 渲染器没有完成导出".into()
                        } else {
                            format!("系统 PDF 渲染失败：{detail}")
                        }));
                    }
                    break;
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::Content("PDF 生成超过 60 秒，已安全停止".into()));
                }
                thread::sleep(Duration::from_millis(50));
            }
            validate_pdf(destination)
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn helper_path() -> Result<PathBuf, AppError> {
    if let Some(value) = std::env::var_os("XUNQI_PDF_RENDERER") {
        let candidate = PathBuf::from(value);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    let current_exe = std::env::current_exe()?;
    let executable_dir = current_exe
        .parent()
        .ok_or_else(|| AppError::Validation("无法确定讯栖程序目录".into()))?;
    let mut candidates = vec![
        executable_dir.join("xunqi-pdf-renderer"),
        executable_dir.join("../Resources/xunqi-pdf-renderer"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "bin/xunqi-pdf-renderer-{}-apple-darwin",
            std::env::consts::ARCH
        )),
    ];
    if let Some(source_helper) = option_env!("XUNQI_PDF_RENDERER_BUILD_PATH") {
        candidates.push(PathBuf::from(source_helper));
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| AppError::Validation("PDF 渲染组件缺失，请重新解压完整的讯栖安装包".into()))
}

#[cfg(target_os = "windows")]
fn render_with_edge(source_html: &Path, destination: &Path) -> Result<u64, AppError> {
    let edge = windows_edge_path().ok_or_else(|| {
        AppError::Validation(
            "没有找到 Microsoft Edge，Windows Preview 暂时无法导出 PDF；仍可导出 Markdown".into(),
        )
    })?;
    let source_url = url::Url::from_file_path(source_html)
        .map_err(|_| AppError::Validation("无法生成离线文章的本地地址".into()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let profile =
        std::env::temp_dir().join(format!("xunqi-edge-pdf-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&profile)?;
    let mut child = Command::new(&edge)
        .arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-pdf-header-footer")
        .arg("--allow-file-access-from-files")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--print-to-pdf={}", destination.display()))
        .arg(source_url.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| AppError::Content(format!("无法启动 Microsoft Edge PDF 渲染：{error}")))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let result = loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                break validate_pdf(destination);
            }
            let mut stderr = String::new();
            if let Some(mut pipe) = child.stderr.take() {
                let _ = pipe.read_to_string(&mut stderr);
            }
            let detail = stderr.trim();
            break Err(AppError::Content(if detail.is_empty() {
                "Microsoft Edge 没有完成 PDF 导出".into()
            } else {
                format!("Microsoft Edge PDF 导出失败：{detail}")
            }));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err(AppError::Content("PDF 生成超过 60 秒，已安全停止".into()));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let _ = fs::remove_dir_all(profile);
    result
}

#[cfg(target_os = "windows")]
fn windows_edge_path() -> Option<PathBuf> {
    ["PROGRAMFILES(X86)", "PROGRAMFILES", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .map(|root| {
            root.join("Microsoft")
                .join("Edge")
                .join("Application")
                .join("msedge.exe")
        })
        .find(|candidate| candidate.is_file())
}

fn validate_pdf(path: &Path) -> Result<u64, AppError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() < 16 {
        return Err(AppError::Content("生成的 PDF 文件不完整".into()));
    }
    let mut header = [0_u8; 5];
    fs::File::open(path)?.read_exact(&mut header)?;
    if &header != b"%PDF-" {
        return Err(AppError::Content(
            "系统渲染器返回的不是有效 PDF 文件".into(),
        ));
    }
    Ok(metadata.len())
}
