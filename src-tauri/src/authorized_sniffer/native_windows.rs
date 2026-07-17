use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    thread,
    time::Duration as StdDuration,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use serde::Deserialize;
use sha1::{Digest, Sha1};
use time::{Duration, OffsetDateTime};
use windows_sys::Win32::Networking::WinInet::{
    InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
};

use crate::AppError;

use super::{
    AutoProxyState, NetworkSnapshot, ProxyState, RecoveryJournal, SniffConflict,
    WindowsProxySnapshot,
};

const INTERNET_SETTINGS_KEY: &str =
    r"HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsProxyValues {
    proxy_enable: Option<i64>,
    proxy_server: Option<String>,
    auto_config_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsVpnValues {
    #[serde(default)]
    active_connections: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsHelperManifest {
    display_source: String,
    binary_sha256: String,
}

fn helper_manifest() -> &'static WindowsHelperManifest {
    static MANIFEST: OnceLock<WindowsHelperManifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/authorized-sniffer-helper-windows.json"
        )))
        .expect("the pinned Windows helper manifest must be valid JSON")
    })
}

pub(super) fn helper_source() -> &'static str {
    &helper_manifest().display_source
}

pub(super) fn helper_candidates(executable_dir: &Path) -> Vec<PathBuf> {
    vec![
        executable_dir.join("xunqi-authorized-sniffer.exe"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("bin/xunqi-authorized-sniffer-x86_64-pc-windows-msvc.exe"),
    ]
}

pub(super) fn helper_hash_matches(actual: &str) -> bool {
    actual == helper_manifest().binary_sha256.as_str()
}

pub(super) fn validate_platform_prerequisites() -> Result<(), SniffConflict> {
    for required in [
        "powershell.exe",
        "certutil.exe",
        "icacls.exe",
        "reg.exe",
        "taskkill.exe",
    ] {
        let available = Command::new("where.exe")
            .arg(required)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !available {
            return Err(SniffConflict {
                code: "system_tool_missing".into(),
                message: format!("Windows 系统缺少授权助手需要的组件：{required}"),
            });
        }
    }
    Ok(())
}

pub(super) fn generate_session_certificate(
    name: &str,
    cert: &Path,
    key: &Path,
) -> Result<(), AppError> {
    let now = OffsetDateTime::now_utc();
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, name);
    let mut params = CertificateParams::default();
    params.distinguished_name = distinguished_name;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
    ];
    params.not_before = now - Duration::minutes(5);
    params.not_after = now + Duration::days(1);
    let key_pair = KeyPair::generate()
        .map_err(|error| AppError::Content(format!("无法生成本次会话的临时密钥：{error}")))?;
    let certificate = params
        .self_signed(&key_pair)
        .map_err(|error| AppError::Content(format!("无法生成本次会话的临时证书：{error}")))?;
    fs::write(cert, certificate.pem())?;
    fs::write(key, key_pair.serialize_pem())?;
    set_private_file(key)?;
    Ok(())
}

pub(super) fn certificate_fingerprint(cert: &Path) -> Result<String, AppError> {
    let pem = fs::read_to_string(cert)?;
    let encoded = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<String>();
    let der = STANDARD
        .decode(encoded)
        .map_err(|error| AppError::Content(format!("临时证书格式无效：{error}")))?;
    let digest = Sha1::digest(der);
    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(""))
}

pub(super) fn default_user_keychain() -> Result<PathBuf, AppError> {
    Ok(PathBuf::from(r"CurrentUser\Root"))
}

pub(super) fn install_certificate(_store: &Path, cert: &Path) -> Result<(), AppError> {
    let output = Command::new("certutil.exe")
        .args(["-user", "-f", "-addstore", "Root"])
        .arg(cert)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(AppError::Content(format!(
            "临时证书没有写入当前用户证书库：{}",
            command_error(&output)
        )));
    }
    Ok(())
}

