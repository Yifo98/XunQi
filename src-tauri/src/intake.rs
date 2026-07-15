use chrono::Utc;
use url::Url;

use crate::{
    public_http::is_allowed_public_https_url, store::SqliteStore, AppError, CaptureKind,
    SubmitLinksResult,
};

#[derive(Clone)]
pub struct IntakeModule {
    store: SqliteStore,
}

impl IntakeModule {
    pub(crate) fn new(store: SqliteStore) -> Self {
        Self { store }
    }

    pub fn submit(&self, raw_text: &str) -> Result<SubmitLinksResult, AppError> {
        let candidates = extract_https_links(raw_text);
        let mut tasks = Vec::new();
        let mut duplicate_count = 0;
        let now = Utc::now().to_rfc3339();

        for candidate in candidates {
            let Some((kind, normalized)) = classify_wechat_link(&candidate)? else {
                continue;
            };
            let (source_name, title) = match kind {
                CaptureKind::Article => ("微信公众号", "正在读取公众号文章"),
                CaptureKind::Video => ("视频号", "正在识别视频号内容"),
            };
            let (task, duplicate) =
                self.store
                    .insert_task(kind, source_name, title, &normalized, &now)?;
            duplicate_count += usize::from(duplicate);
            if !tasks
                .iter()
                .any(|current: &crate::CaptureTask| current.id == task.id)
            {
                tasks.push(task);
            }
        }

        if tasks.is_empty() {
            return Err(AppError::Validation(
                "没有找到支持的微信公众号或视频号分享链接；请在微信中点“分享 → 复制链接”".into(),
            ));
        }

        Ok(SubmitLinksResult {
            tasks,
            duplicate_count,
        })
    }
}

fn classify_wechat_link(raw: &str) -> Result<Option<(CaptureKind, String)>, AppError> {
    let mut url = Url::parse(raw).map_err(|_| AppError::Validation("分享链接格式不正确".into()))?;
    if !is_allowed_public_https_url(&url) {
        return Err(AppError::Validation(
            "只接受不含账号凭据的公网 HTTPS 微信分享链接".into(),
        ));
    }
    let hostname = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let kind = if hostname == "mp.weixin.qq.com"
        && (url.path() == "/s" || url.path().starts_with("/s/"))
    {
        Some(CaptureKind::Article)
    } else if (hostname == "channels.weixin.qq.com"
        && url.path().starts_with("/finder-preview/pages/sph"))
        || hostname == "finder.video.qq.com"
        || (hostname == "weixin.qq.com" && url.path().starts_with("/sph/"))
    {
        Some(CaptureKind::Video)
    } else {
        None
    };
    normalize_wechat_url(&mut url);
    Ok(kind.map(|kind| (kind, url.to_string())))
}

fn normalize_wechat_url(url: &mut Url) {
    const REMOVED_QUERY_KEYS: &[&str] = &[
        "ascene",
        "clicktime",
        "enterid",
        "exportkey",
        "from",
        "isappinstalled",
        "key",
        "lang",
        "mpshare",
        "nettype",
        "pass_ticket",
        "scene",
        "sessionid",
        "subscene",
        "version",
        "wx_header",
    ];

    url.set_fragment(None);
    let mut pairs = url
        .query_pairs()
        .filter(|(key, _)| !REMOVED_QUERY_KEYS.contains(&key.to_ascii_lowercase().as_str()))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    pairs.sort();
    url.set_query(None);
    if !pairs.is_empty() {
        let mut query = url.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(&key, &value);
        }
    }
}

fn extract_https_links(raw: &str) -> Vec<String> {
    let mut remaining = raw;
    let mut links = Vec::new();
    while let Some(start) = remaining.find("https://") {
        let candidate = &remaining[start..];
        let end = candidate
            .char_indices()
            .skip(1)
            .find_map(|(index, character)| is_link_terminator(character).then_some(index))
            .unwrap_or(candidate.len());
        let link = candidate[..end]
            .trim_end_matches(['，', '。', '；', '、', '）', '》', '】'])
            .to_owned();
        if !link.is_empty() {
            links.push(link);
        }
        remaining = &candidate[end..];
        if end == 0 {
            break;
        }
    }
    links
}

fn is_link_terminator(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '<' | '>' | '"' | '\'' | '，' | '。' | '；' | '、' | '）' | '》' | '】'
        )
}

#[cfg(test)]
mod tests {
    use super::{classify_wechat_link, extract_https_links};
    use crate::CaptureKind;

    #[test]
    fn extracts_and_classifies_wechat_links_from_shared_text() {
        let links = extract_https_links(
            "文章 https://mp.weixin.qq.com/s/abc，视频 https://weixin.qq.com/sph/demo 。",
        );
        assert_eq!(links.len(), 2);
        assert_eq!(
            classify_wechat_link(&links[0])
                .expect("classify")
                .map(|value| value.0),
            Some(CaptureKind::Article)
        );
        assert_eq!(
            classify_wechat_link(&links[1])
                .expect("classify")
                .map(|value| value.0),
            Some(CaptureKind::Video)
        );
    }

    #[test]
    fn normalizes_tracking_parameters_and_rejects_non_article_mp_pages() {
        let first = classify_wechat_link(
            "https://mp.weixin.qq.com/s?sn=story&mid=1&scene=1&pass_ticket=secret",
        )
        .expect("classify first")
        .expect("article link")
        .1;
        let second = classify_wechat_link(
            "https://mp.weixin.qq.com/s?scene=9&mid=1&sn=story#wechat_redirect",
        )
        .expect("classify second")
        .expect("article link")
        .1;
        assert_eq!(first, second);
        assert!(!first.contains("pass_ticket"));
        assert!(classify_wechat_link("https://mp.weixin.qq.com/")
            .expect("classify home page")
            .is_none());
    }

    #[test]
    fn rejects_internal_channels_pages_that_are_not_share_links() {
        assert!(classify_wechat_link(
            "https://channels.weixin.qq.com/web/pages/home?context_id=internal&entrance_id=1020",
        )
        .expect("classify internal page")
        .is_none());

        assert_eq!(
            classify_wechat_link(
                "https://channels.weixin.qq.com/finder-preview/pages/sph?id=public-share",
            )
            .expect("classify public share")
            .map(|value| value.0),
            Some(CaptureKind::Video),
        );
    }
}
