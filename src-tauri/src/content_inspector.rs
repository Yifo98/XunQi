use std::{collections::HashSet, io::Read, time::Duration};

use chrono::{DateTime, Utc};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use url::Url;

use crate::{
    public_http::{get_public_https, is_allowed_public_https_url, post_public_https_json},
    AppError, ArticleSnapshot, DetectedVideo, DetectedVideoKind, VideoPageInspection,
};

const MAX_HTML_BYTES: u64 = 8 * 1024 * 1024;
const MAX_JSON_BYTES: u64 = 2 * 1024 * 1024;

pub trait ContentInspector: Send + Sync {
    fn inspect_article(&self, url: &str) -> Result<ArticleSnapshot, AppError>;
    fn inspect_video_page(&self, url: &str) -> Result<VideoPageInspection, AppError>;
}

pub struct PublicContentInspector;

impl ContentInspector for PublicContentInspector {
    fn inspect_article(&self, url: &str) -> Result<ArticleSnapshot, AppError> {
        let (final_url, html) = fetch_html(url)?;
        Self::inspect_article_html(final_url.as_str(), &html)
    }

    fn inspect_video_page(&self, url: &str) -> Result<VideoPageInspection, AppError> {
        let (final_url, html) = fetch_html(url)?;
        if let Some(short_uri) = finder_short_uri(&final_url) {
            return Self::inspect_wechat_finder_page(&final_url, &short_uri);
        }
        Self::inspect_video_html(final_url.as_str(), &html)
    }
}

impl PublicContentInspector {
    pub fn inspect_article_html(url: &str, html: &str) -> Result<ArticleSnapshot, AppError> {
        let document = Html::parse_document(html);
        let page_text = normalize_whitespace(&document.root_element().text().collect::<String>());
        if ["环境异常", "访问过于频繁", "请在微信客户端打开"]
            .iter()
            .any(|marker| page_text.contains(marker))
        {
            return Err(AppError::Content(
                "页面要求验证或在微信内打开，未读取任何登录态或 Cookie".into(),
            ));
        }

        if let Some(text_post) = wechat_text_post(html) {
            return text_post_snapshot(url, text_post);
        }

        let title = page_title(&document)
            .ok_or_else(|| AppError::Content("没有在页面中找到文章标题".into()))?;
        let author = first_text(
            &document,
            &["#js_name", ".rich_media_meta_text", "[rel=author]"],
        )
        .or_else(|| meta_content(&document, &["article:author", "author"]))
        .unwrap_or_default();
        let published_at = first_text(&document, &["#publish_time", "time[datetime]"])
            .or_else(|| meta_content(&document, &["article:published_time", "date"]));
        let canonical_url = meta_content(&document, &["og:url"])
            .or_else(|| canonical_link(&document))
            .filter(|candidate| Url::parse(candidate).is_ok())
            .unwrap_or_else(|| url.to_owned());
        let body_markdown = readable_body(&document)?;
        let (body_html, image_urls) = article_body_html(url, &document)?;
        let cover_image_url = meta_content(&document, &["og:image", "twitter:image"])
            .and_then(|candidate| resolve_public_url(url, &candidate));
        let word_count = body_markdown
            .chars()
            .filter(|character| !character.is_whitespace())
            .count();

        Ok(ArticleSnapshot {
            title,
            author,
            published_at,
            canonical_url,
            body_markdown,
            body_html,
            cover_image_url,
            image_urls,
            word_count,
        })
    }

