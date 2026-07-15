use std::{io::Read, time::Duration};

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};

use crate::{public_http::get_public_https, AppError};

const MAX_ARTICLE_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArticleAsset {
    pub extension: String,
    pub bytes: Vec<u8>,
}

pub trait ArticleAssetFetcher: Send + Sync {
    fn fetch(&self, url: &str) -> Result<ArticleAsset, AppError>;
}

pub struct PublicArticleAssetFetcher;

impl ArticleAssetFetcher for PublicArticleAssetFetcher {
    fn fetch(&self, url: &str) -> Result<ArticleAsset, AppError> {
        let mut response = get_public_https(url, Duration::from_secs(60))
            .map_err(|error| AppError::Content(format!("图片请求失败：{error}")))?
            .response;
        if let Some(length) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
        {
            if length > MAX_ARTICLE_IMAGE_BYTES {
                return Err(AppError::Content("单张图片超过 20 MiB 安全上限".into()));
            }
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(MAX_ARTICLE_IMAGE_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_ARTICLE_IMAGE_BYTES {
            return Err(AppError::Content("单张图片超过 20 MiB 安全上限".into()));
        }
        let extension = detected_image_extension(&bytes, &content_type).ok_or_else(|| {
            AppError::Content(format!(
                "图片响应不是支持的 JPEG、PNG、GIF 或 WebP（{content_type}）"
            ))
        })?;
        Ok(ArticleAsset {
            extension: extension.into(),
            bytes,
        })
    }
}

fn detected_image_extension(bytes: &[u8], content_type: &str) -> Option<&'static str> {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) && media_type == "image/jpeg" {
        return Some("jpg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && media_type == "image/png" {
        return Some("png");
    }
    if (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) && media_type == "image/gif" {
        return Some("gif");
    }
    if bytes.len() >= 12
        && bytes.starts_with(b"RIFF")
        && &bytes[8..12] == b"WEBP"
        && media_type == "image/webp"
    {
        return Some("webp");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::detected_image_extension;

    #[test]
    fn image_extension_requires_matching_content_type_and_signature() {
        assert_eq!(
            detected_image_extension(b"\xff\xd8\xfffixture", "image/jpeg"),
            Some("jpg")
        );
        assert_eq!(
            detected_image_extension(b"\x89PNG\r\n\x1a\nfixture", "image/png"),
            Some("png")
        );
        assert_eq!(
            detected_image_extension(b"GIF89afixture", "image/gif"),
            Some("gif")
        );
        assert_eq!(
            detected_image_extension(b"RIFF1234WEBPfixture", "image/webp"),
            Some("webp")
        );
        assert_eq!(
            detected_image_extension(b"<!doctype html>", "text/html"),
            None
        );
        assert_eq!(
            detected_image_extension(b"\xff\xd8\xfffixture", "text/html"),
            None
        );
    }
}