pub(super) fn remove_certificate(journal: &RecoveryJournal) -> Result<(), AppError> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$thumbprint = $env:XUNQI_CERT_FINGERPRINT.ToUpperInvariant()
$matches = @(Get-ChildItem -Path Cert:\CurrentUser\Root | Where-Object { $_.Thumbprint -eq $thumbprint })
if ($matches.Count -gt 1) {
  throw 'more than one certificate matched the session fingerprint'
}
if ($matches.Count -eq 1) {
  $matches[0] | Remove-Item -Force
}
if (@(Get-ChildItem -Path Cert:\CurrentUser\Root | Where-Object { $_.Thumbprint -eq $thumbprint }).Count -ne 0) {
  throw 'certificate still exists'
}
"#;
    let output = powershell(
        script,
        &[("XUNQI_CERT_FINGERPRINT", &journal.certificate_fingerprint)],
    )?;
    if !output.status.success() {
        return Err(AppError::Content(format!(
            "未能移除本次授权会话证书：{}",
            command_error(&output)
        )));
    }
    Ok(())
}

pub(super) fn snapshot_network() -> Result<NetworkSnapshot, String> {
    reject_active_vpn()?;
    read_network(INTERNET_SETTINGS_KEY)
}

pub(super) fn validate_platform_network_conflicts() -> Result<(), SniffConflict> {
    reject_active_vpn().map_err(|message| SniffConflict {
        code: "vpn_or_tun_enabled".into(),
        message,
    })
}

pub(super) fn read_network(_service: &str) -> Result<NetworkSnapshot, String> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$p = Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
function Read-OptionalValue([string]$name) {
  $property = $p.PSObject.Properties[$name]
  if ($null -eq $property -or $null -eq $property.Value) { return $null }
  return $property.Value
}
[PSCustomObject]@{
  proxyEnable = Read-OptionalValue 'ProxyEnable'
  proxyServer = Read-OptionalValue 'ProxyServer'
  autoConfigUrl = Read-OptionalValue 'AutoConfigURL'
} | ConvertTo-Json -Compress
"#;
    let output = powershell(script, &[]).map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "无法读取 Windows 系统代理：{}",
            command_error(&output)
        ));
    }
    let values: WindowsProxyValues = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Windows 系统代理返回了无效数据：{error}"))?;
    let proxy_server = values.proxy_server.clone().unwrap_or_default();
    let auto_config_url = values.auto_config_url.clone().unwrap_or_default();
    let (server, port) = split_proxy_server(&proxy_server);
    let state = ProxyState {
        enabled: values.proxy_enable.unwrap_or(0) != 0,
        server: if proxy_server.is_empty() {
            server
        } else {
            proxy_server
        },
        port,
    };
    Ok(NetworkSnapshot {
        service: INTERNET_SETTINGS_KEY.into(),
        web: state.clone(),
        secure_web: state,
        auto_proxy: AutoProxyState {
            enabled: !auto_config_url.is_empty(),
            url: auto_config_url,
        },
        windows: Some(WindowsProxySnapshot {
            proxy_enable: values.proxy_enable,
            proxy_server: values.proxy_server,
            auto_config_url: values.auto_config_url,
        }),
    })
}