    pub fn inspect_video_html(url: &str, html: &str) -> Result<VideoPageInspection, AppError> {
        let page_url =
            Url::parse(url).map_err(|_| AppError::Content("页面链接格式不正确".into()))?;
        let document = Html::parse_document(html);
        let mut raw_candidates = Vec::new();
        for selector_text in ["video[src]", "video source[src]", "source[src]"] {
            let selector = Selector::parse(selector_text).expect("static selector");
            raw_candidates.extend(
                document
                    .select(&selector)
                    .filter_map(|element| element.value().attr("src"))
                    .map(|value| (value.to_owned(), "HTML5 视频".to_owned())),
            );
        }
        for property in [
            "og:video",
            "og:video:url",
            "og:video:secure_url",
            "twitter:player:stream",
        ] {
            if let Some(value) = meta_content(&document, &[property]) {
                raw_candidates.push((value, "页面视频元数据".into()));
            }
        }
        let link_selector = Selector::parse("a[href]").expect("static selector");
        raw_candidates.extend(document.select(&link_selector).filter_map(|element| {
            let href = element.value().attr("href")?;
            media_extension(href).map(|_| (href.to_owned(), "页面媒体链接".to_owned()))
        }));

        let mut seen = HashSet::new();
        let candidates = raw_candidates
            .into_iter()
            .filter_map(|(raw_url, label)| {
                let resolved = if raw_url.starts_with("blob:") || raw_url.starts_with("data:") {
                    raw_url
                } else {
                    page_url.join(&raw_url).ok()?.to_string()
                };
                seen.insert(resolved.clone())
                    .then(|| detected_video(resolved, label))
            })
            .collect::<Vec<_>>();
        let limitation = if candidates.is_empty() {
            "没有在静态页面中发现媒体地址；视频可能由脚本、登录态、blob/MediaSource 或加密流加载。讯栖不会抓取 Cookie、安装根证书或解密。"
        } else {
            "只允许下载页面公开声明的公网 HTTPS 直接视频；HLS、DASH、blob、登录态和加密媒体只显示识别结果。"
        };
        Ok(VideoPageInspection {
            page_title: page_title(&document).unwrap_or_else(|| "未命名网页".into()),
            page_url: page_url.to_string(),
            source_name: meta_content(&document, &["og:site_name", "author"])
                .unwrap_or_else(|| "视频号".into()),
            description: String::new(),
            published_at: None,
            cover_image_url: None,
            candidates,
            limitation: limitation.into(),
        })
    }

    fn inspect_wechat_finder_page(
        page_url: &Url,
        short_uri: &str,
    ) -> Result<VideoPageInspection, AppError> {
        let endpoint = page_url
            .join("/finder-preview/api/feed/get_feed_info")
            .map_err(|_| AppError::Content("视频号公开接口地址无效".into()))?;
        let short_body = serde_json::to_string(&serde_json::json!({
            "baseReq": { "generalToken": "" },
            "shortUri": short_uri,
        }))?;
        let short_json = fetch_public_json(&endpoint, page_url, &short_body)?;
        let parsed_short = parse_finder_response(&short_json)?;
        let dynamic_export_id = parsed_short
            .data
            .as_ref()
            .and_then(|data| data.scene_info.as_ref())
            .and_then(|scene| scene.dynamic_export_id.as_deref())
            .filter(|value| !value.trim().is_empty());
        let should_check_feed = !finder_response_has_video(&parsed_short);
        let feed_json = if should_check_feed {
            dynamic_export_id
                .map(|export_id| {
                    let body = serde_json::to_string(&serde_json::json!({
                        "baseReq": { "generalToken": "" },
                        "exportId": export_id,
                    }))?;
                    fetch_public_json(&endpoint, page_url, &body)
                })
                .transpose()?
        } else {
            None
        };
        Self::inspect_wechat_finder_json(page_url.as_str(), &short_json, feed_json.as_deref())
    }

