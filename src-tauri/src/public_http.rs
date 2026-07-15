use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
    time::Duration,
};

use reqwest::{
    blocking::{Client, Response},
    header::{ACCEPT, CONTENT_TYPE, LOCATION, ORIGIN, REFERER},
    redirect::Policy,
};
use url::{Host, Url};

const PUBLIC_BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 \
     (KHTML, like Gecko) Version/18.5 Safari/605.1.15";

pub(crate) struct PublicHttpResponse {
    pub response: Response,
    pub final_url: Url,
}

pub(crate) fn get_public_https(url: &str, timeout: Duration) -> Result<PublicHttpResponse, String> {
    let mut current_url = Url::parse(url).map_err(|_| "链接格式不正确".to_owned())?;
    let mut redirect_count = 0;
    loop {
        let client = client_pinned_to_public_addresses(&current_url, timeout)?;
        let response = client
            .get(current_url.clone())
            .send()
            .map_err(|error| error.to_string())?;
        if response.status().is_redirection() {
            if redirect_count >= 3 {
                return Err("重定向次数过多".into());
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "重定向缺少目标地址".to_owned())?;
            current_url = current_url
                .join(location)
                .map_err(|_| "重定向目标地址无效".to_owned())?;
            redirect_count += 1;
            continue;
        }
        if !response.status().is_success() {
            return Err(format!("服务器返回 {}", response.status()));
        }
        return Ok(PublicHttpResponse {
            response,
            final_url: current_url,
        });
    }
}

pub(crate) fn post_public_https_json(
    url: &str,
    body: &str,
    referer: &str,
    timeout: Duration,
) -> Result<PublicHttpResponse, String> {
    let parsed_url = Url::parse(url).map_err(|_| "链接格式不正确".to_owned())?;
    let parsed_referer = Url::parse(referer).map_err(|_| "来源页面格式不正确".to_owned())?;
    if !is_allowed_public_https_url(&parsed_referer)
        || parsed_url.origin() != parsed_referer.origin()
    {
        return Err("公开接口与来源页面必须使用同一 HTTPS 站点".into());
    }
    let origin = parsed_url.origin().ascii_serialization();
    let client = client_pinned_to_public_addresses(&parsed_url, timeout)?;
    let response = client
        .post(parsed_url.clone())
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .header(ORIGIN, origin)
        .header(REFERER, parsed_referer.as_str())
        .body(body.to_owned())
        .send()
        .map_err(|error| error.to_string())?;
    if response.status().is_redirection() {
        return Err("公开接口返回了意外重定向".into());
    }
    if !response.status().is_success() {
        return Err(format!("服务器返回 {}", response.status()));
    }
    Ok(PublicHttpResponse {
        response,
        final_url: parsed_url,
    })
}

pub fn is_allowed_public_https_url(url: &Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    match url.host() {
        Some(Host::Domain(domain)) => is_allowed_domain(domain),
        Some(Host::Ipv4(address)) => is_public_ip(IpAddr::V4(address)),
        Some(Host::Ipv6(address)) => is_public_ip(IpAddr::V6(address)),
        None => false,
    }
}

fn client_pinned_to_public_addresses(url: &Url, timeout: Duration) -> Result<Client, String> {
    if !is_allowed_public_https_url(url) {
        return Err("地址不是允许访问的公网 HTTPS 地址".into());
    }
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(timeout)
        .user_agent(PUBLIC_BROWSER_USER_AGENT)
        .no_proxy()
        .redirect(Policy::none());
    if let Some(Host::Domain(domain)) = url.host() {
        let port = url.port_or_known_default().unwrap_or(443);
        let addresses = (domain, port)
            .to_socket_addrs()
            .map_err(|error| format!("无法解析地址：{error}"))?
            .collect::<Vec<SocketAddr>>();
        if addresses.is_empty()
            || addresses
                .iter()
                .any(|address| !is_safe_domain_resolution(address.ip()))
        {
            return Err("域名解析到了非公网地址，已停止访问".into());
        }
        builder = builder.resolve_to_addrs(domain, &addresses);
    }
    builder.build().map_err(|error| error.to_string())
}

fn is_safe_domain_resolution(address: IpAddr) -> bool {
    is_public_ip(address) || is_synthetic_proxy_dns_ip(address)
}

fn is_synthetic_proxy_dns_ip(address: IpAddr) -> bool {
    matches!(address, IpAddr::V4(address) if {
        let [a, b, _, _] = address.octets();
        a == 198 && (b == 18 || b == 19)
    })
}

fn is_allowed_domain(domain: &str) -> bool {
    let normalized = domain.trim_end_matches('.').to_ascii_lowercase();
    !normalized.is_empty()
        && normalized != "localhost"
        && !normalized.ends_with(".localhost")
        && !normalized.ends_with(".local")
}

fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224)
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    if let Some(mapped) = address.to_ipv4_mapped() {
        return is_public_ipv4(mapped);
    }
    let segments = address.segments();
    !(address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || (segments[0] & 0xe000) != 0x2000
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{is_allowed_public_https_url, is_safe_domain_resolution};

    #[test]
    fn rejects_literal_non_public_addresses() {
        for url in [
            "https://127.0.0.1/video.mp4",
            "https://100.64.0.1/video.mp4",
            "https://198.18.0.43/video.mp4",
            "https://192.0.2.1/video.mp4",
            "https://[::1]/video.mp4",
            "https://[fc00::1]/video.mp4",
            "https://[2001:db8::1]/video.mp4",
            "https://user:password@example.com/video.mp4",
        ] {
            assert!(!is_allowed_public_https_url(
                &url::Url::parse(url).expect("valid URL")
            ));
        }
    }

    #[test]
    fn accepts_public_https_hosts_without_resolving_them() {
        for url in [
            "https://1.1.1.1/video.mp4",
            "https://cdn.example.com/video.mp4",
        ] {
            assert!(is_allowed_public_https_url(
                &url::Url::parse(url).expect("valid URL")
            ));
        }
    }

    #[test]
    fn accepts_proxy_synthetic_dns_for_domain_resolution() {
        assert!(is_safe_domain_resolution(IpAddr::V4(Ipv4Addr::new(
            198, 18, 0, 43
        ))));
        assert!(!is_allowed_public_https_url(
            &url::Url::parse("https://198.18.0.43/private").expect("valid URL")
        ));
    }

    #[test]
    fn still_rejects_private_addresses_for_domain_resolution() {
        for address in [
            Ipv4Addr::LOCALHOST,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(169, 254, 1, 1),
            Ipv4Addr::new(192, 168, 1, 1),
        ] {
            assert!(!is_safe_domain_resolution(IpAddr::V4(address)));
        }
    }
}