pub(super) fn apply_network_state(state: &NetworkSnapshot) -> Result<(), AppError> {
    let fallback_proxy_server = proxy_server_value(&state.web);
    let registry = state
        .windows
        .clone()
        .unwrap_or_else(|| WindowsProxySnapshot {
            proxy_enable: Some(if state.web.enabled { 1 } else { 0 }),
            proxy_server: (!fallback_proxy_server.is_empty()).then_some(fallback_proxy_server),
            auto_config_url: (!state.auto_proxy.url.is_empty())
                .then_some(state.auto_proxy.url.clone()),
        });
    let proxy_enable = registry.proxy_enable.unwrap_or(0).to_string();
    let proxy_server = registry.proxy_server.clone().unwrap_or_default();
    let auto_config_url = registry.auto_config_url.clone().unwrap_or_default();
    let script = r#"
$ErrorActionPreference = 'Stop'
$path = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
function Remove-OptionalValue([string]$name) {
  $current = Get-ItemProperty -LiteralPath $path -ErrorAction Stop
  if ($null -ne $current.PSObject.Properties[$name]) {
    Remove-ItemProperty -LiteralPath $path -Name $name -ErrorAction Stop
  }
}
if ($env:XUNQI_PROXY_ENABLE_PRESENT -eq '1') {
  New-ItemProperty -LiteralPath $path -Name ProxyEnable -PropertyType DWord -Value ([int64]$env:XUNQI_PROXY_ENABLE) -Force | Out-Null
} else {
  Remove-OptionalValue 'ProxyEnable'
}
if ($env:XUNQI_PROXY_SERVER_PRESENT -eq '1') {
  New-ItemProperty -LiteralPath $path -Name ProxyServer -PropertyType String -Value $env:XUNQI_PROXY_SERVER -Force | Out-Null
} else {
  Remove-OptionalValue 'ProxyServer'
}
if ($env:XUNQI_AUTO_CONFIG_PRESENT -eq '1') {
  New-ItemProperty -LiteralPath $path -Name AutoConfigURL -PropertyType String -Value $env:XUNQI_AUTO_CONFIG_URL -Force | Out-Null
} else {
  Remove-OptionalValue 'AutoConfigURL'
}
$current = Get-ItemProperty -LiteralPath $path -ErrorAction Stop
function Assert-OptionalValue([string]$name, [string]$expected, [bool]$present) {
  $property = $current.PSObject.Properties[$name]
  if (-not $present) {
    if ($null -ne $property) { throw "$name was not removed" }
    return
  }
  if ($null -eq $property -or [string]$property.Value -cne $expected) {
    throw "$name was not written exactly"
  }
}
Assert-OptionalValue 'ProxyEnable' $env:XUNQI_PROXY_ENABLE ($env:XUNQI_PROXY_ENABLE_PRESENT -eq '1')
Assert-OptionalValue 'ProxyServer' $env:XUNQI_PROXY_SERVER ($env:XUNQI_PROXY_SERVER_PRESENT -eq '1')
Assert-OptionalValue 'AutoConfigURL' $env:XUNQI_AUTO_CONFIG_URL ($env:XUNQI_AUTO_CONFIG_PRESENT -eq '1')
"#;
    let output = powershell(
        script,
        &[
            (
                "XUNQI_PROXY_ENABLE_PRESENT",
                if registry.proxy_enable.is_some() {
                    "1"
                } else {
                    "0"
                },
            ),
            ("XUNQI_PROXY_ENABLE", &proxy_enable),
            (
                "XUNQI_PROXY_SERVER_PRESENT",
                if registry.proxy_server.is_some() {
                    "1"
                } else {
                    "0"
                },
            ),
            ("XUNQI_PROXY_SERVER", &proxy_server),
            (
                "XUNQI_AUTO_CONFIG_PRESENT",
                if registry.auto_config_url.is_some() {
                    "1"
                } else {
                    "0"
                },
            ),
            ("XUNQI_AUTO_CONFIG_URL", &auto_config_url),
        ],
    )?;
    if !output.status.success() {
        return Err(AppError::Content(format!(
            "未能更新 Windows 系统代理：{}",
            command_error(&output)
        )));
    }
    let settings_changed = unsafe {
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        )
    };
    if settings_changed == 0 {
        return Err(AppError::Content(format!(
            "Windows 没有接受代理变更通知：{}",
            std::io::Error::last_os_error()
        )));
    }
    let refreshed = unsafe {
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        )
    };
    if refreshed == 0 {
        return Err(AppError::Content(format!(
            "Windows 没有完成代理刷新：{}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

fn reject_active_vpn() -> Result<(), String> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$names = @()
$checkErrors = @()
if ($null -ne (Get-Command Get-VpnConnection -ErrorAction SilentlyContinue)) {
  try {
    $names += @(Get-VpnConnection -ErrorAction Stop |
      Where-Object { $_.ConnectionStatus -eq 'Connected' } |
      ForEach-Object { $_.Name })
  } catch {
    $checkErrors += 'current-user VPN query failed'
  }
  try {
    $names += @(Get-VpnConnection -AllUserConnection -ErrorAction Stop |
      Where-Object { $_.ConnectionStatus -eq 'Connected' } |
      ForEach-Object { $_.Name })
  } catch {
    $checkErrors += 'all-user VPN query failed'
  }
} else {
  $checkErrors += 'Windows VpnClient command is unavailable'
}
$pattern = '(?i)(vpn|wireguard|wintun|tailscale|zerotier|tap-windows|openvpn|clash|sing-box|v2ray|tun adapter|nordlynx|anyconnect|globalprotect|pangp|cloudflare|warp|fortinet|forticlient|pulse secure|juniper|hamachi|softether|proton|mullvad|surfshark|expressvpn)'
try {
  $names += @(Get-CimInstance Win32_NetworkAdapter -ErrorAction Stop |
    Where-Object {
      $_.NetConnectionStatus -eq 2 -and
      (($_.Name -match $pattern) -or ($_.ServiceName -match $pattern))
    } |
    ForEach-Object { $_.Name })
} catch {
  $checkErrors += 'network-adapter query failed'
}
if ($checkErrors.Count -ne 0) {
  throw 'Windows VPN state could not be confirmed safely'
}
[PSCustomObject]@{
  activeConnections = @($names | Where-Object { $_ } | Sort-Object -Unique)
} | ConvertTo-Json -Compress
"#;
    let output = powershell(script, &[]).map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "无法确认 Windows VPN 状态：{}",
            command_error(&output)
        ));
    }
    let values: WindowsVpnValues = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Windows VPN 检查返回了无效数据：{error}"))?;
    if values.active_connections.is_empty() {
        return Ok(());
    }
    Err(format!(
        "检测到正在运行的 VPN/TUN（{}）。请先关闭 VPN，再重新点击“授权嗅探下载”；讯栖不会覆盖现有网络连接。",
        values.active_connections.join("、")
    ))
}