    pub fn inspect_wechat_finder_json(
        page_url: &str,
        short_json: &str,
        feed_json: Option<&str>,
    ) -> Result<VideoPageInspection, AppError> {
        let page_url = Url::parse(page_url)
            .map_err(|_| AppError::Content("视频号页面链接格式不正确".into()))?;
        let short = parse_finder_response(short_json)?;
        let feed = feed_json.map(parse_finder_response).transpose()?;
        let short_data = short.data.as_ref();
        let feed_data = feed.as_ref().and_then(|response| response.data.as_ref());

        let source_name = short_data
            .and_then(|data| data.author_info.as_ref())
            .and_then(|author| non_empty_string(author.nickname.as_deref()))
            .or_else(|| {
                feed_data
                    .and_then(|data| data.author_info.as_ref())
                    .and_then(|author| non_empty_string(author.nickname.as_deref()))
            })
            .unwrap_or("视频号")
            .to_owned();
        let description = short_data
            .and_then(|data| data.feed_info.as_ref())
            .and_then(|info| non_empty_string(info.description.as_deref()))
            .or_else(|| {
                feed_data
                    .and_then(|data| data.feed_info.as_ref())
                    .and_then(|info| non_empty_string(info.description.as_deref()))
            })
            .unwrap_or("视频号内容")
            .to_owned();
        let published_at = short_data
            .and_then(|data| data.feed_info.as_ref())
            .and_then(|info| info.create_time)
            .or_else(|| {
                feed_data
                    .and_then(|data| data.feed_info.as_ref())
                    .and_then(|info| info.create_time)
            })
            .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
            .map(|value| value.to_rfc3339());
        let cover_image_url = short_data
            .and_then(|data| data.feed_info.as_ref())
            .and_then(|info| allowed_finder_url(info.cover_url.as_deref()))
            .or_else(|| {
                feed_data
                    .and_then(|data| data.feed_info.as_ref())
                    .and_then(|info| allowed_finder_url(info.cover_url.as_deref()))
            });

        let mut seen = HashSet::new();
        let mut candidates = Vec::new();
        if let Some(info) = short_data.and_then(|data| data.feed_info.as_ref()) {
            collect_finder_video_candidates(info, &mut seen, &mut candidates);
        }
        if let Some(info) = feed_data.and_then(|data| data.feed_info.as_ref()) {
            collect_finder_video_candidates(info, &mut seen, &mut candidates);
        }
        let playback_error = feed_data
            .and_then(|data| data.err_msg.as_ref())
            .and_then(|error| non_empty_string(error.title.as_deref()))
            .or_else(|| {
                short_data
                    .and_then(|data| data.err_msg.as_ref())
                    .and_then(|error| non_empty_string(error.title.as_deref()))
            });
        let limitation = if candidates.is_empty() {
            let reason = playback_error.unwrap_or("微信公开页面当前只提供作者、文案和封面");
            format!(
                "已读取公开视频号信息，但微信公开接口没有提供视频地址；{reason}。讯栖没有读取 Cookie，也不会抓包或解密。"
            )
        } else {
            "已从微信公开页面读取到公开视频地址；可手动下载，下载器仍会校验响应类型和视频文件头。"
                .into()
        };

        Ok(VideoPageInspection {
            page_title: description.clone(),
            page_url: page_url.to_string(),
            source_name,
            description,
            published_at,
            cover_image_url,
            candidates,
            limitation,
        })
    }
}

struct WechatTextPost {
    title: String,
    author: String,
    published_at: Option<String>,
    canonical_url: Option<String>,
    content: String,
}

fn wechat_text_post(source: &str) -> Option<WechatTextPost> {
    if !source.contains("content_noencode") {
        return None;
    }
    let anchor = source.find("content_noencode")?;
    let preceding_start = anchor.saturating_sub(16 * 1024);
    let preceding = &source[preceding_start..anchor];
    let following = &source[anchor..];
    let content = first_js_string_property(following, "content_noencode")?;
    if content.trim().chars().count() < 20 {
        return None;
    }
    let title = last_js_string_property(preceding, "title")
        .or_else(|| first_js_assignment(source, "window.msg_title"))?;
    let author = last_js_string_property(preceding, "nick_name")
        .or_else(|| first_js_assignment(source, "window.nickname"))
        .unwrap_or_default();
    let published_at =
        first_js_string_property(following, "create_time").filter(|value| !value.trim().is_empty());
    let canonical_url = first_js_string_property(following, "link").filter(|candidate| {
        Url::parse(candidate)
            .ok()
            .is_some_and(|url| is_allowed_public_https_url(&url))
    });
    Some(WechatTextPost {
        title,
        author,
        published_at,
        canonical_url,
        content,
    })
}

