use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    time::Duration,
};

use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};

use crate::{public_http::get_public_https, AppError};

const MAX_VIDEO_BYTES: u64 = 1_073_741_824;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedMedia {
    pub bytes_written: u64,
    pub extension: String,
}

pub trait MediaDownloader: Send + Sync {
    fn download(&self, url: &str, destination: &Path) -> Result<DownloadedMedia, AppError>;
}

pub struct PublicVideoDownloader;

impl MediaDownloader for PublicVideoDownloader {
    fn download(&self, url: &str, destination: &Path) -> Result<DownloadedMedia, AppError> {
        let mut response = get_public_https(url, Duration::from_secs(180))
            .map_err(AppError::Download)?
            .response;
        if let Some(length) = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
        {
            if length > MAX_VIDEO_BYTES {
                return Err(AppError::Download("文件超过 1 GiB 安全上限".into()));
            }
        }
        if let Some(content_type) = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
        {
            if !is_video_content_type(content_type) {
                return Err(AppError::Download(format!(
                    "服务器返回的不是视频文件（{content_type}）"
                )));
            }
        }

        let prefix = read_video_prefix(&mut response)?;
        let extension = detect_video_extension(&prefix)?;

        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            file.write_all(&prefix)?;
            let mut limited = response
                .by_ref()
                .take(MAX_VIDEO_BYTES + 1 - prefix.len() as u64);
            let bytes_written = prefix.len() as u64 + io::copy(&mut limited, &mut file)?;
            if bytes_written > MAX_VIDEO_BYTES {
                return Err(AppError::Download("文件超过 1 GiB 安全上限".into()));
            }
            file.sync_all()?;
            Ok(DownloadedMedia {
                bytes_written,
                extension: extension.into(),
            })
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(destination);
        }
        write_result
    }
}

fn is_video_content_type(content_type: &str) -> bool {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    media_type.starts_with("video/") || media_type == "application/octet-stream"
}

fn read_video_prefix(reader: &mut impl Read) -> Result<Vec<u8>, AppError> {
    let mut prefix = Vec::with_capacity(64);
    reader.take(64).read_to_end(&mut prefix)?;
    Ok(prefix)
}

fn detect_video_extension(prefix: &[u8]) -> Result<&'static str, AppError> {
    let is_iso_media = prefix.len() >= 8
        && u32::from_be_bytes(prefix[0..4].try_into().expect("four-byte prefix")) >= 8
        && matches!(
            &prefix[4..8],
            b"ftyp" | b"moov" | b"mdat" | b"free" | b"wide" | b"skip"
        );
    let is_webm = prefix.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]);
    if is_webm {
        return Ok("webm");
    }
    if is_iso_media {
        let major_brand = prefix.get(8..12).unwrap_or_default();
        return Ok(if major_brand == b"qt  " { "mov" } else { "mp4" });
    }
    Err(AppError::Download(
        "响应内容不是可识别的 MP4、MOV、M4V 或 WebM 视频".into(),
    ))
}

#[cfg(test)]
mod tests {
    use std::io::{self, Read};

    use super::{detect_video_extension, is_video_content_type, read_video_prefix};

    struct ChunkedReader {
        bytes: Vec<u8>,
        offset: usize,
    }

    impl Read for ChunkedReader {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.offset >= self.bytes.len() {
                return Ok(0);
            }
            let length = output.len().min(3).min(self.bytes.len() - self.offset);
            output[..length].copy_from_slice(&self.bytes[self.offset..self.offset + length]);
            self.offset += length;
            Ok(length)
        }
    }

    #[test]
    fn validates_content_type_and_video_file_signatures() {
        assert!(is_video_content_type("video/mp4; charset=binary"));
        assert!(is_video_content_type("application/octet-stream"));
        assert!(!is_video_content_type("text/html"));

        let mp4 = b"\0\0\0\x18ftypisom\0\0\0\0";
        let mov = b"\0\0\0\x18ftypqt  \0\0\0\0";
        let webm = [0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86, 0x81];
        assert_eq!(detect_video_extension(mp4).expect("detect mp4"), "mp4");
        assert_eq!(detect_video_extension(mov).expect("detect mov"), "mov");
        assert_eq!(detect_video_extension(&webm).expect("detect webm"), "webm");
        assert!(detect_video_extension(b"<!doctype html><title>login</title>").is_err());
        assert!(detect_video_extension(b"unknown bytes").is_err());
    }

    #[test]
    fn reads_enough_prefix_bytes_across_short_network_reads() {
        let mp4 = b"\0\0\0\x18ftypisom\0\0\0\0".to_vec();
        let mut reader = ChunkedReader {
            bytes: mp4.clone(),
            offset: 0,
        };
        let prefix = read_video_prefix(&mut reader).expect("read chunked video prefix");
        assert_eq!(prefix, mp4);
        assert_eq!(
            detect_video_extension(&prefix).expect("detect chunked mp4"),
            "mp4"
        );
    }
}