pub(super) fn terminate_recovered_helper(journal: &RecoveryJournal) -> Result<(), AppError> {
    let Some(pid) = journal.helper_pid else {
        return Ok(());
    };
    let Some(command) = recovered_process_command(pid)? else {
        return Ok(());
    };
    if !super::is_sniffer_process_command(&command, &journal.session_dir.join("config.yaml")) {
        return Ok(());
    }
    let pid_text = pid.to_string();
    let status = Command::new("taskkill.exe")
        .args(["/PID", &pid_text, "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    for _ in 0..20 {
        if recovered_process_command(pid)?.is_none() {
            return Ok(());
        }
        thread::sleep(StdDuration::from_millis(50));
    }
    Err(AppError::Content(if status.success() {
        "Windows 授权嗅探助手进程仍在运行，恢复记录已保留".into()
    } else {
        "Windows 没有停止授权嗅探助手，恢复记录已保留".into()
    }))
}

fn recovered_process_command(pid: u32) -> Result<Option<String>, AppError> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$process = Get-CimInstance Win32_Process -Filter ("ProcessId = " + $env:XUNQI_HELPER_PID)
if ($null -ne $process) { [Console]::Out.Write($process.CommandLine) }
"#;
    let pid_text = pid.to_string();
    let output = powershell(script, &[("XUNQI_HELPER_PID", &pid_text)])?;
    if !output.status.success() {
        return Err(AppError::Content(format!(
            "无法确认 Windows 授权嗅探助手是否已停止：{}",
            command_error(&output)
        )));
    }
    let command = String::from_utf8_lossy(&output.stdout);
    if command.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(command.into_owned()))
}

pub(super) fn set_private_file(path: &Path) -> Result<(), AppError> {
    set_private_acl(path, false)
}

pub(super) fn set_private_directory(path: &Path) -> Result<(), AppError> {
    set_private_acl(path, true)
}