fn text_post_snapshot(url: &str, text_post: WechatTextPost) -> Result<ArticleSnapshot, AppError> {
    let content_html = text_post
        .content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .map(|paragraph| format!("<p>{paragraph}</p>"))
        .collect::<String>();
    let synthesized = Html::parse_fragment(&format!("<article>{content_html}</article>"));
    let body_markdown = readable_body(&synthesized)?;
    let (body_html, image_urls) = article_body_html(url, &synthesized)?;
    let word_count = body_markdown
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    Ok(ArticleSnapshot {
        title: text_post.title,
        author: text_post.author,
        published_at: text_post.published_at,
        canonical_url: text_post.canonical_url.unwrap_or_else(|| url.to_owned()),
        body_markdown,
        body_html,
        cover_image_url: None,
        image_urls,
        word_count,
    })
}

fn first_js_assignment(source: &str, name: &str) -> Option<String> {
    let mut remaining = source;
    while let Some(index) = remaining.find(name) {
        let after_name = &remaining[index + name.len()..];
        if let Some(after_equals) = after_name.trim_start().strip_prefix('=') {
            if let Some((value, _)) = decode_js_string(after_equals.trim_start()) {
                return Some(value);
            }
        }
        remaining = &after_name[1.min(after_name.len())..];
    }
    None
}

fn first_js_string_property(source: &str, name: &str) -> Option<String> {
    js_string_properties(source, name).into_iter().next()
}

fn last_js_string_property(source: &str, name: &str) -> Option<String> {
    js_string_properties(source, name).into_iter().last()
}

fn js_string_properties(source: &str, name: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut remaining = source;
    while let Some(index) = remaining.find(name) {
        let after_name = &remaining[index + name.len()..];
        if let Some(after_colon) = after_name.trim_start().strip_prefix(':') {
            if let Some((value, _)) = decode_js_string(after_colon.trim_start()) {
                values.push(value);
            }
        }
        remaining = &after_name[1.min(after_name.len())..];
    }
    values
}

fn decode_js_string(source: &str) -> Option<(String, usize)> {
    let quote = *source.as_bytes().first()?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let bytes = source.as_bytes();
    let mut output = String::new();
    let mut index = 1_usize;
    while index < bytes.len() {
        if bytes[index] == quote {
            return Some((output, index + 1));
        }
        if bytes[index] != b'\\' {
            let character = source[index..].chars().next()?;
            output.push(character);
            index += character.len_utf8();
            continue;
        }
        index += 1;
        let escape = *bytes.get(index)?;
        match escape {
            b'x' => {
                let value = parse_hex_escape(bytes.get(index + 1..index + 3)?)?;
                output.push(char::from(u8::try_from(value).ok()?));
                index += 3;
            }
            b'u' => {
                let high = parse_hex_escape(bytes.get(index + 1..index + 5)?)? as u32;
                index += 5;
                let codepoint = if (0xd800..=0xdbff).contains(&high)
                    && bytes.get(index..index + 2) == Some(b"\\u")
                {
                    let low = parse_hex_escape(bytes.get(index + 2..index + 6)?)? as u32;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return None;
                    }
                    index += 6;
                    0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                } else {
                    high
                };
                output.push(char::from_u32(codepoint)?);
            }
            b'n' => {
                output.push('\n');
                index += 1;
            }
            b'r' => {
                output.push('\r');
                index += 1;
            }
            b't' => {
                output.push('\t');
                index += 1;
            }
            b'b' => {
                output.push('\u{0008}');
                index += 1;
            }
            b'f' => {
                output.push('\u{000c}');
                index += 1;
            }
            b'v' => {
                output.push('\u{000b}');
                index += 1;
            }
            b'0' => {
                output.push('\0');
                index += 1;
            }
            b'\n' => index += 1,
            b'\r' => {
                index += 1;
                if bytes.get(index) == Some(&b'\n') {
                    index += 1;
                }
            }
            other => {
                output.push(char::from(other));
                index += 1;
            }
        }
    }
    None
}

fn parse_hex_escape(bytes: &[u8]) -> Option<u16> {
    bytes.iter().try_fold(0_u16, |value, byte| {
        let digit = match byte {
            b'0'..=b'9' => u16::from(byte - b'0'),
            b'a'..=b'f' => u16::from(byte - b'a' + 10),
            b'A'..=b'F' => u16::from(byte - b'A' + 10),
            _ => return None,
        };
        Some(value * 16 + digit)
    })
}

fn finder_short_uri(url: &Url) -> Option<String> {
    if url.host_str() != Some("channels.weixin.qq.com") || url.path() != "/finder-preview/pages/sph"
    {
        return None;
    }
    url.query_pairs()
        .find(|(key, _)| key == "id")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.trim().is_empty())
}

fn fetch_public_json(endpoint: &Url, referer: &Url, body: &str) -> Result<String, AppError> {
    let mut fetched = post_public_https_json(
        endpoint.as_str(),
        body,
        referer.as_str(),
        Duration::from_secs(30),
    )
    .map_err(AppError::Content)?;
    if let Some(length) = fetched
        .response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        if length > MAX_JSON_BYTES {
            return Err(AppError::Content(
                "视频号公开响应超过 2 MiB 安全上限".into(),
            ));
        }
    }
    let content_type = fetched
        .response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !content_type.starts_with("application/json") {
        return Err(AppError::Content(format!(
            "视频号公开接口返回的不是 JSON（{content_type}）"
        )));
    }
    let mut bytes = Vec::new();
    fetched
        .response
        .by_ref()
        .take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(AppError::Content(
            "视频号公开响应超过 2 MiB 安全上限".into(),
        ));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinderApiResponse {
    #[serde(default)]
    data: Option<FinderData>,
    #[serde(default)]
    err_code: i64,
    #[serde(default)]
    err_msg: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinderData {
    #[serde(default)]
    author_info: Option<FinderAuthorInfo>,
    #[serde(default)]
    feed_info: Option<FinderFeedInfo>,
    #[serde(default)]
    scene_info: Option<FinderSceneInfo>,
    #[serde(default)]
    err_msg: Option<FinderPageError>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinderAuthorInfo {
    #[serde(default)]
    nickname: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinderFeedInfo {
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "createtime")]
    create_time: Option<i64>,
    #[serde(default)]
    cover_url: Option<String>,
    #[serde(default)]
    video_url: Option<String>,
    #[serde(default)]
    origin_video_url: Option<String>,
    #[serde(default)]
    h264_video_info: Option<FinderVideoInfo>,
    #[serde(default)]
    h265_video_info: Option<FinderVideoInfo>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinderVideoInfo {
    #[serde(default)]
    video_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinderSceneInfo {
    #[serde(default)]
    dynamic_export_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FinderPageError {
    #[serde(default)]
    title: Option<String>,
}

fn parse_finder_response(value: &str) -> Result<FinderApiResponse, AppError> {
    let response: FinderApiResponse = serde_json::from_str(value)
        .map_err(|error| AppError::Content(format!("视频号公开响应无法解析：{error}")))?;
    if response.err_code != 0 {
        let detail = non_empty_string(Some(response.err_msg.as_str())).unwrap_or("未知错误");
        return Err(AppError::Content(format!(
            "视频号公开接口返回错误 {}：{detail}",
            response.err_code
        )));
    }
    Ok(response)
}

fn finder_response_has_video(response: &FinderApiResponse) -> bool {
    response
        .data
        .as_ref()
        .and_then(|data| data.feed_info.as_ref())
        .is_some_and(|info| finder_video_urls(info).next().is_some())
}

fn collect_finder_video_candidates(
    info: &FinderFeedInfo,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<DetectedVideo>,
) {
    for (label, candidate) in finder_video_urls(info) {
        let Some(url) = allowed_finder_url(Some(candidate)) else {
            continue;
        };
        if seen.insert(url.clone()) {
            candidates.push(DetectedVideo {
                url,
                kind: DetectedVideoKind::DirectFile,
                label: label.into(),
                downloadable: true,
            });
        }
    }
}

fn finder_video_urls(info: &FinderFeedInfo) -> impl Iterator<Item = (&'static str, &str)> {
    [
        (
            "微信公开视频（H.264）",
            info.h264_video_info
                .as_ref()
                .and_then(|item| item.video_url.as_deref()),
        ),
        (
            "微信公开视频（H.265）",
            info.h265_video_info
                .as_ref()
                .and_then(|item| item.video_url.as_deref()),
        ),
        ("微信原始视频", info.origin_video_url.as_deref()),
        ("微信公开视频", info.video_url.as_deref()),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|url| (label, url)))
}

fn allowed_finder_url(value: Option<&str>) -> Option<String> {
    let value = non_empty_string(value)?;
    let url = Url::parse(value).ok()?;
    is_allowed_public_https_url(&url).then(|| url.to_string())
}

fn non_empty_string(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn detected_video(url: String, label: String) -> DetectedVideo {
    let extension = media_extension(&url).unwrap_or_default();
    let (kind, downloadable) = match extension.as_str() {
        "mp4" | "mov" | "m4v" | "webm" => {
            let allowed = Url::parse(&url)
                .ok()
                .is_some_and(|parsed| is_allowed_public_https_url(&parsed));
            (DetectedVideoKind::DirectFile, allowed)
        }
        "m3u8" => (DetectedVideoKind::HlsPlaylist, false),
        "mpd" => (DetectedVideoKind::DashManifest, false),
        _ => (DetectedVideoKind::Unsupported, false),
    };
    DetectedVideo {
        url,
        kind,
        label,
        downloadable,
    }
}

fn media_extension(value: &str) -> Option<String> {
    let path = Url::parse(value)
        .ok()
        .map(|url| url.path().to_owned())
        .unwrap_or_else(|| value.split(['?', '#']).next().unwrap_or(value).to_owned());
    path.rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .filter(|extension| {
            matches!(
                extension.as_str(),
                "mp4" | "mov" | "m4v" | "webm" | "m3u8" | "mpd"
            )
        })
}

fn fetch_html(url: &str) -> Result<(Url, String), AppError> {
    let mut fetched = get_public_https(url, Duration::from_secs(45)).map_err(AppError::Content)?;
    if let Some(length) = fetched
        .response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        if length > MAX_HTML_BYTES {
            return Err(AppError::Content("页面超过 8 MiB 安全上限".into()));
        }
    }
    if let Some(content_type) = fetched
        .response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
    {
        if !content_type.starts_with("text/html")
            && !content_type.starts_with("application/xhtml+xml")
        {
            return Err(AppError::Content(format!(
                "链接返回的不是 HTML 页面（{content_type}）"
            )));
        }
    }
    let mut bytes = Vec::new();
    fetched
        .response
        .by_ref()
        .take(MAX_HTML_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_HTML_BYTES {
        return Err(AppError::Content("页面超过 8 MiB 安全上限".into()));
    }
    Ok((
        fetched.final_url,
        String::from_utf8_lossy(&bytes).into_owned(),
    ))
}

fn page_title(document: &Html) -> Option<String> {
    meta_content(document, &["og:title", "twitter:title"])
        .or_else(|| first_text(document, &["#activity-name", "h1", "title"]))
}

fn meta_content(document: &Html, names: &[&str]) -> Option<String> {
    for name in names {
        for attribute in ["property", "name"] {
            let selector = Selector::parse(&format!("meta[{attribute}=\"{name}\"]")).ok()?;
            if let Some(content) = document
                .select(&selector)
                .next()
                .and_then(|element| element.value().attr("content"))
                .map(normalize_whitespace)
                .filter(|value| !value.is_empty())
            {
                return Some(content);
            }
        }
    }
    None
}

fn canonical_link(document: &Html) -> Option<String> {
    let selector = Selector::parse("link[rel=canonical]").ok()?;
    document
        .select(&selector)
        .next()
        .and_then(|element| element.value().attr("href"))
        .map(str::to_owned)
}

fn article_body_html(page_url: &str, document: &Html) -> Result<(String, Vec<String>), AppError> {
    let root = [
        "#js_content",
        ".rich_media_content",
        "article",
        "main",
        "body",
    ]
    .iter()
    .find_map(|selector| {
        Selector::parse(selector)
            .ok()
            .and_then(|selector| document.select(&selector).next())
    })
    .ok_or_else(|| AppError::Content("没有在页面中找到可导出的正文结构".into()))?;
    let mut html = root.inner_html();
    let image_selector = Selector::parse("img").expect("static selector");
    let mut image_urls = Vec::new();
    for image in root.select(&image_selector) {
        let Some((attribute, raw_url)) = image
            .value()
            .attr("data-src")
            .map(|value| ("data-src", value))
            .or_else(|| image.value().attr("src").map(|value| ("src", value)))
        else {
            continue;
        };
        let Some(resolved) = resolve_public_url(page_url, raw_url) else {
            continue;
        };
        let image_index = image_urls
            .iter()
            .position(|url| url == &resolved)
            .unwrap_or_else(|| {
                image_urls.push(resolved);
                image_urls.len() - 1
            });
        let replacement = format!("data-xunqi-image=\"{image_index}\"");
        let escaped_url = escape_html_attribute(raw_url);
        let candidates = [
            format!("{attribute}=\"{escaped_url}\""),
            format!("{attribute}=\"{raw_url}\""),
        ];
        if let Some(candidate) = candidates
            .iter()
            .find(|candidate| html.contains(*candidate))
        {
            html = html.replacen(candidate, &replacement, 1);
        }
        if attribute == "data-src" {
            if let Some(fallback_source) = image.value().attr("src") {
                let escaped_source = escape_html_attribute(fallback_source);
                for candidate in [
                    format!("src=\"{escaped_source}\""),
                    format!("src=\"{fallback_source}\""),
                ] {
                    if html.contains(&candidate) {
                        html = html.replacen(&candidate, "", 1);
                        break;
                    }
                }
            }
        }
    }

    let mut cleaner = ammonia::Builder::default();
    cleaner
        .add_generic_attributes(&["class"])
        .add_tag_attributes("img", &["data-xunqi-image", "alt", "width", "height"])
        .rm_tag_attributes("img", &["src"]);
    Ok((cleaner.clean(&html).to_string(), image_urls))
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn resolve_public_url(page_url: &str, candidate: &str) -> Option<String> {
    let base = Url::parse(page_url).ok()?;
    let resolved = base.join(candidate).ok()?;
    is_allowed_public_https_url(&resolved).then(|| resolved.to_string())
}

fn first_text(document: &Html, selectors: &[&str]) -> Option<String> {
    selectors.iter().find_map(|selector| {
        let selector = Selector::parse(selector).ok()?;
        document
            .select(&selector)
            .next()
            .map(|element| normalize_whitespace(&element.text().collect::<String>()))
            .filter(|value| !value.is_empty())
    })
}

fn readable_body(document: &Html) -> Result<String, AppError> {
    let root = [
        "#js_content",
        ".rich_media_content",
        "article",
        "main",
        "body",
    ]
    .iter()
    .find_map(|selector| {
        Selector::parse(selector)
            .ok()
            .and_then(|selector| document.select(&selector).next())
    });
    let root = root.ok_or_else(|| AppError::Content("没有在页面中找到可阅读正文".into()))?;
    let block_selector =
        Selector::parse("h1, h2, h3, h4, h5, h6, p, blockquote, li, pre").expect("static selector");
    let mut seen = HashSet::new();
    let mut blocks = root
        .select(&block_selector)
        .filter_map(|element| markdown_block(element))
        .filter(|block| seen.insert(block.clone()))
        .collect::<Vec<_>>();
    if blocks.is_empty() {
        let fallback = normalize_whitespace(&root.text().collect::<String>());
        if !fallback.is_empty() {
            blocks.push(fallback);
        }
    }
    let body = blocks.join("\n\n");
    if body.chars().count() < 20 {
        return Err(AppError::Content(
            "页面没有返回足够的可阅读正文，可能需要登录、验证或由脚本加载".into(),
        ));
    }
    Ok(body)
}

fn markdown_block(element: ElementRef<'_>) -> Option<String> {
    let text = normalize_whitespace(&element.text().collect::<String>());
    if text.is_empty() {
        return None;
    }
    let prefix = match element.value().name() {
        "h1" => "# ",
        "h2" => "## ",
        "h3" => "### ",
        "h4" => "#### ",
        "h5" => "##### ",
        "h6" => "###### ",
        "blockquote" => "> ",
        "li" => "- ",
        _ => "",
    };
    Some(format!("{prefix}{text}"))
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