fn set_private_acl(path: &Path, directory: bool) -> Result<(), AppError> {
    let sid_output = powershell(
        "[Console]::Out.Write([Security.Principal.WindowsIdentity]::GetCurrent().User.Value)",
        &[],
    )?;
    if !sid_output.status.success() {
        return Err(AppError::Content(format!(
            "无法确认当前 Windows 用户：{}",
            command_error(&sid_output)
        )));
    }
    let sid = String::from_utf8_lossy(&sid_output.stdout)
        .trim()
        .to_string();
    if !sid.starts_with("S-") {
        return Err(AppError::Content(
            "Windows 返回了无效的当前用户安全标识".into(),
        ));
    }
    let permission = if directory {
        format!("*{sid}:(OI)(CI)(F)")
    } else {
        format!("*{sid}:(F)")
    };
    let output = Command::new("icacls.exe")
        .arg(path)
        .args(["/inheritance:r", "/grant:r", &permission])
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(AppError::Content(format!(
            "无法保护授权会话的临时文件：{}",
            command_error(&output)
        )));
    }
    Ok(())
}

fn powershell(
    script: &str,
    environment: &[(&str, &str)],
) -> Result<std::process::Output, AppError> {
    let mut command = Command::new("powershell.exe");
    let script = format!(
        "$OutputEncoding = [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)\n{script}"
    );
    command
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"])
        .arg(script)
        .stdin(Stdio::null());
    for (key, value) in environment {
        command.env(key, value);
    }
    command.output().map_err(AppError::Io)
}

fn proxy_server_value(state: &ProxyState) -> String {
    if state.server.is_empty() {
        return String::new();
    }
    if state.port != 0
        && !state.server.contains(';')
        && !state.server.contains('=')
        && !state.server.ends_with(&format!(":{}", state.port))
    {
        return format!("{}:{}", state.server, state.port);
    }
    state.server.clone()
}

fn split_proxy_server(value: &str) -> (String, u16) {
    let candidate = value
        .split(';')
        .find_map(|part| {
            let part = part.trim();
            part.strip_prefix("http=")
                .or_else(|| part.strip_prefix("https="))
                .or_else(|| (!part.contains('=')).then_some(part))
        })
        .unwrap_or(value)
        .trim();
    let Some((host, port)) = candidate.rsplit_once(':') else {
        return (value.to_string(), 0);
    };
    let Ok(port) = port.parse::<u16>() else {
        return (value.to_string(), 0);
    };
    (host.trim_matches(['[', ']']).to_string(), port)
}

fn command_error(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        certificate_fingerprint, generate_session_certificate, helper_manifest, proxy_server_value,
        split_proxy_server,
    };
    use crate::authorized_sniffer::native::ProxyState;
    use tempfile::tempdir;

    #[test]
    fn preserves_complex_windows_proxy_values_for_recovery() {
        let value = "http=127.0.0.1:7890;https=127.0.0.1:7890";
        let (host, port) = split_proxy_server(value);
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 7890);
        assert_eq!(
            proxy_server_value(&ProxyState {
                enabled: false,
                server: value.into(),
                port,
            }),
            value
        );
    }

    #[test]
    fn formats_the_owned_loopback_proxy() {
        assert_eq!(
            proxy_server_value(&ProxyState {
                enabled: true,
                server: "127.0.0.1".into(),
                port: 2023,
            }),
            "127.0.0.1:2023"
        );
    }

    #[test]
    fn session_certificate_uses_the_windows_store_thumbprint_shape() {
        let directory = tempdir().unwrap();
        let certificate = directory.path().join("session-ca.pem");
        let key = directory.path().join("session-ca-key.pem");
        generate_session_certificate("XunQi-test", &certificate, &key).unwrap();

        let fingerprint = certificate_fingerprint(&certificate).unwrap();

        assert_eq!(fingerprint.len(), 40);
        assert!(fingerprint
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'A'..=b'F').contains(&value)));
    }

    #[test]
    fn helper_metadata_is_loaded_from_the_shared_pin_manifest() {
        let manifest = helper_manifest();
        assert!(manifest.display_source.contains("wx_channels_download"));
        assert_eq!(manifest.binary_sha256.len(), 64);
    }
}
